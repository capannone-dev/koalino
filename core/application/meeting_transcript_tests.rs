//! Specifies the readable transcript shown by the CLI `meetings view` and `GET /api/meetings/{id}`.
//!
//! Tests use in-memory SQLite and real Automerge documents; texts are hand-written fixtures.

use crate::{
    adapters::storage::sqlite_adapter::SqliteNoteRepository,
    application::{meeting_crdt::MeetingCrdtService, meeting_transcript::readable_transcript},
    domain::models::{MeetingId, Transcript, TranscriptSegment},
    ports::storage::NoteRepository,
};

/// Creates an in-memory repository holding one meeting with no transcript.
fn repository_with_meeting() -> (SqliteNoteRepository, MeetingId) {
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite should open: {error}"));
    let meeting_id = MeetingId::new("meeting-transcript");
    MeetingCrdtService::new(&repository)
        .create_meeting(&meeting_id)
        .unwrap_or_else(|error| panic!("meeting should be created: {error}"));
    (repository, meeting_id)
}

/// Reads the readable transcript, failing the test on a storage error.
fn read(repository: &SqliteNoteRepository, meeting_id: &MeetingId) -> Option<String> {
    readable_transcript(repository, meeting_id)
        .unwrap_or_else(|error| panic!("transcript should load: {error}"))
}

/// A recorded meeting has only segments; they are shown in time order, one per line.
#[test]
fn recorded_segments_are_joined_in_time_order() {
    let (repository, meeting_id) = repository_with_meeting();
    repository
        .save_transcript_segments(
            &meeting_id,
            &[
                TranscriptSegment::new(meeting_id.clone(), 4_000, 7_000, None, " second part "),
                TranscriptSegment::new(meeting_id.clone(), 0, 3_500, None, "first part"),
                TranscriptSegment::new(meeting_id.clone(), 7_000, 8_000, None, "   "),
            ],
        )
        .unwrap_or_else(|error| panic!("segments should be saved: {error}"));

    assert_eq!(
        read(&repository, &meeting_id).as_deref(),
        Some("first part\nsecond part")
    );
}

/// A meeting imported as flat text, without segments, shows that text.
#[test]
fn a_flat_transcript_is_used_when_there_are_no_segments() {
    let (repository, meeting_id) = repository_with_meeting();
    repository
        .save_transcript(&Transcript::new(meeting_id.clone(), "imported text"))
        .unwrap_or_else(|error| panic!("transcript should be saved: {error}"));

    assert_eq!(
        read(&repository, &meeting_id).as_deref(),
        Some("imported text")
    );
}

/// A meeting with neither segments nor text has no transcript.
#[test]
fn a_meeting_without_text_has_no_transcript() {
    let (repository, meeting_id) = repository_with_meeting();

    assert_eq!(read(&repository, &meeting_id), None);
}
