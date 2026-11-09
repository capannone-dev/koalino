//! Serves the owner's edits to a meeting (#96): its notes, its summary's text and single
//! transcript segments.
//!
//! | Route | Success |
//! | --- | --- |
//! | `GET /api/meetings/{id}/notes` | 200 `{"notes": text or null}` |
//! | `PUT /api/meetings/{id}/notes` `{"text"}` | 200 `{"notes"}` |
//! | `PUT /api/meetings/{id}/summary` `{"text"}` (registered by `llm_routes`) | 200 `{"summary"}`, marked `edited` |
//! | `PUT /api/meetings/{id}/segments` `{"start_time_ms","end_time_ms","text"}` | 200 `{"segment": {...} or null}` (blank text removes it) |
//!
//! Failures answer `{"ok":false,"error_code","error"}`: 400 `invalid_request`, `empty` or
//! `too_long`; 404 `meeting_not_found` or `segment_not_found`; 500 `storage_failed`. Every edit
//! goes into the meeting document and is announced to paired devices; a segment edit also flags
//! the transcript as edited, so the after-meeting pass no longer replaces it. Note text is never
//! logged.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, put},
};
use serde_json::{Value, json};

use super::api::ServerAppState;
use crate::{
    application::meeting_edits::{EditError, edit_segment, edit_summary, load_notes, set_notes},
    domain::models::MeetingId,
    llm_commands::summary_json,
};

/// Registers the routes; `build_router` merges them before its layers.
pub(super) fn routes() -> Router<ServerAppState> {
    Router::new()
        .route(
            "/api/meetings/{meeting_id}/notes",
            get(read_notes).put(replace_notes),
        )
        .route("/api/meetings/{meeting_id}/segments", put(replace_segment))
}

/// Answers with `status` and `body`.
fn answer(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

/// Words a refusal with its status.
fn failure(status: StatusCode, error_code: &str, error: &str) -> Response {
    answer(
        status,
        json!({"ok": false, "error_code": error_code, "error": error}),
    )
}

/// Maps an edit failure to its status and code.
fn edit_failure(error: &EditError) -> Response {
    let (status, code) = match error {
        EditError::MeetingNotFound => (StatusCode::NOT_FOUND, "meeting_not_found"),
        EditError::SegmentNotFound => (StatusCode::NOT_FOUND, "segment_not_found"),
        EditError::Empty => (StatusCode::BAD_REQUEST, "empty"),
        EditError::TooLong { .. } => (StatusCode::BAD_REQUEST, "too_long"),
        EditError::Storage(_) => (StatusCode::INTERNAL_SERVER_ERROR, "storage_failed"),
    };
    let message = match error {
        EditError::Storage(_) => String::from("the meeting could not be saved"),
        other => other.to_string(),
    };
    failure(status, code, &message)
}

/// Reads the `text` field of a JSON body.
fn text_of(body: Option<&Value>) -> Option<String> {
    body.and_then(|body| body.get("text"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// The refusal for a body without `text`.
fn missing_text() -> Response {
    failure(
        StatusCode::BAD_REQUEST,
        "invalid_request",
        "`text` is required",
    )
}

/// `GET /api/meetings/{meeting_id}/notes`.
///
/// Side effects: Reads the meeting document.
async fn read_notes(
    State(state): State<ServerAppState>,
    Path(meeting_id): Path<String>,
) -> Response {
    tracing::info!(
        endpoint = "/api/meetings/{meeting_id}/notes",
        "headless API request received"
    );
    let meeting_id = MeetingId::new(meeting_id);
    let repositories = state.repositories();
    let repositories = repositories.lock().await;
    match load_notes(&**repositories, &meeting_id) {
        Ok(notes) => answer(StatusCode::OK, json!({"ok": true, "notes": notes})),
        Err(error) => edit_failure(&error),
    }
}

/// `PUT /api/meetings/{meeting_id}/notes`.
///
/// Side effects: Writes the meeting document and tells paired devices.
async fn replace_notes(
    State(state): State<ServerAppState>,
    Path(meeting_id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    tracing::info!(
        endpoint = "/api/meetings/{meeting_id}/notes",
        "headless API request received"
    );
    let meeting_id = MeetingId::new(meeting_id);
    let Some(text) = text_of(body.as_ref().map(|Json(body)| body)) else {
        return missing_text();
    };
    let result = {
        let repositories = state.repositories();
        let repositories = repositories.lock().await;
        set_notes(&**repositories, &meeting_id, &text)
    };
    match result {
        Ok(()) => {
            state.notify_local_change(&meeting_id);
            answer(StatusCode::OK, json!({"ok": true, "notes": text}))
        }
        Err(error) => edit_failure(&error),
    }
}

/// `PUT /api/meetings/{meeting_id}/summary`: replaces the summary's text with the owner's.
///
/// Side effects: Writes the meeting document and tells paired devices.
pub(super) async fn replace_summary_text(
    State(state): State<ServerAppState>,
    Path(meeting_id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    tracing::info!(
        endpoint = "/api/meetings/{meeting_id}/summary",
        "headless API request received"
    );
    let meeting_id = MeetingId::new(meeting_id);
    let Some(text) = text_of(body.as_ref().map(|Json(body)| body)) else {
        return missing_text();
    };
    let result = {
        let repositories = state.repositories();
        let repositories = repositories.lock().await;
        edit_summary(&**repositories, &meeting_id, &text, chrono::Utc::now())
    };
    match result {
        Ok(summary) => {
            state.notify_local_change(&meeting_id);
            answer(
                StatusCode::OK,
                json!({"ok": true, "summary": summary_json(&summary)}),
            )
        }
        Err(error) => edit_failure(&error),
    }
}

/// `PUT /api/meetings/{meeting_id}/segments`.
///
/// Side effects: Flags the transcript as edited, rewrites its segments, and tells paired devices.
async fn replace_segment(
    State(state): State<ServerAppState>,
    Path(meeting_id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    tracing::info!(
        endpoint = "/api/meetings/{meeting_id}/segments",
        "headless API request received"
    );
    let meeting_id = MeetingId::new(meeting_id);
    let body = body.map(|Json(body)| body);
    let field = |name: &str| {
        body.as_ref()
            .and_then(|body| body.get(name))
            .and_then(Value::as_u64)
    };
    let (Some(start), Some(end), Some(text)) = (
        field("start_time_ms"),
        field("end_time_ms"),
        text_of(body.as_ref()),
    ) else {
        return failure(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "`start_time_ms`, `end_time_ms` and `text` are required",
        );
    };
    let result = {
        let repositories = state.repositories();
        let repositories = repositories.lock().await;
        edit_segment(&**repositories, &meeting_id, start, end, &text)
    };
    match result {
        Ok(segment) => {
            state.notify_local_change(&meeting_id);
            let segment = segment.map(|segment| {
                json!({
                    "start_time_ms": segment.start_time_ms(),
                    "end_time_ms": segment.end_time_ms(),
                    "text": segment.text(),
                })
            });
            answer(StatusCode::OK, json!({"ok": true, "segment": segment}))
        }
        Err(error) => edit_failure(&error),
    }
}
