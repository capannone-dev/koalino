//! Renames an existing meeting: the single use case behind `meetings rename` (CLI),
//! `PATCH /api/meetings/{id}` (HTTP), and `note_taker_rename_meeting` (FFI).
//!
//! The driving adapters only decode input and format output. Validation is the domain's
//! `MeetingTitle::parse` (trimmed, 1–200 characters, no control characters such as newlines), the
//! same rules as titled creation. Persistence is `MeetingCrdtService::set_meeting_title`, which
//! writes the replicated Automerge title (Issue #39) and then the SQLite projection, so a rename
//! reaches peers on their next sync. Titles are never logged.

use crate::{
    application::meeting_crdt::MeetingCrdtService,
    domain::{
        errors::{MeetingTitleError, StorageError},
        models::{MeetingId, MeetingTitle},
    },
    ports::storage::NoteRepository,
};

/// Reports why a meeting could not be renamed.
#[derive(Debug, thiserror::Error)]
pub enum MeetingRenameError {
    /// The requested title is blank, longer than 200 characters, or contains control characters.
    #[error("invalid title: {0}")]
    InvalidTitle(#[from] MeetingTitleError),
    /// No meeting with this identifier exists.
    #[error("meeting {} does not exist", meeting_id.as_str())]
    MeetingNotFound {
        /// The identifier that was looked up.
        meeting_id: MeetingId,
    },
    /// The repository failed to read or write the meeting, or its CRDT document is corrupt.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// Describes a completed rename, for confirmation output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeetingRename {
    meeting_id: MeetingId,
    /// The display title before the rename: the stored title, or the dated fallback.
    previous_title: String,
    new_title: MeetingTitle,
}

impl MeetingRename {
    /// Borrows the renamed meeting's identifier.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Borrows the display title the meeting had before the rename (the dated fallback, such as
    /// `Meeting 2026-10-27 09:05 UTC`, when it was untitled).
    pub fn previous_title(&self) -> &str {
        &self.previous_title
    }

    /// Borrows the validated, trimmed title that is now stored.
    pub fn new_title(&self) -> &MeetingTitle {
        &self.new_title
    }
}

/// Validates `raw_title` and stores it as the title of an existing meeting.
///
/// Inputs: `repository` is any note repository; `meeting_id` identifies the meeting;
/// `raw_title` is unvalidated user text (surrounding whitespace is trimmed).
/// Outputs: The rename (with the previous display title), `InvalidTitle` when the text is not a
/// valid title, `MeetingNotFound` for an unknown meeting, or `Storage` for repository failures.
/// Side effects: Validates before any I/O, so invalid titles never touch storage. Otherwise reads
/// the meeting's metadata, records the title in its Automerge document (no change when it is
/// already the current title), and refreshes the SQLite projection. Neither title is logged.
pub fn rename_meeting<RepositoryPort>(
    repository: &RepositoryPort,
    meeting_id: &MeetingId,
    raw_title: &str,
) -> Result<MeetingRename, MeetingRenameError>
where
    RepositoryPort: NoteRepository + ?Sized,
{
    let new_title = MeetingTitle::parse(raw_title)?;
    let previous_metadata = repository
        .load_meeting_metadata(meeting_id)?
        .ok_or_else(|| MeetingRenameError::MeetingNotFound {
            meeting_id: meeting_id.clone(),
        })?;
    MeetingCrdtService::new(repository).set_meeting_title(meeting_id, &new_title)?;
    tracing::info!(meeting_id = %meeting_id.as_str(), "meeting renamed");
    Ok(MeetingRename {
        meeting_id: meeting_id.clone(),
        previous_title: previous_metadata.display_title(),
        new_title,
    })
}
