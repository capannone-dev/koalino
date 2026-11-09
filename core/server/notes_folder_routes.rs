//! Serves the notes folder's Settings (#98) and starts the daemon's rounds.
//!
//! | Route | Success |
//! | --- | --- |
//! | `GET /api/settings/notes-folder` | 200 the status |
//! | `PUT /api/settings/notes-folder` `{"enabled","path"?}` | 200 the status (turning it on writes the folder at once) |
//! | `POST /api/settings/notes-folder/sync` | 200 the status, after a round |
//!
//! The status is that of `notes_folder_commands`. Failures answer `{"ok":false,"error_code",
//! "error"}`: 400 `invalid_request` or `invalid_folder`, 500 `folder_failed` or
//! `storage_failed`, 503 `unavailable` when the process keeps no notes folder. The folder lives on
//! the daemon's machine. Rounds take the store's lock on a blocking thread and announce the
//! meetings they changed to paired devices. Nothing here logs names or text.

use std::{path::Path, sync::Arc};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::{Value, json};

use super::api::ServerAppState;
use crate::{
    auto_summaries::AutoSummaryWake,
    notes_folder::{FolderState, NotesFolderError, RoundReport, run_round},
    search_commands::speaker_names,
};

/// Registers the routes; `build_router` merges them before its layers.
pub(super) fn routes() -> Router<ServerAppState> {
    Router::new()
        .route(
            "/api/settings/notes-folder",
            get(read_status).put(replace_settings),
        )
        .route("/api/settings/notes-folder/sync", post(sync_now))
}

/// Runs one round against the daemon's store, then tells paired devices what changed.
///
/// Side effects: Those of `run_round`; blocks on the store's locks, so it runs on a blocking
/// thread.
fn daemon_round(
    state: &ServerAppState,
    root: &Path,
    folder_state: &mut FolderState,
) -> Result<RoundReport, NotesFolderError> {
    let Some(project_files) = state.project_files() else {
        return Err(NotesFolderError::Settings(String::from(
            "this process keeps no project files",
        )));
    };
    let names = {
        let voices = state.voice_database();
        let voices = voices.blocking_lock();
        speaker_names(&**voices)?
    };
    let report = {
        let repositories = state.repositories();
        let repositories = repositories.blocking_lock();
        run_round(
            &**repositories,
            &project_files,
            &names,
            root,
            folder_state,
            chrono::Utc::now(),
        )?
    };
    for meeting_id in &report.changed_meetings {
        state.notify_local_change(meeting_id);
    }
    Ok(report)
}

/// Runs a command on a blocking thread and words the answer.
async fn run_command(state: ServerAppState, request: Value) -> Response {
    let Some(keeper) = state.notes_folder() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"ok": false, "error_code": "unavailable",
                        "error": "this process keeps no notes folder"})),
        )
            .into_response();
    };
    let answer = tokio::task::spawn_blocking(move || {
        keeper.command(
            &request.to_string(),
            &mut |root: &Path, folder_state: &mut FolderState| {
                daemon_round(&state, root, folder_state)
            },
        )
    })
    .await
    .unwrap_or_else(
        |_| json!({"ok": false, "error_code": "folder_failed", "error": "the round stopped"}),
    );
    let status = match answer.get("error_code").and_then(Value::as_str) {
        None => StatusCode::OK,
        Some("invalid_request" | "invalid_folder") => StatusCode::BAD_REQUEST,
        Some(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(answer)).into_response()
}

/// `GET /api/settings/notes-folder`.
async fn read_status(State(state): State<ServerAppState>) -> Response {
    tracing::info!(
        endpoint = "/api/settings/notes-folder",
        "headless API request received"
    );
    run_command(state, json!({"command": "get"})).await
}

/// `PUT /api/settings/notes-folder`. Side effects: Saves the settings; turning the folder on
/// writes it.
async fn replace_settings(
    State(state): State<ServerAppState>,
    body: Option<Json<Value>>,
) -> Response {
    tracing::info!(
        endpoint = "/api/settings/notes-folder",
        "headless API request received"
    );
    let mut request = match body {
        Some(Json(Value::Object(fields))) => fields,
        _ => serde_json::Map::new(),
    };
    request.insert(String::from("command"), Value::from("set"));
    run_command(state, Value::Object(request)).await
}

/// `POST /api/settings/notes-folder/sync`. Side effects: Runs a round when the folder is on.
async fn sync_now(State(state): State<ServerAppState>) -> Response {
    tracing::info!(
        endpoint = "/api/settings/notes-folder/sync",
        "headless API request received"
    );
    run_command(state, json!({"command": "sync_now"})).await
}

/// Starts the daemon's rounds on their own thread.
///
/// Outputs: The wake-up, or `None` when the state keeps no notes folder or the thread could not
/// start. Side effects: Spawns the thread; while the folder is off a round only reads settings.
pub fn start_notes_folder(state: &ServerAppState) -> Option<Arc<AutoSummaryWake>> {
    let keeper = state.notes_folder()?;
    let wake = Arc::new(AutoSummaryWake::default());
    let round_state = state.clone();
    let runner = Box::new(move |root: &Path, folder_state: &mut FolderState| {
        daemon_round(&round_state, root, folder_state)
    });
    match keeper.spawn(Arc::clone(&wake), runner) {
        Ok(_) => Some(wake),
        Err(error) => {
            tracing::warn!(error = %error, "notes folder rounds could not start");
            None
        }
    }
}
