//! Deletes meetings and unknown voices for the CLI and the HTTP API, with the same rules for both.
//!
//! The repository removes rows and leaves the records that spread a deletion to paired devices.
//! This use case adds what the repository cannot know: a recording may be writing to the meeting
//! (possibly in another process), and attachment files live outside the database. Both come
//! through `MeetingDeletionHooks`, implemented by the composition roots.

use uuid::Uuid;

use crate::{
    domain::{
        errors::StorageError,
        models::{MeetingId, meeting_voice_chunk_id},
    },
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

/// Deletes the unknown voices that recordings' final passes left behind (#82).
///
/// A recording made before #82 kept a live voice for every diarized turn, even one that got no
/// words, and its final pass removed only voices a transcript line pointed to. Such a voice is
/// useless to name and holds raw review audio. A meeting is cleaned only once a transcript line
/// points at a final-pass voice (a `meeting-voice-…` member): its transcript is then final, so a
/// voice no line points at, and that is not itself a final voice, will never be needed. Meetings
/// without that evidence (a crash, no diarization, a transcript not yet synced) and meetings being
/// recorded are left alone.
///
/// Inputs: The repository and the hooks (`None` skips the recording check).
/// Outputs: How many voices were deleted, or the first storage error (earlier deletions stand).
/// Side effects: Reads every meeting's transcript and voices; irreversibly deletes leftover
/// embeddings and review audio, leaving records so paired devices delete them too.
pub fn remove_leftover_voices<RepositoryPort>(
    repository: &RepositoryPort,
    hooks: Option<&dyn MeetingDeletionHooks>,
) -> Result<usize, StorageError>
where
    RepositoryPort: NoteRepository + ?Sized,
{
    let mut removed_count = 0_usize;
    for meeting_id in repository.list_meeting_ids()? {
        if hooks.is_some_and(|hooks| hooks.is_recording(&meeting_id)) {
            continue;
        }
        let clusters = repository.load_unknown_speaker_clusters(&meeting_id)?;
        if clusters.is_empty() {
            continue;
        }
        let referenced: Vec<Uuid> = repository
            .load_transcript_segments(&meeting_id)?
            .iter()
            .filter_map(|segment| segment.unknown_cluster_id())
            .collect();
        let mut finalized = false;
        for cluster_id in &referenced {
            if let Some(cluster) = repository.load_unknown_speaker_cluster(*cluster_id)?
                && cluster
                    .chunk_ids()
                    .contains(&meeting_voice_chunk_id(*cluster_id))
            {
                finalized = true;
                break;
            }
        }
        if !finalized {
            continue;
        }
        for cluster in clusters {
            let cluster_id = cluster.cluster_id();
            let is_final_voice = cluster
                .chunk_ids()
                .contains(&meeting_voice_chunk_id(cluster_id));
            if !is_final_voice
                && !referenced.contains(&cluster_id)
                && repository.delete_unknown_voice(cluster_id)?
            {
                removed_count += 1;
            }
        }
    }
    if removed_count > 0 {
        tracing::info!(
            removed_count,
            "Unknown voices left behind by finished recordings were deleted"
        );
    }
    Ok(removed_count)
}
