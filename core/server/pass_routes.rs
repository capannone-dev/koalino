//! Serves the web Settings page's after-meeting pass (#87): whether each meeting is transcribed
//! again after Stop, and with which model.
//!
//! | Route | Success |
//! | --- | --- |
//! | `GET /api/settings/after-meeting-pass` | 200 `{"enabled","model","models":[{"file_name","name"}]}` |
//! | `PUT /api/settings/after-meeting-pass` `{"enabled"?,"model"?}` | 200 the same, as saved |
//!
//! `model` is a catalog file name on this machine, or `null` for the model recordings use;
//! `models` lists the catalog models on this machine. The choices live in the transcription
//! settings file beside the database, which the apps, the CLI and this daemon read; the daemon
//! applies them when it next builds its pipeline. Failures answer `{"error_code","error"}`: 400
//! `invalid_model` (not a catalog model on this machine; nothing is saved), 500 `save_failed`,
//! and 503 `unavailable` when the process has no settings file.

use std::path::{Path, PathBuf};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::Value;

use super::api::ServerAppState;
use crate::{
    domain::whisper_catalog::WHISPER_MODELS,
    transcription_settings::{
        TranscriptionPaths, load_settings, save_settings, transcript_pass_enabled,
    },
};

/// The suffix `settings_file_for` adds to the database path.
const SETTINGS_SUFFIX: &str = ".transcription-settings.json";

/// Registers the routes; `build_router` merges them before its layers.
pub(super) fn routes() -> Router<ServerAppState> {
    Router::new().route(
        "/api/settings/after-meeting-pass",
        get(current_pass).put(replace_pass),
    )
}

/// Words a failure as `{"error_code","error"}`.
fn failure(status: StatusCode, error_code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({ "error_code": error_code, "error": message })),
    )
        .into_response()
}

/// The model folders of the database a settings file belongs to.
///
/// Inputs: The settings file. Outputs: Paths with that database, and the shipped model from this
/// process's environment. Side effects: Reads the environment.
fn paths_for(settings_file: &Path) -> TranscriptionPaths {
    let settings_text = settings_file.to_string_lossy();
    let database = settings_text
        .strip_suffix(SETTINGS_SUFFIX)
        .map_or_else(|| PathBuf::from("note_taker.sqlite3"), PathBuf::from);
    let database_text = database.to_string_lossy().into_owned();
    TranscriptionPaths::from_lookup(&|name| {
        if name == "NOTE_TAKER_DATABASE_PATH" {
            Some(database_text.clone())
        } else {
            std::env::var(name).ok()
        }
    })
}

/// Describes the saved choices and the models on this machine, as both routes answer.
///
/// Side effects: Reads the settings file and looks for model files.
fn pass_json(settings_file: &Path) -> Response {
    let paths = paths_for(settings_file);
    let saved = load_settings(settings_file);
    let models: Vec<Value> = WHISPER_MODELS
        .iter()
        .filter(|model| paths.locate_model(model.file_name()).is_some())
        .map(|model| serde_json::json!({ "file_name": model.file_name(), "name": model.name() }))
        .collect();
    Json(serde_json::json!({
        "enabled": transcript_pass_enabled(&saved),
        "model": saved
            .transcript_pass_model
            .as_deref()
            .filter(|file| paths.locate_model(file).is_some()),
        "models": models,
    }))
    .into_response()
}

/// Reports the pass's choices.
///
/// Outputs: 200, or 503 without a settings file. Side effects: Reads files.
async fn current_pass(State(application_state): State<ServerAppState>) -> Response {
    tracing::info!(
        endpoint = "/api/settings/after-meeting-pass",
        "headless API request received"
    );
    match application_state.settings_file() {
        Some(settings_file) => pass_json(&settings_file),
        None => unavailable(),
    }
}

/// Changes the pass's choices.
///
/// Inputs: `{"enabled"?: bool, "model"?: string | null}`; a missing field keeps its value.
/// Outputs: 200 with the saved choices, 400 `invalid_model` or `invalid_request`, 500
/// `save_failed`, or 503. Side effects: Rewrites the settings file, keeping its other choices.
async fn replace_pass(
    State(application_state): State<ServerAppState>,
    Json(request): Json<Value>,
) -> Response {
    tracing::info!(
        endpoint = "/api/settings/after-meeting-pass",
        "headless API request received"
    );
    let Some(settings_file) = application_state.settings_file() else {
        return unavailable();
    };
    let enabled = match request.get("enabled") {
        None => None,
        Some(Value::Bool(enabled)) => Some(*enabled),
        Some(_) => {
            return failure(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "enabled must be true or false",
            );
        }
    };
    let model = match request.get("model") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(Value::String(file_name))
            if paths_for(&settings_file).locate_model(file_name).is_some() =>
        {
            Some(Some(file_name.clone()))
        }
        Some(_) => {
            return failure(
                StatusCode::BAD_REQUEST,
                "invalid_model",
                "choose a model that is on this machine, or null",
            );
        }
    };
    let mut saved = load_settings(&settings_file);
    if let Some(enabled) = enabled {
        saved.transcript_pass = Some(enabled);
    }
    if let Some(model) = model {
        saved.transcript_pass_model = model;
    }
    match save_settings(&settings_file, &saved) {
        Ok(()) => pass_json(&settings_file),
        Err(error) => {
            tracing::warn!(error = %error, "the after-meeting pass choices could not be saved");
            failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "save_failed",
                "the choices could not be saved",
            )
        }
    }
}

/// The answer when the process has no settings file.
fn unavailable() -> Response {
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "the after-meeting pass settings are unavailable in this process",
    )
}
