//! Serves the web Settings page's routes (REQUIREMENTS 6.1, 6.2; Issue #46).
//!
//! | Route | Success |
//! | --- | --- |
//! | `GET /api/settings/log-level` | 200 `{"level"}`, the level in effect |
//! | `PUT /api/settings/log-level` `{"level"}` | 200 `{"level"}`; lasts until the daemon restarts |
//! | `GET /api/logs/export` | 200, the newest week of logs as one `application/x-ndjson` attachment |
//!
//! Failures answer `{"error_code","error"}`: 400 `invalid_level`, 404 `no_logs`, 500
//! `export_failed` or `logging_unavailable`, and 503 `unavailable` when the process has no log
//! control. The routes sit behind the same request guard as the rest of the API. The logs were
//! redacted when written, so the download holds no meeting content.

use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;

use super::api::ServerAppState;
use crate::{
    adapters::observability::{LogExportError, export_log_files},
    application::log_control::LogControlError,
};

/// Registers the routes; `build_router` merges them before its layers.
pub(super) fn routes() -> Router<ServerAppState> {
    Router::new()
        .route(
            "/api/settings/log-level",
            get(current_level).put(change_level),
        )
        .route("/api/logs/export", get(export_logs))
}

/// The body of `PUT /api/settings/log-level`.
#[derive(Deserialize)]
struct LevelRequest {
    level: String,
}

/// Words a failure as `{"error_code","error"}`.
fn failure(status: StatusCode, error_code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({ "error_code": error_code, "error": message })),
    )
        .into_response()
}

/// The answer when the process has no log control.
fn unavailable() -> Response {
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "log settings are unavailable in this process",
    )
}

/// Reports the level in effect.
async fn current_level(State(application_state): State<ServerAppState>) -> Response {
    match application_state.log_control() {
        Some(log_control) => {
            Json(serde_json::json!({ "level": log_control.current_level() })).into_response()
        }
        None => unavailable(),
    }
}

/// Applies a new level.
async fn change_level(
    State(application_state): State<ServerAppState>,
    Json(request): Json<LevelRequest>,
) -> Response {
    let Some(log_control) = application_state.log_control() else {
        return unavailable();
    };
    match log_control.set_level(&request.level) {
        Ok(level) => {
            tracing::info!(level = %level, "log level changed from the web settings");
            Json(serde_json::json!({ "level": level })).into_response()
        }
        Err(LogControlError::InvalidLevel(message)) => {
            failure(StatusCode::BAD_REQUEST, "invalid_level", &message)
        }
        Err(LogControlError::Unavailable(message)) => failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "logging_unavailable",
            &message,
        ),
    }
}

/// Joins the newest week of logs and returns it as a download.
///
/// Side effects: Writes the export to a per-request temporary directory, reads it, and removes
/// the directory, on a blocking thread.
async fn export_logs(State(application_state): State<ServerAppState>) -> Response {
    let Some(log_control) = application_state.log_control() else {
        return unavailable();
    };
    let log_directory = log_control.log_directory();
    let file_name = format!(
        "note_taker_logs_{}.jsonl",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );
    let request_directory = std::env::temp_dir()
        .join("note-taker-http-log-exports")
        .join(uuid::Uuid::new_v4().to_string());
    let output_path = request_directory.join(&file_name);
    let outcome = tokio::task::spawn_blocking(move || {
        let read = export_log_files(&log_directory, &output_path)
            .and_then(|_| std::fs::read(&output_path).map_err(LogExportError::from));
        let _ = std::fs::remove_dir_all(&request_directory);
        read
    })
    .await;
    match outcome {
        Ok(Ok(file_bytes)) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/x-ndjson".to_owned()),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{file_name}\""),
                ),
            ],
            file_bytes,
        )
            .into_response(),
        Ok(Err(LogExportError::NoLogs)) => failure(
            StatusCode::NOT_FOUND,
            "no_logs",
            "there are no logs to export yet",
        ),
        Ok(Err(error)) => failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "export_failed",
            &error.to_string(),
        ),
        Err(_) => failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "export_failed",
            "the export task stopped",
        ),
    }
}
