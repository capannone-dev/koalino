//! Serves the web Settings page's switch for keeping online-meeting participants' voices (#103).
//!
//! | Route | Success |
//! | --- | --- |
//! | `GET /api/settings/meeting-voices` | 200 `{"keep"}` |
//! | `PUT /api/settings/meeting-voices` `{"keep"}` | 200 the same, as saved |
//!
//! When on, a participant whose track carried one voice for long enough gets that voice in the
//! gallery, so the person is recognised in later meetings, in the room too. Voice prints are
//! biometric, so it is off until the owner turns it on. The choice lives in the transcription
//! settings file beside the database and is read at every offer. Failures answer
//! `{"error_code","error"}`: 500 `save_failed`, and 503 `unavailable` when the process has no
//! settings file.

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;

use super::api::ServerAppState;
use crate::transcription_settings::{keeps_meeting_voices, save_keep_meeting_voices};

/// The routes' path.
const ROUTE: &str = "/api/settings/meeting-voices";

/// Registers the routes; `build_router` merges them before its layers.
pub(super) fn routes() -> Router<ServerAppState> {
    Router::new().route(ROUTE, get(current_choice).put(replace_choice))
}

/// The body of `PUT /api/settings/meeting-voices`.
#[derive(Deserialize)]
struct MeetingVoicesRequest {
    /// Whether participants' voices may be kept.
    keep: bool,
}

/// Words a failure as `{"error_code","error"}`.
fn failure(status: StatusCode, error_code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({ "error_code": error_code, "error": message })),
    )
        .into_response()
}

/// Reports the choice.
///
/// Outputs: 200 `{"keep"}`, or 503 without a settings file. Side effects: Reads the file.
async fn current_choice(State(application_state): State<ServerAppState>) -> Response {
    tracing::info!(endpoint = ROUTE, "headless API request received");
    match application_state.settings_file() {
        Some(settings_file) => {
            Json(serde_json::json!({ "keep": keeps_meeting_voices(&settings_file) }))
                .into_response()
        }
        None => unavailable(),
    }
}

/// Changes the choice.
///
/// Inputs: `{"keep"}`. Outputs: 200 with the saved choice, 500 `save_failed`, or 503.
/// Side effects: Rewrites the settings file, keeping its other choices.
async fn replace_choice(
    State(application_state): State<ServerAppState>,
    Json(request): Json<MeetingVoicesRequest>,
) -> Response {
    tracing::info!(
        endpoint = ROUTE,
        keep = request.keep,
        "headless API request received"
    );
    let Some(settings_file) = application_state.settings_file() else {
        return unavailable();
    };
    match save_keep_meeting_voices(&settings_file, request.keep) {
        Ok(()) => Json(serde_json::json!({ "keep": request.keep })).into_response(),
        Err(error) => {
            tracing::warn!(error = %error, "the meeting voices choice could not be saved");
            failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "save_failed",
                "the choice could not be saved",
            )
        }
    }
}

/// The answer when the process has no settings file.
fn unavailable() -> Response {
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "the meeting voices choice is unavailable in this process",
    )
}
