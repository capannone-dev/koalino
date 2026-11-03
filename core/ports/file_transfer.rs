//! Declares how attachments travel between paired devices (Issue #57).
//!
//! `FileTransferNetwork` moves chunks over the dedicated encrypted protocol, and
//! `AttachmentLibrary` is the local side: what can be shared, and where arriving files go.

use crate::domain::{
    errors::{NetworkError, StorageError},
    models::{AttachmentOwner, DeviceId, FileChunk, MeetingId, SharedAttachment},
};
use uuid::Uuid;

/// The most file bytes one chunk carries.
pub const FILE_TRANSFER_CHUNK_BYTES: usize = 64 * 1024;
/// The largest attachment file accepted, matching what can be attached locally.
pub const MAX_FILE_TRANSFER_BYTES: u64 = 100 * 1024 * 1024;

/// What arrived on the file-transfer protocol from an authenticated, paired device.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileTransferEvent {
    /// A device asks for the chunk of the file `hash` starting at `offset`.
    ChunkRequested {
        /// The device asking.
        peer: DeviceId,
        /// The file's content address.
        hash: String,
        /// Where the chunk starts.
        offset: u64,
    },
    /// A device sent a chunk.
    ChunkReceived {
        /// The device that sent it.
        peer: DeviceId,
        /// The chunk, not yet validated.
        chunk: FileChunk,
    },
}

/// Exchanges attachment chunks without exposing libp2p types. The receiver asks for one chunk at
/// a time, so a large file cannot flood the connection or time out while queued.
pub trait FileTransferNetwork {
    /// Asks a paired device for the chunk of `hash` starting at `offset`.
    ///
    /// Outputs: Success once queued, or an error when the device is unpaired or disconnected.
    /// Side effects: Queues one encrypted frame.
    fn request_file_chunk(
        &mut self,
        hash: &str,
        offset: u64,
        from_peer: &DeviceId,
    ) -> Result<(), NetworkError>;

    /// Sends one chunk to a paired device.
    ///
    /// Outputs: Success once queued, or an error when the device is unpaired or disconnected.
    /// Side effects: Queues one encrypted frame.
    fn send_file_chunk(
        &mut self,
        to_peer: &DeviceId,
        chunk: &FileChunk,
    ) -> Result<(), NetworkError>;

    /// Takes the next received event, if any. Side effects: Removes it from the queue.
    fn poll_file_transfer(&mut self) -> Option<FileTransferEvent>;
}

/// The attachments this device holds, and where files from paired devices are stored.
pub trait AttachmentLibrary {
    /// Removes the files this library stored for a meeting that was deleted.
    ///
    /// Inputs: The deleted meeting. Outputs: Success, or a storage error.
    /// Side effects: Deletes only the meeting's folder in the private attachment store, so a file
    /// the owner linked from elsewhere stays. Libraries without files do nothing.
    fn remove_meeting_files(&mut self, _meeting_id: &MeetingId) -> Result<(), StorageError> {
        Ok(())
    }

    /// Removes the resource files this library stored for a project that was deleted (Issue #85).
    ///
    /// Inputs: The deleted project. Outputs: Success, or a storage error.
    /// Side effects: Deletes only the project's folder in the private attachment store. Libraries
    /// without files do nothing.
    fn remove_project_files(
        &mut self,
        _project_id: crate::domain::projects::ProjectId,
    ) -> Result<(), StorageError> {
        Ok(())
    }

    /// Computes the content hash of one linked attachment that has none yet.
    ///
    /// Outputs: Success (also when there is nothing to do), or a storage error.
    /// Side effects: Reads one file and writes its hash. A file that cannot be read is skipped.
    fn hash_next_attachment(&mut self) -> Result<(), StorageError>;

    /// Lists the attachments linked to a meeting, and project resources, whose content hash is
    /// known.
    ///
    /// Outputs: Metadata ordered by attachment ID, or a storage error. Side effects: Reads rows.
    fn shared_attachments(&self) -> Result<Vec<SharedAttachment>, StorageError>;

    /// Tells whether this attachment is already stored here. Side effects: Reads one row.
    fn has_attachment(&self, attachment_id: Uuid) -> Result<bool, StorageError>;

    /// Tells whether the meeting or project an attachment belongs to has synced here.
    /// Side effects: Reads one row.
    fn has_owner(&self, owner: &AttachmentOwner) -> Result<bool, StorageError>;

    /// Reads the chunk of a file linked to a meeting.
    ///
    /// Outputs: The chunk, `None` when no linked attachment has this hash or `offset` is past the
    /// end (so nothing else on disk can be asked for), or a storage error.
    /// Side effects: Reads part of one file.
    fn read_chunk(&self, hash: &str, offset: u64) -> Result<Option<FileChunk>, StorageError>;

    /// Starts receiving a file, discarding any earlier partial copy.
    ///
    /// Outputs: Success, or a rejection for an unusable meeting ID, type, or size.
    /// Side effects: Creates private folders and an empty partial file.
    fn begin_incoming(&mut self, attachment: &SharedAttachment) -> Result<(), StorageError>;

    /// Adds received bytes at the end of the partial file.
    ///
    /// Side effects: Writes to the partial file.
    fn append_incoming(
        &mut self,
        attachment: &SharedAttachment,
        data: &[u8],
    ) -> Result<(), StorageError>;

    /// Verifies the complete partial file and stores it as an attachment of its meeting.
    ///
    /// Outputs: Success, or a rejection when the size or SHA-256 differs from the metadata.
    /// Side effects: Moves the file to its place and writes the row; removes the file on failure.
    fn finish_incoming(&mut self, attachment: &SharedAttachment) -> Result<(), StorageError>;

    /// Drops a partial file. Side effects: Removes it; a failure is only logged.
    fn discard_incoming(&mut self, attachment: &SharedAttachment);
}
