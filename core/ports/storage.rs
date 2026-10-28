//! Declares persistence ports so use cases never depend directly on SQLite or CRDT libraries.

use crate::domain::{
    errors::StorageError,
    models::{
        Attachment, MeetingId, MeetingMetadata, MeetingTitle, MeetingWindow, Transcript,
        TranscriptSegment, UnassignedVoiceClip, UnknownSpeakerChunk, UnknownSpeakerCluster,
    },
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

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

    /// Stores or replaces the user-chosen title of one existing meeting.
    ///
    /// Inputs: `meeting_id` must exist; `title` is validated user content and must not be logged.
    /// Outputs: Success, an unknown-meeting rejection, or `Unavailable` for repositories that do
    /// not store titles.
    /// Side effects: Writes local meeting metadata only. This column is the local projection of
    /// the replicated CRDT title (Issue #39): callers that want the title to reach peers use
    /// `MeetingCrdtService::set_meeting_title`, which writes both.
    fn save_meeting_title(
        &self,
        _meeting_id: &MeetingId,
        _title: &MeetingTitle,
    ) -> Result<(), StorageError> {
        Err(StorageError::Unavailable)
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

    /// Upserts an unknown-speaker cluster and records one new member chunk atomically.
    ///
    /// Inputs: `cluster` carries the updated representative embedding and totals; its meeting must
    /// exist; `chunk` is the member being added and must not already belong to the cluster.
    /// Outputs: Success or a storage error (unknown meeting, duplicate chunk, adapter failure).
    /// Side effects: Writes derived biometric vectors in one transaction; adapters must not log them.
    fn save_unknown_speaker_cluster_chunk(
        &self,
        _cluster: &UnknownSpeakerCluster,
        _chunk: &UnknownSpeakerChunk,
    ) -> Result<(), StorageError> {
        Err(StorageError::Unavailable)
    }

    /// Loads one meeting's unresolved unknown-speaker clusters that have at least one member.
    ///
    /// Inputs: `meeting_id` identifies the meeting.
    /// Outputs: Clusters in creation order (empty when none exist), or a storage error.
    /// Side effects: Reads derived biometric vectors; callers must not log them.
    fn load_unknown_speaker_clusters(
        &self,
        _meeting_id: &MeetingId,
    ) -> Result<Vec<UnknownSpeakerCluster>, StorageError> {
        Ok(Vec::new())
    }

    /// Loads one unknown-speaker cluster by ID, whether resolved or not.
    ///
    /// Inputs: `cluster_id` identifies the cluster.
    /// Outputs: The cluster, `None` when it does not exist, or a storage error.
    /// Side effects: Reads derived biometric vectors; callers must not log them.
    fn load_unknown_speaker_cluster(
        &self,
        _cluster_id: Uuid,
    ) -> Result<Option<UnknownSpeakerCluster>, StorageError> {
        Ok(None)
    }

    /// Loads every member chunk of one cluster, including its derived embedding.
    ///
    /// Inputs: `cluster_id` identifies the cluster.
    /// Outputs: Members in insertion order (empty for an unknown cluster), or a storage error.
    /// Side effects: Reads derived biometric vectors; callers must not log them.
    fn load_unknown_speaker_chunks(
        &self,
        _cluster_id: Uuid,
    ) -> Result<Vec<UnknownSpeakerChunk>, StorageError> {
        Ok(Vec::new())
    }

    /// Marks a cluster resolved to an identity and deletes the review clips that represented it.
    ///
    /// Inputs: `cluster_id` must exist; `identity_id` is the identity the user assigned.
    /// Outputs: Success or a storage error, including unknown-cluster rejection.
    /// Side effects: Updates the cluster and deletes its temporary review audio in one transaction.
    fn resolve_unknown_speaker_cluster(
        &self,
        _cluster_id: Uuid,
        _identity_id: Uuid,
    ) -> Result<(), StorageError> {
        Err(StorageError::Unavailable)
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

/// Forwards every repository call through a shared reference so application services can borrow
/// one adapter (or an unsized `dyn` port object) without taking ownership of it.
impl<T> NoteRepository for &T
where
    T: NoteRepository + ?Sized,
{
    fn list_meeting_ids(&self) -> Result<Vec<MeetingId>, StorageError> {
        (*self).list_meeting_ids()
    }

    fn create_meeting(
        &self,
        meeting_id: &MeetingId,
        initial_crdt_document: &[u8],
    ) -> Result<(), StorageError> {
        (*self).create_meeting(meeting_id, initial_crdt_document)
    }

    fn save_transcript(&self, transcript: &Transcript) -> Result<(), StorageError> {
        (*self).save_transcript(transcript)
    }

    fn load_transcript(&self, meeting_id: &MeetingId) -> Result<Option<Transcript>, StorageError> {
        (*self).load_transcript(meeting_id)
    }

    fn save_crdt_document(
        &self,
        meeting_id: &MeetingId,
        crdt_document: &[u8],
    ) -> Result<(), StorageError> {
        (*self).save_crdt_document(meeting_id, crdt_document)
    }

    fn load_crdt_document(&self, meeting_id: &MeetingId) -> Result<Vec<u8>, StorageError> {
        (*self).load_crdt_document(meeting_id)
    }

    fn save_unassigned_clip(&self, clip: UnassignedVoiceClip) -> Result<(), StorageError> {
        (*self).save_unassigned_clip(clip)
    }

    fn get_unassigned_clips(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Vec<UnassignedVoiceClip>, StorageError> {
        (*self).get_unassigned_clips(meeting_id)
    }

    fn get_meetings_overlapping(
        &self,
        timestamp: DateTime<Utc>,
    ) -> Result<Vec<MeetingWindow>, StorageError> {
        (*self).get_meetings_overlapping(timestamp)
    }

    fn load_meeting_metadata(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Option<MeetingMetadata>, StorageError> {
        (*self).load_meeting_metadata(meeting_id)
    }

    fn save_meeting_title(
        &self,
        meeting_id: &MeetingId,
        title: &MeetingTitle,
    ) -> Result<(), StorageError> {
        (*self).save_meeting_title(meeting_id, title)
    }

    fn save_transcript_segments(
        &self,
        meeting_id: &MeetingId,
        segments: &[TranscriptSegment],
    ) -> Result<(), StorageError> {
        (*self).save_transcript_segments(meeting_id, segments)
    }

    fn load_transcript_segments(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Vec<TranscriptSegment>, StorageError> {
        (*self).load_transcript_segments(meeting_id)
    }

    fn save_unknown_speaker_cluster_chunk(
        &self,
        cluster: &UnknownSpeakerCluster,
        chunk: &UnknownSpeakerChunk,
    ) -> Result<(), StorageError> {
        (*self).save_unknown_speaker_cluster_chunk(cluster, chunk)
    }

    fn load_unknown_speaker_clusters(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Vec<UnknownSpeakerCluster>, StorageError> {
        (*self).load_unknown_speaker_clusters(meeting_id)
    }

    fn load_unknown_speaker_cluster(
        &self,
        cluster_id: Uuid,
    ) -> Result<Option<UnknownSpeakerCluster>, StorageError> {
        (*self).load_unknown_speaker_cluster(cluster_id)
    }

    fn load_unknown_speaker_chunks(
        &self,
        cluster_id: Uuid,
    ) -> Result<Vec<UnknownSpeakerChunk>, StorageError> {
        (*self).load_unknown_speaker_chunks(cluster_id)
    }

    fn resolve_unknown_speaker_cluster(
        &self,
        cluster_id: Uuid,
        identity_id: Uuid,
    ) -> Result<(), StorageError> {
        (*self).resolve_unknown_speaker_cluster(cluster_id, identity_id)
    }
}
