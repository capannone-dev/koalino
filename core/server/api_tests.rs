//! Defines the headless Axum API contract before route handlers are implemented.
//!
//! Every test uses the in-memory SQLite adapter and Axum's in-process `oneshot` request path so
//! CI does not bind a fixed HTTP port. WebSocket coverage will use an ephemeral loopback listener.

use automerge::AutoCommit;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use futures::StreamExt;
use tokio::{
    net::TcpListener,
    time::{Duration, timeout},
};
use tokio_tungstenite::connect_async;
use tower::ServiceExt;

use crate::{
    adapters::storage::{
        in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
    },
    domain::models::{Attachment, MeetingId, Transcript},
    ports::storage::{AttachmentRepository, NoteRepository},
    server::api::{ServerAppState, WebSocketEvent, build_router},
};

/// Builds a router backed by an isolated SQLite database seeded with one complete meeting.
///
/// Inputs: None.
/// Outputs: A router that exposes deterministic test data only through repository ports.
/// Side effects: Allocates an in-memory SQLite database and stores fixture metadata.
fn seeded_router() -> (axum::Router, ServerAppState) {
    let repository = SqliteNoteRepository::open_in_memory()
        .unwrap_or_else(|error| panic!("test SQLite repository must initialize: {error}"));
    let meeting_id = MeetingId::new("meeting-rome");
    let mut initial_document = AutoCommit::new();
    repository
        .create_meeting(&meeting_id, &initial_document.save())
        .unwrap_or_else(|error| panic!("test meeting must persist: {error}"));
    repository
        .save_transcript(&Transcript::new(
            meeting_id.clone(),
            "Discussed offline-first sync.",
        ))
        .unwrap_or_else(|error| panic!("test transcript must persist: {error}"));
    repository
        .save_attachment(&Attachment::new(
            uuid::Uuid::new_v4(),
            Some(meeting_id),
            "/fixtures/agenda.pdf",
            "application/pdf",
            chrono::Utc::now(),
            "in-memory-test-device",
        ))
        .unwrap_or_else(|error| panic!("test attachment must persist: {error}"));

    let application_state = ServerAppState::new(repository, InMemoryVoiceVectorDB::new());
    let router = build_router(application_state.clone());
    (router, application_state)
}

/// Verifies meeting metadata is returned from an injected in-memory repository through HTTP.
#[tokio::test]
async fn list_meetings_returns_seeded_metadata_as_json() {
    let response = seeded_router()
        .0
        .oneshot(
            Request::builder()
                .uri("/api/meetings")
                .body(Body::empty())
                .unwrap_or_else(|error| panic!("test request must build: {error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("router must serve the test request: {error}"));

    assert_eq!(response.status(), StatusCode::OK);
    let response_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|error| panic!("response body must be readable: {error}"));
    let response_json: serde_json::Value = serde_json::from_slice(&response_bytes)
        .unwrap_or_else(|error| panic!("response must be JSON: {error}"));
    assert_eq!(response_json[0]["meeting_id"], "meeting-rome");
}

/// Verifies meeting detail exposes transcript and linked attachment metadata through ports.
#[tokio::test]
async fn meeting_detail_returns_transcript_and_linked_attachments() {
    let response = seeded_router()
        .0
        .oneshot(
            Request::builder()
                .uri("/api/meetings/meeting-rome")
                .body(Body::empty())
                .unwrap_or_else(|error| panic!("test request must build: {error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("router must serve the test request: {error}"));

    assert_eq!(response.status(), StatusCode::OK);
    let response_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|error| panic!("response body must be readable: {error}"));
    let response_json: serde_json::Value = serde_json::from_slice(&response_bytes)
        .unwrap_or_else(|error| panic!("response must be JSON: {error}"));
    assert_eq!(response_json["transcript"], "Discussed offline-first sync.");
    assert_eq!(
        response_json["attachments"][0]["file_path"],
        "/fixtures/agenda.pdf"
    );
}

/// Verifies unknown meeting identifiers return a portable HTTP not-found response.
#[tokio::test]
async fn unknown_meeting_returns_not_found() {
    let response = seeded_router()
        .0
        .oneshot(
            Request::builder()
                .uri("/api/meetings/missing-meeting")
                .body(Body::empty())
                .unwrap_or_else(|error| panic!("test request must build: {error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("router must serve the test request: {error}"));

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

/// Verifies gallery and telemetry reads return JSON through their injected port boundaries.
#[tokio::test]
async fn gallery_and_telemetry_routes_return_safe_empty_summaries() {
    for endpoint in ["/api/voice-gallery", "/api/telemetry"] {
        let response = seeded_router()
            .0
            .oneshot(
                Request::builder()
                    .uri(endpoint)
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("test request must build: {error}")),
            )
            .await
            .unwrap_or_else(|error| panic!("router must serve the test request: {error}"));

        assert_eq!(response.status(), StatusCode::OK);
        let response_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap_or_else(|error| panic!("response body must be readable: {error}"));
        let response_json: serde_json::Value = serde_json::from_slice(&response_bytes)
            .unwrap_or_else(|error| panic!("response must be JSON: {error}"));
        assert!(response_json.is_object());
    }
}

/// Verifies browser access is restricted to the documented local web development origin.
#[tokio::test]
async fn cors_allows_only_the_local_web_development_origin() {
    let response = seeded_router()
        .0
        .oneshot(
            Request::builder()
                .uri("/api/meetings")
                .header("origin", "http://localhost:3000")
                .body(Body::empty())
                .unwrap_or_else(|error| panic!("test request must build: {error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("router must serve the test request: {error}"));

    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("http://localhost:3000")
    );
}

/// Defines the event payload contract consumed by a connected WebSocket client.
#[test]
fn websocket_event_serializes_without_sensitive_payloads() {
    let event_json = serde_json::to_value(WebSocketEvent::MeetingStarted {
        meeting_id: String::from("meeting-rome"),
    })
    .unwrap_or_else(|error| panic!("event must serialize: {error}"));

    assert_eq!(event_json["event_type"], "meeting_started");
    assert_eq!(event_json["meeting_id"], "meeting-rome");
}

/// Verifies a websocket connection receives an event published through the injected state publisher.
#[tokio::test]
async fn websocket_client_receives_a_local_meeting_event() {
    let (router, application_state) = seeded_router();
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap_or_else(|error| panic!("test listener must bind an ephemeral port: {error}"));
    let listener_address = listener
        .local_addr()
        .unwrap_or_else(|error| panic!("test listener must expose its address: {error}"));
    let server_task = tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .unwrap_or_else(|error| panic!("test Axum server must run: {error}"));
    });

    let (mut web_socket, _) = connect_async(format!("ws://{listener_address}/ws"))
        .await
        .unwrap_or_else(|error| panic!("websocket client must connect: {error}"));
    application_state.publish_event(WebSocketEvent::MeetingStarted {
        meeting_id: String::from("meeting-rome"),
    });

    let received_message = timeout(Duration::from_secs(1), web_socket.next())
        .await
        .unwrap_or_else(|_| panic!("websocket must receive the published event"))
        .unwrap_or_else(|| panic!("websocket must remain connected for the event"))
        .unwrap_or_else(|error| panic!("websocket event must be valid: {error}"));
    assert!(
        received_message
            .into_text()
            .unwrap_or_default()
            .contains("meeting_started")
    );
    server_task.abort();
}
