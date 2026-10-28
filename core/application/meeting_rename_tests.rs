//! Specifies the meeting-rename use case shared by the CLI, HTTP, and FFI driving adapters.
//!
//! Tests use in-memory SQLite and real Automerge documents; titles are hand-written fixtures.

use crate::{
    adapters::storage::sqlite_adapter::SqliteNoteRepository,
    application::{
        meeting_crdt::MeetingCrdtService,
        meeting_rename::{MeetingRenameError, rename_meeting},
    },
    domain::{
        errors::MeetingTitleError,
        models::{MeetingId, MeetingTitle},
    },
    ports::storage::NoteRepository,
};

/// Creates an in-memory repository holding one untitled meeting.
///
/// Outputs: The repository and the meeting's ID.
fn repository_with_meeting() -> (SqliteNoteRepository, MeetingId) {
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite should open: {error}"));
    let meeting_id = MeetingId::new("meeting-rename");
    MeetingCrdtService::new(&repository)
        .create_meeting(&meeting_id)
        .unwrap_or_else(|error| panic!("meeting should be created: {error}"));
    (repository, meeting_id)
}

/// Reads a meeting's stored title, `None` when it is untitled.
fn stored_title(repository: &SqliteNoteRepository, meeting_id: &MeetingId) -> Option<String> {
    repository
        .load_meeting_metadata(meeting_id)
        .unwrap_or_else(|error| panic!("metadata should load: {error}"))
        .unwrap_or_else(|| panic!("meeting {} should exist", meeting_id.as_str()))
        .title()
        .map(|title| title.as_str().to_owned())
}

/// Verifies a rename stores the trimmed title and reports the previous display title, which is
/// the dated fallback for an untitled meeting and the stored title afterwards.
#[test]
fn rename_stores_the_trimmed_title_and_reports_the_previous_one() {
    let (repository, meeting_id) = repository_with_meeting();

    let first_rename = rename_meeting(&repository, &meeting_id, "  Project Sync \n")
        .unwrap_or_else(|error| panic!("first rename should succeed: {error}"));
    assert_eq!(first_rename.meeting_id(), &meeting_id);
    assert!(
        first_rename.previous_title().starts_with("Meeting "),
        "untitled meetings report their fallback: {}",
        first_rename.previous_title()
    );
    assert_eq!(first_rename.new_title().as_str(), "Project Sync");
    assert_eq!(
        stored_title(&repository, &meeting_id).as_deref(),
        Some("Project Sync")
    );

    let second_rename = rename_meeting(&repository, &meeting_id, "Project Sync (Q4)")
        .unwrap_or_else(|error| panic!("second rename should succeed: {error}"));
    assert_eq!(second_rename.previous_title(), "Project Sync");
    assert_eq!(second_rename.new_title().as_str(), "Project Sync (Q4)");
    assert_eq!(
        stored_title(&repository, &meeting_id).as_deref(),
        Some("Project Sync (Q4)")
    );
}

/// Verifies a rename is written to the replicated Automerge document, not only to the SQLite
/// projection, so peers receive it on their next sync (Issue #39).
#[test]
fn rename_writes_the_replicated_crdt_title() {
    let (repository, meeting_id) = repository_with_meeting();

    rename_meeting(&repository, &meeting_id, "Replicated title")
        .unwrap_or_else(|error| panic!("rename should succeed: {error}"));

    let crdt_title = MeetingCrdtService::new(&repository)
        .load_title(&meeting_id)
        .unwrap_or_else(|error| panic!("CRDT title should load: {error}"));
    assert_eq!(
        crdt_title.as_ref().map(MeetingTitle::as_str),
        Some("Replicated title")
    );
}

/// Verifies renaming an unknown meeting fails with `MeetingNotFound` and creates nothing.
#[test]
fn rename_rejects_an_unknown_meeting() {
    let (repository, _) = repository_with_meeting();
    let missing_meeting_id = MeetingId::new("meeting-missing");

    let rename_error = rename_meeting(&repository, &missing_meeting_id, "Orphan")
        .expect_err("an unknown meeting must not be renamed");

    assert!(
        matches!(
            &rename_error,
            MeetingRenameError::MeetingNotFound { meeting_id } if meeting_id == &missing_meeting_id
        ),
        "{rename_error:?}"
    );
    assert!(
        rename_error.to_string().contains("meeting-missing"),
        "{rename_error}"
    );
    let meeting_ids = repository
        .list_meeting_ids()
        .unwrap_or_else(|error| panic!("meeting IDs should load: {error}"));
    assert_eq!(meeting_ids, vec![MeetingId::new("meeting-rename")]);
}

/// Verifies blank, oversized, multi-line, and control-character titles are rejected with the
/// domain reason and leave the stored title unchanged.
#[test]
fn rename_rejects_invalid_titles_without_writing() {
    let (repository, meeting_id) = repository_with_meeting();
    rename_meeting(&repository, &meeting_id, "Original")
        .unwrap_or_else(|error| panic!("fixture rename should succeed: {error}"));
    let oversized_title = "x".repeat(MeetingTitle::MAX_CHARACTERS + 1);

    for (invalid_title, expected_reason) in [
        ("", MeetingTitleError::Blank),
        (" \t ", MeetingTitleError::Blank),
        (
            oversized_title.as_str(),
            MeetingTitleError::TooLong {
                max_characters: MeetingTitle::MAX_CHARACTERS,
            },
        ),
        (
            "First line\nSecond line",
            MeetingTitleError::ControlCharacter,
        ),
        ("Sync\u{001b}[31m", MeetingTitleError::ControlCharacter),
    ] {
        let rename_error = rename_meeting(&repository, &meeting_id, invalid_title)
            .expect_err("an invalid title must be rejected");
        assert!(
            matches!(&rename_error, MeetingRenameError::InvalidTitle(reason) if reason == &expected_reason),
            "{invalid_title:?} gave {rename_error:?}"
        );
    }
    assert_eq!(
        stored_title(&repository, &meeting_id).as_deref(),
        Some("Original")
    );
}

/// Verifies an exactly 200-character title is accepted, matching the creation limit.
#[test]
fn rename_accepts_a_title_at_the_length_limit() {
    let (repository, meeting_id) = repository_with_meeting();
    let longest_title = "é".repeat(MeetingTitle::MAX_CHARACTERS);

    rename_meeting(&repository, &meeting_id, &longest_title)
        .unwrap_or_else(|error| panic!("a 200-character title should be accepted: {error}"));

    assert_eq!(
        stored_title(&repository, &meeting_id).as_deref(),
        Some(longest_title.as_str())
    );
}
