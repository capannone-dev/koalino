//! The owner's edits to a meeting (#96): its notes, its summary, and single transcript segments.
//!
//! Every edit goes into the meeting document, so it syncs to paired devices and is sealed at rest.
//! A summary changed by hand is marked `edited`, and automatic summaries never replace a meeting
//! that has one (#101). A transcript edit keeps the segment's speaker, timing and overlap mark,
//! drops its uncertain-word marks (they described the old words), and sets the document's root
//! `transcript_edited` flag. The after-meeting passes read that flag and leave the transcript
//! alone, so a later re-transcription can't undo the owner's corrections.

use automerge::{AutoCommit, ROOT, ReadDoc, ScalarValue, Value, transaction::Transactable};
use chrono::{DateTime, Utc};

use super::meeting_summary_document::{read_summary, with_summary};
use crate::{
    domain::{
        errors::StorageError,
        llm::MeetingSummary,
        models::{MeetingId, TranscriptSegment},
    },
    ports::storage::NoteRepository,
};

/// The longest notes or summary accepted, in characters.
pub const MAX_NOTES_CHARS: usize = 200_000;

/// The longest transcript segment accepted, in characters.
pub const MAX_SEGMENT_CHARS: usize = 20_000;

/// Root key of the owner's notes (shared with `MeetingCrdtService`).
const NOTES_KEY: &str = "note_text";

/// Root key of the flag set when the owner edits the transcript.
const TRANSCRIPT_EDITED_KEY: &str = "transcript_edited";

/// Why an edit was refused.
#[derive(Debug, thiserror::Error)]
pub enum EditError {
    /// The meeting does not exist.
    #[error("there is no such meeting")]
    MeetingNotFound,
    /// No segment has the given start and end.
    #[error("there is no such transcript segment")]
    SegmentNotFound,
    /// A summary must have text.
    #[error("the text is empty")]
    Empty,
    /// The text is longer than allowed.
    #[error("the text is longer than {limit} characters")]
    TooLong {
        /// The limit, in characters.
        limit: usize,
    },
    /// The database could not be read or written.
    #[error("the meeting could not be saved: {0}")]
    Storage(StorageError),
}

impl From<StorageError> for EditError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

/// Refuses text over `limit` characters.
fn check_length(text: &str, limit: usize) -> Result<(), EditError> {
    if text.chars().count() > limit {
        return Err(EditError::TooLong { limit });
    }
    Ok(())
}

/// Fails with `MeetingNotFound` unless the meeting exists.
fn require_meeting<Repository>(
    repository: &Repository,
    meeting_id: &MeetingId,
) -> Result<(), EditError>
where
    Repository: NoteRepository + ?Sized,
{
    match repository.load_meeting_metadata(meeting_id)? {
        Some(_) => Ok(()),
        None => Err(EditError::MeetingNotFound),
    }
}

/// Reads the owner's notes.
///
/// Outputs: The notes, `None` when there are none, or a storage error. Side effects: Reads the
/// meeting document.
pub fn load_notes<Repository>(
    repository: &Repository,
    meeting_id: &MeetingId,
) -> Result<Option<String>, EditError>
where
    Repository: NoteRepository + ?Sized,
{
    require_meeting(repository, meeting_id)?;
    Ok(read_notes(&repository.load_crdt_document(meeting_id)?)?)
}

/// Reads the owner's notes from a serialized meeting document.
///
/// Outputs: The notes, `None` when there are none, or a storage error for a corrupt document.
/// Side effects: None.
pub fn read_notes(document: &[u8]) -> Result<Option<String>, StorageError> {
    let document = load(document)?;
    Ok(match document.get(ROOT, NOTES_KEY) {
        Ok(Some((Value::Scalar(value), _))) => match value.as_ref() {
            ScalarValue::Str(text) => Some(text.to_string()),
            _ => None,
        },
        _ => None,
    })
}

/// Replaces the owner's notes (Markdown).
///
/// Inputs: The meeting and the text, at most `MAX_NOTES_CHARS`. Outputs: Success, or why not.
/// Side effects: Writes the meeting document, which syncs to paired devices.
pub fn set_notes<Repository>(
    repository: &Repository,
    meeting_id: &MeetingId,
    text: &str,
) -> Result<(), EditError>
where
    Repository: NoteRepository + ?Sized,
{
    check_length(text, MAX_NOTES_CHARS)?;
    require_meeting(repository, meeting_id)?;
    repository.edit_crdt_document(meeting_id, &|document| {
        let mut automerge_document = load(document)?;
        automerge_document
            .put(ROOT, NOTES_KEY, text)
            .map_err(|error| crdt_error("unable to write the notes", error))?;
        Ok(automerge_document.save())
    })?;
    Ok(())
}

/// Replaces the summary's text with the owner's, or writes a summary by hand when there is none.
///
/// Inputs: The meeting, the Markdown text (not blank, at most `MAX_NOTES_CHARS`) and the current
/// time, used only for a summary written by hand. Outputs: The summary as stored, marked
/// `edited`. Side effects: Writes the meeting document, which syncs to paired devices.
pub fn edit_summary<Repository>(
    repository: &Repository,
    meeting_id: &MeetingId,
    text: &str,
    now: DateTime<Utc>,
) -> Result<MeetingSummary, EditError>
where
    Repository: NoteRepository + ?Sized,
{
    if text.trim().is_empty() {
        return Err(EditError::Empty);
    }
    check_length(text, MAX_NOTES_CHARS)?;
    require_meeting(repository, meeting_id)?;
    let stored = std::cell::RefCell::new(None);
    repository.edit_crdt_document(meeting_id, &|document| {
        let summary = match read_summary(document)? {
            Some(earlier) => MeetingSummary {
                text: text.to_owned(),
                edited: true,
                ..earlier
            },
            None => MeetingSummary {
                text: text.to_owned(),
                connection_label: String::new(),
                model: String::new(),
                created_at: now,
                prompt: String::new(),
                edited: true,
                truncated: false,
            },
        };
        let updated = with_summary(document, &summary)?;
        stored.replace(Some(summary));
        Ok(updated)
    })?;
    stored
        .into_inner()
        .ok_or(EditError::Storage(StorageError::Unavailable))
}

/// Replaces one transcript segment's text, or removes the segment when `text` is blank.
///
/// Inputs: The meeting, the segment's start and end (as listed), and the new text (at most
/// `MAX_SEGMENT_CHARS`). Outputs: The segment as stored, or `None` when it was removed.
/// Side effects: Flags the transcript as edited, then rewrites the meeting's segments; both
/// reach paired devices through the meeting document.
pub fn edit_segment<Repository>(
    repository: &Repository,
    meeting_id: &MeetingId,
    start_time_ms: u64,
    end_time_ms: u64,
    text: &str,
) -> Result<Option<TranscriptSegment>, EditError>
where
    Repository: NoteRepository + ?Sized,
{
    check_length(text, MAX_SEGMENT_CHARS)?;
    require_meeting(repository, meeting_id)?;
    let mut segments = repository.load_transcript_segments(meeting_id)?;
    let position = segments
        .iter()
        .position(|segment| {
            segment.start_time_ms() == start_time_ms && segment.end_time_ms() == end_time_ms
        })
        .ok_or(EditError::SegmentNotFound)?;
    // Flag first: should the rewrite fail, a pass skips an unchanged transcript, which is safe.
    repository.edit_crdt_document(meeting_id, &|document| {
        let mut automerge_document = load(document)?;
        automerge_document
            .put(ROOT, TRANSCRIPT_EDITED_KEY, true)
            .map_err(|error| crdt_error("unable to flag the transcript", error))?;
        Ok(automerge_document.save())
    })?;
    let text = text.trim();
    let edited = if text.is_empty() {
        segments.remove(position);
        None
    } else {
        let segment = segments.remove(position).with_text(text);
        segments.insert(position, segment.clone());
        Some(segment)
    };
    repository.save_transcript_segments(meeting_id, &segments)?;
    Ok(edited)
}

/// Reports whether the owner edited the meeting's transcript.
///
/// Outputs: The flag, or a storage error. Side effects: Reads the meeting document.
pub fn transcript_edited<Repository>(
    repository: &Repository,
    meeting_id: &MeetingId,
) -> Result<bool, EditError>
where
    Repository: NoteRepository + ?Sized,
{
    Ok(read_transcript_edited(
        &repository.load_crdt_document(meeting_id)?,
    ))
}

/// Reads the transcript-edited flag from a serialized document; a corrupt document reads as not
/// edited. Side effects: None.
pub fn read_transcript_edited(document: &[u8]) -> bool {
    AutoCommit::load(document).is_ok_and(|automerge_document| {
        matches!(
            automerge_document.get(ROOT, TRANSCRIPT_EDITED_KEY),
            Ok(Some((Value::Scalar(value), _))) if matches!(value.as_ref(), ScalarValue::Boolean(true))
        )
    })
}

/// Deserializes a stored document.
fn load(document: &[u8]) -> Result<AutoCommit, StorageError> {
    AutoCommit::load(document).map_err(|error| crdt_error("meeting document is corrupt", error))
}

/// Converts an Automerge failure into a portable storage error.
fn crdt_error(operation: &str, error: automerge::AutomergeError) -> StorageError {
    StorageError::Rejected {
        reason: format!("{operation}: {error}"),
    }
}
