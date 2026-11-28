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
    http::{HeaderMap, StatusCode, header},
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

/// One byte range of a body, both ends included.
#[derive(Debug, PartialEq, Eq)]
enum ByteRange {
    /// From `start` to `end`.
    Part { start: usize, end: usize },
    /// Starts past the end of the body.
    Unsatisfiable,
    /// No range, an unreadable one, or several: the whole body.
    Whole,
}

/// Reads a `Range` header against a body of `length` bytes.
///
/// Inputs: The header's value, if any. Outputs: One range (`bytes=a-b`, `bytes=a-` or the suffix
/// `bytes=-n`), `Unsatisfiable`, or `Whole`. Side effects: None.
fn byte_range(header_value: Option<&str>, length: usize) -> ByteRange {
    let Some(spec) = header_value.and_then(|value| value.trim().strip_prefix("bytes=")) else {
        return ByteRange::Whole;
    };
    if spec.contains(',') || length == 0 {
        return ByteRange::Whole;
    }
    let Some((first, last)) = spec.split_once('-') else {
        return ByteRange::Whole;
    };
    let (first, last) = (first.trim(), last.trim());
    let parse = |text: &str| text.parse::<usize>().ok();
    let (start, end) = match (first.is_empty(), last.is_empty()) {
        (true, false) => match parse(last) {
            Some(0) | None => return ByteRange::Whole,
            Some(suffix) => (length.saturating_sub(suffix), length - 1),
        },
        (false, true) => match parse(first) {
            Some(start) => (start, length - 1),
            None => return ByteRange::Whole,
        },
        (false, false) => match (parse(first), parse(last)) {
            (Some(start), Some(end)) if start <= end => (start, end.min(length - 1)),
            _ => return ByteRange::Whole,
        },
        (true, true) => return ByteRange::Whole,
    };
    if start >= length {
        ByteRange::Unsatisfiable
    } else {
        ByteRange::Part { start, end }
    }
}

/// Answers with the WAV, or the part `range` asks for.
///
/// Outputs: 200 with the whole file, 206 with a part, or 416 for a range past the end; each says
/// ranges are accepted. Side effects: None.
fn wav_response(wav: Vec<u8>, range: Option<&header::HeaderValue>) -> Response {
    let length = wav.len();
    let common = [
        (header::CONTENT_TYPE, "audio/wav"),
        (header::CACHE_CONTROL, "no-store"),
        (header::ACCEPT_RANGES, "bytes"),
    ];
    match byte_range(range.and_then(|value| value.to_str().ok()), length) {
        ByteRange::Whole => (common, Body::from(wav)).into_response(),
        ByteRange::Part { start, end } => (
            StatusCode::PARTIAL_CONTENT,
            common,
            [(
                header::CONTENT_RANGE,
                format!("bytes {start}-{end}/{length}"),
            )],
            Body::from(wav[start..=end].to_vec()),
        )
            .into_response(),
        ByteRange::Unsatisfiable => (
            StatusCode::RANGE_NOT_SATISFIABLE,
            [
                (header::ACCEPT_RANGES, "bytes".to_owned()),
                (header::CONTENT_RANGE, format!("bytes */{length}")),
            ],
        )
            .into_response(),
    }
}

/// Plays a meeting's kept recording, or one folded into it (#88), as WAV.
///
/// A request for one byte range (`Range: bytes=…`) gets that part (206), so a browser can move
/// anywhere in a long recording (#197); several ranges get the whole file.
/// Side effects: Decrypts and decodes the recording in memory on a blocking thread.
async fn recording(
    State(state): State<ServerAppState>,
    Path(meeting_id): Path<String>,
    headers: HeaderMap,
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
        Ok(Ok(wav)) => wav_response(wav, headers.get(header::RANGE)),
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
