//! Defines mock-first contracts for content-addressed attachment transfer orchestration.

use std::{cell::RefCell, collections::HashMap, path::PathBuf, rc::Rc};

use crate::{
    application::file_sync_manager::{FILE_TRANSFER_CHUNK_BYTES, FileSyncManager, sha256_hex},
    domain::{
        errors::{NetworkError, StorageError},
        models::{DeviceId, FileChunk, FileReference, FileTransferRequest, MeetingId},
    },
    ports::file_transfer::{FileStorage, FileTransferNetwork},
};

#[derive(Clone, Default)]
struct MockFileTransferNetwork {
    requested_hashes: Rc<RefCell<Vec<(String, DeviceId)>>>,
    announced_references: Rc<RefCell<Vec<FileReference>>>,
    sent_chunks: Rc<RefCell<Vec<(DeviceId, FileChunk)>>>,
}

impl FileTransferNetwork for MockFileTransferNetwork {
    fn request_file(&mut self, hash: &str, from_peer: &DeviceId) -> Result<(), NetworkError> {
        self.requested_hashes
            .borrow_mut()
            .push((hash.to_owned(), from_peer.clone()));
        Ok(())
    }

    fn broadcast_availability(
        &mut self,
        file_references: Vec<FileReference>,
    ) -> Result<(), NetworkError> {
        self.announced_references
            .borrow_mut()
            .extend(file_references);
        Ok(())
    }

    fn send_chunk(
        &mut self,
        to_peer: &DeviceId,
        file_chunk: FileChunk,
    ) -> Result<(), NetworkError> {
        self.sent_chunks
            .borrow_mut()
            .push((to_peer.clone(), file_chunk));
        Ok(())
    }
}

#[derive(Clone, Default)]
struct MockFileStorage {
    files_by_hash: Rc<RefCell<HashMap<String, Vec<u8>>>>,
    committed_hashes: Rc<RefCell<Vec<String>>>,
}

impl MockFileStorage {
    fn with_file(hash: &str, data: Vec<u8>) -> Self {
        let storage = Self::default();
        storage
            .files_by_hash
            .borrow_mut()
            .insert(hash.to_owned(), data);
        storage
    }
}

impl FileStorage for MockFileStorage {
    fn save_file(&self, hash: &str, data: &[u8]) -> Result<PathBuf, StorageError> {
        self.files_by_hash
            .borrow_mut()
            .insert(hash.to_owned(), data.to_vec());
        self.committed_hashes.borrow_mut().push(hash.to_owned());
        Ok(PathBuf::from(format!("/cas/{hash}")))
    }
    fn load_file(&self, hash: &str) -> Result<Vec<u8>, StorageError> {
        self.files_by_hash
            .borrow()
            .get(hash)
            .cloned()
            .ok_or(StorageError::Rejected {
                reason: "unknown hash".to_owned(),
            })
    }
    fn has_file(&self, hash: &str) -> bool {
        self.files_by_hash.borrow().contains_key(hash)
    }
    fn list_available_hashes(&self) -> Result<Vec<String>, StorageError> {
        Ok(self.files_by_hash.borrow().keys().cloned().collect())
    }
}

fn reference(hash: &str, size_bytes: u64) -> FileReference {
    FileReference::new(
        hash,
        size_bytes,
        "image/png",
        MeetingId::new("meeting-21"),
        "slide.png",
    )
}

#[test]
fn missing_file_requests_advertising_peer_while_present_file_skips_request() {
    let network = MockFileTransferNetwork::default();
    let storage = MockFileStorage::default();
    let requested_hashes = network.requested_hashes.clone();
    let peer = DeviceId::new("peer-a");
    let mut manager = FileSyncManager::new(network, storage);
    manager.record_availability(peer.clone(), vec![reference("a".repeat(64).as_str(), 1)]);
    manager
        .handle_file_reference(reference("a".repeat(64).as_str(), 1))
        .unwrap_or_else(|error| panic!("request should queue: {error}"));
    assert_eq!(*requested_hashes.borrow(), vec![("a".repeat(64), peer)]);
}

#[test]
fn incoming_request_chunks_authorized_cas_file_in_order_including_exact_multiple() {
    let data = vec![42_u8; FILE_TRANSFER_CHUNK_BYTES * 2];
    let hash = sha256_hex(&data);
    let network = MockFileTransferNetwork::default();
    let sent_chunks = network.sent_chunks.clone();
    let mut manager = FileSyncManager::new(network, MockFileStorage::with_file(&hash, data));
    manager.authorize_hash_for_meeting(&hash, MeetingId::new("meeting-21"));
    manager
        .handle_request(FileTransferRequest::new(
            hash.clone(),
            DeviceId::new("peer-b"),
        ))
        .unwrap_or_else(|error| panic!("authorized transfer should succeed: {error}"));
    let chunks = sent_chunks.borrow();
    assert_eq!(chunks.len(), 2);
    assert!(chunks[1].1.is_final());
    assert_eq!(chunks[1].1.offset(), FILE_TRANSFER_CHUNK_BYTES as u64);
}

#[test]
fn zero_byte_file_is_one_final_explicit_frame() {
    let hash = sha256_hex(&[]);
    let network = MockFileTransferNetwork::default();
    let sent_chunks = network.sent_chunks.clone();
    let mut manager = FileSyncManager::new(network, MockFileStorage::with_file(&hash, Vec::new()));
    manager.authorize_hash_for_meeting(&hash, MeetingId::new("meeting-21"));
    manager
        .handle_request(FileTransferRequest::new(hash, DeviceId::new("peer-c")))
        .unwrap_or_else(|error| panic!("zero-byte transfer should succeed: {error}"));
    let chunks = sent_chunks.borrow();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].1.total_size(), 0);
    assert!(chunks[0].1.is_final());
}

#[test]
fn rejects_out_of_order_oversized_and_hash_mismatched_streams_without_commit() {
    let expected_hash = "d".repeat(64);
    let storage = MockFileStorage::default();
    let committed_hashes = storage.committed_hashes.clone();
    let mut manager = FileSyncManager::new(MockFileTransferNetwork::default(), storage);
    let peer = DeviceId::new("peer-d");
    assert!(
        manager
            .receive_chunk(
                &peer,
                FileChunk::new(expected_hash.clone(), 2, 1, 1, true, vec![1])
            )
            .is_err()
    );
    assert!(
        manager
            .receive_chunk(
                &peer,
                FileChunk::new(expected_hash, 1, 2, 0, true, vec![1, 2])
            )
            .is_err()
    );
    assert!(committed_hashes.borrow().is_empty());
}
