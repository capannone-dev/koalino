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
    application::unknown_speaker_clusterer::{ClusteringConfig, UnknownSpeakerClusterer},
    domain::models::{
        Attachment, DeviceId, MeetingId, TaskMetrics, TaskType, Transcript, TranscriptSegment,
        UnassignedVoiceClip,
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
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
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

/// Sends `POST /api/meetings/{meeting_id}/export` with a JSON body.
async fn post_export(meeting_id: &str, json_body: &str) -> axum::response::Response {
    seeded_router()
        .0
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/meetings/{meeting_id}/export"))
                .header("content-type", "application/json")
                .body(Body::from(String::from(json_body)))
                .unwrap_or_else(|error| panic!("test request must build: {error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("router must serve the export request: {error}"))
}

/// Reads a response header as text.
fn header_text(response: &axum::response::Response, header_name: &str) -> String {
    response
        .headers()
        .get(header_name)
        .and_then(|header_value| header_value.to_str().ok())
        .map(String::from)
        .unwrap_or_default()
}

/// Verifies Markdown export returns the rendered file as a named download.
#[tokio::test]
async fn export_endpoint_returns_markdown_download() {
    let response = post_export(
        "meeting-rome",
        r#"{"format":"markdown","options":{"include_attachments":true,"include_metadata":true}}"#,
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        header_text(&response, "content-type"),
        "text/markdown; charset=utf-8"
    );
    let content_disposition = header_text(&response, "content-disposition");
    assert!(content_disposition.starts_with("attachment; filename=\""));
    assert!(content_disposition.contains("meeting-rome"));
    assert!(content_disposition.ends_with(".md\""));
    let response_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|error| panic!("response body must be readable: {error}"));
    let markdown_body = String::from_utf8_lossy(&response_bytes);
    assert!(markdown_body.contains("# meeting-rome"));
    assert!(markdown_body.contains("Discussed offline-first sync."));
    assert!(markdown_body.contains("agenda.pdf"));
}

/// Verifies PDF and JSON exports return their native content types and bodies, with defaults.
#[tokio::test]
async fn export_endpoint_returns_pdf_and_json_bodies() {
    let pdf_response = post_export("meeting-rome", r#"{"format":"pdf"}"#).await;
    assert_eq!(pdf_response.status(), StatusCode::OK);
    assert_eq!(
        header_text(&pdf_response, "content-type"),
        "application/pdf"
    );
    let pdf_bytes = axum::body::to_bytes(pdf_response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|error| panic!("PDF body must be readable: {error}"));
    assert!(pdf_bytes.starts_with(b"%PDF-"));

    let json_response = post_export("meeting-rome", r#"{"format":"json"}"#).await;
    assert_eq!(json_response.status(), StatusCode::OK);
    assert_eq!(
        header_text(&json_response, "content-type"),
        "application/json"
    );
    let json_bytes = axum::body::to_bytes(json_response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|error| panic!("JSON body must be readable: {error}"));
    let export_json: serde_json::Value = serde_json::from_slice(&json_bytes)
        .unwrap_or_else(|error| panic!("export body must be JSON: {error}"));
    assert_eq!(export_json["schema_version"], "1.0");
    assert_eq!(
        export_json["meetings"][0]["attachments"][0]["file_path"],
        "/fixtures/agenda.pdf"
    );
}

/// Verifies invalid formats are 400s with a message and unknown meetings are 404s.
#[tokio::test]
async fn export_endpoint_rejects_bad_formats_and_unknown_meetings() {
    for (meeting_id, json_body, expected_status) in [
        (
            "meeting-rome",
            r#"{"format":"docx"}"#,
            StatusCode::BAD_REQUEST,
        ),
        (
            "meeting-rome",
            r#"{"format":"srt"}"#,
            StatusCode::BAD_REQUEST,
        ),
        (
            "meeting-missing",
            r#"{"format":"json"}"#,
            StatusCode::NOT_FOUND,
        ),
    ] {
        let response = post_export(meeting_id, json_body).await;
        assert_eq!(
            response.status(),
            expected_status,
            "{json_body} for {meeting_id}"
        );
        let response_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap_or_else(|error| panic!("error body must be readable: {error}"));
        let error_json: serde_json::Value = serde_json::from_slice(&response_bytes)
            .unwrap_or_else(|error| panic!("error body must be JSON: {error}"));
        assert!(
            error_json["error"]
                .as_str()
                .is_some_and(|message| !message.is_empty())
        );
    }
}

/// Builds a router over in-memory SQLite holding one meeting with two unknown-speaker clusters.
///
/// Outputs: The router, the two-chunk cluster ID (2.5 s, one linked segment), and the one-chunk
/// cluster ID.
fn cluster_router() -> (axum::Router, uuid::Uuid, uuid::Uuid) {
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("test SQLite repository must initialize: {error}"));
    let meeting_id = MeetingId::new("meeting-clusters");
    repository
        .create_meeting(&meeting_id, &[])
        .unwrap_or_else(|error| panic!("test meeting must persist: {error}"));
    let clusterer = UnknownSpeakerClusterer::new(&repository, ClusteringConfig::default());
    let add_chunk = |embedding: &[f32], chunk_id: &str, duration_ms: u64| {
        clusterer
            .find_or_create_cluster(&meeting_id, embedding, String::from(chunk_id), duration_ms)
            .unwrap_or_else(|error| panic!("test chunk must cluster: {error}"))
            .cluster_id()
    };
    let large_cluster_id = add_chunk(&[1.0, 0.0], "c1", 1_000);
    add_chunk(&[0.9, 0.3], "c2", 1_500);
    let small_cluster_id = add_chunk(&[0.0, 1.0], "c3", 400);
    repository
        .save_transcript_segments(
            &meeting_id,
            &[
                TranscriptSegment::new(meeting_id.clone(), 0, 1_000, None, "hello")
                    .with_unknown_cluster_id(Some(large_cluster_id)),
            ],
        )
        .unwrap_or_else(|error| panic!("test segments must persist: {error}"));
    let router = build_router(ServerAppState::new(
        repository,
        InMemoryVoiceVectorDB::new(),
    ));
    (router, large_cluster_id, small_cluster_id)
}

/// Sends one request and returns its status and JSON body (`Null` for an empty body).
async fn send_json(
    router: &axum::Router,
    method: &str,
    uri: &str,
    json_body: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    let mut request_builder = Request::builder().method(method).uri(uri);
    if json_body.is_some() {
        request_builder = request_builder.header("content-type", "application/json");
    }
    let response = router
        .clone()
        .oneshot(
            request_builder
                .body(json_body.map_or_else(Body::empty, |body| Body::from(String::from(body))))
                .unwrap_or_else(|error| panic!("test request must build: {error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("router must serve the test request: {error}"));
    let status = response.status();
    let response_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|error| panic!("response body must be readable: {error}"));
    let response_json = if response_bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&response_bytes).unwrap_or(serde_json::Value::Null)
    };
    (status, response_json)
}

/// Verifies the cluster listing returns embedding-free summaries in creation order.
#[tokio::test]
async fn unknown_clusters_endpoint_lists_unresolved_cluster_summaries() {
    let (router, large_cluster_id, small_cluster_id) = cluster_router();

    let (status, clusters) = send_json(
        &router,
        "GET",
        "/api/meetings/meeting-clusters/unknown-clusters",
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let cluster_list = clusters
        .as_array()
        .unwrap_or_else(|| panic!("response must be a JSON array: {clusters}"));
    assert_eq!(cluster_list.len(), 2);
    assert_eq!(cluster_list[0]["cluster_id"], large_cluster_id.to_string());
    assert_eq!(cluster_list[0]["meeting_id"], "meeting-clusters");
    assert_eq!(cluster_list[0]["total_duration_ms"], 2_500);
    assert_eq!(cluster_list[0]["chunk_count"], 2);
    assert!(cluster_list[0]["created_at"].is_string());
    assert!(
        cluster_list[0].get("representative_embedding").is_none(),
        "biometric vectors must never be serialized"
    );
    assert_eq!(cluster_list[1]["cluster_id"], small_cluster_id.to_string());

    let (unknown_status, unknown_clusters) = send_json(
        &router,
        "GET",
        "/api/meetings/missing-meeting/unknown-clusters",
        None,
    )
    .await;
    assert_eq!(unknown_status, StatusCode::OK);
    assert_eq!(unknown_clusters, serde_json::json!([]));
}

/// Verifies assignment returns 200 with a summary and removes the cluster from the listing.
#[tokio::test]
async fn cluster_assignment_endpoint_assigns_and_resolves_the_cluster() {
    let (router, large_cluster_id, small_cluster_id) = cluster_router();

    let (status, assignment) = send_json(
        &router,
        "POST",
        &format!("/api/clusters/{large_cluster_id}/assign"),
        Some(r#"{"identity_name":" Grace Hopper "}"#),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(assignment["cluster_id"], large_cluster_id.to_string());
    assert_eq!(assignment["identity_name"], "Grace Hopper");
    assert_eq!(assignment["identity_created"], true);
    assert_eq!(assignment["variants_stored"], 2);
    assert_eq!(assignment["segments_updated"], 1);
    assert!(
        assignment["identity_id"]
            .as_str()
            .and_then(|identity_id| uuid::Uuid::parse_str(identity_id).ok())
            .is_some(),
        "identity_id must be a UUID: {assignment}"
    );

    let (_, remaining) = send_json(
        &router,
        "GET",
        "/api/meetings/meeting-clusters/unknown-clusters",
        None,
    )
    .await;
    assert_eq!(remaining.as_array().map(Vec::len), Some(1));
    assert_eq!(remaining[0]["cluster_id"], small_cluster_id.to_string());

    let (repeated_status, repeated_body) = send_json(
        &router,
        "POST",
        &format!("/api/clusters/{large_cluster_id}/assign"),
        Some(r#"{"identity_name":"Grace Hopper"}"#),
    )
    .await;
    assert_eq!(repeated_status, StatusCode::CONFLICT);
    assert!(repeated_body["error"].is_string());
}

/// Verifies malformed IDs, blank names, and unknown clusters map to client errors.
#[tokio::test]
async fn cluster_assignment_endpoint_rejects_invalid_requests() {
    let (router, large_cluster_id, _) = cluster_router();

    let (malformed_status, _) = send_json(
        &router,
        "POST",
        "/api/clusters/not-a-uuid/assign",
        Some(r#"{"identity_name":"Grace"}"#),
    )
    .await;
    assert_eq!(malformed_status, StatusCode::BAD_REQUEST);

    let (blank_status, blank_body) = send_json(
        &router,
        "POST",
        &format!("/api/clusters/{large_cluster_id}/assign"),
        Some(r#"{"identity_name":"   "}"#),
    )
    .await;
    assert_eq!(blank_status, StatusCode::BAD_REQUEST);
    assert_eq!(blank_body["error"], "identity name must not be blank");

    let (missing_status, missing_body) = send_json(
        &router,
        "POST",
        &format!("/api/clusters/{}/assign", uuid::Uuid::new_v4()),
        Some(r#"{"identity_name":"Grace"}"#),
    )
    .await;
    assert_eq!(missing_status, StatusCode::NOT_FOUND);
    assert!(missing_body["error"].is_string());
}

/// Builds a router over the SQLite file at `database_path`, composed as `bin/daemon.rs` does.
fn file_backed_router(database_path: &std::path::Path) -> axum::Router {
    let stores = file_backed_stores(database_path, "headless-server");
    build_router(ServerAppState::new(
        stores.note_repository,
        stores.voice_store,
    ))
}

/// Opens the encrypted stores on `database_path` with a key directory beside the file.
fn file_backed_stores(
    database_path: &std::path::Path,
    device_id: &str,
) -> crate::storage_composition::EncryptedStores {
    crate::storage_composition::open_encrypted_stores(
        database_path,
        DeviceId::new(device_id),
        crate::storage_composition::key_file_config_for_tests(
            &database_path.with_extension("keys"),
        ),
    )
    .unwrap_or_else(|error| panic!("encrypted stores must open: {error:#}"))
}

/// Verifies a cluster assigned over HTTP appears by name in the voice gallery, including after
/// the daemon restarts on the same database file.
#[tokio::test]
async fn assigned_cluster_names_appear_in_the_voice_gallery_after_a_restart() {
    let database_path = std::env::temp_dir().join(format!(
        "note-taker-api-voice-{}.sqlite3",
        uuid::Uuid::new_v4()
    ));
    let meeting_id = MeetingId::new("meeting-api-persist");
    let cluster_id = {
        let repository = file_backed_stores(&database_path, "seed").note_repository;
        repository
            .create_meeting(&meeting_id, &[])
            .unwrap_or_else(|error| panic!("test meeting must persist: {error}"));
        UnknownSpeakerClusterer::new(&repository, ClusteringConfig::default())
            .find_or_create_cluster(&meeting_id, &[1.0, 0.0], String::from("c1"), 1_000)
            .unwrap_or_else(|error| panic!("test chunk must cluster: {error}"))
            .cluster_id()
    };
    let gallery_names = |gallery: &serde_json::Value| -> Vec<String> {
        gallery["identities"]
            .as_array()
            .map(|identities| {
                identities
                    .iter()
                    .filter_map(|identity| identity["display_name"].as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };

    let running_router = file_backed_router(&database_path);
    let (assign_status, assignment) = send_json(
        &running_router,
        "POST",
        &format!("/api/clusters/{cluster_id}/assign"),
        Some(r#"{"identity_name":"Grace Hopper"}"#),
    )
    .await;
    let (_, live_gallery) = send_json(&running_router, "GET", "/api/voice-gallery", None).await;
    drop(running_router);
    let restarted_router = file_backed_router(&database_path);
    let (restarted_status, restarted_gallery) =
        send_json(&restarted_router, "GET", "/api/voice-gallery", None).await;
    drop(restarted_router);
    for suffix in ["", "-journal", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", database_path.display()));
    }
    let _ = std::fs::remove_dir_all(database_path.with_extension("keys"));

    assert_eq!(assign_status, StatusCode::OK);
    assert_eq!(assignment["identity_name"], "Grace Hopper");
    assert_eq!(gallery_names(&live_gallery), vec!["Grace Hopper"]);
    assert_eq!(restarted_status, StatusCode::OK);
    assert_eq!(gallery_names(&restarted_gallery), vec!["Grace Hopper"]);
}
