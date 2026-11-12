//! Specifies the edit commands that keep an answer from the owner's own AI app as the summary,
//! and undo it (#114).

use serde_json::{Value, json};

use crate::{
    adapters::storage::sqlite_adapter::SqliteNoteRepository,
    application::crypto::voice_encryptor::ephemeral_cipher, domain::models::MeetingId,
    ffi_edits::run_edit_command, ports::storage::NoteRepository,
};

fn repository() -> SqliteNoteRepository {
    let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .unwrap_or_else(|error| panic!("{error}"));
    repository
        .create_meeting(&MeetingId::new("m-1"), &[])
        .unwrap_or_else(|error| panic!("{error}"));
    repository
}

fn run(repository: &SqliteNoteRepository, request: Value) -> Value {
    run_edit_command(repository, &request.to_string())
}

#[test]
fn an_answer_is_kept_and_undone_with_the_summary_it_replaced() {
    let repository = repository();
    let first = run(
        &repository,
        json!({"command": "keep_answer", "meeting_id": "m-1", "text": "## First",
               "source": "Claude app", "prompt": "Summarize."}),
    );
    assert_eq!(first["ok"], true);
    assert_eq!(first["summary"]["text"], "## First");
    assert_eq!(first["summary"]["connection_label"], "Claude app");
    assert_eq!(first["previous"], Value::Null);

    let second = run(
        &repository,
        json!({"command": "keep_answer", "meeting_id": "m-1", "text": "## Second",
               "source": "ChatGPT app"}),
    );
    assert_eq!(second["previous"]["text"], "## First");

    let undone = run(
        &repository,
        json!({"command": "restore_summary", "meeting_id": "m-1", "summary": second["previous"]}),
    );
    assert_eq!(undone["ok"], true);
    let third = run(
        &repository,
        json!({"command": "keep_answer", "meeting_id": "m-1", "text": "## Third", "source": "x"}),
    );
    assert_eq!(
        third["previous"]["text"], "## First",
        "Undo put the first one back"
    );
    assert_eq!(
        third["previous"]["created_at"],
        first["summary"]["created_at"]
    );

    let cleared = run(
        &repository,
        json!({"command": "restore_summary", "meeting_id": "m-1", "summary": null}),
    );
    assert_eq!(cleared["ok"], true);
    let fourth = run(
        &repository,
        json!({"command": "keep_answer", "meeting_id": "m-1", "text": "## Fourth", "source": "x"}),
    );
    assert_eq!(
        fourth["previous"],
        Value::Null,
        "Undo of the first answer left none"
    );
}

#[test]
fn a_blank_answer_or_a_malformed_summary_is_refused() {
    let repository = repository();
    let blank = run(
        &repository,
        json!({"command": "keep_answer", "meeting_id": "m-1", "text": " ", "source": "x"}),
    );
    assert_eq!(blank["error_code"], "empty");
    let malformed = run(
        &repository,
        json!({"command": "restore_summary", "meeting_id": "m-1", "summary": {"text": 3}}),
    );
    assert_eq!(malformed["error_code"], "invalid_request");
    let missing = run(
        &repository,
        json!({"command": "keep_answer", "meeting_id": "m-1"}),
    );
    assert_eq!(missing["error_code"], "invalid_request");
}
