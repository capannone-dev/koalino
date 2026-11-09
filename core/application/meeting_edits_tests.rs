//! Specifies the owner's edits to a meeting (#96): its notes, its summary (marked as edited), and
//! single transcript segments that keep their speaker and timing and flag the transcript as
//! edited, so later passes leave it alone.

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use super::{
    meeting_crdt::MeetingCrdtService,
    meeting_document::AutomergeMeetingDocumentCodec,
    meeting_edits::{
        EditError, MAX_NOTES_CHARS, edit_segment, edit_summary, load_notes, set_notes,
        transcript_edited,
    },
    meeting_summary_document::{read_summary, with_summary},
};
use crate::{
    adapters::storage::sqlite_adapter::SqliteNoteRepository,
    application::crypto::voice_encryptor::ephemeral_cipher,
    domain::{
        llm::MeetingSummary,
        models::{MeetingId, MeetingTitle, TranscriptSegment},
        transcript_confidence::SegmentConfidence,
    },
    ports::storage::NoteRepository,
};

fn repository() -> SqliteNoteRepository {
    let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .and_then(|repository| {
            repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
        })
        .unwrap_or_else(|error| panic!("repository must open: {error}"));
    let title = MeetingTitle::parse("Budget review").unwrap_or_else(|e| panic!("{e}"));
    MeetingCrdtService::new(&repository)
        .create_titled_meeting(&meeting(), &title)
        .unwrap_or_else(|e| panic!("{e}"));
    repository
}

fn meeting() -> MeetingId {
    MeetingId::new("m-1")
}

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 9, 12, 0, 0)
        .single()
        .unwrap_or_default()
}

#[test]
fn notes_are_saved_read_back_and_limited() {
    let repository = repository();
    assert_eq!(load_notes(&repository, &meeting()).ok(), Some(None));

    set_notes(&repository, &meeting(), "## Actions\n- Ask about Q3")
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        load_notes(&repository, &meeting())
            .ok()
            .flatten()
            .as_deref(),
        Some("## Actions\n- Ask about Q3")
    );

    let too_long = "x".repeat(MAX_NOTES_CHARS + 1);
    assert!(matches!(
        set_notes(&repository, &meeting(), &too_long),
        Err(EditError::TooLong { .. })
    ));
    assert!(matches!(
        set_notes(&repository, &MeetingId::new("missing"), "x"),
        Err(EditError::MeetingNotFound)
    ));
}

#[test]
fn an_edited_summary_keeps_where_it_came_from_and_is_marked_edited() {
    let repository = repository();
    let written = MeetingSummary {
        text: "## Overview\nBudget.".to_owned(),
        connection_label: "Office".to_owned(),
        model: "llama".to_owned(),
        created_at: now(),
        prompt: "p".to_owned(),
        edited: false,
        truncated: true,
    };
    repository
        .edit_crdt_document(&meeting(), &|document| with_summary(document, &written))
        .unwrap_or_else(|e| panic!("{e}"));

    let edited = edit_summary(
        &repository,
        &meeting(),
        "## Overview\nBudget approved.",
        now(),
    )
    .unwrap_or_else(|e| panic!("{e}"));

    assert_eq!(edited.text, "## Overview\nBudget approved.");
    assert!(edited.edited);
    assert_eq!(edited.connection_label, "Office");
    assert_eq!(edited.model, "llama");
    let stored = repository
        .load_crdt_document(&meeting())
        .ok()
        .and_then(|document| read_summary(&document).ok().flatten());
    assert_eq!(stored, Some(edited));
}

#[test]
fn a_summary_can_be_written_by_hand() {
    let repository = repository();
    let written = edit_summary(&repository, &meeting(), "Decisions: none.", now())
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(written.edited);
    assert!(written.connection_label.is_empty());
    assert_eq!(written.created_at, now());
    assert!(matches!(
        edit_summary(&repository, &meeting(), "   ", now()),
        Err(EditError::Empty)
    ));
}

#[test]
fn a_segment_edit_keeps_speaker_and_timing_and_flags_the_transcript() {
    let repository = repository();
    let speaker = Uuid::new_v4();
    repository
        .save_transcript_segments(
            &meeting(),
            &[
                TranscriptSegment::new(meeting(), 0, 1_000, Some(speaker), "We aprove it.")
                    .with_confidence(SegmentConfidence {
                        overlapping_speech: true,
                        uncertain_speaker: false,
                        uncertain_words: vec![1],
                    }),
                TranscriptSegment::new(meeting(), 1_000, 2_000, None, "Uh."),
            ],
        )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(transcript_edited(&repository, &meeting()).ok(), Some(false));

    let edited = edit_segment(&repository, &meeting(), 0, 1_000, "We approve it.")
        .unwrap_or_else(|e| panic!("{e}"));

    let edited = edited.unwrap_or_else(|| panic!("the segment stays"));
    assert_eq!(edited.text(), "We approve it.");
    assert_eq!(edited.speaker_identity_id(), Some(speaker));
    assert_eq!((edited.start_time_ms(), edited.end_time_ms()), (0, 1_000));
    // The word marks described the old text; the overlap still holds.
    assert!(edited.confidence().uncertain_words.is_empty());
    assert!(edited.confidence().overlapping_speech);
    let stored = repository
        .load_transcript_segments(&meeting())
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(stored[0].text(), "We approve it.");
    assert_eq!(stored[1].text(), "Uh.");
    assert_eq!(transcript_edited(&repository, &meeting()).ok(), Some(true));

    // Emptying a segment removes it.
    assert!(
        edit_segment(&repository, &meeting(), 1_000, 2_000, "  ")
            .unwrap_or_else(|e| panic!("{e}"))
            .is_none()
    );
    assert_eq!(
        repository
            .load_transcript_segments(&meeting())
            .map(|segments| segments.len())
            .ok(),
        Some(1)
    );
    assert!(matches!(
        edit_segment(&repository, &meeting(), 5, 6, "x"),
        Err(EditError::SegmentNotFound)
    ));
}
