//! Serves chat with notes (#97).
//!
//! | Route | Success |
//! | --- | --- |
//! | `GET /api/chat?scope=meeting\|project\|all&id=` | 200 `{"messages"}` |
//! | `POST /api/chat` `{"scope","id"?,"question","connection_id"?}` | 200 `{"question","answer"}` |
//! | `DELETE /api/chat?scope=&id=` | 200 `{"cleared"}` |
//!
//! Messages are as in `bin/notes_chat_commands.rs`. Failures answer `{"ok":false,"error_code",
//! "error"}`: 400 `invalid_request`, `empty`, `too_long`; 404 `meeting_not_found`,
//! `project_not_found`, `connection_not_found`; 409 `no_connection`,
//! `nothing_to_chat`, `context_too_small`; 502 `llm_*`; 503 `unavailable` without AI settings.
//! The model's answer is awaited on a blocking thread; the store's lock is held only to read
//! and to write. Questions and answers are never logged.

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use serde_json::{Value, json};

use std::sync::Arc;

use super::api::ServerAppState;
use crate::{
    domain::{chat::ChatScope, models::DeviceId},
    llm_commands::CommandError,
    notes_chat_commands::{
        ChatCommand, answer_for_paired_device, ask_paired_computer_with, ask_with, gather_for_ask,
        gather_material, offers_paired_chat, parse_chat_request, run_chat_store_command,
        store_messages,
    },
    p2p_composition::P2pHandle,
    paired_chat_host::ThreadedPairedChatHost,
};

/// Registers the routes; `build_router` merges them before its layers.
pub(super) fn routes() -> Router<ServerAppState> {
    Router::new().route(
        "/api/chat",
        get(read_history).post(ask_question).delete(clear_chat),
    )
}

/// The query naming a chat.
#[derive(Deserialize)]
struct ChatQuery {
    scope: Option<String>,
    id: Option<String>,
}

/// The status of an answer.
fn status_of(answer: &Value) -> StatusCode {
    if answer["ok"] == true {
        return StatusCode::OK;
    }
    match answer["error_code"].as_str().unwrap_or_default() {
        "invalid_request" | "empty" | "too_long" => StatusCode::BAD_REQUEST,
        "meeting_not_found" | "project_not_found" | "connection_not_found" => StatusCode::NOT_FOUND,
        "no_connection" | "nothing_to_chat" | "context_too_small" => StatusCode::CONFLICT,
        code if code.starts_with("llm_") => StatusCode::BAD_GATEWAY,
        "unavailable"
        | "p2p_unavailable"
        | "no_paired_computer"
        | "paired_computer_left"
        | "busy" => StatusCode::SERVICE_UNAVAILABLE,
        "paired_computer_timeout" => StatusCode::GATEWAY_TIMEOUT,
        "rate_limited" => StatusCode::TOO_MANY_REQUESTS,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn respond(answer: Value) -> Response {
    (status_of(&answer), Json(answer)).into_response()
}

/// Runs `history` or `clear` named by the query.
async fn store_command(state: &ServerAppState, command: &str, query: ChatQuery) -> Response {
    let request = json!({"command": command, "scope": query.scope, "id": query.id});
    let command = match parse_chat_request(&request.to_string()) {
        Ok(command) => command,
        Err(error) => return respond(error.to_json()),
    };
    let repositories = state.repositories();
    let repositories = repositories.lock().await;
    respond(run_chat_store_command(&**repositories, &command))
}

/// `GET /api/chat`.
async fn read_history(
    State(state): State<ServerAppState>,
    Query(query): Query<ChatQuery>,
) -> Response {
    tracing::info!(endpoint = "/api/chat", "headless API request received");
    store_command(&state, "history", query).await
}

/// `DELETE /api/chat`. Side effects: Deletes the chat.
async fn clear_chat(
    State(state): State<ServerAppState>,
    Query(query): Query<ChatQuery>,
) -> Response {
    tracing::info!(endpoint = "/api/chat", "headless API request received");
    store_command(&state, "clear", query).await
}

/// `POST /api/chat`. Side effects: Sends the question and the material that fits to the
/// connection's server, and stores the question and the answer.
async fn ask_question(State(state): State<ServerAppState>, body: Option<Json<Value>>) -> Response {
    tracing::info!(endpoint = "/api/chat", "headless API request received");
    let mut request = match body {
        Some(Json(Value::Object(fields))) => fields,
        _ => serde_json::Map::new(),
    };
    request.insert(String::from("command"), Value::from("ask"));
    let (scope, question, connection_id) =
        match parse_chat_request(&Value::Object(request).to_string()) {
            Ok(ChatCommand::Ask {
                scope,
                question,
                on_paired_computer: true,
                ..
            }) => return respond(ask_on_paired_computer(state, scope, question).await),
            Ok(ChatCommand::Ask {
                scope,
                question,
                connection_id,
                on_paired_computer: false,
            }) => (scope, question, connection_id),
            Ok(_) => return respond(json!({"ok": false, "error_code": "invalid_request"})),
            Err(error) => return respond(error.to_json()),
        };
    let Some(services) = state.llm() else {
        return respond(json!({"ok": false, "error_code": "unavailable",
                              "error": "AI connections are unavailable in this process"}));
    };
    let answer = tokio::task::spawn_blocking(move || {
        let files = state.project_files();
        ask_with(
            &services,
            connection_id.as_deref(),
            &question,
            || {
                let repositories = state.repositories();
                let repositories = repositories.blocking_lock();
                let voices = state.voice_database();
                let voices = voices.blocking_lock();
                gather_for_ask(
                    &**repositories,
                    &**voices,
                    files.as_deref(),
                    &scope,
                    &question,
                )
            },
            |messages| {
                let repositories = state.repositories();
                let repositories = repositories.blocking_lock();
                store_messages(&**repositories, &scope, messages)
            },
        )
    })
    .await
    .unwrap_or_else(|_| json!({"ok": false, "error_code": "failed", "error": "the chat failed"}));
    respond(answer)
}

/// Asks a paired computer and keeps its answer in this device's chat (#107).
///
/// Outputs: The JSON answer, or `p2p_unavailable` without a P2P task. Side effects: Sends the
/// question and the chat so far, sealed, to the paired computers; stores both messages. Waits on
/// a blocking thread for minutes at most.
async fn ask_on_paired_computer(
    state: ServerAppState,
    scope: ChatScope,
    question: String,
) -> Value {
    let Some(p2p) = state.p2p_control() else {
        return json!({"ok": false, "error_code": "p2p_unavailable",
                      "error": "Paired devices are off on this device."});
    };
    tokio::task::spawn_blocking(move || {
        ask_paired_computer_with(
            &question,
            || {
                let repositories = state.repositories();
                let repositories = repositories.blocking_lock();
                repositories
                    .chat_history(&scope)
                    .map_err(|error| CommandError::new("storage_failed", error.to_string()))
            },
            |question, turns| p2p.ask_paired_computer(scope.clone(), question, turns),
            |messages| {
                let repositories = state.repositories();
                let repositories = repositories.blocking_lock();
                store_messages(&**repositories, &scope, messages)
            },
        )
    })
    .await
    .unwrap_or_else(|_| json!({"ok": false, "error_code": "failed", "error": "the chat failed"}))
}

/// Answers paired devices' chat questions from the daemon's notes when the owner chose a
/// connection for them (#107).
///
/// Inputs: The state (with AI services) and the P2P task. Side effects: Starts the answering
/// thread and attaches it to the task; nothing without AI services.
pub fn answer_paired_chats(state: &ServerAppState, p2p: &P2pHandle) {
    let Some(services) = state.llm() else {
        return;
    };
    let offering = Arc::clone(&services);
    let state = state.clone();
    let host = ThreadedPairedChatHost::spawn(
        move || offers_paired_chat(&offering),
        move |_peer: &DeviceId, request| {
            answer_for_paired_device(&services, &request, |scope, question| {
                let files = state.project_files();
                let repositories = state.repositories();
                let repositories = repositories.blocking_lock();
                let voices = state.voice_database();
                let voices = voices.blocking_lock();
                gather_material(
                    &**repositories,
                    &**voices,
                    files.as_deref(),
                    scope,
                    question,
                )
            })
        },
    );
    match host {
        Ok(host) => p2p.answer_paired_chats_with(Box::new(host)),
        Err(error) => tracing::warn!(error = %error, "paired devices' chats cannot be answered"),
    }
}
