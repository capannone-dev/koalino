//! Specifies how a meeting's summary is prepared, written and kept (#95): what is gathered from
//! the meeting, which prompt is used, and that the result and the meeting's own prompt are stored
//! in its document.

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use super::meeting_summaries::{
    load_summary, prepare_summary, set_meeting_prompt, store_summary, summary_template_for,
};
use crate::{
    adapters::storage::{
        in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
    },
    application::{
        crypto::voice_encryptor::ephemeral_cipher, meeting_crdt::MeetingCrdtService,
        meeting_document::AutomergeMeetingDocumentCodec,
    },
    domain::{
        llm::{DEFAULT_SUMMARY_TEMPLATE, MeetingSummary, PromptTemplate},
        models::{Attachment, MeetingId, MeetingTitle, TranscriptSegment},
    },
    ports::storage::{AttachmentRepository, NoteRepository},
};

fn repository() -> SqliteNoteRepository {
    SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .and_then(|repository| {
            repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
        })
        .unwrap_or_else(|error| panic!("repository must open: {error}"))
}

fn meeting() -> MeetingId {
    MeetingId::new("m-1")
}

/// Records a titled meeting with two segments, notes and a photo plus a PDF.
fn record(repository: &SqliteNoteRepository) -> std::path::PathBuf {
    let crdt = MeetingCrdtService::new(repository);
    let title = MeetingTitle::parse("Budget review").unwrap_or_else(|e| panic!("{e}"));
    crdt.create_titled_meeting(&meeting(), &title)
        .unwrap_or_else(|e| panic!("{e}"));
    crdt.replace_note_text(&meeting(), "Ask about Q3")
        .unwrap_or_else(|e| panic!("{e}"));
    repository
        .save_transcript_segments(
            &meeting(),
            &[
                TranscriptSegment::new(meeting(), 0, 1_000, None, "We approve it."),
                TranscriptSegment::new(meeting(), 1_000, 2_700_000, None, "Agreed."),
            ],
        )
        .unwrap_or_else(|e| panic!("{e}"));
    let folder = std::env::temp_dir().join(format!("meeting-summaries-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|e| panic!("{e}"));
    let photo = folder.join("board.png");
    std::fs::write(&photo, [0x89, b'P', b'N', b'G']).unwrap_or_else(|e| panic!("{e}"));
    let pdf = folder.join("deck.pdf");
    std::fs::write(&pdf, b"%PDF").unwrap_or_else(|e| panic!("{e}"));
    for (path, mime) in [(&photo, "image/png"), (&pdf, "application/pdf")] {
        repository
            .save_attachment(&Attachment::new(
                Uuid::new_v4(),
                Some(meeting()),
                path.display().to_string(),
                mime,
                Utc::now(),
                "device",
            ))
            .unwrap_or_else(|e| panic!("{e}"));
    }
    folder
}

#[test]
fn preparing_gathers_the_transcript_notes_photos_and_values() {
    let repository = repository();
    let folder = record(&repository);

    let prepared = prepare_summary(
        &repository,
        &InMemoryVoiceVectorDB::new(),
        &meeting(),
        &["OKR".to_owned()],
        "Laptop",
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let material = &prepared.material;
    assert_eq!(material.values.title, "Budget review");
    assert_eq!(material.values.duration, "45 min");
    assert_eq!(material.values.device, "Laptop");
    assert_eq!(material.values.project, None);
    assert_eq!(material.values.glossary, vec!["OKR".to_owned()]);
    assert_eq!(material.notes.as_deref(), Some("Ask about Q3"));
    assert_eq!(material.lines.len(), 2);
    assert_eq!(material.lines[0].text, "We approve it.");
    assert_eq!(material.images.len(), 1, "only the photo is sent");
    assert_eq!(prepared.meeting_prompt, None);
    drop(std::fs::remove_dir_all(folder));
}

#[test]
fn the_meetings_own_prompt_wins_and_a_summary_is_kept_in_its_document() {
    let repository = repository();
    let folder = record(&repository);
    assert!(set_meeting_prompt(&repository, &meeting(), Some("{{bogus}}")).is_err());
    set_meeting_prompt(
        &repository,
        &meeting(),
        Some("Only decisions in {{title}}."),
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let prepared = prepare_summary(
        &repository,
        &InMemoryVoiceVectorDB::new(),
        &meeting(),
        &[],
        "Laptop",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let template = summary_template_for(&prepared, &PromptTemplate::default_summary());
    assert_eq!(template.as_str(), "Only decisions in {{title}}.");

    set_meeting_prompt(&repository, &meeting(), None).unwrap_or_else(|e| panic!("{e}"));
    let prepared = prepare_summary(
        &repository,
        &InMemoryVoiceVectorDB::new(),
        &meeting(),
        &[],
        "Laptop",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        summary_template_for(&prepared, &PromptTemplate::default_summary()).as_str(),
        DEFAULT_SUMMARY_TEMPLATE
    );

    let summary = MeetingSummary {
        text: "## Overview".to_owned(),
        connection_label: "Office".to_owned(),
        model: "llama".to_owned(),
        created_at: Utc
            .timestamp_millis_opt(1_000)
            .single()
            .unwrap_or_else(|| panic!("time")),
        prompt: "p".to_owned(),
        edited: false,
        truncated: false,
    };
    store_summary(&repository, &meeting(), &summary).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        load_summary(&repository, &meeting()).ok().flatten(),
        Some(summary)
    );
    assert!(
        repository
            .load_transcript_segments(&meeting())
            .map(|s| s.len())
            .ok()
            == Some(2),
        "the transcript is untouched"
    );
    drop(std::fs::remove_dir_all(folder));
}
