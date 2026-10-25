//! Declares persistence ports so use cases never depend directly on SQLite or CRDT libraries.

use crate::domain::{
    errors::StorageError,
    models::{MeetingId, Transcript},
};

/// Persists meeting text in an offline-capable repository.
pub trait NoteRepository {
    /// Creates a meeting record before its note or CRDT document can be changed.
    ///
    /// Inputs: `meeting_id` must be globally stable; `initial_crdt_document` must be a valid serialized document.
    /// Outputs: Success or a portable storage error, including duplicate-meeting rejection.
    /// Side effects: An implementation persists the initial local meeting state.
    fn create_meeting(
        &self,
        meeting_id: &MeetingId,
        initial_crdt_document: &[u8],
    ) -> Result<(), StorageError>;

    /// Stores a transcript transactionally according to the adapter's synchronization strategy.
    ///
    /// Inputs: `transcript` must contain normalized content and its owning meeting identifier.
    /// Outputs: Success or a portable storage error.
    /// Side effects: An implementation writes encrypted local state and may enqueue CRDT sync.
    fn save_transcript(&self, transcript: &Transcript) -> Result<(), StorageError>;

    /// Loads a transcript for an existing meeting, if one has been generated.
    ///
    /// Inputs: `meeting_id` identifies a previously created meeting.
    /// Outputs: The stored transcript, `None` when it has not yet been saved, or a storage error.
    /// Side effects: An implementation reads local persistent state.
    fn load_transcript(&self, meeting_id: &MeetingId) -> Result<Option<Transcript>, StorageError>;

    /// Replaces a meeting's serialized CRDT document.
    ///
    /// Inputs: `meeting_id` must already exist; `crdt_document` is opaque adapter-safe CRDT state.
    /// Outputs: Success or a storage error, including unknown-meeting rejection.
    /// Side effects: An implementation writes local persistent state and may enqueue synchronization.
    fn save_crdt_document(
        &self,
        meeting_id: &MeetingId,
        crdt_document: &[u8],
    ) -> Result<(), StorageError>;

    /// Loads the serialized CRDT document for an existing meeting.
    ///
    /// Inputs: `meeting_id` identifies a previously created meeting.
    /// Outputs: Opaque CRDT bytes or a portable storage error.
    /// Side effects: An implementation reads local persistent state.
    fn load_crdt_document(&self, meeting_id: &MeetingId) -> Result<Vec<u8>, StorageError>;
}
