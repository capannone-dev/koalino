//! Serves the web Settings page's glossary: the names and terms transcription is prompted with.
//!
//! | Route | Success |
//! | --- | --- |
//! | `GET /api/settings/glossary` | 200 `{"terms"}`, in order |
//! | `PUT /api/settings/glossary` `{"terms"}` | 200 `{"terms"}` as saved: trimmed, blanks dropped, repeats kept once |
//!
//! The glossary lives in the transcription settings file beside the database, which the apps,
//! the CLI and this daemon all read; it applies from the next recording. Failures answer
//! `{"error_code","error"}`: 400 `invalid_glossary` (a term over 60 characters, a control
//! character, or over 500 characters in all), 500 `save_failed`, and 503 `unavailable` when the
//! process has no settings file. The terms are user content and are never logged.

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
    domain::transcription_glossary::TranscriptionGlossary,
    transcription_settings::{load_settings, save_glossary},
};

/// Registers the routes; `build_router` merges them before its layers.
pub(super) fn routes() -> Router<ServerAppState> {
    Router::new().route(
        "/api/settings/glossary",
        get(current_glossary).put(replace_glossary),
    )
}

/// The body of `PUT /api/settings/glossary`.
#[derive(Deserialize)]
struct GlossaryRequest {
    terms: Vec<String>,
}

/// Words a failure as `{"error_code","error"}`.
fn failure(status: StatusCode, error_code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({ "error_code": error_code, "error": message })),
    )
        .into_response()
}

/// The answer when the process has no settings file.
fn unavailable() -> Response {
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "the glossary is unavailable in this process",
    )
}

/// Reports the saved terms.
///
/// Side effects: Reads the settings file.
async fn current_glossary(State(application_state): State<ServerAppState>) -> Response {
    tracing::info!(
        endpoint = "/api/settings/glossary",
        "headless API request received"
    );
    match application_state.settings_file() {
        Some(settings_file) => {
            Json(serde_json::json!({ "terms": load_settings(&settings_file).glossary }))
                .into_response()
        }
        None => unavailable(),
    }
}

/// Replaces the saved terms.
///
/// Side effects: Rewrites the settings file, keeping its other choices.
async fn replace_glossary(
    State(application_state): State<ServerAppState>,
    Json(request): Json<GlossaryRequest>,
) -> Response {
    tracing::info!(
        endpoint = "/api/settings/glossary",
        "headless API request received"
    );
    let Some(settings_file) = application_state.settings_file() else {
        return unavailable();
    };
    let glossary = match TranscriptionGlossary::parse(request.terms.iter().map(String::as_str)) {
        Ok(glossary) => glossary,
        Err(error) => {
            return failure(
                StatusCode::BAD_REQUEST,
                "invalid_glossary",
                &error.to_string(),
            );
        }
    };
    match save_glossary(&settings_file, &glossary) {
        Ok(()) => Json(serde_json::json!({ "terms": glossary.terms() })).into_response(),
        Err(error) => {
            tracing::warn!(error = %error, "the glossary could not be saved");
            failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "save_failed",
                "the glossary could not be saved",
            )
        }
    }
}
