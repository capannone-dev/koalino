//! Builds the readable text of a meeting's transcript for the CLI and HTTP read paths.
//!
//! A recorded meeting stores its transcript as timed segments only; the flat `transcript` field
//! is filled only for meetings imported as text. Export already prefers segments; this applies
//! the same rule to `meetings view` and `GET /api/meetings/{id}`, which otherwise showed a
//! recorded meeting as having no transcript.

use crate::{
    domain::{errors::StorageError, models::MeetingId},
    ports::storage::NoteRepository,
};

/// Returns the meeting's transcript as plain text.
///
/// Inputs: The repository and an existing meeting's ID.
/// Outputs: The non-blank segments in start-time order, trimmed and one per line; when there are
/// none, the flat transcript if it is not blank; otherwise `None`. A storage failure is returned
/// as is.
/// Side effects: Reads (and decrypts) the meeting's segments and transcript.
pub fn readable_transcript<Repository: NoteRepository + ?Sized>(
    note_repository: &Repository,
    meeting_id: &MeetingId,
) -> Result<Option<String>, StorageError> {
    let mut segments = note_repository.load_transcript_segments(meeting_id)?;
    segments.sort_by_key(|segment| (segment.start_time_ms(), segment.end_time_ms()));
    let segment_lines: Vec<&str> = segments
        .iter()
        .map(|segment| segment.text().trim())
        .filter(|text| !text.is_empty())
        .collect();
    if !segment_lines.is_empty() {
        return Ok(Some(segment_lines.join("\n")));
    }
    Ok(note_repository
        .load_transcript(meeting_id)?
        .map(|transcript| transcript.content().trim().to_owned())
        .filter(|text| !text.is_empty()))
}
