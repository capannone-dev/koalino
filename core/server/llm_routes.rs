//! Serves the web app's AI connections (Settings) and meeting summaries (#95).
//!
//! | Route | Success |
//! | --- | --- |
//! | `GET /api/settings/llm` | 200 the settings (see `llm_commands`) |
//! | `PUT /api/settings/llm` `{"default_connection"?,"template"?,"auto_summarize"?}` | 200 the settings |
//! | `POST /api/settings/llm/connections` | 200 `{"connection"}`, created |
//! | `PUT /api/settings/llm/connections/{id}` | 200 `{"connection"}`, edited |
//! | `DELETE /api/settings/llm/connections/{id}` | 200 `{"deleted":true}` |
//! | `POST /api/settings/llm/connections/{id}/test` | 200 `{"reply"}` |
//! | `GET /api/meetings/{id}/summary` | 200 `{"summary": {...} or null, "meeting_prompt": text or null}` |
//! | `POST /api/meetings/{id}/summary` `{"connection_id"?}` | 200 `{"summary","parts","images_sent","images_skipped"}` |
//! | `PUT /api/meetings/{id}/summary-prompt` `{"prompt": text or null}` | 200 `{"meeting_prompt"}` |
//!
//! The app keeps no API keys; connections are servers the owner runs, or Claude Code and Codex.
//! Failures answer `{"ok":false,"error_code","error"}`: 400 for invalid requests, connections and
//! templates, 404 for an unknown connection or meeting, 409 when no connection is set up or there
//! is nothing to summarize, 422 when the meeting is too long, 502 when the model failed, 500 for
//! storage failures, and 503 when the process has no AI settings. Model calls
//! run on a blocking thread, and the repository is not held while the model works.
//!
//! `start_auto_summaries` runs the daemon's automatic summaries (#101) over the same state.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use serde_json::{Value, json};

use super::api::ServerAppState;
use crate::{
    application::{combined_meetings::HeldSpeech, meeting_summary::SummaryError},
    auto_summaries::{AutoSummarizer, AutoSummaryHost, AutoSummaryWake},
    domain::{llm::MeetingSummary, models::MeetingId},
    llm_commands::{LlmServices, run_llm_command, summarize_with, summary_json},
    meeting_summaries::PreparedSummary,
    meeting_summaries::{load_summary, prepare_summary, set_meeting_prompt, store_summary},
    ports::storage::MeetingRecording,
};

/// Registers the routes; `build_router` merges them before its layers.
pub(super) fn routes() -> Router<ServerAppState> {
    Router::new()
        .route("/api/settings/llm", get(read_settings).put(update_settings))
        .route("/api/settings/llm/connections", post(create_connection))
        .route(
            "/api/settings/llm/connections/{id}",
            put(edit_connection).delete(delete_connection),
        )
        .route(
            "/api/settings/llm/connections/{id}/test",
            post(test_connection),
        )
        .route(
            "/api/meetings/{meeting_id}/summary",
            get(read_summary)
                .post(write_summary)
                .put(super::edit_routes::replace_summary_text),
        )
        .route(
            "/api/meetings/{meeting_id}/summary-prompt",
            put(replace_meeting_prompt),
        )
}

/// Chooses the HTTP status for a command answer.
fn status_of(answer: &Value) -> StatusCode {
    if answer["ok"] == true {
        return StatusCode::OK;
    }
    match answer["error_code"].as_str().unwrap_or_default() {
        "invalid_request" | "invalid_connection" | "invalid_template" => StatusCode::BAD_REQUEST,
        "connection_not_found" | "meeting_not_found" => StatusCode::NOT_FOUND,
        "no_connection" | "nothing_to_summarize" => StatusCode::CONFLICT,
        "too_long" => StatusCode::UNPROCESSABLE_ENTITY,
        code if code.starts_with("llm_") => StatusCode::BAD_GATEWAY,
        "unavailable" => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Sends a command answer with its status.
fn respond(answer: Value) -> Response {
    (status_of(&answer), Json(answer)).into_response()
}

/// Words a failure.
fn failure(error_code: &str, error: &str) -> Response {
    respond(json!({"ok": false, "error_code": error_code, "error": error}))
}

/// The answer when the process has no AI settings.
fn unavailable() -> Response {
    failure(
        "unavailable",
        "AI connections are unavailable in this process",
    )
}

/// Runs one settings command on a blocking thread, since testing a connection waits on the
/// network. Side effects: As the command says.
async fn command(application_state: ServerAppState, request: Value) -> Response {
    let Some(services) = application_state.llm() else {
        return unavailable();
    };
    let answer =
        tokio::task::spawn_blocking(move || run_llm_command(&services, &request.to_string()))
            .await
            .ok()
            .and_then(|answer| serde_json::from_str::<Value>(&answer).ok())
            .unwrap_or_else(
                || json!({"ok": false, "error_code": "failed", "error": "the command failed"}),
            );
    respond(answer)
}

/// Merges route values into a JSON body; a body that is not an object counts as empty.
fn with_fields(body: Option<Json<Value>>, fields: &[(&str, Value)]) -> Value {
    let mut request = match body {
        Some(Json(Value::Object(map))) => Value::Object(map),
        _ => json!({}),
    };
    for (name, value) in fields {
        request[*name] = value.clone();
    }
    request
}

/// `GET /api/settings/llm`. Side effects: Reads the settings.
async fn read_settings(State(state): State<ServerAppState>) -> Response {
    tracing::info!(
        endpoint = "/api/settings/llm",
        "headless API request received"
    );
    command(state, json!({"command": "settings"})).await
}

/// `PUT /api/settings/llm`. Side effects: Rewrites the settings file.
async fn update_settings(
    State(state): State<ServerAppState>,
    body: Option<Json<Value>>,
) -> Response {
    tracing::info!(
        endpoint = "/api/settings/llm",
        "headless API request received"
    );
    command(state, with_fields(body, &[("command", json!("update"))])).await
}

/// `POST /api/settings/llm/connections`. Side effects: Saves the connection.
async fn create_connection(
    State(state): State<ServerAppState>,
    body: Option<Json<Value>>,
) -> Response {
    tracing::info!(
        endpoint = "/api/settings/llm/connections",
        "headless API request received"
    );
    let mut request = with_fields(body, &[("command", json!("save_connection"))]);
    if let Some(map) = request.as_object_mut() {
        map.remove("id");
    }
    command(state, request).await
}

/// `PUT /api/settings/llm/connections/{id}`. Side effects: Saves the connection.
async fn edit_connection(
    State(state): State<ServerAppState>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    tracing::info!(
        endpoint = "/api/settings/llm/connections/{id}",
        "headless API request received"
    );
    command(
        state,
        with_fields(
            body,
            &[("command", json!("save_connection")), ("id", json!(id))],
        ),
    )
    .await
}

/// `DELETE /api/settings/llm/connections/{id}`. Side effects: Removes it.
async fn delete_connection(
    State(state): State<ServerAppState>,
    Path(id): Path<String>,
) -> Response {
    tracing::info!(
        endpoint = "/api/settings/llm/connections/{id}",
        "headless API request received"
    );
    command(state, json!({"command": "delete_connection", "id": id})).await
}

/// `POST /api/settings/llm/connections/{id}/test`. Side effects: One request to the server.
async fn test_connection(State(state): State<ServerAppState>, Path(id): Path<String>) -> Response {
    tracing::info!(
        endpoint = "/api/settings/llm/connections/{id}/test",
        "headless API request received"
    );
    command(state, json!({"command": "test_connection", "id": id})).await
}

/// Answers 404 for an unknown meeting, or 500 when it cannot be read; `None` when it exists.
async fn refuse_missing_meeting(
    state: &ServerAppState,
    meeting_id: &MeetingId,
) -> Option<Response> {
    let repositories = state.repositories();
    let repositories = repositories.lock().await;
    match repositories.load_meeting_metadata(meeting_id) {
        Ok(Some(_)) => None,
        Ok(None) => Some(no_meeting()),
        Err(_) => Some(failure("storage_failed", "the meeting could not be read")),
    }
}

/// The answer for an unknown meeting.
fn no_meeting() -> Response {
    failure("meeting_not_found", "there is no such meeting")
}

/// `GET /api/meetings/{meeting_id}/summary`. Side effects: Reads the meeting document.
async fn read_summary(
    State(state): State<ServerAppState>,
    Path(meeting_id): Path<String>,
) -> Response {
    tracing::info!(
        endpoint = "/api/meetings/{meeting_id}/summary",
        "headless API request received"
    );
    let meeting_id = MeetingId::new(meeting_id);
    if let Some(refusal) = refuse_missing_meeting(&state, &meeting_id).await {
        return refusal;
    }
    let repositories = state.repositories();
    let repositories = repositories.lock().await;
    let summary = load_summary(&**repositories, &meeting_id);
    let prompt = repositories
        .load_crdt_document(&meeting_id)
        .ok()
        .and_then(|document| {
            crate::application::meeting_summary_document::read_summary_prompt(&document)
                .ok()
                .flatten()
        });
    match summary {
        Ok(summary) => respond(json!({
            "ok": true,
            "summary": summary.as_ref().map(summary_json),
            "meeting_prompt": prompt,
        })),
        Err(_) => failure("storage_failed", "the summary could not be read"),
    }
}

/// `POST /api/meetings/{meeting_id}/summary`.
///
/// Side effects: Sends the meeting's transcript, notes and photos to the chosen connection's
/// server, stores the summary in the meeting document and tells paired devices.
async fn write_summary(
    State(state): State<ServerAppState>,
    Path(meeting_id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    tracing::info!(
        endpoint = "/api/meetings/{meeting_id}/summary",
        "headless API request received"
    );
    let Some(services) = state.llm() else {
        return unavailable();
    };
    let meeting_id = MeetingId::new(meeting_id);
    if let Some(refusal) = refuse_missing_meeting(&state, &meeting_id).await {
        return refusal;
    }
    let connection_id = body.and_then(|Json(body)| {
        body.get("connection_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
    });
    let worker_state = state.clone();
    let worker_meeting = meeting_id.clone();
    let answer = tokio::task::spawn_blocking(move || {
        summarize_in_background(
            &services,
            &worker_state,
            &worker_meeting,
            connection_id.as_deref(),
        )
    })
    .await
    .unwrap_or_else(
        |_| json!({"ok": false, "error_code": "failed", "error": "the summary failed"}),
    );
    if answer["ok"] == true {
        state.notify_local_change(&meeting_id);
    }
    respond(answer)
}

/// Prepares, runs and stores a summary, taking the repository lock only to read and to write.
///
/// Side effects: As `write_summary`. Blocks: runs on a blocking thread.
fn summarize_in_background(
    services: &LlmServices,
    state: &ServerAppState,
    meeting_id: &MeetingId,
    connection_id: Option<&str>,
) -> Value {
    summarize_with(
        services,
        connection_id,
        |glossary, device_name| {
            let repositories = state.repositories();
            let repositories = repositories.blocking_lock();
            let voices = state.voice_database();
            let voices = voices.blocking_lock();
            prepare_summary(
                &**repositories,
                &**voices,
                meeting_id,
                glossary,
                device_name,
            )
        },
        |summary| {
            let repositories = state.repositories();
            let repositories = repositories.blocking_lock();
            store_summary(&**repositories, meeting_id, summary)
        },
    )
}

/// `PUT /api/meetings/{meeting_id}/summary-prompt`.
///
/// Side effects: Writes the meeting document and tells paired devices.
async fn replace_meeting_prompt(
    State(state): State<ServerAppState>,
    Path(meeting_id): Path<String>,
    body: Option<Json<Value>>,
) -> Response {
    tracing::info!(
        endpoint = "/api/meetings/{meeting_id}/summary-prompt",
        "headless API request received"
    );
    let meeting_id = MeetingId::new(meeting_id);
    if let Some(refusal) = refuse_missing_meeting(&state, &meeting_id).await {
        return refusal;
    }
    let prompt = body.and_then(|Json(body)| {
        body.get("prompt")
            .and_then(Value::as_str)
            .map(str::to_owned)
    });
    let result = {
        let repositories = state.repositories();
        let repositories = repositories.lock().await;
        set_meeting_prompt(&**repositories, &meeting_id, prompt.as_deref())
    };
    match result {
        Ok(()) => {
            state.notify_local_change(&meeting_id);
            respond(json!({"ok": true, "meeting_prompt": prompt}))
        }
        Err(crate::meeting_summaries::MeetingPromptError::Template(error)) => {
            failure("invalid_template", &error.to_string())
        }
        Err(error) => failure("storage_failed", &error.to_string()),
    }
}

/// Reads and writes meetings for the daemon's automatic summaries (#101), taking each lock only
/// for one read or write.
struct ServerAutoSummaryHost {
    state: ServerAppState,
}

impl AutoSummaryHost for ServerAutoSummaryHost {
    fn recordings(&self) -> Result<Vec<MeetingRecording>, String> {
        let repositories = self.state.repositories();
        let repositories = repositories.blocking_lock();
        repositories
            .meeting_recordings()
            .map_err(|error| error.to_string())
    }

    fn has_summary(&self, meeting_id: &MeetingId) -> Result<bool, String> {
        let repositories = self.state.repositories();
        let repositories = repositories.blocking_lock();
        load_summary(&**repositories, meeting_id)
            .map(|summary| summary.is_some())
            .map_err(|error| error.to_string())
    }

    fn is_recording(&self, meeting_id: &MeetingId) -> bool {
        self.state.is_recording(meeting_id)
    }

    fn held_speech(&self, meeting_id: &MeetingId) -> Option<HeldSpeech> {
        self.state
            .held_speech(meeting_id)
            .and_then(|(_, held)| held)
    }

    fn prepare(
        &self,
        meeting_id: &MeetingId,
        glossary: &[String],
        device_name: &str,
    ) -> Result<PreparedSummary, SummaryError> {
        let repositories = self.state.repositories();
        let repositories = repositories.blocking_lock();
        let voices = self.state.voice_database();
        let voices = voices.blocking_lock();
        prepare_summary(
            &**repositories,
            &**voices,
            meeting_id,
            glossary,
            device_name,
        )
    }

    fn store(&self, meeting_id: &MeetingId, summary: &MeetingSummary) -> Result<(), SummaryError> {
        let repositories = self.state.repositories();
        let repositories = repositories.blocking_lock();
        store_summary(&**repositories, meeting_id, summary)
    }
}

/// Starts the daemon's automatic summaries (#101) on their own thread.
///
/// Inputs: The state, with AI services and this device attached. Outputs: The wake-up to signal
/// when a recording ends, or `None` when the state has no AI services or device, or the thread
/// could not start. Side effects: Spawns a thread that summarizes due meetings with the default
/// connection while the owner keeps automatic summaries on.
pub fn start_auto_summaries(state: &ServerAppState) -> Option<Arc<AutoSummaryWake>> {
    let services = state.llm()?;
    let (device_id, _) = state.held_speech(&MeetingId::new(""))?;
    let wake = Arc::new(AutoSummaryWake::default());
    let host = ServerAutoSummaryHost {
        state: state.clone(),
    };
    match AutoSummarizer::new(services, device_id).spawn(Box::new(host), Arc::clone(&wake)) {
        Ok(_) => Some(wake),
        Err(error) => {
            tracing::warn!(error = %error, "automatic summaries could not start");
            None
        }
    }
}
