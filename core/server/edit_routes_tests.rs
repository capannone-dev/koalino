//! Specifies the web routes for the owner's edits to a meeting (#96): notes, the summary's text
//! and single transcript segments.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::{
    adapters::storage::{
        in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
    },
    application::{
        crypto::voice_encryptor::ephemeral_cipher, meeting_crdt::MeetingCrdtService,
        meeting_document::AutomergeMeetingDocumentCodec,
    },
    domain::models::{MeetingId, TranscriptSegment},
    ports::storage::NoteRepository,
    server::api::{ServerAppState, build_router},
};

fn router() -> axum::Router {
    let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .and_then(|repository| {
            repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
        })
        .unwrap_or_else(|error| panic!("{error}"));
    MeetingCrdtService::new(&repository)
        .create_meeting(&MeetingId::new("m-1"))
        .unwrap_or_else(|error| panic!("{error}"));
    repository
        .save_transcript_segments(
            &MeetingId::new("m-1"),
            &[TranscriptSegment::new(
                MeetingId::new("m-1"),
                0,
                1_000,
                None,
                "We aprove it.",
            )],
        )
        .unwrap_or_else(|error| panic!("{error}"));
    build_router(ServerAppState::new(
        repository,
        InMemoryVoiceVectorDB::new(),
    ))
}

async fn send(
    router: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    let response = router
        .clone()
        .oneshot(
            request
                .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
                .unwrap_or_else(|error| panic!("{error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({})),
    )
}

#[tokio::test]
async fn notes_are_read_and_replaced() {
    let router = router();
    let (status, empty) = send(&router, "GET", "/api/meetings/m-1/notes", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(empty["notes"], Value::Null);

    let (status, saved) = send(
        &router,
        "PUT",
        "/api/meetings/m-1/notes",
        Some(json!({"text": "## Actions\n- Call the courier"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (_, read) = send(&router, "GET", "/api/meetings/m-1/notes", None).await;
    assert_eq!(read["notes"], "## Actions\n- Call the courier");

    let (status, missing) = send(
        &router,
        "PUT",
        "/api/meetings/nope/notes",
        Some(json!({"text": "x"})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(missing["error_code"], "meeting_not_found");
    let (status, _) = send(&router, "PUT", "/api/meetings/m-1/notes", Some(json!({}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn the_summary_text_is_edited_and_marked() {
    let router = router();
    let (status, written) = send(
        &router,
        "PUT",
        "/api/meetings/m-1/summary",
        Some(json!({"text": "Decisions: approved."})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{written}");
    assert_eq!(written["summary"]["edited"], true);
    let (_, read) = send(&router, "GET", "/api/meetings/m-1/summary", None).await;
    assert_eq!(read["summary"]["text"], "Decisions: approved.");

    let (status, blank) = send(
        &router,
        "PUT",
        "/api/meetings/m-1/summary",
        Some(json!({"text": " "})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(blank["error_code"], "empty");
}

#[tokio::test]
async fn a_segment_is_corrected_and_an_unknown_one_refused() {
    let router = router();
    let (status, edited) = send(
        &router,
        "PUT",
        "/api/meetings/m-1/segments",
        Some(json!({"start_time_ms": 0, "end_time_ms": 1000, "text": "We approve it."})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{edited}");
    assert_eq!(edited["segment"]["text"], "We approve it.");

    let (status, unknown) = send(
        &router,
        "PUT",
        "/api/meetings/m-1/segments",
        Some(json!({"start_time_ms": 5, "end_time_ms": 6, "text": "x"})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(unknown["error_code"], "segment_not_found");
}
