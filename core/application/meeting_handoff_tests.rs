//! Specifies the hand-off of a meeting to the owner's own AI app (#114): the message and the
//! attached document for a summary or a chat, and keeping the answer as the summary with Undo.

use chrono::{TimeZone, Utc};

use super::meeting_handoff::{
    HandoffDelivery, HandoffPurpose, handoff_file_name, prepare_handoff, restore_summary,
    store_external_summary,
};
use super::meeting_summary::{SpokenLine, SummaryError, SummaryMaterial};
use crate::{
    adapters::storage::sqlite_adapter::SqliteNoteRepository,
    application::meeting_summary_document::read_summary,
    domain::{
        llm::{MeetingSummary, PromptTemplate, PromptValues},
        models::MeetingId,
    },
    ports::storage::NoteRepository,
};

fn material(lines: Vec<SpokenLine>, notes: Option<&str>) -> SummaryMaterial {
    SummaryMaterial {
        values: PromptValues {
            title: "Budget review".to_owned(),
            date: "2026-11-08".to_owned(),
            time: "10:00 UTC".to_owned(),
            duration: "45 min".to_owned(),
            device: "Laptop".to_owned(),
            project: None,
            speakers: vec!["Anna".to_owned()],
            glossary: Vec::new(),
            language: "it".to_owned(),
        },
        notes: notes.map(str::to_owned),
        lines,
        images: Vec::new(),
    }
}

fn spoken(speaker: &str, start_ms: u64, text: &str) -> SpokenLine {
    SpokenLine {
        speaker: speaker.to_owned(),
        start_ms,
        text: text.to_owned(),
    }
}

fn template() -> PromptTemplate {
    PromptTemplate::parse("Summarize {{title}} in {{language}}.").unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn a_summary_hand_off_asks_with_the_owners_prompt_and_attaches_the_meeting() {
    let handoff = prepare_handoff(
        &template(),
        &material(
            vec![spoken("Anna", 61_000, "We ship on Friday.")],
            Some("Ask about Q3"),
        ),
        HandoffPurpose::Summary,
        HandoffDelivery::Attached,
    )
    .unwrap_or_else(|e| panic!("{e}"));

    assert_eq!(handoff.prompt, "Summarize Budget review in it.");
    assert!(
        handoff
            .message
            .starts_with("Summarize Budget review in it.")
    );
    assert!(
        handoff.message.contains("Budget review.md"),
        "names the attached file"
    );
    assert_eq!(handoff.file_name, "Budget review.md");
    assert!(handoff.document.starts_with("# Budget review\n"));
    assert!(handoff.document.contains("Ask about Q3"));
    assert!(
        handoff
            .document
            .contains("[00:01:01] Anna: We ship on Friday.")
    );
}

#[test]
fn a_chat_hand_off_attaches_the_meeting_without_the_summary_prompt() {
    let handoff = prepare_handoff(
        &template(),
        &material(vec![spoken("Anna", 0, "Hello.")], None),
        HandoffPurpose::Chat,
        HandoffDelivery::Attached,
    )
    .unwrap_or_else(|e| panic!("{e}"));

    assert_eq!(handoff.prompt, "");
    assert!(!handoff.message.contains("Summarize"));
    assert!(handoff.message.contains("Budget review"));
    assert!(handoff.message.contains("questions"));
    assert!(
        !handoff.document.contains("notes"),
        "no notes section without notes"
    );
}

/// On a computer the message is pasted into a website, so it carries the meeting itself (#126).
#[test]
fn a_pasted_hand_off_carries_the_meeting_in_the_message() {
    let meeting = material(vec![spoken("Anna", 61_000, "We ship on Friday.")], None);

    let summary = prepare_handoff(
        &template(),
        &meeting,
        HandoffPurpose::Summary,
        HandoffDelivery::Pasted,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let chat = prepare_handoff(
        &template(),
        &meeting,
        HandoffPurpose::Chat,
        HandoffDelivery::Pasted,
    )
    .unwrap_or_else(|e| panic!("{e}"));

    assert!(
        summary
            .message
            .starts_with("Summarize Budget review in it.\n\n")
    );
    assert!(summary.message.ends_with(&summary.document));
    assert!(!summary.message.contains("attached"), "nothing is attached");
    assert_eq!(summary.prompt, "Summarize Budget review in it.");
    assert!(chat.message.contains("questions"));
    assert!(chat.message.ends_with(&chat.document));
    assert!(!chat.message.contains("attached"));
}

#[test]
fn a_meeting_without_words_or_notes_is_not_handed_off() {
    let refused = prepare_handoff(
        &template(),
        &material(Vec::new(), Some("  ")),
        HandoffPurpose::Chat,
        HandoffDelivery::Attached,
    );
    assert!(matches!(refused, Err(SummaryError::NothingToSummarize)));
}

#[test]
fn the_attached_file_is_named_after_the_title_without_path_characters() {
    assert_eq!(
        handoff_file_name("Budget / Q3: plan?"),
        "Budget - Q3- plan-.md"
    );
    assert_eq!(handoff_file_name("   "), "Meeting.md");
}

fn repository_with_meeting() -> (SqliteNoteRepository, MeetingId) {
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let meeting_id = MeetingId::new("meeting-handoff");
    repository
        .create_meeting(&meeting_id, &[])
        .unwrap_or_else(|e| panic!("{e}"));
    (repository, meeting_id)
}

fn stored(repository: &SqliteNoteRepository, meeting_id: &MeetingId) -> Option<MeetingSummary> {
    read_summary(
        &repository
            .load_crdt_document(meeting_id)
            .unwrap_or_else(|e| panic!("{e}")),
    )
    .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn an_answer_from_the_owners_app_becomes_the_summary_and_undo_brings_back_the_earlier_one() {
    let (repository, meeting_id) = repository_with_meeting();
    let when = Utc
        .with_ymd_and_hms(2026, 11, 12, 20, 0, 0)
        .single()
        .unwrap_or_else(|| panic!());

    let first = store_external_summary(
        &repository,
        &meeting_id,
        "## First",
        "Claude app",
        "Summarize.",
        when,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(first.previous, None);
    let kept = stored(&repository, &meeting_id).unwrap_or_else(|| panic!("a summary is kept"));
    assert_eq!(kept.text, "## First");
    assert_eq!(kept.connection_label, "Claude app");
    assert_eq!(kept.prompt, "Summarize.");
    assert!(kept.edited, "automatic summaries leave it alone");

    let second = store_external_summary(
        &repository,
        &meeting_id,
        "## Second",
        "ChatGPT app",
        "",
        when,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        second
            .previous
            .as_ref()
            .map(|summary| summary.text.as_str()),
        Some("## First")
    );

    restore_summary(&repository, &meeting_id, second.previous.as_ref())
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        stored(&repository, &meeting_id).map(|summary| summary.text),
        Some("## First".to_owned())
    );
    restore_summary(&repository, &meeting_id, None).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        stored(&repository, &meeting_id),
        None,
        "undoing the first answer leaves no summary"
    );
}

#[test]
fn a_blank_answer_is_not_kept() {
    let (repository, meeting_id) = repository_with_meeting();
    let refused = store_external_summary(
        &repository,
        &meeting_id,
        " \n",
        "Claude app",
        "",
        Utc::now(),
    );
    assert!(refused.is_err());
    assert_eq!(stored(&repository, &meeting_id), None);
}
