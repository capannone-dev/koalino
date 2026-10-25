//! Declares portable content-addressed file-transfer and CAS storage contracts.

use std::path::PathBuf;

use crate::domain::{
    errors::{NetworkError, StorageError},
    models::{DeviceId, FileChunk, FileReference},
};

/// Exchanges attachment-transfer control and data frames without exposing libp2p types.
pub trait FileTransferNetwork {
    /// Queues a request for one verified hash from an authenticated peer.
    fn request_file(&mut self, hash: &str, from_peer: &DeviceId) -> Result<(), NetworkError>;
    /// Broadcasts safe attachment availability metadata to currently connected peers.
    fn broadcast_availability(
        &mut self,
        file_references: Vec<FileReference>,
    ) -> Result<(), NetworkError>;
    /// Queues one explicit-header binary frame for an authenticated peer.
    fn send_chunk(&mut self, to_peer: &DeviceId, file_chunk: FileChunk)
    -> Result<(), NetworkError>;
}

/// Persists complete files in a verified content-addressed local store.
pub trait FileStorage {
    /// Atomically validates and commits `data` under its expected SHA-256 hash.
    fn save_file(&self, hash: &str, data: &[u8]) -> Result<PathBuf, StorageError>;
    /// Loads the exact bytes for an already-authorized content hash.
    fn load_file(&self, hash: &str) -> Result<Vec<u8>, StorageError>;
    /// Reports whether a verified object already exists locally.
    fn has_file(&self, hash: &str) -> bool;
    /// Lists verified hashes during startup availability-registry reconstruction.
    fn list_available_hashes(&self) -> Result<Vec<String>, StorageError>;
}
