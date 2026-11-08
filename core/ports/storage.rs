//! Declares persistence ports so use cases never depend directly on SQLite or CRDT libraries.

use crate::domain::{
    errors::StorageError,
    models::{
        AppliedDeletions, Attachment, ClusterDigest, ClusterShare, ClusterShareImport,
        DeletionRecords, DeviceId, MeetingId, MeetingMetadata, MeetingTitle, MeetingWindow,
        Transcript, TranscriptSegment, UnassignedVoiceClip, UnknownSpeakerChunk,
        UnknownSpeakerCluster,
    },
    transcription_language::TranscriptionLanguage,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::ports::meeting_document::MeetingCombination;

/// Persists meeting text in an offline-capable repository.
pub trait NoteRepository {
    /// Lists the meetings whose document was written after `after_revision`, by any connection.
    ///
    /// Inputs: The `latest_revision` of an earlier call, or 0.
    /// Outputs: The changed meetings and the newest revision now stored, or a storage error.
    /// Repositories without revisions report no changes.
    /// Side effects: Reads meeting metadata only; never document bytes.
    fn document_changes_since(&self, after_revision: u64) -> Result<DocumentChanges, StorageError> {
        Ok(DocumentChanges {
            meeting_ids: Vec::new(),
            latest_revision: after_revision,
        })
    }

    /// Records that a meeting's speech was transcribed in `language`, for its metadata.
    ///
    /// Inputs: The meeting, the language, and the offset (ms into the recording) of the first
    /// chunk that used it. Outputs: Success, or a storage error. A language already recorded
    /// changes nothing. Repositories without meeting documents ignore it.
    /// Side effects: Writes the meeting's CRDT document and index, which syncs to paired devices.
    fn record_transcription_language(
        &self,
        _meeting_id: &MeetingId,
        _language: TranscriptionLanguage,
        _first_used_at_ms: u64,
    ) -> Result<(), StorageError> {
        Ok(())
    }

    /// Lists every meeting with what decides whether two of them are one meeting recorded on two
    /// devices (#88).
    ///
    /// Outputs: The meetings, oldest first, or a storage error. Repositories without meeting
    /// documents list none.
    /// Side effects: Reads meeting metadata only.
    fn meeting_recordings(&self) -> Result<Vec<MeetingRecording>, StorageError> {
        Ok(Vec::new())
    }

    /// Folds `meeting_id` into another device's recording of the same meeting (#88).
    ///
    /// Inputs: The meeting to fold and the combination (the meeting it joins, the clock offset,
    /// and the device that combines their audio).
    /// Outputs: Success, a rejection for an unknown meeting, or a storage error. Repositories
    /// without meeting documents refuse.
    /// Side effects: Writes the meeting's CRDT document and index, which syncs to paired devices.
    fn combine_meeting(
        &self,
        _meeting_id: &MeetingId,
        _combination: &MeetingCombination,
    ) -> Result<(), StorageError> {
        Err(StorageError::Unavailable)
    }

    /// Splits a folded meeting from the meeting it joined, so both are listed again and never
    /// joined again (#88).
    ///
    /// Inputs: The folded meeting. Outputs: Whether it was folded, or a storage error.
    /// Side effects: Writes the meeting's CRDT document and index, which syncs to paired devices.
    fn split_meeting(&self, _meeting_id: &MeetingId) -> Result<bool, StorageError> {
        Err(StorageError::Unavailable)
    }

    /// Notes on a meeting recorded here whether this device is a phone, unless it says already
    /// (#88). Paired devices use it to agree which device combines a meeting's recordings.
    ///
    /// Inputs: The meeting and whether this device is a phone. Outputs: Success, a rejection for
    /// an unknown meeting, or a storage error; repositories without meeting documents refuse.
    /// Side effects: Writes the meeting's CRDT document and index, which syncs to paired devices.
    fn describe_recording_device(
        &self,
        _meeting_id: &MeetingId,
        _on_phone: bool,
    ) -> Result<(), StorageError> {
        Err(StorageError::Unavailable)
    }

    /// Records that a meeting's transcript was built from these folded recordings' speech too.
    ///
    /// Inputs: The meeting the recordings were folded into and the recordings. Outputs: Success, a
    /// rejection for an unknown meeting, or a storage error; repositories without meeting
    /// documents refuse.
    /// Side effects: Writes the meeting's CRDT document and index, which syncs to paired devices.
    fn mark_combined(
        &self,
        _meeting_id: &MeetingId,
        _recordings: &[MeetingId],
    ) -> Result<(), StorageError> {
        Err(StorageError::Unavailable)
    }

    /// Deletes a meeting and everything it holds, and records that it was deleted.
    ///
    /// Inputs: The meeting; an unknown one changes nothing.
    /// Outputs: Whether it existed, or a storage error. Repositories that cannot delete refuse.
    /// Side effects: Irreversibly removes its document, transcript, attachment rows, unknown voices
    /// and review audio, in one step. Attachment files are the caller's to remove.
    fn delete_meeting(&self, _meeting_id: &MeetingId) -> Result<bool, StorageError> {
        Err(StorageError::Rejected {
            reason: String::from("this repository cannot delete meetings"),
        })
    }

    /// Deletes an unknown voice for good, and records that it was deleted. Unlike
    /// `delete_unknown_speaker_cluster`, which reorganizes clusters, this stops paired devices
    /// sharing the voice back.
    ///
    /// Inputs: The cluster; an unknown one changes nothing.
    /// Outputs: Whether it existed, or a storage error. Repositories that cannot delete refuse.
    /// Side effects: Irreversibly removes its embeddings and review audio, in one step.
    fn delete_unknown_voice(&self, _cluster_id: Uuid) -> Result<bool, StorageError> {
        Err(StorageError::Rejected {
            reason: String::from("this repository cannot delete unknown voices"),
        })
    }

    /// Tells whether `meeting_id` was deleted here or on a paired device.
    ///
    /// Outputs: The answer, or a storage error. Repositories without records answer no.
    /// Side effects: Reads local records only.
    fn is_meeting_deleted(&self, _meeting_id: &MeetingId) -> Result<bool, StorageError> {
        Ok(false)
    }

    /// Lists every deletion record, for paired devices.
    ///
    /// Outputs: The records, or a storage error. Repositories without records list none.
    /// Side effects: Reads local records only.
    fn deletion_records(&self) -> Result<DeletionRecords, StorageError> {
        Ok(DeletionRecords::default())
    }

    /// Applies deletion records from a paired device; items not held here stay blocked.
    ///
    /// Inputs: Records decoded from a paired device; applying them twice changes nothing.
    /// Outputs: The meetings and project resources removed by this call, or a storage error.
    /// Side effects: As the delete methods, in one step. Attachment files are the caller's.
    fn apply_deletion_records(
        &self,
        _records: &DeletionRecords,
    ) -> Result<AppliedDeletions, StorageError> {
        Ok(AppliedDeletions::default())
    }

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

    /// Creates a cluster or replaces its members, after the final pass has regrouped the meeting.
    ///
    /// Inputs: `cluster` lists exactly `chunks`; its meeting must exist.
    /// Outputs: Success or a storage error. Side effects: Rewrites biometric vectors in one transaction.
    fn replace_unknown_speaker_cluster_members(
        &self,
        _cluster: &UnknownSpeakerCluster,
        _chunks: &[UnknownSpeakerChunk],
    ) -> Result<(), StorageError> {
        Err(StorageError::Unavailable)
    }

    /// Deletes a cluster the final pass found no speaker for, with its members and review clips.
    ///
    /// Inputs: `cluster_id`; an unknown one is not an error. Outputs: Success or a storage error.
    /// Side effects: Deletes biometric vectors and temporary review audio in one transaction.
    fn delete_unknown_speaker_cluster(&self, _cluster_id: Uuid) -> Result<(), StorageError> {
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

    /// Lists every unresolved cluster with at least one member, with what it holds (Issue #50).
    ///
    /// Outputs: Digests in creation order, or a storage error. Side effects: Reads row counts only.
    fn unresolved_cluster_digests(&self) -> Result<Vec<ClusterDigest>, StorageError> {
        Ok(Vec::new())
    }

    /// Builds the share of one unresolved cluster: the cluster, its chunks, and all its clips.
    ///
    /// Inputs: `cluster_id` identifies the cluster.
    /// Outputs: The share, `None` for an unknown or resolved cluster, or a storage error.
    /// Side effects: Reads and decrypts biometric data and raw audio; callers must not log it.
    fn load_cluster_share(&self, _cluster_id: Uuid) -> Result<Option<ClusterShare>, StorageError> {
        Ok(None)
    }

    /// Stores a share received from a paired device, sealed with this device's key.
    ///
    /// Inputs: A decoded, validated share.
    /// Outputs: What the import did (see `ClusterShareImport`), or a storage error, including a
    /// rejection when this device keeps the cluster under another meeting.
    /// Side effects: Upserts the cluster and inserts chunks and clips not stored yet, in one
    /// transaction; never for a resolved cluster or a missing meeting.
    fn import_cluster_share(
        &self,
        _share: &ClusterShare,
    ) -> Result<ClusterShareImport, StorageError> {
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

/// One meeting as a recording: when and where it was recorded and how it relates to other
/// devices' recordings of the same meeting (#88).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeetingRecording {
    /// The meeting.
    pub meeting_id: MeetingId,
    /// When its recording started, on its device's clock.
    pub created_at: DateTime<Utc>,
    /// The device that recorded it.
    pub acquisition_device_id: DeviceId,
    /// The meeting it was folded into, if any.
    pub combined_into: Option<MeetingCombination>,
    /// Meetings the owner split from it.
    pub kept_apart_from: Vec<MeetingId>,
    /// Whether a phone recorded it; `None` when its device has not said yet.
    pub recorded_on_phone: Option<bool>,
    /// Other recordings whose speech its transcript was built from.
    pub combined_with: Vec<MeetingId>,
}

/// Meetings whose documents changed since a revision (Issue #43).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentChanges {
    /// The changed meetings, oldest change first.
    pub meeting_ids: Vec<MeetingId>,
    /// The newest revision stored; pass it to the next call.
    pub latest_revision: u64,
}

/// Forwards every repository call through a shared reference so application services can borrow
/// one adapter (or an unsized `dyn` port object) without taking ownership of it.
impl<T> NoteRepository for &T
where
    T: NoteRepository + ?Sized,
{
    fn delete_meeting(&self, meeting_id: &MeetingId) -> Result<bool, StorageError> {
        (**self).delete_meeting(meeting_id)
    }

    fn delete_unknown_voice(&self, cluster_id: Uuid) -> Result<bool, StorageError> {
        (**self).delete_unknown_voice(cluster_id)
    }

    fn is_meeting_deleted(&self, meeting_id: &MeetingId) -> Result<bool, StorageError> {
        (**self).is_meeting_deleted(meeting_id)
    }

    fn deletion_records(&self) -> Result<DeletionRecords, StorageError> {
        (**self).deletion_records()
    }

    fn record_transcription_language(
        &self,
        meeting_id: &MeetingId,
        language: TranscriptionLanguage,
        first_used_at_ms: u64,
    ) -> Result<(), StorageError> {
        (**self).record_transcription_language(meeting_id, language, first_used_at_ms)
    }

    fn apply_deletion_records(
        &self,
        records: &DeletionRecords,
    ) -> Result<AppliedDeletions, StorageError> {
        (**self).apply_deletion_records(records)
    }

    fn list_meeting_ids(&self) -> Result<Vec<MeetingId>, StorageError> {
        (*self).list_meeting_ids()
    }

    fn meeting_recordings(&self) -> Result<Vec<MeetingRecording>, StorageError> {
        (**self).meeting_recordings()
    }

    fn combine_meeting(
        &self,
        meeting_id: &MeetingId,
        combination: &MeetingCombination,
    ) -> Result<(), StorageError> {
        (**self).combine_meeting(meeting_id, combination)
    }

    fn split_meeting(&self, meeting_id: &MeetingId) -> Result<bool, StorageError> {
        (**self).split_meeting(meeting_id)
    }

    fn describe_recording_device(
        &self,
        meeting_id: &MeetingId,
        on_phone: bool,
    ) -> Result<(), StorageError> {
        (**self).describe_recording_device(meeting_id, on_phone)
    }

    fn mark_combined(
        &self,
        meeting_id: &MeetingId,
        recordings: &[MeetingId],
    ) -> Result<(), StorageError> {
        (**self).mark_combined(meeting_id, recordings)
    }

    fn document_changes_since(&self, after_revision: u64) -> Result<DocumentChanges, StorageError> {
        (*self).document_changes_since(after_revision)
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

    fn replace_unknown_speaker_cluster_members(
        &self,
        cluster: &UnknownSpeakerCluster,
        chunks: &[UnknownSpeakerChunk],
    ) -> Result<(), StorageError> {
        (*self).replace_unknown_speaker_cluster_members(cluster, chunks)
    }

    fn delete_unknown_speaker_cluster(&self, cluster_id: Uuid) -> Result<(), StorageError> {
        (*self).delete_unknown_speaker_cluster(cluster_id)
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

    fn unresolved_cluster_digests(&self) -> Result<Vec<ClusterDigest>, StorageError> {
        (*self).unresolved_cluster_digests()
    }

    fn load_cluster_share(&self, cluster_id: Uuid) -> Result<Option<ClusterShare>, StorageError> {
        (*self).load_cluster_share(cluster_id)
    }

    fn import_cluster_share(
        &self,
        share: &ClusterShare,
    ) -> Result<ClusterShareImport, StorageError> {
        (*self).import_cluster_share(share)
    }
}
