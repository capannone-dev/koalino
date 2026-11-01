//! Deletes meetings and unknown voices for the CLI and the HTTP API, with the same rules for both.
//!
//! The repository removes rows and leaves the records that spread a deletion to paired devices.
//! This use case adds what the repository cannot know: a recording may be writing to the meeting
//! (possibly in another process), and attachment files live outside the database. Both come
//! through `MeetingDeletionHooks`, implemented by the composition roots.

use uuid::Uuid;

use crate::{
    domain::{errors::StorageError, models::MeetingId},
    ports::storage::NoteRepository,
};

/// What deleting a meeting needs from outside the database.
pub trait MeetingDeletionHooks: Send + Sync {
    /// Tells whether `meeting_id` is being recorded now, by any process on this database.
    ///
    /// Side effects: May read a control file.
    fn is_recording(&self, meeting_id: &MeetingId) -> bool;

    /// Removes a deleted meeting's private attachment files.
    ///
    /// Side effects: Deletes files; a failure is logged, as the meeting is already gone.
    fn remove_files(&self, meeting_id: &MeetingId);
}

/// Why a deletion did not happen.
#[derive(Debug, thiserror::Error)]
pub enum DeletionError {
    /// No such meeting or unknown voice is stored here.
    #[error("{item} does not exist")]
    NotFound {
        /// What was asked for, safe to show.
        item: String,
    },
    /// The running pipeline still writes to this meeting.
    #[error("meeting {meeting_id} is being recorded; stop the recording first")]
    Recording {
        /// The meeting being recorded.
        meeting_id: String,
    },
    /// The repository failed; nothing was deleted.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// Deletes a meeting and everything it holds.
///
/// Inputs: The repository, the hooks (`None` skips the recording check and file removal), and the
/// meeting.
/// Outputs: Success, or `Recording`, `NotFound` or `Storage`; in each of those nothing changed.
/// Side effects: Irreversibly deletes its rows and private files. Paired devices delete it too.
pub fn delete_meeting(
    repository: impl NoteRepository,
    hooks: Option<&dyn MeetingDeletionHooks>,
    meeting_id: &MeetingId,
) -> Result<(), DeletionError> {
    if hooks.is_some_and(|hooks| hooks.is_recording(meeting_id)) {
        return Err(DeletionError::Recording {
            meeting_id: meeting_id.as_str().to_owned(),
        });
    }
    if !repository.delete_meeting(meeting_id)? {
        return Err(DeletionError::NotFound {
            item: format!("meeting {}", meeting_id.as_str()),
        });
    }
    if let Some(hooks) = hooks {
        hooks.remove_files(meeting_id);
    }
    Ok(())
}

/// Deletes an unknown voice without naming it. Its transcript stays.
///
/// Inputs: The repository, the hooks (`None` skips the recording check), and the cluster.
/// Outputs: Success, or `Recording`, `NotFound` or `Storage`; in each of those nothing changed.
/// Side effects: Irreversibly deletes its embeddings and review audio. Paired devices delete it too.
pub fn delete_unknown_voice(
    repository: impl NoteRepository,
    hooks: Option<&dyn MeetingDeletionHooks>,
    cluster_id: Uuid,
) -> Result<(), DeletionError> {
    if let (Some(hooks), Some(cluster)) =
        (hooks, repository.load_unknown_speaker_cluster(cluster_id)?)
        && hooks.is_recording(cluster.meeting_id())
    {
        return Err(DeletionError::Recording {
            meeting_id: cluster.meeting_id().as_str().to_owned(),
        });
    }
    if !repository.delete_unknown_voice(cluster_id)? {
        return Err(DeletionError::NotFound {
            item: format!("unknown voice {cluster_id}"),
        });
    }
    Ok(())
}
