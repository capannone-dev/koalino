//! Serves the `/api/projects` routes the web Projects pages use (Issue #85).
//!
//! | Route | Success |
//! | --- | --- |
//! | `GET /api/projects` | 200, project summaries by name |
//! | `POST /api/projects` `{"name"}` | 201, the summary |
//! | `GET /api/projects/{project_id}` | 200 `{"project","meetings","resources"}` |
//! | `PATCH /api/projects/{project_id}` `{"name"}` | 200, the summary |
//! | `DELETE /api/projects/{project_id}` | 204 (its meetings stay, its resources go) |
//! | `PUT /api/meetings/{meeting_id}/project` `{"project_id"}` (null moves it out) | 204 |
//! | `POST /api/projects/{project_id}/resources?name=<file name>`, the file as the body | 201 `{"added","skipped_attachments"}` |
//! | `POST /api/projects/{project_id}/notes` `{"title","text"}` | 201, the Markdown resource |
//! | `DELETE /api/projects/{project_id}/resources/{attachment_id}` | 204 |
//! | `GET /api/projects/{project_id}/resources/{attachment_id}/file` | 200, the bytes |
//! | `GET /api/projects/{project_id}/resources/{attachment_id}/email` | 200, the e-mail as text |
//!
//! Failures answer `{"error_code","error"}` (codes from `project_commands`): 400 for a bad ID,
//! name, title or file type; 404 for an unknown project, meeting or resource; 503 when the
//! process has no project files (`ServerAppState::with_project_files`); 500 for storage errors.
//!
//! A resource's bytes are served with its stored type, `nosniff` and a sandboxing Content Security
//! Policy, so a file can never run script on the API's origin. A resource the app does not show
//! (`document` kind, such as any file type a project took, #94) is sent as a download. Resources never carry local paths.
//! File work runs on a blocking thread. Names, notes and e-mail contents are never logged.

use std::sync::Arc;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::api::{ServerAppState, ServerRepository};
use crate::{
    domain::models::MeetingId,
    project_commands::{
        ProjectCommandError, ProjectFiles, delete_project, delete_resource, email_json,
        existing_project, list_projects, move_meeting, parse_name, parse_project_id,
        project_detail, read_email, resource_file_name, resource_json, resource_kind, resource_of,
        summary_json,
    },
};

/// Largest upload accepted, matching the attachment store's 100 MiB limit plus headroom.
const MAX_UPLOAD_BYTES: usize = 101 * 1024 * 1024;

/// Registers the project routes; `build_router` merges them before its layers.
pub(super) fn routes() -> Router<ServerAppState> {
    Router::new()
        .route("/api/projects", get(list).post(create))
        .route(
            "/api/projects/{project_id}",
            get(detail).patch(rename).delete(remove),
        )
        .route("/api/meetings/{meeting_id}/project", put(move_into_project))
        .route(
            "/api/projects/{project_id}/resources",
            post(upload).layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES)),
        )
        .route("/api/projects/{project_id}/notes", post(add_note))
        .route(
            "/api/projects/{project_id}/resources/{attachment_id}",
            delete(remove_resource),
        )
        .route(
            "/api/projects/{project_id}/resources/{attachment_id}/file",
            get(resource_file),
        )
        .route(
            "/api/projects/{project_id}/resources/{attachment_id}/email",
            get(resource_email),
        )
}

/// Why a project route failed.
enum RouteFailure {
    /// A command failed with its code.
    Command(ProjectCommandError),
    /// The process has no project files (503).
    Unavailable,
    /// A blocking task failed (500).
    Internal,
}

impl From<ProjectCommandError> for RouteFailure {
    fn from(error: ProjectCommandError) -> Self {
        Self::Command(error)
    }
}

impl From<crate::domain::errors::StorageError> for RouteFailure {
    fn from(error: crate::domain::errors::StorageError) -> Self {
        Self::Command(error.into())
    }
}

impl IntoResponse for RouteFailure {
    fn into_response(self) -> Response {
        match self {
            Self::Command(error) => {
                let status = match error.error_code {
                    "invalid_request" | "invalid_name" | "rejected" | "not_an_email" => {
                        StatusCode::BAD_REQUEST
                    }
                    "project_not_found" | "meeting_not_found" | "resource_not_found" => {
                        StatusCode::NOT_FOUND
                    }
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                };
                (
                    status,
                    Json(json!({ "error_code": error.error_code, "error": error.message })),
                )
                    .into_response()
            }
            Self::Unavailable => StatusCode::SERVICE_UNAVAILABLE.into_response(),
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
}

/// Runs file work on a blocking thread with the repository and the project files.
///
/// Outputs: What `work` returns, `Unavailable` without project files, or `Internal` when the
/// thread fails. Side effects: Holds the repository lock until `work` ends.
async fn with_files<Output, Work>(
    application_state: &ServerAppState,
    work: Work,
) -> Result<Output, RouteFailure>
where
    Output: Send + 'static,
    Work: FnOnce(&dyn ServerRepository, &ProjectFiles) -> Result<Output, ProjectCommandError>
        + Send
        + 'static,
{
    let project_files: Arc<ProjectFiles> = application_state
        .project_files()
        .ok_or(RouteFailure::Unavailable)?;
    let repositories = application_state.repositories().lock_owned().await;
    tokio::task::spawn_blocking(move || work(&**repositories, &project_files))
        .await
        .map_err(|_| RouteFailure::Internal)?
        .map_err(RouteFailure::Command)
}

/// Lists projects.
async fn list(
    State(application_state): State<ServerAppState>,
) -> Result<Json<Value>, RouteFailure> {
    tracing::info!(
        endpoint = "GET /api/projects",
        "headless API request received"
    );
    let repositories = application_state.repositories().lock_owned().await;
    Ok(Json(Value::Array(list_projects(&**repositories)?)))
}

/// Body of `POST /api/projects` and `PATCH /api/projects/{project_id}`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NameRequest {
    name: String,
}

/// Creates a project.
async fn create(
    State(application_state): State<ServerAppState>,
    Json(request): Json<NameRequest>,
) -> Result<(StatusCode, Json<Value>), RouteFailure> {
    tracing::info!(
        endpoint = "POST /api/projects",
        "headless API request received"
    );
    let name = parse_name(&request.name)?;
    let repositories = application_state.repositories().lock_owned().await;
    let project = repositories.create_project(&name)?;
    let summary = repositories
        .list_projects()?
        .into_iter()
        .find(|summary| summary.project.project_id() == project.project_id())
        .ok_or(RouteFailure::Internal)?;
    Ok((StatusCode::CREATED, Json(summary_json(&summary))))
}

/// Describes a project with its meetings and resources.
async fn detail(
    State(application_state): State<ServerAppState>,
    Path(project_id): Path<String>,
) -> Result<Json<Value>, RouteFailure> {
    tracing::info!(
        endpoint = "GET /api/projects/{id}",
        "headless API request received"
    );
    let repositories = application_state.repositories().lock_owned().await;
    Ok(Json(project_detail(&**repositories, &project_id, false)?))
}

/// Renames a project.
async fn rename(
    State(application_state): State<ServerAppState>,
    Path(project_id): Path<String>,
    Json(request): Json<NameRequest>,
) -> Result<Json<Value>, RouteFailure> {
    tracing::info!(
        endpoint = "PATCH /api/projects/{id}",
        "headless API request received"
    );
    let name = parse_name(&request.name)?;
    let repositories = application_state.repositories().lock_owned().await;
    let project_id = existing_project(&**repositories, &project_id)?.project_id();
    repositories.rename_project(project_id, &name)?;
    let summary = repositories
        .list_projects()?
        .into_iter()
        .find(|summary| summary.project.project_id() == project_id)
        .ok_or(RouteFailure::Internal)?;
    Ok(Json(summary_json(&summary)))
}

/// Deletes a project; its meetings stay, its resources and their folder go.
async fn remove(
    State(application_state): State<ServerAppState>,
    Path(project_id): Path<String>,
) -> Result<StatusCode, RouteFailure> {
    tracing::info!(
        endpoint = "DELETE /api/projects/{id}",
        "headless API request received"
    );
    let project_id = parse_project_id(&project_id)?;
    let deleted = match application_state.project_files() {
        Some(_) => {
            with_files(&application_state, move |repository, files| {
                delete_project(repository, files, project_id)
            })
            .await?
        }
        None => application_state
            .repositories()
            .lock_owned()
            .await
            .delete_project(project_id)?,
    };
    Ok(if deleted {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    })
}

/// Body of `PUT /api/meetings/{meeting_id}/project`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveRequest {
    project_id: Option<String>,
}

/// Moves a meeting into a project, or out with a null `project_id`.
async fn move_into_project(
    State(application_state): State<ServerAppState>,
    Path(meeting_id): Path<String>,
    Json(request): Json<MoveRequest>,
) -> Result<StatusCode, RouteFailure> {
    tracing::info!(
        endpoint = "PUT /api/meetings/{id}/project",
        "headless API request received"
    );
    {
        let repositories = application_state.repositories().lock_owned().await;
        move_meeting(&**repositories, &meeting_id, request.project_id.as_deref())?;
    }
    application_state.notify_local_change(&MeetingId::new(meeting_id.trim()));
    Ok(StatusCode::NO_CONTENT)
}

/// Query of an upload: the file's name, whose extension gives its type.
#[derive(Deserialize)]
struct UploadQuery {
    name: String,
}

/// Adds an uploaded file to a project; an `.eml` also adds its attachments.
async fn upload(
    State(application_state): State<ServerAppState>,
    Path(project_id): Path<String>,
    Query(query): Query<UploadQuery>,
    contents: Bytes,
) -> Result<(StatusCode, Json<Value>), RouteFailure> {
    tracing::info!(
        endpoint = "POST /api/projects/{id}/resources",
        "headless API request received"
    );
    let imported = with_files(&application_state, move |repository, files| {
        let project_id = existing_project(repository, &project_id)?.project_id();
        Ok(files.add_contents(repository, project_id, &query.name, &contents)?)
    })
    .await?;
    let added: Vec<Value> = std::iter::once(&imported.email)
        .chain(imported.extracted.iter())
        .map(|resource| resource_json(resource, false))
        .collect();
    Ok((
        StatusCode::CREATED,
        Json(json!({ "added": added, "skipped_attachments": imported.skipped_attachments })),
    ))
}

/// Body of `POST /api/projects/{project_id}/notes`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoteRequest {
    title: String,
    text: String,
}

/// Adds a note typed on the web as a Markdown resource.
async fn add_note(
    State(application_state): State<ServerAppState>,
    Path(project_id): Path<String>,
    Json(request): Json<NoteRequest>,
) -> Result<(StatusCode, Json<Value>), RouteFailure> {
    tracing::info!(
        endpoint = "POST /api/projects/{id}/notes",
        "headless API request received"
    );
    let resource = with_files(&application_state, move |repository, files| {
        let project_id = existing_project(repository, &project_id)?.project_id();
        Ok(files.add_note(repository, project_id, &request.title, &request.text)?)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(resource_json(&resource, false))))
}

/// Deletes one resource and its file.
async fn remove_resource(
    State(application_state): State<ServerAppState>,
    Path((project_id, attachment_id)): Path<(String, String)>,
) -> Result<StatusCode, RouteFailure> {
    tracing::info!(
        endpoint = "DELETE /api/projects/{id}/resources/{id}",
        "headless API request received"
    );
    with_files(&application_state, move |repository, files| {
        delete_resource(
            repository,
            files,
            parse_project_id(&project_id)?,
            &attachment_id,
        )
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Serves a resource's bytes with headers that keep them inert in the browser.
async fn resource_file(
    State(application_state): State<ServerAppState>,
    Path((project_id, attachment_id)): Path<(String, String)>,
) -> Result<Response, RouteFailure> {
    tracing::info!(
        endpoint = "GET /api/projects/{id}/resources/{id}/file",
        "headless API request received"
    );
    let (resource, contents) = with_files(&application_state, move |repository, files| {
        let resource = resource_of(
            repository,
            files,
            parse_project_id(&project_id)?,
            &attachment_id,
        )?;
        let contents = std::fs::read(resource.file_path()).map_err(|_| {
            ProjectCommandError::new(
                "resource_not_found",
                "the resource's file has not arrived here yet",
            )
        })?;
        Ok((resource, contents))
    })
    .await?;
    let content_type = if resource.mime_type().starts_with("text/") {
        format!("{}; charset=utf-8", resource.mime_type())
    } else {
        resource.mime_type().to_owned()
    };
    // The stored name holds only ASCII letters, digits, `.`, `-` and `_`.
    // A file the app does not show (#94) is only downloaded, never rendered by the browser.
    let presentation = if resource_kind(resource.mime_type()) == "document" {
        "attachment"
    } else {
        "inline"
    };
    let disposition = format!(
        "{presentation}; filename=\"{}\"",
        resource_file_name(&resource)
    );
    let mut response = contents.into_response();
    let headers = response.headers_mut();
    for (name, value) in [
        (header::CONTENT_TYPE, content_type),
        (header::CONTENT_DISPOSITION, disposition),
        (header::X_CONTENT_TYPE_OPTIONS, String::from("nosniff")),
        (
            header::CONTENT_SECURITY_POLICY,
            String::from("sandbox; default-src 'none'; img-src 'self'; style-src 'unsafe-inline'"),
        ),
    ] {
        headers.insert(
            name,
            HeaderValue::from_str(&value).map_err(|_| RouteFailure::Internal)?,
        );
    }
    Ok(response)
}

/// Reads an e-mail resource as plain text.
async fn resource_email(
    State(application_state): State<ServerAppState>,
    Path((project_id, attachment_id)): Path<(String, String)>,
) -> Result<Json<Value>, RouteFailure> {
    tracing::info!(
        endpoint = "GET /api/projects/{id}/resources/{id}/email",
        "headless API request received"
    );
    let email = with_files(&application_state, move |repository, files| {
        read_email(
            repository,
            files,
            parse_project_id(&project_id)?,
            &attachment_id,
        )
    })
    .await?;
    Ok(Json(email_json(&email)))
}
