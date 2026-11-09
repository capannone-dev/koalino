//! The search answer shared by the app (FFI), the web (`GET /api/search`) and the CLI (#100).
//!
//! A request is `{"query", "scope"?: "meetings" | "projects" | "project", "project_id"?}`; the
//! scope defaults to `meetings`, and `project` needs `project_id`. The answer is
//! `{"ok":true,"hits":[...],"truncated":bool}` or `{"ok":false,"error_code","error"}` with codes
//! `invalid_request`, `empty`, `too_short`, `too_long`, `project_not_found` and
//! `storage_failed`.
//!
//! Each hit is `{"place", "meeting", "project", "resource", "snippet"}` plus `start_time_ms`,
//! `end_time_ms` and `speaker` for a transcript hit:
//! - `place`: `meeting_title`, `summary`, `notes`, `transcript`, `transcript_text`,
//!   `project_name`, `resource_name` or `resource_text`;
//! - `meeting`: `{"meeting_id","title","created_at"}` or null;
//! - `project`: `{"project_id","name"}` or null;
//! - `resource`: `{"attachment_id","file_name","mime_type","kind"}` or null;
//! - `snippet`: `[{"text","match"}]`, the passage with its matched words marked.
//!
//! The query and the text found are never logged.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    application::search::{
        SearchError, SearchHit, SearchPlace, SearchQuery, SearchResults, SearchScope,
        SearchSources, search,
    },
    domain::{errors::StorageError, models::Attachment, projects::ProjectId},
    ports::{projects::ProjectRepository, storage::NoteRepository, voice::VoiceVectorDB},
    project_commands::{ProjectFiles, resource_kind},
};

/// A search request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchRequest {
    /// What the owner typed.
    pub query: String,
    /// `meetings` (the default), `projects` or `project`.
    #[serde(default)]
    pub scope: Option<String>,
    /// The project to search, for the `project` scope.
    #[serde(default)]
    pub project_id: Option<String>,
}

/// Speaker names and project files, as the hosts have them.
pub struct HostSearchSources<'files> {
    speaker_names: HashMap<Uuid, String>,
    files: Option<&'files ProjectFiles>,
}

impl<'files> HostSearchSources<'files> {
    /// Creates the sources; without `files`, project files are found by name only.
    pub fn new(speaker_names: HashMap<Uuid, String>, files: Option<&'files ProjectFiles>) -> Self {
        Self {
            speaker_names,
            files,
        }
    }
}

impl SearchSources for HostSearchSources<'_> {
    fn speaker_name(&self, identity_id: Uuid) -> Option<String> {
        self.speaker_names.get(&identity_id).cloned()
    }

    fn resource_text(&self, resource: &Attachment) -> Option<String> {
        self.files?.searchable_text(resource)
    }
}

/// Reads every saved person's name.
///
/// Outputs: Names by identity, or a storage error. Side effects: Reads the voice library.
pub fn speaker_names<VoiceDatabase: VoiceVectorDB + ?Sized>(
    voice_database: &VoiceDatabase,
) -> Result<HashMap<Uuid, String>, StorageError> {
    Ok(voice_database
        .list_speaker_identities()?
        .into_iter()
        .map(|identity| (identity.identity_id(), identity.display_name().to_owned()))
        .collect())
}

/// Runs a search and words the answer.
///
/// Inputs: The repository, the sources and the request. Outputs: The JSON answer described in
/// the module documentation. Side effects: Reads (and so unseals) meetings and project files.
pub fn run_search<Repository>(
    repository: &Repository,
    sources: &dyn SearchSources,
    request: &SearchRequest,
) -> Value
where
    Repository: NoteRepository + ProjectRepository + ?Sized,
{
    let scope = match (request.scope.as_deref(), request.project_id.as_deref()) {
        (None | Some("meetings"), _) => SearchScope::Meetings,
        (Some("projects"), _) => SearchScope::Projects,
        (Some("project"), Some(project_id)) => match Uuid::parse_str(project_id.trim()) {
            Ok(project_id) => SearchScope::Project(ProjectId::from_uuid(project_id)),
            Err(_) => return failure("invalid_request", "the project ID is not valid"),
        },
        (Some("project"), None) => {
            return failure("invalid_request", "the project scope needs `project_id`");
        }
        (Some(_), _) => {
            return failure(
                "invalid_request",
                "`scope` must be meetings, projects or project",
            );
        }
    };
    let outcome = SearchQuery::parse(&request.query)
        .and_then(|query| search(repository, sources, &query, &scope));
    match outcome {
        Ok(results) => results_json(&results),
        Err(error) => failure(error_code(&error), &message(&error)),
    }
}

/// The stable code of a search failure.
pub fn error_code(error: &SearchError) -> &'static str {
    match error {
        SearchError::Empty => "empty",
        SearchError::TooShort => "too_short",
        SearchError::TooLong => "too_long",
        SearchError::ProjectNotFound => "project_not_found",
        SearchError::Storage(_) => "storage_failed",
    }
}

/// A failure's message, without storage details.
fn message(error: &SearchError) -> String {
    match error {
        SearchError::Storage(_) => String::from("the notes could not be read"),
        other => other.to_string(),
    }
}

/// A refusal.
fn failure(error_code: &str, error: &str) -> Value {
    json!({"ok": false, "error_code": error_code, "error": error})
}

/// The answer for found hits.
pub fn results_json(results: &SearchResults) -> Value {
    json!({
        "ok": true,
        "hits": results.hits.iter().map(hit_json).collect::<Vec<Value>>(),
        "truncated": results.truncated,
    })
}

/// One hit as JSON.
fn hit_json(hit: &SearchHit) -> Value {
    let place = match &hit.place {
        SearchPlace::MeetingTitle => "meeting_title",
        SearchPlace::Summary => "summary",
        SearchPlace::Notes => "notes",
        SearchPlace::Transcript { .. } => "transcript",
        SearchPlace::TranscriptText => "transcript_text",
        SearchPlace::ProjectName => "project_name",
        SearchPlace::ResourceName => "resource_name",
        SearchPlace::ResourceText => "resource_text",
    };
    let mut fields = json!({
        "place": place,
        "meeting": hit.meeting.as_ref().map(|meeting| json!({
            "meeting_id": meeting.meeting_id.as_str(),
            "title": meeting.title,
            "created_at": meeting.created_at.to_rfc3339(),
        })),
        "project": hit.project.as_ref().map(|project| json!({
            "project_id": project.project_id.to_string(),
            "name": project.name,
        })),
        "resource": hit.resource.as_ref().map(|resource| json!({
            "attachment_id": resource.attachment_id.to_string(),
            "file_name": resource.file_name,
            "mime_type": resource.mime_type,
            "kind": resource_kind(&resource.mime_type),
        })),
        "snippet": hit.snippet.iter().map(|part| json!({
            "text": part.text,
            "match": part.matched,
        })).collect::<Vec<Value>>(),
    });
    if let (
        SearchPlace::Transcript {
            start_time_ms,
            end_time_ms,
            speaker,
        },
        Value::Object(map),
    ) = (&hit.place, &mut fields)
    {
        map.insert(String::from("start_time_ms"), json!(start_time_ms));
        map.insert(String::from("end_time_ms"), json!(end_time_ms));
        map.insert(String::from("speaker"), json!(speaker));
    }
    fields
}
