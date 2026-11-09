//! Serves search (#100): `GET /api/search?q=<words>&scope=<meetings|projects|project>&project=<id>`.
//!
//! The scope defaults to `meetings`; `project` needs `project`. Success is 200 with
//! `{"ok":true,"hits":[...],"truncated":bool}` (see `search_commands` for a hit's fields).
//! Refusals are 400 (`invalid_request`, `empty`, `too_short`, `too_long`), 404
//! (`project_not_found`) or 500 (`storage_failed`). The query and the text found are never
//! logged.

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use serde_json::Value;

use super::api::ServerAppState;
use crate::search_commands::{HostSearchSources, SearchRequest, run_search, speaker_names};

/// Registers the route; `build_router` merges it before its layers.
pub(super) fn routes() -> Router<ServerAppState> {
    Router::new().route("/api/search", get(search_route))
}

/// The query string.
#[derive(Deserialize)]
struct SearchParameters {
    q: Option<String>,
    scope: Option<String>,
    project: Option<String>,
}

/// `GET /api/search`.
///
/// Side effects: Reads (and so unseals) meetings, the voice library and project files.
async fn search_route(
    State(state): State<ServerAppState>,
    Query(parameters): Query<SearchParameters>,
) -> Response {
    tracing::info!(endpoint = "/api/search", "headless API request received");
    let names = speaker_names(&**state.voice_database().lock().await).unwrap_or_default();
    let project_files = state.project_files();
    let sources = HostSearchSources::new(names, project_files.as_deref());
    let request = SearchRequest {
        query: parameters.q.unwrap_or_default(),
        scope: parameters.scope,
        project_id: parameters.project,
    };
    let answer = {
        let repositories = state.repositories();
        let repositories = repositories.lock().await;
        run_search(&**repositories, &sources, &request)
    };
    let status = match answer.get("error_code").and_then(Value::as_str) {
        None => StatusCode::OK,
        Some("project_not_found") => StatusCode::NOT_FOUND,
        Some("storage_failed") => StatusCode::INTERNAL_SERVER_ERROR,
        Some(_) => StatusCode::BAD_REQUEST,
    };
    (status, Json(answer)).into_response()
}
