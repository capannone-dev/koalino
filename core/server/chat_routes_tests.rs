//! Specifies the chat routes (#97) with a fake model, so nothing leaves the machine: asking about
//! a meeting, the history, clearing it, and refusals with their statuses.

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

/// Says whether the transcript reached it.
struct ReadingEngine;

impl LlmEngine for ReadingEngine {
    fn complete(&self, request: &LlmRequest) -> Result<LlmCompletion, LlmError> {
        Ok(LlmCompletion {
            text: if request.system.contains("We approve it.") {
                String::from("Yes, it was approved.")
            } else {
                String::from("I can't tell.")
            },
            truncated: false,
        })
    }
}

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
                "We approve it.",
            )],
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let folder = std::env::temp_dir().join(format!("chat-routes-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
    let services = LlmServices::new(folder.join("db.llm-settings.json"), "Server")
        .with_engine_factory(Arc::new(|_| {
            Ok(Box::new(ReadingEngine) as Box<dyn LlmEngine>)
        }));
    build_router(
        ServerAppState::new(repository, InMemoryVoiceVectorDB::new())
            .with_llm_services(Arc::new(services)),
    )
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
async fn a_meeting_chat_is_asked_kept_and_cleared() {
    let router = router();
    let ask = json!({"scope": "meeting", "id": "m-1", "question": "Was it approved?"});
    let (status, body) = send(&router, "POST", "/api/chat", Some(ask.clone())).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error_code"], "no_connection");

    let (status, body) = send(
        &router,
        "POST",
        "/api/settings/llm/connections",
        Some(json!({"label": "LAN", "kind": "open_ai_compatible",
                    "base_url": "http://192.168.1.5:11434/v1", "model": "llama",
                    "context_tokens": 8192})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = send(&router, "POST", "/api/chat", Some(ask)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["answer"]["text"], "Yes, it was approved.");
    assert_eq!(body["answer"]["connection"], "LAN");

    let (status, body) = send(&router, "GET", "/api/chat?scope=meeting&id=m-1", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["messages"].as_array().map(Vec::len), Some(2), "{body}");
    let (_, body) = send(&router, "GET", "/api/chat?scope=all", None).await;
    assert_eq!(
        body["messages"].as_array().map(Vec::len),
        Some(0),
        "chats are separate"
    );

    let (status, body) = send(&router, "DELETE", "/api/chat?scope=meeting&id=m-1", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["cleared"], true);
}

#[tokio::test(flavor = "multi_thread")]
async fn refusals_have_their_statuses() {
    let router = router();
    let (status, body) = send(&router, "GET", "/api/chat?scope=meeting", None).await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("invalid_request"))
    );
    let (status, _) = send(
        &router,
        "POST",
        "/api/settings/llm/connections",
        Some(json!({"label": "LAN", "kind": "open_ai_compatible",
                    "base_url": "http://192.168.1.5:11434/v1", "model": "llama",
                    "context_tokens": 8192})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = send(
        &router,
        "POST",
        "/api/chat",
        Some(json!({"scope": "meeting", "id": "nope", "question": "Hi?"})),
    )
    .await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::NOT_FOUND, Some("meeting_not_found"))
    );
    let (status, body) = send(
        &router,
        "POST",
        "/api/chat",
        Some(json!({"scope": "all", "question": " "})),
    )
    .await;
    assert_eq!(
        (status, body["error_code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("empty"))
    );
}

/// A P2P task whose paired computer answers every question, or none is connected.
struct ComputerControl {
    answers: bool,
}

#[async_trait::async_trait]
impl crate::application::p2p_service::P2pControl for ComputerControl {
    fn snapshot(&self) -> crate::application::p2p_service::P2pSnapshot {
        panic!("the chat never reads the snapshot")
    }

    async fn command(
        &self,
        _command: crate::application::p2p_service::P2pCommand,
    ) -> Result<
        crate::application::p2p_service::P2pCommandOutcome,
        crate::application::p2p_service::P2pControlError,
    > {
        Err(crate::application::p2p_service::P2pControlError::Stopped)
    }

    fn notify_local_change(&self, _meeting_id: &MeetingId) {}

    fn ask_paired_computer(
        &self,
        _scope: crate::domain::chat::ChatScope,
        question: String,
        history: Vec<crate::application::paired_chat_exchange::PairedChatTurn>,
    ) -> crate::application::paired_chat_relay::PairedChatResult {
        if !self.answers {
            return Err("no_paired_computer".to_owned());
        }
        Ok(crate::application::paired_chat_exchange::PairedChatAnswer {
            text: format!("{question} Yes, after {} turns.", history.len()),
            included: Vec::new(),
            omitted: Vec::new(),
            sources: Vec::new(),
            connection_label: "Claude Code on Laptop".to_owned(),
            truncated: false,
            images_sent: 0,
        })
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_question_can_go_to_the_paired_computer() {
    let router = |answers: bool| {
        let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
            .and_then(|repository| {
                repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
            })
            .unwrap_or_else(|error| panic!("{error}"));
        build_router(
            ServerAppState::new(repository, InMemoryVoiceVectorDB::new())
                .with_p2p_control(Arc::new(ComputerControl { answers })),
        )
    };
    let asking = router(true);
    let (status, body) = send(
        &asking,
        "POST",
        "/api/chat",
        Some(json!({"scope": "all", "question": "Approved?", "on_paired_computer": true})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["answer"]["text"], "Approved? Yes, after 0 turns.");
    assert_eq!(body["answer"]["connection"], "Claude Code on Laptop");
    let (_, history) = send(&asking, "GET", "/api/chat?scope=all", None).await;
    assert_eq!(history["messages"].as_array().map(Vec::len), Some(2));

    let (status, body) = send(
        &router(false),
        "POST",
        "/api/chat",
        Some(json!({"scope": "all", "question": "Approved?", "on_paired_computer": true})),
    )
    .await;
    assert_eq!(body["error_code"], "no_paired_computer", "{body}");
    assert_ne!(status, StatusCode::OK);
}
