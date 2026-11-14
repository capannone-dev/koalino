//! Specifies the web routes for AI connections and meeting summaries (#95), with a fake model so
//! nothing leaves the machine.

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
    domain::{
        errors::LlmError,
        llm::{LlmCompletion, LlmRequest},
        models::{MeetingId, TranscriptSegment},
    },
    llm_commands::LlmServices,
    ports::{ai::LlmEngine, storage::NoteRepository},
    server::api::{ServerAppState, build_router},
};

/// Answers with the length of what it was sent, so a test can see the transcript arrived.
struct CountingEngine;

impl LlmEngine for CountingEngine {
    fn complete(&self, request: &LlmRequest) -> Result<LlmCompletion, LlmError> {
        let sent = &request.turns[0].text;
        Ok(LlmCompletion {
            text: if sent.contains("We approve it.") {
                "## Overview\\nApproved.".to_owned()
            } else {
                "OK".to_owned()
            },
            truncated: false,
        })
    }
}

fn router(with_llm: bool) -> axum::Router {
    let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .and_then(|repository| {
            repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
        })
        .unwrap_or_else(|error| panic!("{error}"));
    let crdt = MeetingCrdtService::new(&repository);
    crdt.create_meeting(&MeetingId::new("m-1"))
        .unwrap_or_else(|error| panic!("{error}"));
    repository
        .save_transcript_segments(
            &MeetingId::new("m-1"),
            &[TranscriptSegment::new(
                MeetingId::new("m-1"),
                0,
                1_000,
                None,
                "We approve it.",
            )],
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let state = ServerAppState::new(repository, InMemoryVoiceVectorDB::new());
    if !with_llm {
        return build_router(state);
    }
    let folder = std::env::temp_dir().join(format!("llm-routes-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
    let services = LlmServices::new(folder.join("db.llm-settings.json"), "Server")
        .with_engine_factory(Arc::new(|_| {
            Ok(Box::new(CountingEngine) as Box<dyn LlmEngine>)
        }));
    build_router(state.with_llm_services(Arc::new(services)))
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
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn without_ai_settings_the_routes_answer_unavailable() {
    let router = router(false);
    let (status, body) = send(&router, "GET", "/api/settings/llm", None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_connection_is_added_and_a_meeting_summarized_and_stored() {
    let router = router(true);
    let (status, body) = send(&router, "POST", "/api/meetings/m-1/summary", None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error_code"], "no_connection");

    let (status, body) = send(
        &router,
        "POST",
        "/api/settings/llm/connections",
        Some(json!({"label": "LAN", "kind": "open_ai_compatible",
                    "base_url": "http://192.168.1.5:11434/v1", "model": "llama",
                    "context_tokens": 8192, "api_key": "k-123"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.to_string().contains("k-123"));
    let id = body["connection"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();

    let (status, body) = send(
        &router,
        "POST",
        &format!("/api/settings/llm/connections/{id}/test"),
        None,
    )
    .await;
    assert_eq!(
        (status, body["reply"].clone()),
        (StatusCode::OK, json!("OK"))
    );

    let (status, body) = send(
        &router,
        "POST",
        "/api/meetings/m-1/summary",
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["summary"]["text"], "## Overview\\nApproved.");

    let (status, body) = send(&router, "GET", "/api/meetings/m-1/summary", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["summary"]["model"], "llama");
    assert_eq!(body["meeting_prompt"], Value::Null);
    assert!(
        body.get("request").is_some_and(Value::is_null),
        "no request to a paired computer (#107): {body}"
    );

    let (status, body) = send(&router, "POST", "/api/meetings/missing/summary", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_meetings_own_prompt_is_checked_and_settings_are_updated() {
    let router = router(true);
    let (status, body) = send(
        &router,
        "PUT",
        "/api/meetings/m-1/summary-prompt",
        Some(json!({"prompt": "{{nope}}"})),
    )
    .await;
    assert_eq!(
        (status, body["error_code"].clone()),
        (StatusCode::BAD_REQUEST, json!("invalid_template"))
    );
    let (status, _) = send(
        &router,
        "PUT",
        "/api/meetings/m-1/summary-prompt",
        Some(json!({"prompt": "Decisions in {{title}}."})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = send(&router, "GET", "/api/meetings/m-1/summary", None).await;
    assert_eq!(body["meeting_prompt"], "Decisions in {{title}}.");

    let (status, body) = send(
        &router,
        "PUT",
        "/api/settings/llm",
        Some(json!({"auto_summarize": false})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["auto_summarize"], false);
    let (status, _) = send(
        &router,
        "DELETE",
        "/api/settings/llm/connections/nope",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
