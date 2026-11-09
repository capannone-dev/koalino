//! Specifies the web search route (#100): hits for the default scope, and refusals with their
//! statuses.

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
                "Il preventivo è approvato.",
            )],
        )
        .unwrap_or_else(|error| panic!("{error}"));
    build_router(ServerAppState::new(
        repository,
        InMemoryVoiceVectorDB::new(),
    ))
}

async fn get(router: &axum::Router, uri: &str) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
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
async fn meetings_are_searched_by_default() {
    let router = router();
    let (status, answer) = get(&router, "/api/search?q=APPROVATO").await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["hits"][0]["place"], "transcript");
    assert_eq!(answer["hits"][0]["snippet"][1]["text"], "approvato");
    assert_eq!(answer["hits"][0]["snippet"][1]["match"], true);
}

#[tokio::test]
async fn refusals_have_statuses() {
    let router = router();
    assert_eq!(get(&router, "/api/search").await.0, StatusCode::BAD_REQUEST);
    assert_eq!(
        get(&router, "/api/search?q=x").await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get(&router, "/api/search?q=budget&scope=project").await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get(
            &router,
            "/api/search?q=budget&scope=project&project=4f0c2a64-1b1c-4b1a-9a43-1b2f3c4d5e6f"
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}
