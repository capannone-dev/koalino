//! Serves the web Settings page's speech retention window (#93): how many days a recording's
//! speech may wait to be combined with another device's recording of the same meeting.
//!
//! | Route | Success |
//! | --- | --- |
//! | `GET /api/settings/retention` | 200 `{"days","min_days","max_days"}` |
//! | `PUT /api/settings/retention` `{"days"}` | 200 the same, as saved |
//!
//! The window lives in the transcription settings file beside the database, which the apps, the
//! CLI and this daemon all read; the after-meeting worker applies it at its next round, deleting
//! speech older than a shorter window. Failures answer `{"error_code","error"}`: 400
//! `invalid_window` (outside `min_days..=max_days`), 500 `save_failed`, and 503 `unavailable`
//! when the process has no settings file.

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;

use super::api::ServerAppState;
use crate::{
    application::combined_meetings::{MAX_KEEP_SPEECH_DAYS, MIN_KEEP_SPEECH_DAYS, keep_for_days},
    transcription_settings::{keep_for_in, keep_speech_days, save_keep_speech_days},
};

/// Registers the routes; `build_router` merges them before its layers.
pub(super) fn routes() -> Router<ServerAppState> {
    Router::new().route(
        "/api/settings/retention",
        get(current_window).put(replace_window),
    )
}

/// The body of `PUT /api/settings/retention`.
#[derive(Deserialize)]
struct RetentionRequest {
    /// The new window in days.
    days: u16,
}

/// Words a failure as `{"error_code","error"}`.
fn failure(status: StatusCode, error_code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({ "error_code": error_code, "error": message })),
    )
        .into_response()
}

/// Describes a window with its bounds, as both routes answer.
fn window_json(days: u16) -> Response {
    Json(serde_json::json!({
        "days": days,
        "min_days": MIN_KEEP_SPEECH_DAYS,
        "max_days": MAX_KEEP_SPEECH_DAYS,
    }))
    .into_response()
}

/// Reports the window.
///
/// Outputs: 200 with the window, or 503 without a settings file. Side effects: Reads the file.
async fn current_window(State(application_state): State<ServerAppState>) -> Response {
    tracing::info!(
        endpoint = "/api/settings/retention",
        "headless API request received"
    );
    match application_state.settings_file() {
        Some(settings_file) => window_json(keep_speech_days(keep_for_in(Some(&settings_file)))),
        None => unavailable(),
    }
}

/// Changes the window.
///
/// Inputs: `{"days"}`. Outputs: 200 with the saved window, 400 `invalid_window`, 500
/// `save_failed`, or 503. Side effects: Rewrites the settings file, keeping its other choices.
async fn replace_window(
    State(application_state): State<ServerAppState>,
    Json(request): Json<RetentionRequest>,
) -> Response {
    tracing::info!(
        endpoint = "/api/settings/retention",
        "headless API request received"
    );
    let Some(settings_file) = application_state.settings_file() else {
        return unavailable();
    };
    if keep_for_days(request.days).is_none() {
        return failure(
            StatusCode::BAD_REQUEST,
            "invalid_window",
            &format!("choose between {MIN_KEEP_SPEECH_DAYS} and {MAX_KEEP_SPEECH_DAYS} days"),
        );
    }
    match save_keep_speech_days(&settings_file, request.days) {
        Ok(()) => window_json(request.days),
        Err(error) => {
            tracing::warn!(error = %error, "the speech retention window could not be saved");
            failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "save_failed",
                "the window could not be saved",
            )
        }
    }
}

/// The answer when the process has no settings file.
fn unavailable() -> Response {
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "the retention window is unavailable in this process",
    )
}
