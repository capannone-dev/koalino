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

/// Exposes the seeded router to the request-guard tests.
pub(super) fn seeded_router_for_guard_tests() -> axum::Router {
    seeded_router().0
}

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
    let fallback_title = response_json[0]["title"]
        .as_str()
        .unwrap_or_else(|| panic!("an untitled meeting must have a display title"));
    assert!(fallback_title.starts_with("Meeting "), "{response_json}");
    assert!(
        response_json[0]["created_at"].is_string(),
        "{response_json}"
    );
}

/// Verifies `POST /api/meetings` creates a titled meeting that `GET /api/meetings` and
/// `GET /api/meetings/{id}` then return with that title.
#[tokio::test]
async fn created_meeting_titles_appear_in_list_and_detail() {
    let router = seeded_router().0;

    let (create_status, created_meeting) = send_json(
        &router,
        "POST",
        "/api/meetings",
        Some(r#"{"title":"  Project Sync  "}"#),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED, "{created_meeting}");
    assert_eq!(created_meeting["title"], "Project Sync");
    let created_meeting_id = created_meeting["meeting_id"]
        .as_str()
        .unwrap_or_else(|| panic!("the created meeting must have an ID: {created_meeting}"))
        .to_owned();
    assert!(created_meeting_id.starts_with("meeting-"));

    let (list_status, listed_meetings) = send_json(&router, "GET", "/api/meetings", None).await;
    assert_eq!(list_status, StatusCode::OK);
    let listed_meeting = listed_meetings
        .as_array()
        .and_then(|meetings| {
            meetings
                .iter()
                .find(|meeting| meeting["meeting_id"] == created_meeting_id.as_str())
        })
        .unwrap_or_else(|| panic!("the created meeting must be listed: {listed_meetings}"));
    assert_eq!(listed_meeting["title"], "Project Sync");

    let (detail_status, meeting_detail) = send_json(
        &router,
        "GET",
        &format!("/api/meetings/{created_meeting_id}"),
        None,
    )
    .await;
    assert_eq!(detail_status, StatusCode::OK);
    assert_eq!(meeting_detail["title"], "Project Sync");
    assert_eq!(meeting_detail["transcript"], serde_json::Value::Null);
}

/// Verifies an untitled `POST /api/meetings` gets the dated fallback and invalid titles are
/// rejected without creating a meeting.
#[tokio::test]
async fn meeting_creation_defaults_and_rejects_invalid_titles() {
    let router = seeded_router().0;

    let (untitled_status, untitled_meeting) =
        send_json(&router, "POST", "/api/meetings", Some("{}")).await;
    assert_eq!(untitled_status, StatusCode::CREATED);
    assert!(
        untitled_meeting["title"]
            .as_str()
            .is_some_and(|title| title.starts_with("Meeting ")),
        "{untitled_meeting}"
    );

    for invalid_body in [
        r#"{"title":"   "}"#,
        r#"{"title":"a\u0007b"}"#,
        r#"{"title":7}"#,
    ] {
        let (status, _) = send_json(&router, "POST", "/api/meetings", Some(invalid_body)).await;
        assert!(status.is_client_error(), "{invalid_body} returned {status}");
    }
    let (_, listed_meetings) = send_json(&router, "GET", "/api/meetings", None).await;
    assert_eq!(
        listed_meetings.as_array().map(Vec::len),
        Some(2),
        "{listed_meetings}"
    );
}

/// Verifies `PATCH /api/meetings/{id}` renames a meeting, returns its summary with the trimmed
/// title, and `GET` then shows the new title.
#[tokio::test]
async fn patch_meeting_renames_it() {
    let router = seeded_router().0;

    let (rename_status, renamed_meeting) = send_json(
        &router,
        "PATCH",
        "/api/meetings/meeting-rome",
        Some(r#"{"title":"  Rome offsite  "}"#),
    )
    .await;
    assert_eq!(rename_status, StatusCode::OK, "{renamed_meeting}");
    assert_eq!(renamed_meeting["meeting_id"], "meeting-rome");
    assert_eq!(renamed_meeting["title"], "Rome offsite");
    assert!(
        renamed_meeting["created_at"].is_string(),
        "{renamed_meeting}"
    );

    let (detail_status, meeting_detail) =
        send_json(&router, "GET", "/api/meetings/meeting-rome", None).await;
    assert_eq!(detail_status, StatusCode::OK);
    assert_eq!(meeting_detail["title"], "Rome offsite");
}

/// Verifies renaming an unknown meeting returns `404` with a JSON error and creates nothing.
#[tokio::test]
async fn patch_unknown_meeting_returns_not_found() {
    let router = seeded_router().0;

    let (status, error_body) = send_json(
        &router,
        "PATCH",
        "/api/meetings/missing-meeting",
        Some(r#"{"title":"Orphan"}"#),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND, "{error_body}");
    assert!(
        error_body["error"]
            .as_str()
            .is_some_and(|message| message.contains("missing-meeting")),
        "{error_body}"
    );
    let (_, listed_meetings) = send_json(&router, "GET", "/api/meetings", None).await;
    assert_eq!(
        listed_meetings.as_array().map(Vec::len),
        Some(1),
        "{listed_meetings}"
    );
}

/// Verifies invalid titles return `400` with a JSON error, malformed bodies a client error, and
/// neither changes the stored title.
#[tokio::test]
async fn patch_meeting_rejects_invalid_titles() {
    let router = seeded_router().0;
    let (fixture_status, _) = send_json(
        &router,
        "PATCH",
        "/api/meetings/meeting-rome",
        Some(r#"{"title":"Original"}"#),
    )
    .await;
    assert_eq!(fixture_status, StatusCode::OK);
    let oversized_body = format!(r#"{{"title":"{}"}}"#, "x".repeat(201));

    for invalid_body in [
        r#"{"title":""}"#,
        r#"{"title":"   "}"#,
        r#"{"title":"line one\nline two"}"#,
        r#"{"title":"a\u0007b"}"#,
        oversized_body.as_str(),
    ] {
        let (status, error_body) = send_json(
            &router,
            "PATCH",
            "/api/meetings/meeting-rome",
            Some(invalid_body),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{invalid_body}");
        assert!(
            error_body["error"]
                .as_str()
                .is_some_and(|message| message.contains("meeting title")),
            "{invalid_body} returned {error_body}"
        );
    }
    for malformed_body in ["{}", r#"{"title":7}"#, r#"{"title":"x","extra":1}"#] {
        let (status, _) = send_json(
            &router,
            "PATCH",
            "/api/meetings/meeting-rome",
            Some(malformed_body),
        )
        .await;
        assert!(
            status.is_client_error(),
            "{malformed_body} returned {status}"
        );
    }

    let (_, meeting_detail) = send_json(&router, "GET", "/api/meetings/meeting-rome", None).await;
    assert_eq!(meeting_detail["title"], "Original");
}

/// Verifies the local web UI's CORS preflight is allowed to send `PATCH`.
#[tokio::test]
async fn cors_preflight_allows_patch_from_the_web_ui() {
    let response = seeded_router()
        .0
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/meetings/meeting-rome")
                .header("origin", "http://localhost:3000")
                .header("access-control-request-method", "PATCH")
                .header("access-control-request-headers", "content-type")
                .body(Body::empty())
                .unwrap_or_else(|error| panic!("test request must build: {error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("router must serve the test request: {error}"));

    let allowed_methods = response
        .headers()
        .get("access-control-allow-methods")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert!(allowed_methods.contains("PATCH"), "{allowed_methods:?}");
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
    assert!(
        response_json["title"]
            .as_str()
            .is_some_and(|title| title.starts_with("Meeting ")),
        "{response_json}"
    );
    assert_eq!(
        response_json["attachments"][0]["file_path"],
        "/fixtures/agenda.pdf"
    );
}

/// A recorded meeting stores only timed segments; its detail shows them as the transcript, so
/// the web meeting page is not empty for a meeting recorded on any device.
#[tokio::test]
async fn meeting_detail_shows_a_recorded_meeting_s_segments_as_its_transcript() {
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("test SQLite repository must initialize: {error}"));
    let meeting_id = MeetingId::new("meeting-recorded");
    repository
        .create_meeting(&meeting_id, &AutoCommit::new().save())
        .unwrap_or_else(|error| panic!("test meeting must persist: {error}"));
    repository
        .save_transcript_segments(
            &meeting_id,
            &[
                TranscriptSegment::new(meeting_id.clone(), 2_000, 4_000, None, "and then"),
                TranscriptSegment::new(meeting_id.clone(), 0, 2_000, None, "We start"),
            ],
        )
        .unwrap_or_else(|error| panic!("test segments must persist: {error}"));
    let router = build_router(ServerAppState::new(
        repository,
        InMemoryVoiceVectorDB::new(),
    ));

    let (status, meeting_detail) =
        send_json(&router, "GET", "/api/meetings/meeting-recorded", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(meeting_detail["transcript"], "We start\nand then");
}

/// The detail lists the segments with their speaker and how doubtful they are, so the web page
/// can grey uncertain words and mark overlap and guessed speakers (#90, #91).
#[tokio::test]
async fn meeting_detail_lists_segments_with_their_confidence_marks() {
    use crate::domain::transcript_confidence::SegmentConfidence;
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("test SQLite repository must initialize: {error}"));
    let meeting_id = MeetingId::new("meeting-marked");
    repository
        .create_meeting(&meeting_id, &AutoCommit::new().save())
        .unwrap_or_else(|error| panic!("test meeting must persist: {error}"));
    repository
        .save_transcript_segments(
            &meeting_id,
            &[
                TranscriptSegment::new(meeting_id.clone(), 0, 2_000, None, "We start now")
                    .with_confidence(SegmentConfidence {
                        overlapping_speech: true,
                        uncertain_speaker: true,
                        uncertain_words: vec![2],
                    }),
                TranscriptSegment::new(meeting_id.clone(), 2_000, 3_000, None, "fine"),
            ],
        )
        .unwrap_or_else(|error| panic!("test segments must persist: {error}"));
    let router = build_router(ServerAppState::new(
        repository,
        InMemoryVoiceVectorDB::new(),
    ));

    let (status, meeting_detail) =
        send_json(&router, "GET", "/api/meetings/meeting-marked", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        meeting_detail["segments"],
        serde_json::json!([
            {
                "start_time_ms": 0, "end_time_ms": 2000, "speaker": "Unattributed",
                "text": "We start now", "overlapping_speech": true, "uncertain_speaker": true,
                "uncertain_words": [2]
            },
            {
                "start_time_ms": 2000, "end_time_ms": 3000, "speaker": "Unattributed",
                "text": "fine", "overlapping_speech": false, "uncertain_speaker": false,
                "uncertain_words": []
            }
        ])
    );
}

/// The speech-to-text languages recorded for a meeting appear, in first-use order, in its
/// detail and in the meeting list, so the web meeting page can show them.
#[tokio::test]
async fn meeting_detail_and_list_name_the_transcription_languages() {
    use crate::{
        application::{
            meeting_crdt::MeetingCrdtService, meeting_document::AutomergeMeetingDocumentCodec,
        },
        domain::transcription_language::TranscriptionLanguage,
    };
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .and_then(|repository| {
        repository.with_document_codec(std::sync::Arc::new(AutomergeMeetingDocumentCodec))
    })
    .unwrap_or_else(|error| panic!("test SQLite repository must initialize: {error}"));
    let meeting_id = MeetingId::new("meeting-languages");
    MeetingCrdtService::new(&repository)
        .create_meeting(&meeting_id)
        .unwrap_or_else(|error| panic!("test meeting must persist: {error}"));
    for (code, offset_ms) in [("en", 5_000), ("it", 0)] {
        let language = TranscriptionLanguage::parse(code)
            .unwrap_or_else(|_| panic!("{code} must be a valid language"));
        repository
            .record_transcription_language(&meeting_id, language, offset_ms)
            .unwrap_or_else(|error| panic!("the language must be recorded: {error}"));
    }
    let router = build_router(ServerAppState::new(
        repository,
        InMemoryVoiceVectorDB::new(),
    ));

    let (detail_status, meeting_detail) =
        send_json(&router, "GET", "/api/meetings/meeting-languages", None).await;
    let (list_status, meetings) = send_json(&router, "GET", "/api/meetings", None).await;

    assert_eq!(detail_status, StatusCode::OK);
    assert_eq!(
        meeting_detail["transcription_languages"],
        serde_json::json!(["it", "en"])
    );
    assert_eq!(list_status, StatusCode::OK);
    assert_eq!(
        meetings[0]["transcription_languages"],
        serde_json::json!(["it", "en"])
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
    assert!(
        markdown_body.starts_with("# Meeting "),
        "untitled meetings export under their dated fallback title: {markdown_body}"
    );
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
    assert_eq!(assignment["variants_stored"], 0);
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

/// Verifies the gallery tells which group each voice is in, and that naming a group names each
/// chosen voice as one person (#82).
#[tokio::test]
async fn a_voice_group_is_listed_and_named_at_once() {
    let (router, large_cluster_id, small_cluster_id) = cluster_router();

    let (_, gallery) = send_json(&router, "GET", "/api/voice-gallery", None).await;
    let clusters = gallery["unknown_clusters"]
        .as_array()
        .unwrap_or_else(|| panic!("clusters must be an array: {gallery}"));
    for cluster in clusters {
        assert!(
            cluster["voice_group_id"]
                .as_str()
                .and_then(|group_id| uuid::Uuid::parse_str(group_id).ok())
                .is_some(),
            "each voice names its group: {cluster}"
        );
    }

    let (status, assignment) = send_json(
        &router,
        "POST",
        "/api/voice-groups/assign",
        Some(&format!(
            r#"{{"cluster_ids":["{large_cluster_id}","{small_cluster_id}"],"identity_name":"Grace"}}"#
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{assignment}");
    assert_eq!(assignment["identity_name"], "Grace");
    assert_eq!(assignment["identity_created"], true);
    assert_eq!(
        assignment["assigned_cluster_ids"],
        serde_json::json!([large_cluster_id.to_string(), small_cluster_id.to_string()])
    );
    assert_eq!(assignment["skipped_cluster_count"], 0);
    let (_, remaining) = send_json(
        &router,
        "GET",
        "/api/meetings/meeting-clusters/unknown-clusters",
        None,
    )
    .await;
    assert_eq!(remaining, serde_json::json!([]));

    let (empty_status, _) = send_json(
        &router,
        "POST",
        "/api/voice-groups/assign",
        Some(r#"{"cluster_ids":[],"identity_name":"Grace"}"#),
    )
    .await;
    assert_eq!(empty_status, StatusCode::BAD_REQUEST);
    let (named_status, _) = send_json(
        &router,
        "POST",
        "/api/voice-groups/assign",
        Some(&format!(
            r#"{{"cluster_ids":["{large_cluster_id}"],"identity_name":"Grace"}}"#
        )),
    )
    .await;
    assert_eq!(named_status, StatusCode::CONFLICT);
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

/// Verifies the gallery lists every meeting's clusters without embeddings, and that a review clip
/// is served only for a cluster that has one.
#[tokio::test]
async fn voice_gallery_lists_clusters_and_serves_review_clips() {
    let (router, large_cluster_id, _) = cluster_router();

    let (status, gallery) = send_json(&router, "GET", "/api/voice-gallery", None).await;
    let clip_status = |cluster_id: uuid::Uuid| {
        let router = router.clone();
        async move {
            router
                .oneshot(
                    Request::builder()
                        .uri(format!("/api/clusters/{cluster_id}/review-clip"))
                        .body(Body::empty())
                        .unwrap_or_else(|error| panic!("test request must build: {error}")),
                )
                .await
                .unwrap_or_else(|error| panic!("router must serve the test request: {error}"))
                .status()
        }
    };

    assert_eq!(status, StatusCode::OK);
    let clusters = gallery["unknown_clusters"]
        .as_array()
        .unwrap_or_else(|| panic!("clusters must be an array: {gallery}"));
    assert_eq!(clusters.len(), 2);
    assert_eq!(clusters[0]["cluster_id"], large_cluster_id.to_string());
    assert_eq!(clusters[0]["has_review_clip"], false);
    assert!(clusters[0].get("representative_embedding").is_none());
    assert_eq!(clip_status(large_cluster_id).await, StatusCode::NOT_FOUND);
    assert_eq!(
        clip_status(uuid::Uuid::new_v4()).await,
        StatusCode::NOT_FOUND
    );
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

/// Hooks reporting `recording` as being recorded and remembering what was removed.
struct RecordingHooks {
    recording: &'static str,
    removed: std::sync::Mutex<Vec<String>>,
}

impl crate::application::deletion::MeetingDeletionHooks for RecordingHooks {
    fn is_recording(&self, meeting_id: &MeetingId) -> bool {
        meeting_id.as_str() == self.recording
    }

    fn remove_files(&self, meeting_id: &MeetingId) {
        if let Ok(mut removed) = self.removed.lock() {
            removed.push(meeting_id.as_str().to_owned());
        }
    }
}

/// Verifies DELETE removes a meeting and its files, and answers 404 afterwards.
#[tokio::test]
async fn delete_meeting_removes_it_and_its_files() {
    let (_, state) = seeded_router();
    let hooks = std::sync::Arc::new(RecordingHooks {
        recording: "meeting-live",
        removed: std::sync::Mutex::new(Vec::new()),
    });
    let router = build_router(state.with_deletion_hooks(hooks.clone()));

    let (status, _) = send_json(&router, "DELETE", "/api/meetings/meeting-rome", None).await;
    let (detail_status, _) = send_json(&router, "GET", "/api/meetings/meeting-rome", None).await;
    let (repeat_status, repeat_body) =
        send_json(&router, "DELETE", "/api/meetings/meeting-rome", None).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(detail_status, StatusCode::NOT_FOUND);
    assert_eq!(repeat_status, StatusCode::NOT_FOUND);
    assert!(repeat_body["error"].is_string(), "{repeat_body}");
    assert_eq!(
        hooks.removed.lock().map(|removed| removed.clone()).ok(),
        Some(vec!["meeting-rome".to_owned()])
    );
}

/// Verifies a meeting being recorded is refused and stays.
#[tokio::test]
async fn delete_meeting_refuses_one_being_recorded() {
    let (_, state) = seeded_router();
    let hooks = std::sync::Arc::new(RecordingHooks {
        recording: "meeting-rome",
        removed: std::sync::Mutex::new(Vec::new()),
    });
    let router = build_router(state.with_deletion_hooks(hooks));

    let (status, _) = send_json(&router, "DELETE", "/api/meetings/meeting-rome", None).await;
    let (detail_status, _) = send_json(&router, "GET", "/api/meetings/meeting-rome", None).await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(detail_status, StatusCode::OK);
}

/// Verifies an unknown voice can be deleted over HTTP, and leaves the gallery.
#[tokio::test]
async fn delete_cluster_removes_it_from_the_gallery() {
    let (router, large_cluster_id, _) = cluster_router();
    let path = format!("/api/clusters/{large_cluster_id}");

    let (status, _) = send_json(&router, "DELETE", &path, None).await;
    let (repeat_status, _) = send_json(&router, "DELETE", &path, None).await;
    let (_, gallery) = send_json(&router, "GET", "/api/voice-gallery", None).await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(repeat_status, StatusCode::NOT_FOUND);
    let clusters = gallery["unknown_clusters"]
        .as_array()
        .unwrap_or_else(|| panic!("clusters must be an array: {gallery}"));
    assert_eq!(clusters.len(), 1);
    assert_ne!(clusters[0]["cluster_id"], large_cluster_id.to_string());
}

/// Verifies a saved person can be deleted over HTTP, and a malformed ID is refused.
#[tokio::test]
async fn delete_identity_removes_them_from_the_gallery() {
    let (router, large_cluster_id, _) = cluster_router();
    let (assign_status, assignment) = send_json(
        &router,
        "POST",
        &format!("/api/clusters/{large_cluster_id}/assign"),
        Some(r#"{"identity_name":"Ada"}"#),
    )
    .await;
    assert_eq!(assign_status, StatusCode::OK, "{assignment}");
    let identity_id = assignment["identity_id"].as_str().unwrap_or_default();

    let (bad_status, _) = send_json(&router, "DELETE", "/api/identities/not-a-uuid", None).await;
    let (status, _) = send_json(
        &router,
        "DELETE",
        &format!("/api/identities/{identity_id}"),
        None,
    )
    .await;
    let (_, gallery) = send_json(&router, "GET", "/api/voice-gallery", None).await;

    assert_eq!(bad_status, StatusCode::BAD_REQUEST);
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(gallery["identities"], serde_json::json!([]));
}

/// Verifies a meeting recorded on two devices is listed once with both devices and the recording
/// combined into it, and that splitting lists that recording again (#88).
#[tokio::test]
async fn a_combined_meeting_is_listed_once_and_can_be_split() {
    let directory = std::env::temp_dir().join(format!("api-combined-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap_or_else(|error| panic!("{error}"));
    let database_path = directory.join("notes.sqlite3");
    let laptop = file_backed_stores(&database_path, "device-laptop");
    let phone = file_backed_stores(&database_path, "device-phone");
    let empty_document = AutoCommit::new().save();
    laptop
        .note_repository
        .create_meeting(&MeetingId::new("meeting-laptop"), &empty_document)
        .unwrap_or_else(|error| panic!("{error}"));
    phone
        .note_repository
        .create_meeting(&MeetingId::new("meeting-phone"), &empty_document)
        .unwrap_or_else(|error| panic!("{error}"));
    laptop
        .note_repository
        .combine_meeting(
            &MeetingId::new("meeting-phone"),
            &crate::ports::meeting_document::MeetingCombination {
                into: MeetingId::new("meeting-laptop"),
                offset_ms: 2_000,
                combine_on: DeviceId::new("device-laptop"),
            },
        )
        .unwrap_or_else(|error| panic!("{error}"));
    drop(phone);
    // The laptop holds its own speech and waits for the phone's.
    let retained_directory = directory.join("retained-audio");
    crate::adapters::audio::retained_audio_store::RetainedAudioStore::new(
        retained_directory.clone(),
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .append_chunk(
        &MeetingId::new("meeting-laptop"),
        crate::application::transcript_refinement::ChunkSpan {
            start_ms: 0,
            end_ms: 100,
        },
        None,
        &[1; 1_600],
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let router = build_router(
        ServerAppState::new(laptop.note_repository, laptop.voice_store)
            .with_recording_device(DeviceId::new("device-laptop"), retained_directory),
    );

    let (_, combined) = send_json(&router, "GET", "/api/meetings", None).await;
    let (_, detail) = send_json(&router, "GET", "/api/meetings/meeting-laptop", None).await;
    let (missing_status, _) =
        send_json(&router, "POST", "/api/meetings/meeting-missing/split", None).await;
    let (split_status, split) =
        send_json(&router, "POST", "/api/meetings/meeting-laptop/split", None).await;
    let (_, apart) = send_json(&router, "GET", "/api/meetings", None).await;
    let _ = std::fs::remove_dir_all(&directory);

    let combined = combined
        .as_array()
        .unwrap_or_else(|| panic!("a list: {combined}"));
    assert_eq!(combined.len(), 1, "{combined:?}");
    assert_eq!(combined[0]["meeting_id"], "meeting-laptop");
    assert_eq!(
        combined[0]["recorded_on"],
        serde_json::json!([
            {"device_id": "device-laptop", "name": null, "this_device": false, "connected": null},
            {"device_id": "device-phone", "name": null, "this_device": false, "connected": null}
        ])
    );
    assert_eq!(
        combined[0]["combined_recordings"],
        serde_json::json!(["meeting-phone"])
    );
    assert_eq!(
        detail["combined_recordings"],
        serde_json::json!(["meeting-phone"])
    );
    assert_eq!(detail["recorded_on"].as_array().map(Vec::len), Some(2));
    assert_eq!(detail["combining"]["state"], "waiting");
    assert_eq!(
        detail["combining"]["waiting_for"][0]["device_id"],
        "device-phone"
    );
    assert!(detail["combining"]["kept_until"].is_string());
    assert_eq!(combined[0]["combining"]["state"], "waiting");
    assert_eq!(missing_status, StatusCode::NOT_FOUND);
    assert_eq!(split_status, StatusCode::OK);
    assert_eq!(split, serde_json::json!({"split": ["meeting-phone"]}));
    assert_eq!(apart.as_array().map(Vec::len), Some(2), "{apart}");
}
