//! Orchestrates non-blocking, content-addressed attachment transfer through hexagonal ports.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use sha2::{Digest, Sha256};

use crate::{
    domain::{
        errors::FileTransferError,
        models::{DeviceId, FileChunk, FileReference, FileTransferRequest, MeetingId},
    },
    ports::file_transfer::{FileStorage, FileTransferNetwork},
};

/// Limits binary frames to 64 KiB so bounded network messages do not spike memory.
pub const FILE_TRANSFER_CHUNK_BYTES: usize = 64 * 1024;
/// Rejects attachments larger than the approved 100 MiB transfer policy.
pub const MAX_FILE_TRANSFER_BYTES: u64 = 100 * 1024 * 1024;
/// Bounds simultaneously active incoming downloads per manager.
pub const MAX_CONCURRENT_TRANSFERS: usize = 2;
/// Expires stale peer availability to avoid requesting from a departed source.
pub const AVAILABILITY_TTL: Duration = Duration::from_secs(300);

#[derive(Clone)]
struct AvailabilityEntry {
    reference: FileReference,
    last_seen: Instant,
}

struct ReceivingTransfer {
    total_size: u64,
    next_offset: u64,
    bytes: Vec<u8>,
}

/// Applies attachment references after CRDT persistence and performs best-effort file fetches.
pub struct FileSyncManager<NetworkPort, StoragePort> {
    file_transfer_network: NetworkPort,
    file_storage: StoragePort,
    availability_by_peer: HashMap<DeviceId, HashMap<String, AvailabilityEntry>>,
    authorized_meetings_by_hash: HashMap<String, HashSet<MeetingId>>,
    receiving_transfers: HashMap<(DeviceId, String), ReceivingTransfer>,
}

impl<NetworkPort, StoragePort> FileSyncManager<NetworkPort, StoragePort>
where
    NetworkPort: FileTransferNetwork,
    StoragePort: FileStorage,
{
    /// Creates a manager that owns only file-transfer and content-addressed storage ports.
    ///
    /// Inputs: injected ports must use encrypted authenticated networking and verified local CAS.
    /// Outputs: a manager with an empty ephemeral availability registry.
    /// Side effects: none; callers rebuild and announce availability separately at startup/discovery.
    pub fn new(file_transfer_network: NetworkPort, file_storage: StoragePort) -> Self {
        Self {
            file_transfer_network,
            file_storage,
            availability_by_peer: HashMap::new(),
            authorized_meetings_by_hash: HashMap::new(),
            receiving_transfers: HashMap::new(),
        }
    }

    /// Records fresh availability metadata advertised by an authenticated peer.
    ///
    /// Inputs: `peer` is authenticated by the network adapter; references contain no bytes.
    /// Outputs: none.
    /// Side effects: updates the in-memory TTL registry used for best-effort source selection.
    pub fn record_availability(&mut self, peer: DeviceId, references: Vec<FileReference>) {
        let now = Instant::now();
        let peer_entries = self.availability_by_peer.entry(peer).or_default();
        for reference in references {
            peer_entries.insert(
                reference.hash().to_owned(),
                AvailabilityEntry {
                    reference,
                    last_seen: now,
                },
            );
        }
    }

    /// Removes a departed peer's availability and cancels its incomplete transfers.
    ///
    /// Inputs: `peer` identifies an expired authenticated peer.
    /// Outputs: none.
    /// Side effects: drops in-memory partial bytes; a later discovery can safely retry from zero.
    pub fn handle_peer_lost(&mut self, peer: &DeviceId) {
        self.availability_by_peer.remove(peer);
        self.receiving_transfers
            .retain(|(source_peer, _), _| source_peer != peer);
    }

    /// Removes stale availability entries older than the policy TTL.
    ///
    /// Inputs: none.
    /// Outputs: none.
    /// Side effects: prevents requests from unconfirmed historical advertisements.
    pub fn evict_expired_availability(&mut self) {
        let now = Instant::now();
        self.availability_by_peer.retain(|_, entries| {
            entries.retain(|_, entry| now.duration_since(entry.last_seen) <= AVAILABILITY_TTL);
            !entries.is_empty()
        });
    }

    /// Returns locally stored hashes so callers can announce rebuilt CAS availability on discovery.
    ///
    /// Inputs: none.
    /// Outputs: verified local hashes or a storage failure.
    /// Side effects: reads CAS directory metadata only.
    pub fn rebuild_local_availability(&self) -> Result<Vec<String>, FileTransferError> {
        Ok(self.file_storage.list_available_hashes()?)
    }

    /// Authorizes serving one hash only for a replicated meeting that references it.
    ///
    /// Inputs: `hash` is a content address and `meeting_id` is a shared meeting reference.
    /// Outputs: none.
    /// Side effects: updates in-memory serving authorization; no file bytes are accessed.
    pub fn authorize_hash_for_meeting(&mut self, hash: &str, meeting_id: MeetingId) {
        self.authorized_meetings_by_hash
            .entry(hash.to_owned())
            .or_default()
            .insert(meeting_id);
    }

    /// Processes a post-persistence CRDT file-reference event without blocking CRDT application.
    ///
    /// Inputs: `reference` was already persisted by the CRDT path.
    /// Outputs: success after a request is queued, or a portable validation/network error.
    /// Side effects: records serving authorization and queues a best-effort request when local CAS lacks it.
    pub fn handle_file_reference(
        &mut self,
        reference: FileReference,
    ) -> Result<(), FileTransferError> {
        validate_reference(&reference)?;
        self.authorize_hash_for_meeting(reference.hash(), reference.meeting_id().clone());
        if self.file_storage.has_file(reference.hash()) {
            return Ok(());
        }
        self.evict_expired_availability();
        let selected_peer = self
            .availability_by_peer
            .iter()
            .find_map(|(peer, entries)| {
                entries.get(reference.hash()).and_then(|entry| {
                    (entry.reference.size_bytes() == reference.size_bytes()).then(|| peer.clone())
                })
            });
        if let Some(peer) = selected_peer {
            self.file_transfer_network
                .request_file(reference.hash(), &peer)?;
            tracing::info!(peer_id = %peer.as_str(), hash = %reference.hash(), size_bytes = reference.size_bytes(), "Queued attachment file request");
        }
        Ok(())
    }

    /// Announces a newly saved local attachment after validating its metadata.
    ///
    /// Inputs: `reference` describes a locally committed CAS object.
    /// Outputs: success after encrypted availability publication is queued.
    /// Side effects: broadcasts metadata only and authorizes future requests for its meeting.
    pub fn announce_local_file(
        &mut self,
        reference: FileReference,
    ) -> Result<(), FileTransferError> {
        validate_reference(&reference)?;
        if !self.file_storage.has_file(reference.hash()) {
            return Err(FileTransferError::Invalid {
                reason: "cannot announce a missing local CAS object".to_owned(),
            });
        }
        self.authorize_hash_for_meeting(reference.hash(), reference.meeting_id().clone());
        self.file_transfer_network
            .broadcast_availability(vec![reference])?;
        Ok(())
    }

    /// Serves a requester with ordered explicit-header chunks when CAS and meeting authorization allow it.
    ///
    /// Inputs: `request` comes from an authenticated peer via the dedicated file-transfer protocol.
    /// Outputs: success after all chunks are queued, otherwise a portable error.
    /// Side effects: reads one CAS object and writes bounded encrypted frames through the network port.
    pub fn handle_request(
        &mut self,
        request: FileTransferRequest,
    ) -> Result<(), FileTransferError> {
        if !self
            .authorized_meetings_by_hash
            .contains_key(request.hash())
            || !self.file_storage.has_file(request.hash())
        {
            return Err(FileTransferError::Unauthorized);
        }
        let file_bytes = self.file_storage.load_file(request.hash())?;
        let total_size =
            u64::try_from(file_bytes.len()).map_err(|_| FileTransferError::Invalid {
                reason: "file length exceeds u64".to_owned(),
            })?;
        if total_size > MAX_FILE_TRANSFER_BYTES {
            return Err(FileTransferError::Invalid {
                reason: "file exceeds maximum transfer size".to_owned(),
            });
        }
        if sha256_hex(&file_bytes) != request.hash() {
            return Err(FileTransferError::Invalid {
                reason: "CAS content does not match requested hash".to_owned(),
            });
        }
        if file_bytes.is_empty() {
            self.file_transfer_network.send_chunk(
                request.requester_peer(),
                FileChunk::new(
                    request.hash(),
                    0,
                    FILE_TRANSFER_CHUNK_BYTES as u32,
                    0,
                    true,
                    Vec::new(),
                ),
            )?;
            return Ok(());
        }
        for (chunk_index, chunk_data) in file_bytes.chunks(FILE_TRANSFER_CHUNK_BYTES).enumerate() {
            let offset = u64::try_from(chunk_index * FILE_TRANSFER_CHUNK_BYTES).map_err(|_| {
                FileTransferError::Invalid {
                    reason: "chunk offset exceeds u64".to_owned(),
                }
            })?;
            let is_final =
                offset + u64::try_from(chunk_data.len()).unwrap_or(u64::MAX) == total_size;
            self.file_transfer_network.send_chunk(
                request.requester_peer(),
                FileChunk::new(
                    request.hash(),
                    total_size,
                    FILE_TRANSFER_CHUNK_BYTES as u32,
                    offset,
                    is_final,
                    chunk_data.to_vec(),
                ),
            )?;
        }
        Ok(())
    }

    /// Validates and accepts one contiguous inbound frame, atomically committing only verified files.
    ///
    /// Inputs: `peer` is authenticated and `file_chunk` is decoded by the dedicated protocol adapter.
    /// Outputs: success for accepted partial or committed complete data, otherwise a rejected transfer error.
    /// Side effects: retains at most two incomplete files in memory and writes CAS only after SHA-256 verification.
    pub fn receive_chunk(
        &mut self,
        peer: &DeviceId,
        file_chunk: FileChunk,
    ) -> Result<(), FileTransferError> {
        validate_chunk(&file_chunk)?;
        let key = (peer.clone(), file_chunk.hash().to_owned());
        if !self.receiving_transfers.contains_key(&key)
            && self.receiving_transfers.len() >= MAX_CONCURRENT_TRANSFERS
        {
            return Err(FileTransferError::Invalid {
                reason: "maximum concurrent transfers reached".to_owned(),
            });
        }
        let transfer = self
            .receiving_transfers
            .entry(key.clone())
            .or_insert_with(|| ReceivingTransfer {
                total_size: file_chunk.total_size(),
                next_offset: 0,
                bytes: Vec::new(),
            });
        if transfer.total_size != file_chunk.total_size()
            || transfer.next_offset != file_chunk.offset()
        {
            self.receiving_transfers.remove(&key);
            return Err(FileTransferError::Invalid {
                reason: "non-contiguous or inconsistent chunk stream".to_owned(),
            });
        }
        transfer.bytes.extend_from_slice(file_chunk.data());
        transfer.next_offset = transfer
            .next_offset
            .checked_add(u64::try_from(file_chunk.data().len()).unwrap_or(u64::MAX))
            .ok_or_else(|| FileTransferError::Invalid {
                reason: "received byte count overflow".to_owned(),
            })?;
        if transfer.next_offset > transfer.total_size {
            self.receiving_transfers.remove(&key);
            return Err(FileTransferError::Invalid {
                reason: "received more bytes than announced".to_owned(),
            });
        }
        if !file_chunk.is_final() {
            if transfer.next_offset == transfer.total_size {
                self.receiving_transfers.remove(&key);
                return Err(FileTransferError::Invalid {
                    reason: "complete stream missing final flag".to_owned(),
                });
            }
            return Ok(());
        }
        let completed =
            self.receiving_transfers
                .remove(&key)
                .ok_or_else(|| FileTransferError::Invalid {
                    reason: "missing transfer state".to_owned(),
                })?;
        if completed.next_offset != completed.total_size {
            return Err(FileTransferError::Invalid {
                reason: "short final stream".to_owned(),
            });
        }
        if sha256_hex(&completed.bytes) != file_chunk.hash() {
            return Err(FileTransferError::Invalid {
                reason: "SHA-256 mismatch".to_owned(),
            });
        }
        self.file_storage
            .save_file(file_chunk.hash(), &completed.bytes)?;
        tracing::info!(peer_id = %peer.as_str(), hash = %file_chunk.hash(), size_bytes = completed.total_size, "Completed verified attachment transfer");
        Ok(())
    }
}

fn validate_reference(reference: &FileReference) -> Result<(), FileTransferError> {
    if !is_sha256(reference.hash()) || reference.size_bytes() > MAX_FILE_TRANSFER_BYTES {
        return Err(FileTransferError::Invalid {
            reason: "invalid hash or file size".to_owned(),
        });
    }
    Ok(())
}
fn validate_chunk(chunk: &FileChunk) -> Result<(), FileTransferError> {
    if !is_sha256(chunk.hash())
        || chunk.total_size() > MAX_FILE_TRANSFER_BYTES
        || chunk.chunk_size() as usize != FILE_TRANSFER_CHUNK_BYTES
        || chunk.data().len() > FILE_TRANSFER_CHUNK_BYTES
        || chunk.offset() > chunk.total_size()
    {
        return Err(FileTransferError::Invalid {
            reason: "invalid frame header or oversized payload".to_owned(),
        });
    }
    if chunk
        .offset()
        .checked_add(u64::try_from(chunk.data().len()).unwrap_or(u64::MAX))
        .is_none_or(|end| end > chunk.total_size())
    {
        return Err(FileTransferError::Invalid {
            reason: "frame exceeds declared total size".to_owned(),
        });
    }
    Ok(())
}
fn is_sha256(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
}
/// Computes the lowercase SHA-256 content address used by every CAS object.
pub fn sha256_hex(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
