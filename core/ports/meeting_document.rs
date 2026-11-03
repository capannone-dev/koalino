//! Declares how meeting data is read from and written into the meeting's CRDT document.
//!
//! Since Issue #43 the Automerge document is the source of truth for everything that syncs: the
//! meeting's origin, the transcript, the transcript segments, (Issue #50) which identity each
//! unknown-speaker cluster was named as, and (Issue #85) the project it belongs to. The SQLite rows
//! are an index
//! derived from the document. Storage adapters receive an implementation of this port (as they
//! receive `EmbeddingCipher`). They call it inside their own transactions, so the document and the
//! index never diverge, and they never depend on the CRDT library.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::{
    errors::StorageError,
    models::{DeviceId, MeetingId, TranscriptSegment},
    projects::ProjectId,
};

/// When and where a meeting was recorded; written once, by the recording device.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeetingOrigin {
    /// Creation time of the meeting.
    pub created_at: DateTime<Utc>,
    /// The device that recorded it (REQUIREMENTS 3.3).
    pub acquisition_device_id: DeviceId,
}

/// The index-relevant contents of a meeting document. `None` means the document does not hold that
/// field yet, so the index must keep its current value.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MeetingProjection {
    /// The meeting's origin.
    pub origin: Option<MeetingOrigin>,
    /// The full transcript text.
    pub transcript: Option<String>,
    /// The speaker-attributed segments, ordered by start time. A segment of a resolved cluster
    /// carries the resolution's identity.
    pub segments: Option<Vec<TranscriptSegment>>,
    /// Unknown-speaker clusters named on any device, as (cluster, identity), ordered by cluster
    /// (Issue #50).
    pub cluster_resolutions: Vec<(Uuid, Uuid)>,
    /// The project the meeting belongs to (Issue #85). Unlike the fields above, `None` is a value:
    /// the meeting is in no project, so the index clears its project too.
    pub project_id: Option<ProjectId>,
}

/// Reads and edits serialized meeting documents without exposing the CRDT library.
pub trait MeetingDocumentCodec: Send + Sync {
    /// Records `origin` unless the document already has one.
    ///
    /// Inputs: The serialized document and the origin. Outputs: The updated document bytes, or
    /// a storage error for a corrupt document. Side effects: None.
    fn with_origin(&self, document: &[u8], origin: &MeetingOrigin)
    -> Result<Vec<u8>, StorageError>;

    /// Replaces the transcript text.
    ///
    /// Inputs: The serialized document and the transcript. Outputs: The updated document bytes,
    /// or a storage error. Side effects: None.
    fn with_transcript(&self, document: &[u8], transcript: &str) -> Result<Vec<u8>, StorageError>;

    /// Makes the document's segments equal to `segments`, changing only fields that differ, so a
    /// concurrent edit of another field on another device still merges.
    ///
    /// Inputs: The serialized document and the meeting's complete segment list.
    /// Outputs: The updated document bytes, or a storage error. Side effects: None.
    fn with_segments(
        &self,
        document: &[u8],
        segments: &[TranscriptSegment],
    ) -> Result<Vec<u8>, StorageError>;

    /// Records that the owner named unknown-speaker cluster `cluster_id` as `identity_id`.
    ///
    /// Inputs: The serialized document, the cluster, and the identity.
    /// Outputs: The updated document bytes (unchanged when that resolution is already recorded),
    /// or a storage error. Concurrent resolutions of one cluster converge on one identity.
    /// Side effects: None.
    fn with_cluster_resolution(
        &self,
        document: &[u8],
        cluster_id: Uuid,
        identity_id: Uuid,
    ) -> Result<Vec<u8>, StorageError>;

    /// Moves the meeting into `project_id`, or out of any project for `None` (Issue #85).
    ///
    /// Inputs: The serialized document and the project. Outputs: The updated document bytes
    /// (unchanged when the meeting is already there), or a storage error. Concurrent moves on two
    /// devices converge on one project. Side effects: None.
    fn with_project(
        &self,
        document: &[u8],
        project_id: Option<ProjectId>,
    ) -> Result<Vec<u8>, StorageError>;

    /// Reads the index-relevant fields of a document.
    ///
    /// Inputs: The meeting the document belongs to and its bytes, which may come from a peer.
    /// Outputs: The projection, or a storage error for a corrupt document. Malformed entries,
    /// such as a segment with a non-numeric time, are skipped with a warning.
    /// Side effects: None.
    fn project(
        &self,
        meeting_id: &MeetingId,
        document: &[u8],
    ) -> Result<MeetingProjection, StorageError>;
}
