//! Decides which attachments to fetch from paired devices and drives each transfer (Issue #57).
//!
//! Each device tells its connected paired devices which attachments it holds (`NTA1` manifests).
//! An attachment this device lacks becomes *wanted*, and once its meeting has synced the file is
//! pulled one chunk at a time over the encrypted file-transfer protocol. The row is written only
//! after the whole file has passed its SHA-256 check, so the app never lists a file it cannot open.
//!
//! Nothing here is persisted: manifests are sent again on every connection, so a device that was
//! offline, or lost a connection mid-transfer, fetches what it misses when it reconnects. This
//! module never logs names or file contents.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    domain::models::{DeviceId, FileChunk, SharedAttachment},
    ports::file_transfer::{
        AttachmentLibrary, FILE_TRANSFER_CHUNK_BYTES, FileTransferEvent, FileTransferNetwork,
    },
};

/// How many files are received at once.
pub const MAX_CONCURRENT_TRANSFERS: usize = 2;
/// How many attachments may wait to be fetched, bounding what a paired device can make this
/// device remember.
pub const MAX_WANTED_ATTACHMENTS: usize = 16_384;
/// How long a requested chunk may take before it is asked for again.
pub const STALLED_CHUNK_TIMEOUT: Duration = Duration::from_secs(15);
/// How many times one chunk is asked for again before giving up on that source until it reconnects.
pub const MAX_CHUNK_RETRIES: u32 = 3;

/// An attachment to fetch and the devices that offered it.
struct WantedAttachment {
    attachment: SharedAttachment,
    sources: HashSet<DeviceId>,
}

/// A file being received.
struct ActiveTransfer {
    attachment: SharedAttachment,
    source: DeviceId,
    next_offset: u64,
    last_progress: Instant,
    retries: u32,
}

/// The manifest to send and the devices that need it.
pub struct PendingManifest {
    /// Identifies this version of the list; pass it to `mark_shared` once sent.
    pub digest: [u8; 32],
    /// The connected devices that were not sent this version.
    pub peers: Vec<DeviceId>,
    /// The list to send.
    pub attachments: Vec<SharedAttachment>,
}

/// Tracks manifests sent, attachments wanted, and files being received.
pub struct AttachmentSync {
    library: Box<dyn AttachmentLibrary>,
    shared_digests: HashMap<DeviceId, [u8; 32]>,
    wanted: HashMap<Uuid, WantedAttachment>,
    active: Vec<ActiveTransfer>,
}

impl AttachmentSync {
    /// Creates a tracker over this device's attachments.
    ///
    /// Inputs: The local library. Outputs: A tracker with nothing wanted. Side effects: None.
    pub fn new(library: Box<dyn AttachmentLibrary>) -> Self {
        Self {
            library,
            shared_digests: HashMap::new(),
            wanted: HashMap::new(),
            active: Vec::new(),
        }
    }

    /// Prepares the manifest for connected devices that were not sent the current list.
    ///
    /// Inputs: The devices connected now.
    /// Outputs: What to send, or `None` when nobody needs it or storage failed (logged).
    /// Side effects: Hashes at most one new attachment file per call, so a call stays short.
    pub fn pending_manifest(&mut self, connected: &HashSet<DeviceId>) -> Option<PendingManifest> {
        if connected.is_empty() {
            return None;
        }
        if let Err(error) = self.library.hash_next_attachment() {
            tracing::warn!(error = %error, "an attachment could not be hashed");
        }
        let attachments = match self.library.shared_attachments() {
            Ok(attachments) => attachments,
            Err(error) => {
                tracing::warn!(error = %error, "the attachments could not be listed");
                return None;
            }
        };
        let digest = manifest_digest(&attachments);
        let peers: Vec<DeviceId> = connected
            .iter()
            .filter(|peer| self.shared_digests.get(*peer) != Some(&digest))
            .cloned()
            .collect();
        (!peers.is_empty()).then_some(PendingManifest {
            digest,
            peers,
            attachments,
        })
    }

    /// Records that `peer` was sent the list identified by `digest`. Side effects: Memory only.
    pub fn mark_shared(&mut self, peer: DeviceId, digest: [u8; 32]) {
        self.shared_digests.insert(peer, digest);
    }

    /// Forgets what was sent to `peer`, so it is sent the list again.
    ///
    /// Use it when a device connects or a delivery to it failed. Side effects: Memory only.
    pub fn forget_shared(&mut self, peer: &DeviceId) {
        self.shared_digests.remove(peer);
    }

    /// Notes the attachments `peer` holds and this device lacks.
    ///
    /// Inputs: A decoded manifest. Outputs: None.
    /// Side effects: Reads rows; adds entries to the wanted list, up to its limit.
    pub fn accept_manifest(&mut self, peer: &DeviceId, attachments: Vec<SharedAttachment>) {
        for attachment in attachments {
            if let Some(wanted) = self.wanted.get_mut(&attachment.attachment_id) {
                // The first offer's metadata stays, so a later one cannot change a running transfer.
                if wanted.attachment == attachment {
                    wanted.sources.insert(peer.clone());
                }
                continue;
            }
            if self.wanted.len() >= MAX_WANTED_ATTACHMENTS {
                tracing::warn!(peer = %peer.as_str(), "too many attachments are waiting; the rest wait for the next connection");
                return;
            }
            match self.library.has_attachment(attachment.attachment_id) {
                Ok(true) => {}
                Ok(false) => {
                    self.wanted.insert(
                        attachment.attachment_id,
                        WantedAttachment {
                            attachment,
                            sources: HashSet::from([peer.clone()]),
                        },
                    );
                }
                Err(error) => {
                    tracing::warn!(error = %error, "an attachment could not be looked up");
                }
            }
        }
    }

    /// Removes a deleted meeting's stored files and stops wanting its attachments.
    ///
    /// Side effects: Deletes files; a failure is logged and the files wait for no retry, as their
    /// rows are already gone.
    pub fn meeting_deleted(&mut self, meeting_id: &crate::domain::models::MeetingId) {
        self.wanted
            .retain(|_, wanted| !matches!(&wanted.attachment.owner, crate::domain::models::AttachmentOwner::Meeting(owner) if owner == meeting_id));
        if let Err(error) = self.library.remove_meeting_files(meeting_id) {
            tracing::warn!(error = %error, "a deleted meeting's files could not be removed");
        }
    }

    /// Removes a deleted project's stored resource files (Issue #85).
    ///
    /// Side effects: Deletes files; a failure is logged, as the rows are already gone.
    pub fn project_deleted(&mut self, project_id: crate::domain::projects::ProjectId) {
        self.wanted.retain(|_, wanted| {
            wanted.attachment.owner != crate::domain::models::AttachmentOwner::Project(project_id)
        });
        if let Err(error) = self.library.remove_project_files(project_id) {
            tracing::warn!(error = %error, "a deleted project's files could not be removed");
        }
    }

    /// Cancels transfers from `peer` and forgets its offers and what it was sent.
    ///
    /// Side effects: Removes partial files. The device's next manifest offers them again, and
    /// their transfers restart from the beginning.
    pub fn peer_lost(&mut self, peer: &DeviceId) {
        self.shared_digests.remove(peer);
        let library = &mut self.library;
        self.active.retain(|transfer| {
            let keep = transfer.source != *peer;
            if !keep {
                library.discard_incoming(&transfer.attachment);
            }
            keep
        });
        self.wanted.retain(|_, wanted| {
            wanted.sources.remove(peer);
            !wanted.sources.is_empty()
        });
    }

    /// Asks again for overdue chunks and starts waiting transfers.
    ///
    /// Inputs: The network and the current time.
    /// Side effects: May queue chunk requests, create partial files, and read rows. A source that
    /// keeps not answering is dropped until it reconnects.
    pub fn pump(&mut self, network: &mut dyn FileTransferNetwork, now: Instant) {
        let mut index = 0;
        while let Some(transfer) = self.active.get_mut(index) {
            if now.duration_since(transfer.last_progress) < STALLED_CHUNK_TIMEOUT {
                index += 1;
                continue;
            }
            transfer.retries += 1;
            transfer.last_progress = now;
            let requested = transfer.retries <= MAX_CHUNK_RETRIES
                && network
                    .request_file_chunk(
                        &transfer.attachment.content_hash,
                        transfer.next_offset,
                        &transfer.source,
                    )
                    .is_ok();
            if requested {
                index += 1;
            } else {
                tracing::warn!(peer = %transfer.source.as_str(), attachment_id = %transfer.attachment.attachment_id, "an attachment transfer stalled");
                self.abandon(index);
            }
        }
        self.start_transfers(network);
    }

    /// Serves or absorbs one event from the file-transfer protocol.
    ///
    /// Inputs: The network and the event, from a paired device but otherwise untrusted.
    /// Side effects: Reads or writes attachment files, queues frames, and on a finished file
    /// writes its row. Files outside linked attachments are never served: such a request is
    /// ignored and the requester's retry limit ends it.
    pub fn handle_event(
        &mut self,
        network: &mut dyn FileTransferNetwork,
        event: FileTransferEvent,
    ) {
        match event {
            FileTransferEvent::ChunkRequested { peer, hash, offset } => {
                self.serve_chunk(network, &peer, &hash, offset);
            }
            FileTransferEvent::ChunkReceived { peer, chunk } => {
                self.absorb_chunk(network, &peer, &chunk);
            }
        }
    }

    /// Sends the requested chunk when it belongs to a linked attachment.
    ///
    /// Side effects: Reads part of a file and queues a frame; failures are logged.
    fn serve_chunk(
        &self,
        network: &mut dyn FileTransferNetwork,
        peer: &DeviceId,
        hash: &str,
        offset: u64,
    ) {
        match self.library.read_chunk(hash, offset) {
            Ok(Some(chunk)) => {
                if let Err(error) = network.send_file_chunk(peer, &chunk) {
                    tracing::warn!(peer = %peer.as_str(), error = %error, "an attachment chunk could not be sent");
                }
            }
            Ok(None) => {
                tracing::warn!(peer = %peer.as_str(), "a device asked for a file that is not shared");
            }
            Err(error) => {
                tracing::warn!(peer = %peer.as_str(), error = %error, "an attachment chunk could not be read");
            }
        }
    }

    /// Validates and stores one received chunk, then asks for the next or finishes the file.
    ///
    /// Side effects: Writes to the partial file; queues a request; on the last chunk verifies the
    /// file and writes its row. A chunk nobody asked for, including a late duplicate after a
    /// retry, is ignored. Invalid data ends the transfer and drops the source until it reconnects.
    fn absorb_chunk(
        &mut self,
        network: &mut dyn FileTransferNetwork,
        peer: &DeviceId,
        chunk: &FileChunk,
    ) {
        let Some(index) = self.active.iter().position(|transfer| {
            transfer.source == *peer && transfer.attachment.content_hash == chunk.hash()
        }) else {
            return;
        };
        let transfer = &mut self.active[index];
        if chunk.offset() != transfer.next_offset {
            return;
        }
        let received_bytes = chunk.data().len() as u64;
        let size_bytes = transfer.attachment.size_bytes;
        let next_offset = transfer.next_offset.saturating_add(received_bytes);
        if chunk.total_size() != size_bytes
            || chunk.data().len() > FILE_TRANSFER_CHUNK_BYTES
            || next_offset > size_bytes
            || (received_bytes == 0 && size_bytes != 0)
        {
            tracing::warn!(peer = %peer.as_str(), attachment_id = %transfer.attachment.attachment_id, "a device sent an invalid attachment chunk");
            self.abandon(index);
            return;
        }
        if let Err(error) = self
            .library
            .append_incoming(&transfer.attachment, chunk.data())
        {
            tracing::warn!(attachment_id = %transfer.attachment.attachment_id, error = %error, "an attachment chunk could not be stored");
            self.abandon(index);
            return;
        }
        transfer.next_offset = next_offset;
        transfer.last_progress = Instant::now();
        transfer.retries = 0;
        if next_offset == size_bytes {
            self.finish(index);
            self.start_transfers(network);
            return;
        }
        if network
            .request_file_chunk(chunk.hash(), next_offset, peer)
            .is_err()
        {
            self.abandon(index);
        }
    }

    /// Verifies and stores a completely received file.
    ///
    /// Side effects: Writes the file's row. A failed check drops the source until it reconnects,
    /// so a bad copy is not fetched again and again.
    fn finish(&mut self, index: usize) {
        let transfer = self.active.remove(index);
        let attachment_id = transfer.attachment.attachment_id;
        match self.library.finish_incoming(&transfer.attachment) {
            Ok(()) => {
                self.wanted.remove(&attachment_id);
                tracing::info!(%attachment_id, "an attachment arrived from a paired device");
            }
            Err(error) => {
                tracing::warn!(%attachment_id, error = %error, "a received attachment was refused");
                self.drop_source(attachment_id, &transfer.source);
            }
        }
    }

    /// Ends a transfer and drops its source until that device reconnects.
    ///
    /// Side effects: Removes the partial file.
    fn abandon(&mut self, index: usize) {
        let transfer = self.active.remove(index);
        self.library.discard_incoming(&transfer.attachment);
        self.drop_source(transfer.attachment.attachment_id, &transfer.source);
    }

    /// Removes one offer, and the wanted entry when no offer is left. Side effects: Memory only.
    fn drop_source(&mut self, attachment_id: Uuid, source: &DeviceId) {
        if let Some(wanted) = self.wanted.get_mut(&attachment_id) {
            wanted.sources.remove(source);
            if wanted.sources.is_empty() {
                self.wanted.remove(&attachment_id);
            }
        }
    }

    /// Starts wanted transfers up to the limit, for attachments whose meeting has synced.
    ///
    /// Side effects: Reads rows, creates partial files, queues first requests. An attachment
    /// whose meeting has not synced stays wanted. Metadata storage refuses is dropped until the
    /// next manifest.
    fn start_transfers(&mut self, network: &mut dyn FileTransferNetwork) {
        if self.active.len() >= MAX_CONCURRENT_TRANSFERS {
            return;
        }
        let mut candidates: Vec<Uuid> = self.wanted.keys().copied().collect();
        candidates.sort_unstable();
        for attachment_id in candidates {
            if self.active.len() >= MAX_CONCURRENT_TRANSFERS {
                return;
            }
            let Some(wanted) = self.wanted.get(&attachment_id) else {
                continue;
            };
            let attachment = wanted.attachment.clone();
            // One hash at a time: chunks name only the hash, so two files with the same contents
            // from one device could not be told apart.
            if self.active.iter().any(|transfer| {
                transfer.attachment.content_hash == attachment.content_hash
                    || transfer.attachment.attachment_id == attachment_id
            }) || !self.library.has_owner(&attachment.owner).unwrap_or(false)
            {
                continue;
            }
            let mut sources: Vec<&DeviceId> = wanted.sources.iter().collect();
            sources.sort_unstable_by_key(|source| source.as_str().to_owned());
            let Some(source) = sources.first().map(|source| (*source).clone()) else {
                continue;
            };
            if let Err(error) = self.library.begin_incoming(&attachment) {
                tracing::warn!(%attachment_id, error = %error, "an offered attachment was refused");
                self.wanted.remove(&attachment_id);
                continue;
            }
            if network
                .request_file_chunk(&attachment.content_hash, 0, &source)
                .is_err()
            {
                self.library.discard_incoming(&attachment);
                self.drop_source(attachment_id, &source);
                continue;
            }
            self.active.push(ActiveTransfer {
                attachment,
                source,
                next_offset: 0,
                last_progress: Instant::now(),
                retries: 0,
            });
        }
    }
}

/// Summarizes a list so a device is sent it only when it changed.
///
/// Inputs: Attachments in a stable order. Outputs: The digest. Side effects: None.
fn manifest_digest(attachments: &[SharedAttachment]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for attachment in attachments {
        hasher.update(attachment.attachment_id.as_bytes());
        hasher.update(attachment.content_hash.as_bytes());
    }
    hasher.finalize().into()
}
