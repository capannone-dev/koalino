//! Specifies the web Settings page's routes (Issue #46): reading and changing the daemon's log
//! level and downloading its logs. A fake `LogControl` over a scratch log directory stands in for
//! the process-wide tracing subscriber.

use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use tower::ServiceExt;
use uuid::Uuid;

use crate::{
    adapters::storage::{
        in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
    },
    application::{
        crypto::voice_encryptor::ephemeral_cipher,
        log_control::{LogControl, LogControlError},
    },
    server::api::{ServerAppState, build_router},
};

/// Holds a level and a log directory; refuses the directive `refused`.
struct FakeLogControl {
    level: Mutex<String>,
    log_directory: PathBuf,
}

impl LogControl for FakeLogControl {
    fn current_level(&self) -> String {
        self.level
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn set_level(&self, filter_directive: &str) -> Result<String, LogControlError> {
        if filter_directive == "refused" {
            return Err(LogControlError::InvalidLevel("not a level".to_owned()));
        }
        filter_directive.clone_into(&mut self.level.lock().unwrap_or_else(PoisonError::into_inner));
        Ok(filter_directive.to_owned())
    }

    fn log_directory(&self) -> PathBuf {
        self.log_directory.clone()
    }
}

/// A scratch log directory, removed on drop.
struct ScratchLogs(PathBuf);

impl ScratchLogs {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("note-taker-log-routes-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory)
            .unwrap_or_else(|error| panic!("scratch directory must be created: {error}"));
        Self(directory)
    }
}

impl Drop for ScratchLogs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A router over the fake control, or without one.
fn router(log_directory: Option<&ScratchLogs>) -> axum::Router {
    let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .unwrap_or_else(|error| panic!("test SQLite repository must initialize: {error}"));
    let state = ServerAppState::new(repository, InMemoryVoiceVectorDB::new());
    build_router(match log_directory {
        Some(logs) => state.with_log_control(Arc::new(FakeLogControl {
            level: Mutex::new("info".to_owned()),
            log_directory: logs.0.clone(),
        })),
        None => state,
    })
}

/// Sends one request and returns its status, headers, and body.
async fn send(
    router: axum::Router,
    method: &str,
    uri: &str,
    body: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut request = Request::builder().method(method).uri(uri);
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    let response = router
        .oneshot(
            request
                .body(body.map_or_else(Body::empty, |text| Body::from(text.to_owned())))
                .unwrap_or_else(|error| panic!("test request must build: {error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("router must serve the test request: {error}"));
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap_or_else(|error| panic!("body must read: {error}"));
    (status, headers, bytes.to_vec())
}

/// Parses a JSON body.
fn json(body: &[u8]) -> serde_json::Value {
    serde_json::from_slice(body).unwrap_or_else(|error| panic!("body must be JSON: {error}"))
}

/// Verifies the level in effect is reported.
#[tokio::test]
async fn reports_the_level() {
    let logs = ScratchLogs::new();

    let (status, _, body) = send(router(Some(&logs)), "GET", "/api/settings/log-level", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json(&body), serde_json::json!({ "level": "info" }));
}

/// Verifies a new level is applied and echoed.
#[tokio::test]
async fn changes_the_level() {
    let logs = ScratchLogs::new();
    let router = router(Some(&logs));

    let (status, _, body) = send(
        router.clone(),
        "PUT",
        "/api/settings/log-level",
        Some(r#"{"level":"debug"}"#),
    )
    .await;
    let (_, _, after) = send(router, "GET", "/api/settings/log-level", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json(&body), serde_json::json!({ "level": "debug" }));
    assert_eq!(json(&after), serde_json::json!({ "level": "debug" }));
}

/// Verifies a refused level answers 400 with the reason.
#[tokio::test]
async fn refuses_an_invalid_level() {
    let logs = ScratchLogs::new();

    let (status, _, body) = send(
        router(Some(&logs)),
        "PUT",
        "/api/settings/log-level",
        Some(r#"{"level":"refused"}"#),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json(&body)["error_code"], "invalid_level");
}

/// Verifies the logs download as one JSON-lines attachment.
#[tokio::test]
async fn downloads_the_logs() {
    let logs = ScratchLogs::new();
    fs::write(logs.0.join("app_log.2026-10-27"), "{\"day\":27}\n")
        .unwrap_or_else(|error| panic!("log must be written: {error}"));
    fs::write(logs.0.join("app_log.2026-10-28"), "{\"day\":28}\n")
        .unwrap_or_else(|error| panic!("log must be written: {error}"));

    let (status, headers, body) = send(router(Some(&logs)), "GET", "/api/logs/export", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, b"{\"day\":27}\n{\"day\":28}\n");
    assert_eq!(
        headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/x-ndjson")
    );
    let disposition = headers
        .get(header::CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        disposition.starts_with("attachment; filename=\"note_taker_logs_")
            && disposition.ends_with(".jsonl\""),
        "{disposition}"
    );
    assert_eq!(
        fs::read_dir(&logs.0)
            .map(Iterator::count)
            .unwrap_or_default(),
        2,
        "the download leaves no file behind in the log directory"
    );
}

/// Verifies an empty log directory answers 404 `no_logs`.
#[tokio::test]
async fn no_logs_is_not_found() {
    let logs = ScratchLogs::new();

    let (status, _, body) = send(router(Some(&logs)), "GET", "/api/logs/export", None).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json(&body)["error_code"], "no_logs");
}

/// Verifies every route answers 503 when the process has no log control.
#[tokio::test]
async fn without_log_control_the_routes_are_unavailable() {
    for (method, uri, body) in [
        ("GET", "/api/settings/log-level", None),
        (
            "PUT",
            "/api/settings/log-level",
            Some(r#"{"level":"debug"}"#),
        ),
        ("GET", "/api/logs/export", None),
    ] {
        let (status, _, _) = send(router(None), method, uri, body).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{method} {uri}");
    }
}

/// Verifies the dashboard may send PUT across origins (CORS preflight).
#[tokio::test]
async fn the_dashboard_may_put() {
    let logs = ScratchLogs::new();
    let response = router(Some(&logs))
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/settings/log-level")
                .header(header::ORIGIN, "http://localhost:3000")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "PUT")
                .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
                .body(Body::empty())
                .unwrap_or_else(|error| panic!("test request must build: {error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("router must serve the preflight: {error}"));

    let allowed = response
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_METHODS)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert!(allowed.contains("PUT"), "{allowed}");
}
