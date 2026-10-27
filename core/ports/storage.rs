//! Declares persistence ports so use cases never depend directly on SQLite or CRDT libraries.

use crate::domain::{
    errors::StorageError,
    models::{
        Attachment, MeetingId, MeetingMetadata, MeetingWindow, Transcript, TranscriptSegment,
        UnassignedVoiceClip,
    },
};
use chrono::{DateTime, Utc};

/// Persists meeting text in an offline-capable repository.
pub trait NoteRepository {
    /// Lists stable identifiers for every locally known meeting.
    ///
    /// Inputs: None.
    /// Outputs: Meeting identifiers in adapter-defined stable order, or a storage error.
    /// Side effects: Reads local meeting metadata only; it never reads transcript or CRDT payloads.
    fn list_meeting_ids(&self) -> Result<Vec<MeetingId>, StorageError> {
        Ok(Vec::new())
    }

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

    /// Persists a temporary audio clip for a weak or absent speaker match.
    ///
    /// Inputs: `clip` belongs to an existing meeting and contains sensitive temporary review audio.
    /// Outputs: Success or a portable storage error.
    /// Side effects: Writes temporary sensitive data; adapters must not log audio bytes.
    fn save_unassigned_clip(&self, clip: UnassignedVoiceClip) -> Result<(), StorageError>;

    /// Retrieves all unresolved review clips owned by a meeting.
    ///
    /// Inputs: `meeting_id` identifies an existing meeting.
    /// Outputs: Temporary unassigned clips or a portable storage error.
    /// Side effects: Reads sensitive temporary audio; callers must not log returned bytes.
    fn get_unassigned_clips(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Vec<UnassignedVoiceClip>, StorageError>;

    /// Retrieves recorded meeting windows containing an attachment capture timestamp.
    ///
    /// Inputs: `timestamp` is a UTC attachment capture time obtained through `MetadataExtractor`.
    /// Outputs: Candidate meeting windows containing that instant, or a storage error.
    /// Side effects: Reads local meeting timeline metadata only.
    fn get_meetings_overlapping(
        &self,
        _timestamp: DateTime<Utc>,
    ) -> Result<Vec<MeetingWindow>, StorageError> {
        Ok(Vec::new())
    }

    /// Loads content-independent facts about one meeting, such as its creation time.
    ///
    /// Inputs: `meeting_id` identifies the requested meeting.
    /// Outputs: Metadata, `None` when the meeting does not exist, or a storage error.
    /// Side effects: Reads local meeting metadata only; it never reads transcript or CRDT payloads.
    fn load_meeting_metadata(
        &self,
        _meeting_id: &MeetingId,
    ) -> Result<Option<MeetingMetadata>, StorageError> {
        Ok(None)
    }

    /// Replaces every speaker-attributed transcript segment of one existing meeting.
    ///
    /// Inputs: `meeting_id` must exist; every element of `segments` must belong to that meeting.
    /// Outputs: Success or a storage error, including unknown-meeting rejection.
    /// Side effects: Transactionally deletes and rewrites the meeting's local segment rows.
    fn save_transcript_segments(
        &self,
        _meeting_id: &MeetingId,
        _segments: &[TranscriptSegment],
    ) -> Result<(), StorageError> {
        Err(StorageError::Unavailable)
    }

    /// Loads one meeting's speaker-attributed transcript segments.
    ///
    /// Inputs: `meeting_id` identifies the meeting.
    /// Outputs: Segments ordered by start time (empty when none were stored), or a storage error.
    /// Side effects: Reads local transcript segment rows.
    fn load_transcript_segments(
        &self,
        _meeting_id: &MeetingId,
    ) -> Result<Vec<TranscriptSegment>, StorageError> {
        Ok(Vec::new())
    }
}

/// Persists attachment metadata independently from filesystem metadata extraction.
pub trait AttachmentRepository {
    /// Saves an attachment record, whether already linked or awaiting user confirmation.
    ///
    /// Inputs: `attachment` contains no file bytes, only metadata and an optional meeting ID.
    /// Outputs: Success or a portable storage error.
    /// Side effects: Persists attachment metadata locally and may enqueue CRDT synchronization.
    fn save_attachment(&self, attachment: &Attachment) -> Result<(), StorageError>;

    /// Returns every attachment explicitly linked to one meeting.
    ///
    /// Inputs: `meeting_id` identifies the meeting whose attachments are requested.
    /// Outputs: Attachment metadata ordered by adapter policy, or a storage error.
    /// Side effects: Reads local attachment metadata without opening attachment files.
    fn get_attachments_for_meeting(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Vec<Attachment>, StorageError>;

    /// Returns attachment records still awaiting a user-selected meeting link.
    ///
    /// Inputs: None.
    /// Outputs: Unlinked attachment metadata, or a storage error.
    /// Side effects: Reads local attachment metadata without opening attachment files.
    fn get_unlinked_attachments(&self) -> Result<Vec<Attachment>, StorageError>;
}
