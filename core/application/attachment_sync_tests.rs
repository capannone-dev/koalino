//! Specifies attachment transfers against an in-memory library and network: files are pulled
//! chunk by chunk and stored once verified, wait for their meeting, resume from nothing after a
//! lost device, retry a silent one a limited number of times, and refuse bad data.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::attachment_sync::{
    AttachmentSync, MAX_CHUNK_RETRIES, MAX_CONCURRENT_TRANSFERS, STALLED_CHUNK_TIMEOUT,
};
use crate::{
    domain::{
        errors::{NetworkError, StorageError},
        models::{AttachmentOwner, DeviceId, FileChunk, MeetingId, SharedAttachment},
        projects::ProjectId,
    },
    ports::file_transfer::{
        AttachmentLibrary, FILE_TRANSFER_CHUNK_BYTES, FileTransferEvent, FileTransferNetwork,
    },
};

/// The library's state, shared with the test.
#[derive(Default)]
struct LibraryState {
    meetings: HashSet<String>,
    projects: HashSet<ProjectId>,
    removed_resources: Vec<Uuid>,
    shared: Vec<SharedAttachment>,
    served: HashMap<String, Vec<u8>>,
    partial: HashMap<Uuid, Vec<u8>>,
    stored: HashMap<Uuid, Vec<u8>>,
}

/// An in-memory `AttachmentLibrary`.
#[derive(Clone, Default)]
struct FakeLibrary(Arc<Mutex<LibraryState>>);

impl FakeLibrary {
    /// Runs `action` on the state.
    fn with<T>(&self, action: impl FnOnce(&mut LibraryState) -> T) -> T {
        action(
            &mut self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
}

impl AttachmentLibrary for FakeLibrary {
    fn hash_next_attachment(&mut self) -> Result<(), StorageError> {
        Ok(())
    }
    fn shared_attachments(&self) -> Result<Vec<SharedAttachment>, StorageError> {
        Ok(self.with(|state| state.shared.clone()))
    }
    fn has_attachment(&self, attachment_id: Uuid) -> Result<bool, StorageError> {
        Ok(self.with(|state| state.stored.contains_key(&attachment_id)))
    }
    fn has_owner(&self, owner: &AttachmentOwner) -> Result<bool, StorageError> {
        Ok(self.with(|state| match owner {
            AttachmentOwner::Meeting(meeting_id) => state.meetings.contains(meeting_id.as_str()),
            AttachmentOwner::Project(project_id) => state.projects.contains(project_id),
        }))
    }
    fn read_chunk(&self, hash: &str, offset: u64) -> Result<Option<FileChunk>, StorageError> {
        Ok(self.with(|state| {
            let contents = state.served.get(hash)?;
            let start = usize::try_from(offset)
                .ok()
                .filter(|start| *start <= contents.len())?;
            let end = (start + FILE_TRANSFER_CHUNK_BYTES).min(contents.len());
            Some(FileChunk::new(
                hash,
                contents.len() as u64,
                offset,
                contents[start..end].to_vec(),
            ))
        }))
    }
    fn begin_incoming(&mut self, attachment: &SharedAttachment) -> Result<(), StorageError> {
        self.with(|state| state.partial.insert(attachment.attachment_id, Vec::new()));
        Ok(())
    }
    fn append_incoming(
        &mut self,
        attachment: &SharedAttachment,
        data: &[u8],
    ) -> Result<(), StorageError> {
        self.with(|state| {
            state
                .partial
                .entry(attachment.attachment_id)
                .or_default()
                .extend_from_slice(data);
        });
        Ok(())
    }
    fn finish_incoming(&mut self, attachment: &SharedAttachment) -> Result<(), StorageError> {
        self.with(|state| {
            let contents = state
                .partial
                .remove(&attachment.attachment_id)
                .unwrap_or_default();
            if format!("{:x}", Sha256::digest(&contents)) != attachment.content_hash {
                return Err(StorageError::Rejected {
                    reason: "mismatch".to_owned(),
                });
            }
            state.stored.insert(attachment.attachment_id, contents);
            Ok(())
        })
    }
    fn discard_incoming(&mut self, attachment: &SharedAttachment) {
        self.with(|state| state.partial.remove(&attachment.attachment_id));
    }
    fn remove_resource_file(
        &mut self,
        attachment_id: Uuid,
        _project_id: ProjectId,
    ) -> Result<(), StorageError> {
        self.with(|state| state.removed_resources.push(attachment_id));
        Ok(())
    }
}

/// Records requests and chunks instead of sending them.
#[derive(Default)]
struct FakeNetwork {
    requests: VecDeque<(String, u64, DeviceId)>,
    chunks: VecDeque<(DeviceId, FileChunk)>,
}

impl FileTransferNetwork for FakeNetwork {
    fn request_file_chunk(
        &mut self,
        hash: &str,
        offset: u64,
        from_peer: &DeviceId,
    ) -> Result<(), NetworkError> {
        self.requests
            .push_back((hash.to_owned(), offset, from_peer.clone()));
        Ok(())
    }
    fn send_file_chunk(
        &mut self,
        to_peer: &DeviceId,
        chunk: &FileChunk,
    ) -> Result<(), NetworkError> {
        self.chunks.push_back((to_peer.clone(), chunk.clone()));
        Ok(())
    }
    fn poll_file_transfer(&mut self) -> Option<FileTransferEvent> {
        None
    }
}

/// The device files come from.
fn laptop() -> DeviceId {
    DeviceId::new("laptop")
}

/// Metadata of attachment `number` of `meeting-1` with these contents.
fn attachment(number: u128, contents: &[u8]) -> SharedAttachment {
    SharedAttachment {
        attachment_id: Uuid::from_u128(number),
        owner: AttachmentOwner::Meeting(MeetingId::new("meeting-1")),
        file_name: "a.png".to_owned(),
        mime_type: "image/png".to_owned(),
        content_hash: format!("{:x}", Sha256::digest(contents)),
        size_bytes: contents.len() as u64,
        original_timestamp: Utc::now(),
        original_device_id: "laptop".to_owned(),
        meeting_time_ms: None,
    }
}

/// A tracker over a library holding `meeting-1`.
fn receiver() -> (AttachmentSync, FakeLibrary) {
    let library = FakeLibrary::default();
    library.with(|state| state.meetings.insert("meeting-1".to_owned()));
    (AttachmentSync::new(Box::new(library.clone())), library)
}

/// The chunk of `contents` at `offset`.
fn chunk_of(contents: &[u8], hash: &str, offset: usize) -> FileChunk {
    let end = (offset + FILE_TRANSFER_CHUNK_BYTES).min(contents.len());
    FileChunk::new(
        hash,
        contents.len() as u64,
        offset as u64,
        contents[offset..end].to_vec(),
    )
}

/// Answers every queued request from `contents` until none are left.
fn answer_requests(sync: &mut AttachmentSync, network: &mut FakeNetwork, contents: &[u8]) {
    while let Some((hash, offset, peer)) = network.requests.pop_front() {
        sync.handle_event(
            network,
            FileTransferEvent::ChunkReceived {
                peer,
                chunk: chunk_of(contents, &hash, offset as usize),
            },
        );
    }
}

#[test]
fn a_file_is_pulled_chunk_by_chunk_and_stored_once_verified() {
    let (mut sync, library) = receiver();
    let mut network = FakeNetwork::default();
    let contents = vec![9_u8; FILE_TRANSFER_CHUNK_BYTES * 2 + 5];
    let offered = attachment(1, &contents);
    sync.accept_manifest(&laptop(), vec![offered.clone()]);

    sync.pump(&mut network, Instant::now());
    assert_eq!(network.requests.len(), 1, "one chunk at a time");
    answer_requests(&mut sync, &mut network, &contents);

    assert_eq!(
        library.with(|state| state.stored.get(&offered.attachment_id).cloned()),
        Some(contents)
    );
}

#[test]
fn an_empty_file_arrives() {
    let (mut sync, library) = receiver();
    let mut network = FakeNetwork::default();
    let offered = attachment(1, b"");
    sync.accept_manifest(&laptop(), vec![offered.clone()]);

    sync.pump(&mut network, Instant::now());
    answer_requests(&mut sync, &mut network, b"");

    assert_eq!(
        library.with(|state| state.stored.get(&offered.attachment_id).cloned()),
        Some(Vec::new())
    );
}

#[test]
fn an_attachment_already_here_is_not_fetched() {
    let (mut sync, library) = receiver();
    let mut network = FakeNetwork::default();
    let offered = attachment(1, b"x");
    library.with(|state| state.stored.insert(offered.attachment_id, b"x".to_vec()));

    sync.accept_manifest(&laptop(), vec![offered]);
    sync.pump(&mut network, Instant::now());

    assert!(network.requests.is_empty());
}

#[test]
fn an_attachment_waits_for_its_meeting_to_sync() {
    let (mut sync, library) = receiver();
    let mut network = FakeNetwork::default();
    library.with(|state| state.meetings.clear());
    sync.accept_manifest(&laptop(), vec![attachment(1, b"x")]);

    sync.pump(&mut network, Instant::now());
    assert!(network.requests.is_empty());

    library.with(|state| state.meetings.insert("meeting-1".to_owned()));
    sync.pump(&mut network, Instant::now());
    assert_eq!(network.requests.len(), 1);
}

#[test]
fn a_project_resource_waits_for_its_project_and_is_dropped_when_it_is_deleted() {
    let (mut sync, library) = receiver();
    let mut network = FakeNetwork::default();
    let project_id = ProjectId::from_uuid(Uuid::from_u128(85));
    let mut resource = attachment(1, b"notes");
    resource.owner = AttachmentOwner::Project(project_id);
    sync.accept_manifest(&laptop(), vec![resource.clone()]);

    sync.pump(&mut network, Instant::now());
    assert!(
        network.requests.is_empty(),
        "the project has not synced yet"
    );

    sync.project_deleted(project_id);
    library.with(|state| state.projects.insert(project_id));
    sync.pump(&mut network, Instant::now());
    assert!(
        network.requests.is_empty(),
        "a deleted project's resource is no longer wanted"
    );

    sync.accept_manifest(&laptop(), vec![resource.clone()]);
    sync.pump(&mut network, Instant::now());
    answer_requests(&mut sync, &mut network, b"notes");
    assert_eq!(
        library.with(|state| state.stored.get(&resource.attachment_id).cloned()),
        Some(b"notes".to_vec())
    );
}

#[test]
fn a_resource_deleted_by_a_peer_is_no_longer_wanted_and_its_file_goes() {
    let (mut sync, library) = receiver();
    let mut network = FakeNetwork::default();
    let project_id = ProjectId::from_uuid(Uuid::from_u128(85));
    let mut resource = attachment(1, b"notes");
    resource.owner = AttachmentOwner::Project(project_id);
    sync.accept_manifest(&laptop(), vec![resource.clone()]);

    sync.resource_deleted(resource.attachment_id, project_id);
    library.with(|state| state.projects.insert(project_id));
    sync.pump(&mut network, Instant::now());

    assert!(network.requests.is_empty());
    assert_eq!(
        library.with(|state| state.removed_resources.clone()),
        vec![resource.attachment_id]
    );
}

#[test]
fn transfers_run_within_the_limit_and_the_next_starts_when_one_ends() {
    let (mut sync, library) = receiver();
    let mut network = FakeNetwork::default();
    let files: Vec<Vec<u8>> = (0..=MAX_CONCURRENT_TRANSFERS as u8)
        .map(|byte| vec![byte; 4])
        .collect();
    let offers: Vec<_> = files
        .iter()
        .enumerate()
        .map(|(index, contents)| attachment(index as u128, contents))
        .collect();
    sync.accept_manifest(&laptop(), offers);

    sync.pump(&mut network, Instant::now());
    assert_eq!(network.requests.len(), MAX_CONCURRENT_TRANSFERS);

    while let Some((hash, offset, peer)) = network.requests.pop_front() {
        let contents = files
            .iter()
            .find(|contents| format!("{:x}", Sha256::digest(contents)) == hash)
            .unwrap_or_else(|| panic!("only offered files are requested"));
        sync.handle_event(
            &mut network,
            FileTransferEvent::ChunkReceived {
                peer,
                chunk: chunk_of(contents, &hash, offset as usize),
            },
        );
    }

    assert_eq!(library.with(|state| state.stored.len()), files.len());
}

#[test]
fn a_lost_device_cancels_its_transfer_and_its_next_manifest_restarts_it() {
    let (mut sync, library) = receiver();
    let mut network = FakeNetwork::default();
    let contents = vec![1_u8; FILE_TRANSFER_CHUNK_BYTES + 1];
    let offered = attachment(1, &contents);
    sync.accept_manifest(&laptop(), vec![offered.clone()]);
    sync.pump(&mut network, Instant::now());
    let (hash, _, _) = network
        .requests
        .pop_front()
        .unwrap_or_else(|| panic!("a request"));
    sync.handle_event(
        &mut network,
        FileTransferEvent::ChunkReceived {
            peer: laptop(),
            chunk: chunk_of(&contents, &hash, 0),
        },
    );

    sync.peer_lost(&laptop());
    network.requests.clear();

    assert!(library.with(|state| state.partial.is_empty()));
    sync.pump(&mut network, Instant::now());
    assert!(network.requests.is_empty(), "nobody offers it now");

    sync.accept_manifest(&laptop(), vec![offered.clone()]);
    sync.pump(&mut network, Instant::now());
    assert_eq!(network.requests.front().map(|request| request.1), Some(0));
    answer_requests(&mut sync, &mut network, &contents);
    assert_eq!(
        library.with(|state| state.stored.get(&offered.attachment_id).cloned()),
        Some(contents)
    );
}

#[test]
fn a_silent_device_is_asked_again_a_limited_number_of_times() {
    let (mut sync, library) = receiver();
    let mut network = FakeNetwork::default();
    let started = Instant::now();
    sync.accept_manifest(&laptop(), vec![attachment(1, b"x")]);
    sync.pump(&mut network, started);
    network.requests.clear();

    let mut time = started;
    for attempt in 1..=MAX_CHUNK_RETRIES {
        time += STALLED_CHUNK_TIMEOUT + Duration::from_secs(1);
        sync.pump(&mut network, time);
        assert_eq!(network.requests.len(), attempt as usize);
    }
    time += STALLED_CHUNK_TIMEOUT + Duration::from_secs(1);
    sync.pump(&mut network, time);

    assert_eq!(network.requests.len(), MAX_CHUNK_RETRIES as usize);
    assert!(library.with(|state| state.partial.is_empty()));
}

#[test]
fn a_late_duplicate_chunk_is_ignored() {
    let (mut sync, library) = receiver();
    let mut network = FakeNetwork::default();
    let contents = vec![3_u8; FILE_TRANSFER_CHUNK_BYTES + 1];
    let offered = attachment(1, &contents);
    sync.accept_manifest(&laptop(), vec![offered.clone()]);
    sync.pump(&mut network, Instant::now());
    let first = chunk_of(&contents, &offered.content_hash, 0);
    network.requests.clear();

    for _ in 0..2 {
        sync.handle_event(
            &mut network,
            FileTransferEvent::ChunkReceived {
                peer: laptop(),
                chunk: first.clone(),
            },
        );
    }

    assert_eq!(network.requests.len(), 1);
    assert_eq!(
        library.with(|state| state.partial[&offered.attachment_id].len()),
        FILE_TRANSFER_CHUNK_BYTES
    );
}

#[test]
fn a_chunk_from_a_device_not_serving_the_file_is_ignored() {
    let (mut sync, library) = receiver();
    let mut network = FakeNetwork::default();
    let offered = attachment(1, b"real");
    sync.accept_manifest(&laptop(), vec![offered.clone()]);
    sync.pump(&mut network, Instant::now());

    sync.handle_event(
        &mut network,
        FileTransferEvent::ChunkReceived {
            peer: DeviceId::new("other"),
            chunk: chunk_of(b"fake", &offered.content_hash, 0),
        },
    );

    assert!(
        library.with(
            |state| state.stored.is_empty() && state.partial[&offered.attachment_id].is_empty()
        )
    );
}

#[test]
fn chunks_that_break_the_limits_end_the_transfer() {
    let oversized = vec![0_u8; FILE_TRANSFER_CHUNK_BYTES + 1];
    let contents = vec![0_u8; FILE_TRANSFER_CHUNK_BYTES * 2];
    let hash = attachment(1, &contents).content_hash;
    for bad_chunk in [
        FileChunk::new(hash.clone(), contents.len() as u64 + 1, 0, vec![0; 4]),
        FileChunk::new(hash.clone(), contents.len() as u64, 0, oversized.clone()),
        FileChunk::new(hash.clone(), contents.len() as u64, 0, Vec::new()),
    ] {
        let (mut sync, library) = receiver();
        let mut network = FakeNetwork::default();
        sync.accept_manifest(&laptop(), vec![attachment(1, &contents)]);
        sync.pump(&mut network, Instant::now());
        network.requests.clear();

        sync.handle_event(
            &mut network,
            FileTransferEvent::ChunkReceived {
                peer: laptop(),
                chunk: bad_chunk,
            },
        );
        sync.pump(&mut network, Instant::now());

        assert!(
            network.requests.is_empty(),
            "the source is dropped until it reconnects"
        );
        assert!(library.with(|state| state.partial.is_empty() && state.stored.is_empty()));
    }
}

#[test]
fn a_file_failing_its_check_is_not_fetched_again_until_the_device_reconnects() {
    let (mut sync, library) = receiver();
    let mut network = FakeNetwork::default();
    let offered = attachment(1, b"real");
    sync.accept_manifest(&laptop(), vec![offered.clone()]);
    sync.pump(&mut network, Instant::now());
    network.requests.clear();

    sync.handle_event(
        &mut network,
        FileTransferEvent::ChunkReceived {
            peer: laptop(),
            chunk: chunk_of(b"fake", &offered.content_hash, 0),
        },
    );
    sync.pump(&mut network, Instant::now());

    assert!(network.requests.is_empty());
    assert!(library.with(|state| state.stored.is_empty()));
}

#[test]
fn only_files_the_library_shares_are_served() {
    let (mut sync, library) = receiver();
    let mut network = FakeNetwork::default();
    library.with(|state| state.served.insert("a".repeat(64), b"shared".to_vec()));

    sync.handle_event(
        &mut network,
        FileTransferEvent::ChunkRequested {
            peer: laptop(),
            hash: "b".repeat(64),
            offset: 0,
        },
    );
    assert!(network.chunks.is_empty());

    sync.handle_event(
        &mut network,
        FileTransferEvent::ChunkRequested {
            peer: laptop(),
            hash: "a".repeat(64),
            offset: 0,
        },
    );
    assert_eq!(
        network
            .chunks
            .pop_front()
            .map(|(_, chunk)| chunk.data().to_vec()),
        Some(b"shared".to_vec())
    );
}

#[test]
fn a_device_is_sent_the_list_once_until_it_changes_or_reconnects() {
    let (mut sync, library) = receiver();
    let connected = HashSet::from([laptop()]);
    library.with(|state| state.shared.push(attachment(1, b"x")));

    let first = sync
        .pending_manifest(&connected)
        .unwrap_or_else(|| panic!("a first send"));
    sync.mark_shared(laptop(), first.digest);
    assert!(sync.pending_manifest(&connected).is_none());

    library.with(|state| state.shared.push(attachment(2, b"y")));
    let changed = sync
        .pending_manifest(&connected)
        .unwrap_or_else(|| panic!("a changed list"));
    sync.mark_shared(laptop(), changed.digest);
    assert!(sync.pending_manifest(&connected).is_none());

    sync.forget_shared(&laptop());
    assert!(sync.pending_manifest(&connected).is_some());
    assert!(sync.pending_manifest(&HashSet::new()).is_none());
}
