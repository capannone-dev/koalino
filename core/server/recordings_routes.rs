//! Serves the owner's kept meeting recordings (#140) to the web UI.
//!
//! | Route | Success |
//! | --- | --- |
//! | `GET /api/settings/recordings` | 200 `{"keep","count","bytes"}` |
//! | `PUT /api/settings/recordings` `{"keep"}` | 200 the same, as saved |
//! | `DELETE /api/recordings` | 200 `{"deleted","bytes"}`: every kept recording is wiped |
//! | `GET /api/meetings/{id}/recording` | 200 `audio/wav`, 16 kHz mono, speech at its time |
//! | `GET /api/meetings/{id}/recording/available` | 200 `{"available"}`, without decrypting |
//!
//! The switch lives in the transcription settings file beside the database, per device. The WAV
//! is built in memory from the sealed recording and never written to disk here. Failures answer
//! `{"error_code","error"}`: 404 `not_found` (no kept recording), 422 `unreadable`, 500
//! `save_failed`, and 503 `unavailable` when this process keeps no recordings.

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get},
};
use serde::Deserialize;

use super::api::ServerAppState;
use crate::{
    domain::models::MeetingId,
    kept_recordings::{PlaybackError, playback_code},
};

/// Registers the routes; `build_router` merges them before its layers.
pub(super) fn routes() -> Router<ServerAppState> {
    Router::new()
        .route("/api/settings/recordings", get(status).put(set_keep))
        .route("/api/recordings", delete(wipe))
        .route("/api/meetings/{meeting_id}/recording", get(recording))
        .route(
            "/api/meetings/{meeting_id}/recording/available",
            get(available),
        )
}

/// The body of `PUT /api/settings/recordings`.
#[derive(Deserialize)]
struct KeepRequest {
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

/// The 503 answer when this process keeps no recordings.
fn unavailable() -> Response {
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "this process keeps no recordings",
    )
}

/// Reports the switch and what is kept. Side effects: Reads the settings file and the folder.
async fn status(State(state): State<ServerAppState>) -> Response {
    tracing::info!(
        endpoint = "/api/settings/recordings",
        "headless API request received"
    );
    match state.recordings() {
        Some(services) => Json(services.status_json()).into_response(),
        None => unavailable(),
    }
}

/// Turns keeping recordings on or off. Side effects: Rewrites the settings file.
async fn set_keep(
    State(state): State<ServerAppState>,
    Json(request): Json<KeepRequest>,
) -> Response {
    tracing::info!(
        endpoint = "/api/settings/recordings",
        "headless API request received"
    );
    let Some(services) = state.recordings() else {
        return unavailable();
    };
    match services.set_keep(request.keep) {
        Ok(status) => Json(status).into_response(),
        Err(error) => {
            tracing::warn!(error = %error, "the recordings switch could not be saved");
            failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "save_failed",
                "the choice could not be saved",
            )
        }
    }
}

/// Deletes every kept recording. Side effects: Deletes files.
async fn wipe(State(state): State<ServerAppState>) -> Response {
    tracing::info!(
        endpoint = "/api/recordings",
        "headless API request received"
    );
    let Some(services) = state.recordings() else {
        return unavailable();
    };
    match tokio::task::spawn_blocking(move || services.wipe()).await {
        Ok(answer) => Json(answer).into_response(),
        Err(_) => failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "save_failed",
            "the recordings could not be wiped",
        ),
    }
}

/// Says whether a meeting has a kept recording (its own or one folded into it).
///
/// Side effects: Reads the repository and the folder; decrypts nothing.
async fn available(
    State(state): State<ServerAppState>,
    Path(meeting_id): Path<String>,
) -> Response {
    let Some(services) = state.recordings() else {
        return unavailable();
    };
    let repositories = state.repositories();
    let repositories = repositories.lock().await;
    let repository: &dyn crate::ports::storage::NoteRepository = &**repositories;
    let available = !services
        .sources(&MeetingId::new(meeting_id), Some(repository))
        .is_empty();
    Json(serde_json::json!({ "available": available })).into_response()
}

/// Plays a meeting's kept recording, or one folded into it (#88), as WAV.
///
/// Side effects: Decrypts and decodes the recording in memory on a blocking thread.
async fn recording(
    State(state): State<ServerAppState>,
    Path(meeting_id): Path<String>,
) -> Response {
    tracing::info!(
        endpoint = "/api/meetings/:meeting_id/recording",
        "headless API request received"
    );
    let Some(services) = state.recordings() else {
        return unavailable();
    };
    let repositories = state.repositories();
    let built = tokio::task::spawn_blocking(move || {
        let meeting_id = MeetingId::new(meeting_id);
        let sources = {
            let repositories = repositories.blocking_lock();
            let repository: &dyn crate::ports::storage::NoteRepository = &**repositories;
            services.sources(&meeting_id, Some(repository))
        };
        let mut wav = Vec::new();
        services.write_recording(&sources, &mut wav).map(|_| wav)
    })
    .await;
    match built {
        Ok(Ok(wav)) => (
            [
                (header::CONTENT_TYPE, "audio/wav"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            Body::from(wav),
        )
            .into_response(),
        Ok(Err(error)) => {
            let status = match error {
                PlaybackError::NotFound => StatusCode::NOT_FOUND,
                PlaybackError::Unreadable => StatusCode::UNPROCESSABLE_ENTITY,
                PlaybackError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
            };
            failure(status, playback_code(&error), &error.to_string())
        }
        Err(_) => failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "storage_failed",
            "the recording could not be read",
        ),
    }
}
