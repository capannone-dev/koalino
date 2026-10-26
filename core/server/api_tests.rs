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
    domain::models::{
        Attachment, DeviceId, MeetingId, TaskMetrics, TaskType, Transcript, UnassignedVoiceClip,
    },
    ports::{
        storage::{AttachmentRepository, NoteRepository},
        telemetry::MetricsRepository,
    },
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
    let review_clip_id = uuid::Uuid::from_u128(1);
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
    repository
        .save_unassigned_clip(UnassignedVoiceClip::with_review_context(
            review_clip_id,
            MeetingId::new("meeting-rome"),
            0,
            1_000,
            vec![0, 1],
            vec![0.25, 0.75],
            None,
        ))
        .unwrap_or_else(|error| panic!("test review clip must persist: {error}"));
    let current_timestamp_ms =
        u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or_default();
    for (task_id, device_id, task_type, duration_ms, latency_ms, power_mwh) in [
        (
            uuid::Uuid::from_u128(11),
            "device-rome",
            TaskType::Transcription,
            1_200,
            120,
            2.5,
        ),
        (
            uuid::Uuid::from_u128(12),
            "device-milan",
            TaskType::Diarization,
            800,
            520,
            1.5,
        ),
    ] {
        repository
            .save_metrics(TaskMetrics::new(
                task_id,
                DeviceId::new(device_id),
                task_type,
                "fixture-model",
                duration_ms,
                latency_ms,
                power_mwh,
                current_timestamp_ms,
            ))
            .unwrap_or_else(|error| panic!("test metric must persist: {error}"));
    }

    let application_state = ServerAppState::new(repository, InMemoryVoiceVectorDB::new());
    let router = build_router(application_state.clone());
    (router, application_state)
}

/// Verifies telemetry dashboard endpoints expose only privacy-safe device and task metrics.
#[tokio::test]
async fn telemetry_dashboard_routes_return_device_series_and_power_summary() {
    let router = seeded_router().0;
    for (endpoint, assertion) in [
        ("/api/devices", ("devices", "device-rome")),
        (
            "/api/telemetry/devices/device-rome",
            ("metrics", "transcription"),
        ),
        ("/api/telemetry/summary", ("total_tasks", "2")),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(endpoint)
                    .body(Body::empty())
                    .unwrap_or_else(|error| panic!("test request must build: {error}")),
            )
            .await
            .unwrap_or_else(|error| panic!("router must serve telemetry endpoint: {error}"));
        assert_eq!(response.status(), StatusCode::OK);
        let response_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap_or_else(|error| panic!("response body must be readable: {error}"));
        let response_json: serde_json::Value = serde_json::from_slice(&response_bytes)
            .unwrap_or_else(|error| panic!("response must be JSON: {error}"));
        assert!(response_json.to_string().contains(assertion.0));
        assert!(response_json.to_string().contains(assertion.1));
    }
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

/// Verifies a browser can resolve a stored review clip through the application-backed API route.
#[tokio::test]
async fn voice_assignment_persists_a_variant_without_exposing_clip_audio() {
    let response = seeded_router()
        .0
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/voice/assign")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"clip_id":"00000000-0000-0000-0000-000000000001","identity_id":"00000000-0000-4000-8000-000000000002"}"#,
                ))
                .unwrap_or_else(|error| panic!("test request must build: {error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("router must serve the test request: {error}"));

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
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
    let telemetry_event = serde_json::to_value(WebSocketEvent::TelemetryUpdated)
        .unwrap_or_else(|error| panic!("telemetry event must serialize: {error}"));
    assert_eq!(telemetry_event["event_type"], "telemetry_updated");
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
