//! Specifies the search answer shared by the app, the web and the CLI (#100): the request's
//! scope, the JSON of each hit, and the refusals.

use std::{collections::HashMap, sync::Arc};

use serde_json::{Value, json};
use uuid::Uuid;

use super::search_commands::{HostSearchSources, SearchRequest, run_search};
use crate::{
    adapters::storage::sqlite_adapter::SqliteNoteRepository,
    application::{
        crypto::voice_encryptor::ephemeral_cipher, meeting_crdt::MeetingCrdtService,
        meeting_document::AutomergeMeetingDocumentCodec,
    },
    domain::{
        models::{MeetingId, MeetingTitle, TranscriptSegment},
        projects::ProjectName,
    },
    ports::{projects::ProjectRepository, storage::NoteRepository},
};

fn repository() -> SqliteNoteRepository {
    let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .and_then(|repository| {
            repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
        })
        .unwrap_or_else(|error| panic!("{error}"));
    let meeting_id = MeetingId::new("m-1");
    let title = MeetingTitle::parse("Budget review").unwrap_or_else(|e| panic!("{e}"));
    MeetingCrdtService::new(&repository)
        .create_titled_meeting(&meeting_id, &title)
        .unwrap_or_else(|e| panic!("{e}"));
    repository
        .save_transcript_segments(
            &meeting_id,
            &[TranscriptSegment::new(
                meeting_id.clone(),
                61_000,
                64_000,
                None,
                "The budget is approved.",
            )],
        )
        .unwrap_or_else(|e| panic!("{e}"));
    repository
}

fn request(value: Value) -> SearchRequest {
    serde_json::from_value(value).unwrap_or_else(|e| panic!("{e}"))
}

fn sources() -> HostSearchSources<'static> {
    HostSearchSources::new(HashMap::<Uuid, String>::new(), None)
}

#[test]
fn a_hit_is_described_as_json() {
    let repository = repository();
    let answer = run_search(
        &repository,
        &sources(),
        &request(json!({"query": "budget"})),
    );

    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(answer["truncated"], false);
    let hits = answer["hits"].as_array().cloned().unwrap_or_default();
    assert_eq!(hits.len(), 2, "{answer}");
    assert_eq!(hits[0]["place"], "meeting_title");
    let transcript = &hits[1];
    assert_eq!(transcript["place"], "transcript");
    assert_eq!(transcript["start_time_ms"], 61_000);
    assert_eq!(transcript["speaker"], "Unattributed");
    assert_eq!(transcript["meeting"]["meeting_id"], "m-1");
    assert_eq!(transcript["meeting"]["title"], "Budget review");
    assert!(transcript["meeting"]["created_at"].is_string());
    assert_eq!(transcript["project"], Value::Null);
    assert_eq!(
        transcript["snippet"],
        json!([
            {"text": "The ", "match": false},
            {"text": "budget", "match": true},
            {"text": " is approved.", "match": false},
        ])
    );
}

#[test]
fn projects_are_searched_one_or_all() {
    let repository = repository();
    let project = repository
        .create_project(&ProjectName::parse("Budget 2027").unwrap_or_else(|e| panic!("{e}")))
        .unwrap_or_else(|e| panic!("{e}"));

    let all = run_search(
        &repository,
        &sources(),
        &request(json!({"query": "budget", "scope": "projects"})),
    );
    assert_eq!(all["hits"][0]["place"], "project_name", "{all}");
    assert_eq!(all["hits"][0]["project"]["name"], "Budget 2027");
    assert_eq!(all["hits"][0]["meeting"], Value::Null);

    let one = run_search(
        &repository,
        &sources(),
        &request(json!({"query": "budget", "scope": "project",
                        "project_id": project.project_id().to_string()})),
    );
    assert_eq!(one["hits"].as_array().map(Vec::len), Some(1), "{one}");
}

#[test]
fn refusals_have_stable_codes() {
    let repository = repository();
    let code =
        |value: Value| run_search(&repository, &sources(), &request(value))["error_code"].clone();

    assert_eq!(code(json!({"query": " "})), "empty");
    assert_eq!(code(json!({"query": "a"})), "too_short");
    assert_eq!(code(json!({"query": "x".repeat(201)})), "too_long");
    assert_eq!(
        code(json!({"query": "budget", "scope": "everything"})),
        "invalid_request"
    );
    assert_eq!(
        code(json!({"query": "budget", "scope": "project"})),
        "invalid_request"
    );
    assert_eq!(
        code(
            json!({"query": "budget", "scope": "project", "project_id": Uuid::new_v4().to_string()})
        ),
        "project_not_found"
    );
}
