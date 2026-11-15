//! Runs the Projects commands (Issue #85) for the app and the web API, so both behave alike.
//!
//! The app sends one JSON object tagged by `command` through `note_taker_project_command`; the web
//! routes in `server/project_routes.rs` call the same functions. Every answer is
//! `{"ok":true,...}` or `{"ok":false,"error_code","error"}`.
//!
//! | `command` | Fields | Answer |
//! | --- | --- | --- |
//! | `list` | | `projects`: summaries by name |
//! | `create` | `name` | `project` |
//! | `rename` | `project_id`, `name` | `project` |
//! | `delete` | `project_id` | `deleted` (its meetings stay, its resources and folder go) |
//! | `detail` | `project_id` | `project`, `meetings` (newest first), `resources` (oldest first) |
//! | `move_meeting` | `meeting_id`, `project_id` (null moves it out) | `{}` |
//! | `meeting_projects` | | `meeting_projects`: `{meeting ID: project ID}` |
//! | `add_files` | `project_id`, `paths` | `added` resources (e-mail attachments included), `failed` `[{"path","error"}]`, `skipped_attachments` |
//! | `add_note` | `project_id`, `title`, `text` | `resource` (a Markdown file) |
//! | `delete_resource` | `project_id`, `attachment_id` | `deleted: true` |
//! | `read_email` | `project_id`, `attachment_id` | `email` (headers, text body, attachment names) |
//!
//! Error codes: `invalid_request`, `invalid_name`, `project_not_found`, `meeting_not_found`,
//! `resource_not_found`, `not_an_email`, `rejected` (a file or note that cannot be stored), and
//! `storage_failed`. Resources carry `file_path` only when the caller reads files itself (the app),
//! never for the web. Names, notes and e-mail contents are never logged.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    adapters::{
        documents::text_extraction::{extract_text, has_text},
        email::mail_parser_reader::MailParserEmailReader,
        storage::{
            attachment_file_store::{AttachmentFileStore, ProjectEmailImport, project_folder_name},
            private_files::remove_meeting_folder,
            sealed_files::SealedFiles,
        },
    },
    domain::{
        email::{EmailMessage, EmailPerson},
        errors::StorageError,
        models::{Attachment, DeviceId, MeetingId, TranscriptSegment},
        projects::{Project, ProjectId, ProjectName},
    },
    ports::{
        email::EmailReader,
        projects::{ProjectRepository, ProjectSummary},
        storage::{AttachmentRepository, NoteRepository},
    },
};

/// What the Projects commands need from a repository.
pub trait ProjectCommandRepository:
    ProjectRepository + NoteRepository + AttachmentRepository
{
}

impl<T> ProjectCommandRepository for T where
    T: ProjectRepository + NoteRepository + AttachmentRepository + ?Sized
{
}

/// How much of a text file search reads (#100).
const MAX_SEARCHED_TEXT_BYTES: u64 = 2 * 1024 * 1024;
/// Largest PDF, office document or e-mail whose text is read for search and chat (#128).
const MAX_EXTRACTED_FILE_BYTES: u64 = 50 * 1024 * 1024;

/// The files side of projects: where resources live and how e-mails are read.
#[derive(Clone, Debug)]
pub struct ProjectFiles {
    attachment_directory: PathBuf,
    file_store: AttachmentFileStore,
    email_reader: MailParserEmailReader,
}

impl ProjectFiles {
    /// Creates the files side over the attachment folder.
    ///
    /// Inputs: The attachment root (normally `<data directory>/attachments`), this device's ID,
    /// recorded as the origin of added resources, and the sealer holding the database key
    /// (#99). Outputs: The value. Side effects: None.
    pub fn new(
        attachment_directory: PathBuf,
        device_id: DeviceId,
        sealed_files: SealedFiles,
    ) -> Self {
        Self {
            file_store: AttachmentFileStore::new(
                attachment_directory.clone(),
                device_id,
                sealed_files,
            ),
            attachment_directory,
            email_reader: MailParserEmailReader::new(),
        }
    }

    /// Adds received contents (a web upload) to a project; see
    /// `AttachmentFileStore::import_project_contents`.
    ///
    /// Outputs: What was stored, or a storage rejection. Side effects: Writes files and rows.
    pub fn add_contents<RepositoryPort: ProjectCommandRepository + ?Sized>(
        &self,
        repository: &RepositoryPort,
        project_id: ProjectId,
        file_name: &str,
        contents: &[u8],
    ) -> Result<ProjectEmailImport, StorageError> {
        self.file_store.import_project_contents(
            repository,
            &self.email_reader,
            project_id,
            Path::new(file_name),
            contents,
        )
    }

    /// Writes a note into a project as a Markdown resource; see
    /// `AttachmentFileStore::import_project_note`.
    ///
    /// Outputs: The resource, or a storage rejection. Side effects: Writes one file and one row.
    pub fn add_note<RepositoryPort: ProjectCommandRepository + ?Sized>(
        &self,
        repository: &RepositoryPort,
        project_id: ProjectId,
        title: &str,
        text: &str,
    ) -> Result<Attachment, StorageError> {
        self.file_store
            .import_project_note(repository, project_id, title, text)
    }

    /// Attaches a file to a meeting (the notes folder's `attachments/`, #98); see
    /// `AttachmentFileStore::import_file`.
    ///
    /// Outputs: The stored attachment, or a storage rejection (an unreadable or refused file).
    /// Side effects: Copies the file into the attachment folder and writes one row.
    /// The sealer stored files are written with (#99), for reading them back.
    pub fn sealed_files(&self) -> &SealedFiles {
        self.file_store.sealed_files()
    }

    /// Reads a stored file's clear contents.
    ///
    /// Outputs: The contents (wiped when dropped), or a rejection for a missing or unreadable
    /// file. Side effects: Reads and decrypts the file.
    pub fn read_file(
        &self,
        stored: &Attachment,
    ) -> Result<zeroize::Zeroizing<Vec<u8>>, StorageError> {
        self.sealed_files().read_all(Path::new(stored.file_path()))
    }

    /// The length of a stored file's clear contents; `None` when it is missing or unreadable
    /// (a resource still syncing).
    ///
    /// Side effects: Reads the file's record headers.
    pub fn clear_len(&self, stored: &Attachment) -> Option<u64> {
        self.sealed_files()
            .clear_len(Path::new(stored.file_path()))
            .ok()
    }

    pub fn attach_to_meeting<RepositoryPort: ProjectCommandRepository + ?Sized>(
        &self,
        repository: &RepositoryPort,
        meeting_id: &MeetingId,
        source_path: &Path,
    ) -> Result<Attachment, StorageError> {
        self.file_store
            .import_file(repository, meeting_id, source_path, None)
    }

    /// Reads a resource's text for search (#100) and chat: a text or Markdown file (its first
    /// 2 MiB), or, through `extract_text` (#128), an e-mail, a PDF, a Word, PowerPoint, Excel or
    /// LibreOffice document, or a CSV, JSON or YAML file. Pictures have no text here.
    ///
    /// Outputs: The text, or `None` for other kinds, for a file over `MAX_EXTRACTED_FILE_BYTES`,
    /// and for a file missing or unreadable here (one still syncing). Side effects: Reads the
    /// file.
    pub fn searchable_text(&self, resource: &Attachment) -> Option<String> {
        use std::io::Read as _;
        let name = resource.shown_file_name();
        match resource.mime_type() {
            "text/plain" | "text/markdown" => {
                let mut contents = zeroize::Zeroizing::new(Vec::new());
                self.sealed_files()
                    .open(Path::new(resource.file_path()))
                    .ok()?
                    .take(MAX_SEARCHED_TEXT_BYTES)
                    .read_to_end(&mut contents)
                    .ok()?;
                Some(String::from_utf8_lossy(&contents).into_owned())
            }
            mime_type if has_text(mime_type, &name) => {
                if self.clear_len(resource)? > MAX_EXTRACTED_FILE_BYTES {
                    return None;
                }
                let contents = self.read_file(resource).ok()?;
                extract_text(mime_type, &name, &contents)
            }
            _ => None,
        }
    }

    /// Finds one resource of a project.
    ///
    /// Outputs: The resource, `None` when the project has no resource with this ID, or a storage
    /// error. Side effects: Reads rows.
    pub fn find_resource<RepositoryPort: ProjectCommandRepository + ?Sized>(
        &self,
        repository: &RepositoryPort,
        project_id: ProjectId,
        attachment_id: Uuid,
    ) -> Result<Option<Attachment>, StorageError> {
        Ok(repository
            .project_resources(project_id)?
            .into_iter()
            .find(|resource| resource.attachment_id() == attachment_id))
    }
}

/// A command from the app, tagged by `command`.
#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum ProjectCommand {
    List,
    Create {
        name: String,
    },
    Rename {
        project_id: String,
        name: String,
    },
    Delete {
        project_id: String,
    },
    Detail {
        project_id: String,
    },
    MoveMeeting {
        meeting_id: String,
        project_id: Option<String>,
    },
    MeetingProjects,
    AddFiles {
        project_id: String,
        paths: Vec<String>,
    },
    AddNote {
        project_id: String,
        title: String,
        text: String,
    },
    DeleteResource {
        project_id: String,
        attachment_id: String,
    },
    ReadEmail {
        project_id: String,
        attachment_id: String,
    },
}

/// A failed command: its error code and a message safe to show.
#[derive(Debug, Eq, PartialEq)]
pub struct ProjectCommandError {
    /// One of the codes listed in the module documentation.
    pub error_code: &'static str,
    /// What went wrong, without names or contents.
    pub message: String,
}

impl ProjectCommandError {
    /// Builds an error.
    pub fn new(error_code: &'static str, message: impl Into<String>) -> Self {
        Self {
            error_code,
            message: message.into(),
        }
    }

    /// The error as the JSON answer.
    pub fn to_json(&self) -> Value {
        json!({ "ok": false, "error_code": self.error_code, "error": self.message })
    }
}

impl From<StorageError> for ProjectCommandError {
    fn from(error: StorageError) -> Self {
        match error {
            StorageError::Rejected { reason } => Self::new("rejected", reason),
            other => Self::new("storage_failed", other.to_string()),
        }
    }
}

/// Runs one command given as JSON.
///
/// Inputs: The repository (with the meeting-document codec, for moves), the files side, the
/// request, and whether resources include their local `file_path`.
/// Outputs: The JSON answer described in the module documentation.
/// Side effects: Those of the command: SQLite writes, files written or removed. The P2P task
/// notices the changes on its own (document revisions, digests, deletion records).
pub fn run_project_command<RepositoryPort: ProjectCommandRepository + ?Sized>(
    repository: &RepositoryPort,
    files: &ProjectFiles,
    request_json: &str,
    include_paths: bool,
) -> String {
    let answer = serde_json::from_str::<ProjectCommand>(request_json)
        .map_err(|_| ProjectCommandError::new("invalid_request", "the command is not understood"))
        .and_then(|command| run(repository, files, command, include_paths));
    match answer {
        Ok(Value::Object(mut fields)) => {
            fields.insert(String::from("ok"), Value::Bool(true));
            Value::Object(fields).to_string()
        }
        Ok(other) => json!({ "ok": true, "value": other }).to_string(),
        Err(error) => error.to_json().to_string(),
    }
}

/// Carries out one parsed command.
fn run<RepositoryPort: ProjectCommandRepository + ?Sized>(
    repository: &RepositoryPort,
    files: &ProjectFiles,
    command: ProjectCommand,
    include_paths: bool,
) -> Result<Value, ProjectCommandError> {
    match command {
        ProjectCommand::List => Ok(json!({ "projects": list_projects(repository)? })),
        ProjectCommand::Create { name } => {
            let project = repository.create_project(&parse_name(&name)?)?;
            Ok(json!({ "project": project_json(&project, 0, 0, None) }))
        }
        ProjectCommand::Rename { project_id, name } => {
            let project_id = existing_project(repository, &project_id)?.project_id();
            let project = repository.rename_project(project_id, &parse_name(&name)?)?;
            Ok(json!({ "project": summary_json(&summary_of(repository, &project)?) }))
        }
        ProjectCommand::Delete { project_id } => Ok(json!({
            "deleted": delete_project(repository, files, parse_project_id(&project_id)?)?
        })),
        ProjectCommand::Detail { project_id } => {
            project_detail(repository, files, &project_id, include_paths)
        }
        ProjectCommand::MoveMeeting {
            meeting_id,
            project_id,
        } => {
            move_meeting(repository, &meeting_id, project_id.as_deref())?;
            Ok(json!({}))
        }
        ProjectCommand::MeetingProjects => {
            let memberships: BTreeMap<String, String> = repository
                .meeting_projects()?
                .into_iter()
                .map(|(meeting_id, project_id)| {
                    (meeting_id.as_str().to_owned(), project_id.to_string())
                })
                .collect();
            Ok(json!({ "meeting_projects": memberships }))
        }
        ProjectCommand::AddFiles { project_id, paths } => {
            let project_id = existing_project(repository, &project_id)?.project_id();
            Ok(add_files(
                repository,
                files,
                project_id,
                &paths,
                include_paths,
            ))
        }
        ProjectCommand::AddNote {
            project_id,
            title,
            text,
        } => {
            let project_id = existing_project(repository, &project_id)?.project_id();
            let resource = files.add_note(repository, project_id, &title, &text)?;
            Ok(json!({ "resource": resource_json(files, &resource, include_paths) }))
        }
        ProjectCommand::DeleteResource {
            project_id,
            attachment_id,
        } => {
            delete_resource(
                repository,
                files,
                parse_project_id(&project_id)?,
                &attachment_id,
            )?;
            Ok(json!({ "deleted": true }))
        }
        ProjectCommand::ReadEmail {
            project_id,
            attachment_id,
        } => {
            let email = read_email(
                repository,
                files,
                parse_project_id(&project_id)?,
                &attachment_id,
            )?;
            Ok(json!({ "email": email_json(&email) }))
        }
    }
}

/// Lists every project with its counts, by name.
///
/// Outputs: The summaries as JSON, or an error. Side effects: Reads rows.
pub fn list_projects<RepositoryPort: ProjectCommandRepository + ?Sized>(
    repository: &RepositoryPort,
) -> Result<Vec<Value>, ProjectCommandError> {
    Ok(repository
        .list_projects()?
        .iter()
        .map(summary_json)
        .collect())
}

/// Validates a project name.
///
/// Outputs: The name, or `invalid_name`. Side effects: None.
pub fn parse_name(name: &str) -> Result<ProjectName, ProjectCommandError> {
    ProjectName::parse(name)
        .map_err(|error| ProjectCommandError::new("invalid_name", error.to_string()))
}

/// Reads a project ID.
///
/// Outputs: The ID, or `invalid_request`. Side effects: None.
pub fn parse_project_id(project_id: &str) -> Result<ProjectId, ProjectCommandError> {
    ProjectId::parse(project_id.trim())
        .ok_or_else(|| ProjectCommandError::new("invalid_request", "the project ID is not valid"))
}

/// Loads a project named by its ID text.
///
/// Outputs: The project, or `invalid_request` / `project_not_found`. Side effects: Reads a row.
pub fn existing_project<RepositoryPort: ProjectCommandRepository + ?Sized>(
    repository: &RepositoryPort,
    project_id: &str,
) -> Result<Project, ProjectCommandError> {
    repository
        .load_project(parse_project_id(project_id)?)?
        .ok_or_else(|| ProjectCommandError::new("project_not_found", "the project does not exist"))
}

/// The list entry of one project.
///
/// Outputs: Its summary. Side effects: Reads rows.
fn summary_of<RepositoryPort: ProjectCommandRepository + ?Sized>(
    repository: &RepositoryPort,
    project: &Project,
) -> Result<ProjectSummary, ProjectCommandError> {
    repository
        .list_projects()?
        .into_iter()
        .find(|summary| summary.project.project_id() == project.project_id())
        .ok_or_else(|| ProjectCommandError::new("project_not_found", "the project does not exist"))
}

/// Deletes a project and its resource folder; its meetings stay, outside any project.
///
/// Outputs: Whether it existed, or an error. Side effects: Deletes rows (leaving a tombstone that
/// paired devices apply) and the `project-<uuid>` folder.
pub fn delete_project<RepositoryPort: ProjectCommandRepository + ?Sized>(
    repository: &RepositoryPort,
    files: &ProjectFiles,
    project_id: ProjectId,
) -> Result<bool, ProjectCommandError> {
    let deleted = repository.delete_project(project_id)?;
    if let Err(error) = remove_meeting_folder(
        &files.attachment_directory,
        &project_folder_name(project_id),
    ) {
        tracing::warn!(%project_id, error = %error, "a deleted project's files could not be removed");
    }
    Ok(deleted)
}

/// Describes a project with its meetings and resources.
///
/// Outputs: `{"project","meetings","resources"}`, or an error. Side effects: Reads rows.
pub fn project_detail<RepositoryPort: ProjectCommandRepository + ?Sized>(
    repository: &RepositoryPort,
    files: &ProjectFiles,
    project_id: &str,
    include_paths: bool,
) -> Result<Value, ProjectCommandError> {
    let project = existing_project(repository, project_id)?;
    let project_id = project.project_id();
    let mut meetings = Vec::new();
    for meeting_id in repository.project_meeting_ids(project_id)? {
        if let Some(metadata) = repository.load_meeting_metadata(&meeting_id)? {
            let duration_ms = repository
                .load_transcript_segments(&meeting_id)?
                .iter()
                .map(TranscriptSegment::end_time_ms)
                .max()
                .unwrap_or_default();
            meetings.push(json!({
                "meeting_id": meeting_id.as_str(),
                "title": metadata.display_title(),
                "created_at": metadata.created_at().to_rfc3339(),
                "duration_ms": duration_ms,
                "project_id": project_id.to_string(),
            }));
        }
    }
    let resources: Vec<Value> = repository
        .project_resources(project_id)?
        .iter()
        .map(|resource| resource_json(files, resource, include_paths))
        .collect();
    Ok(json!({
        "project": summary_json(&summary_of(repository, &project)?),
        "meetings": meetings,
        "resources": resources,
    }))
}

/// Moves a meeting into a project, or out of any with `None`.
///
/// Outputs: Success, or `meeting_not_found`, `project_not_found`, `invalid_request` or a storage
/// error. Side effects: Writes the meeting's CRDT document, which syncs to paired devices.
pub fn move_meeting<RepositoryPort: ProjectCommandRepository + ?Sized>(
    repository: &RepositoryPort,
    meeting_id: &str,
    project_id: Option<&str>,
) -> Result<(), ProjectCommandError> {
    let meeting_id = MeetingId::new(meeting_id.trim());
    if meeting_id.as_str().is_empty() || repository.load_meeting_metadata(&meeting_id)?.is_none() {
        return Err(ProjectCommandError::new(
            "meeting_not_found",
            "the meeting does not exist",
        ));
    }
    let project_id = match project_id {
        Some(project_id) => Some(existing_project(repository, project_id)?.project_id()),
        None => None,
    };
    repository.move_meeting_to_project(&meeting_id, project_id)?;
    Ok(())
}

/// Adds local files to a project; `.eml` files also add their attachments.
///
/// Outputs: `{"added","failed","skipped_attachments"}`. Side effects: Copies files; writes rows.
fn add_files<RepositoryPort: ProjectCommandRepository + ?Sized>(
    repository: &RepositoryPort,
    files: &ProjectFiles,
    project_id: ProjectId,
    paths: &[String],
    include_paths: bool,
) -> Value {
    let mut added = Vec::new();
    let mut failed = Vec::new();
    let mut skipped_attachments = 0;
    for source in paths {
        let source_path = Path::new(source);
        let is_email = source_path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("eml"));
        let imported = if is_email {
            files.file_store.import_project_email(
                repository,
                &files.email_reader,
                project_id,
                source_path,
            )
        } else {
            files
                .file_store
                .import_project_file(repository, project_id, source_path)
                .map(|resource| ProjectEmailImport {
                    email: resource,
                    extracted: Vec::new(),
                    skipped_attachments: 0,
                })
        };
        match imported {
            Ok(imported) => {
                added.push(resource_json(files, &imported.email, include_paths));
                added.extend(
                    imported
                        .extracted
                        .iter()
                        .map(|resource| resource_json(files, resource, include_paths)),
                );
                skipped_attachments += imported.skipped_attachments;
            }
            Err(error) => failed.push(json!({ "path": source, "error": error.to_string() })),
        }
    }
    json!({ "added": added, "failed": failed, "skipped_attachments": skipped_attachments })
}

/// Deletes one resource and its file.
///
/// Outputs: Success, or `invalid_request`, `resource_not_found` or a storage error.
/// Side effects: Deletes the row (leaving a record paired devices apply) and the file.
pub fn delete_resource<RepositoryPort: ProjectCommandRepository + ?Sized>(
    repository: &RepositoryPort,
    files: &ProjectFiles,
    project_id: ProjectId,
    attachment_id: &str,
) -> Result<(), ProjectCommandError> {
    let resource = resource_of(repository, files, project_id, attachment_id)?;
    if !repository.delete_project_resource(resource.attachment_id())? {
        return Err(resource_not_found());
    }
    match std::fs::remove_file(resource.file_path()) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            tracing::warn!(
                attachment_id = %resource.attachment_id(),
                error = %error,
                "a deleted resource's file could not be removed"
            );
        }
        _ => {}
    }
    Ok(())
}

/// Reads an e-mail resource.
///
/// Outputs: The parsed e-mail, or `resource_not_found`, `not_an_email` or a storage error.
/// Side effects: Reads the file.
pub fn read_email<RepositoryPort: ProjectCommandRepository + ?Sized>(
    repository: &RepositoryPort,
    files: &ProjectFiles,
    project_id: ProjectId,
    attachment_id: &str,
) -> Result<EmailMessage, ProjectCommandError> {
    let resource = resource_of(repository, files, project_id, attachment_id)?;
    if resource.mime_type() != "message/rfc822" {
        return Err(ProjectCommandError::new(
            "not_an_email",
            "the resource is not an e-mail",
        ));
    }
    let raw_email = files.read_file(&resource).map_err(|_| {
        ProjectCommandError::new("resource_not_found", "the e-mail file is missing")
    })?;
    files
        .email_reader
        .read_email(&raw_email)
        .map_err(|error| ProjectCommandError::new("not_an_email", error.to_string()))
}

/// Loads one resource of a project by its ID text.
///
/// Outputs: The resource, or `invalid_request` / `resource_not_found`. Side effects: Reads rows.
pub fn resource_of<RepositoryPort: ProjectCommandRepository + ?Sized>(
    repository: &RepositoryPort,
    files: &ProjectFiles,
    project_id: ProjectId,
    attachment_id: &str,
) -> Result<Attachment, ProjectCommandError> {
    let attachment_id = Uuid::parse_str(attachment_id.trim())
        .map_err(|_| ProjectCommandError::new("invalid_request", "the resource ID is not valid"))?;
    files
        .find_resource(repository, project_id, attachment_id)?
        .ok_or_else(resource_not_found)
}

/// The error for a resource that is not in the project.
fn resource_not_found() -> ProjectCommandError {
    ProjectCommandError::new("resource_not_found", "the resource does not exist")
}

/// Serializes a project with its counts.
fn project_json(
    project: &Project,
    meeting_count: usize,
    resource_count: usize,
    latest_meeting_at: Option<String>,
) -> Value {
    json!({
        "project_id": project.project_id().to_string(),
        "name": project.name().as_str(),
        "created_at": project.created_at().to_rfc3339(),
        "updated_at": project.updated_at().to_rfc3339(),
        "meeting_count": meeting_count,
        "resource_count": resource_count,
        "latest_meeting_at": latest_meeting_at,
    })
}

/// Serializes a project list entry.
///
/// Outputs: `{"project_id","name","created_at","updated_at","meeting_count","resource_count",
/// "latest_meeting_at"}` (RFC 3339 times; the last is null without meetings). Side effects: None.
pub fn summary_json(summary: &ProjectSummary) -> Value {
    project_json(
        &summary.project,
        summary.meeting_count,
        summary.resource_count,
        summary.latest_meeting_at.map(|time| time.to_rfc3339()),
    )
}

/// Tells how the app shows a resource, from its MIME type.
///
/// Outputs: `image`, `markdown`, `pdf`, `email`, `text`, or `document` (opened outside the app).
/// Side effects: None.
pub fn resource_kind(mime_type: &str) -> &'static str {
    match mime_type {
        image if image.starts_with("image/") => "image",
        "text/markdown" => "markdown",
        "application/pdf" => "pdf",
        "message/rfc822" => "email",
        "text/plain" => "text",
        _ => "document",
    }
}

/// The name a resource is shown with: its stored file name without the `<attachment ID>-`
/// prefix.
pub fn resource_file_name(resource: &Attachment) -> String {
    resource.shown_file_name()
}

/// Serializes one resource.
///
/// Outputs: `{"attachment_id","file_name","mime_type","kind","added_at","size_bytes"}` plus
/// `file_path` when `include_path` is set; `size_bytes` (of the clear contents) is null when the
/// file is not here yet (still syncing). Side effects: Reads the file's record headers.
pub fn resource_json(files: &ProjectFiles, resource: &Attachment, include_path: bool) -> Value {
    let size_bytes = files.clear_len(resource);
    let mut fields = json!({
        "attachment_id": resource.attachment_id().to_string(),
        "file_name": resource_file_name(resource),
        "mime_type": resource.mime_type(),
        "kind": resource_kind(resource.mime_type()),
        "added_at": resource.original_timestamp().to_rfc3339(),
        "size_bytes": size_bytes,
    });
    if include_path && let Value::Object(map) = &mut fields {
        map.insert(
            String::from("file_path"),
            Value::String(resource.file_path().to_owned()),
        );
    }
    fields
}

/// Serializes a parsed e-mail for a viewer: plain text only.
///
/// Outputs: `{"subject","from","to","cc","sent_at","body_text","body_truncated","attachments"}`,
/// people as `{"name","address"}` and attachments as `{"file_name","mime_type","size_bytes"}`.
/// Side effects: None.
pub fn email_json(email: &EmailMessage) -> Value {
    let people = |people: &[EmailPerson]| -> Vec<Value> {
        people
            .iter()
            .map(|person| json!({ "name": person.name, "address": person.address }))
            .collect()
    };
    json!({
        "subject": email.subject,
        "from": people(&email.from),
        "to": people(&email.to),
        "cc": people(&email.cc),
        "sent_at": email.sent_at.map(|time| time.to_rfc3339()),
        "body_text": email.body_text,
        "body_truncated": email.body_truncated,
        "attachments": email.attachments.iter().map(|attachment| json!({
            "file_name": attachment.file_name,
            "mime_type": attachment.mime_type,
            "size_bytes": attachment.contents.len(),
        })).collect::<Vec<Value>>(),
    })
}
