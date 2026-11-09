//! Chat with notes (#97) for the hosts: the app over the C ABI, the daemon's routes and the CLI.
//!
//! A request is JSON tagged by `command`; the scope is `"scope": "meeting" | "project" | "all"`
//! with `"id"` for a meeting or a project:
//!
//! | `command` | Fields | Answer |
//! | --- | --- | --- |
//! | `history` | `scope`, `id`? | `messages` |
//! | `ask` | `scope`, `id`?, `question`, `connection_id`? | `question`, `answer` (messages) |
//! | `clear` | `scope`, `id`? | `cleared` |
//!
//! A message is `{"role","text","created_at","included","omitted","connection"}`. Error codes:
//! `invalid_request`, `empty`, `too_long`, `meeting_not_found`, `project_not_found`,
//! `nothing_to_chat`, `context_too_small`, `no_connection`, `connection_not_found`,
//! `missing_key`, the `llm_*` codes of `llm_commands`, and `storage_failed`.
//!
//! `gather_material` reads what a chat is about: one meeting's transcript, summary, notes and
//! photos; one project's files (text, Markdown, e-mails), images and meetings, newest first; or
//! everything, starting with a list of all meetings and projects, then the meetings and files
//! that best match the question. `ask_with` holds the host's store only while gathering and
//! storing, never during the model's answer. Nothing here logs questions, answers or material.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use crate::{
    application::{
        combined_meetings::MeetingListing,
        export_manager::{ExportManager, ExportWriter},
        meeting_crdt::MeetingCrdtService,
        meeting_summary::{SpokenLine, render_transcript},
        notes_chat::{ChatError, ChatMaterial, ContextItem, ask, relevance},
    },
    domain::{
        chat::{ChatMessage, ChatRole, ChatScope},
        errors::StorageError,
        llm::{LlmImage, MAX_LLM_IMAGE_BYTES},
        models::{ExportFormat, ExportOptions, MeetingId},
        projects::ProjectId,
    },
    llm_commands::{CommandError, LlmServices},
    meeting_summaries::{load_summary, prepare_summary},
    ports::{chat::ChatHistoryRepository, voice::VoiceVectorDB},
    project_commands::{ProjectCommandRepository, ProjectFiles},
};

/// Images sent with a project chat, at most (the newest).
const MAX_PROJECT_IMAGES: usize = 4;

/// A parsed chat request.
#[derive(Clone, Debug)]
pub enum ChatCommand {
    History(ChatScope),
    Ask {
        scope: ChatScope,
        question: String,
        connection_id: Option<String>,
    },
    Clear(ChatScope),
}

/// Reads a request.
///
/// Outputs: The command, or an `invalid_request` error. Side effects: None.
pub fn parse_chat_request(request_json: &str) -> Result<ChatCommand, CommandError> {
    let invalid = |message: &str| CommandError::new("invalid_request", message);
    let request: Value =
        serde_json::from_str(request_json).map_err(|_| invalid("the request is not JSON"))?;
    let scope = ChatScope::parse(
        request.get("scope").and_then(Value::as_str).unwrap_or(""),
        request.get("id").and_then(Value::as_str),
    )
    .ok_or_else(|| invalid("the scope must be meeting or project with an id, or all"))?;
    match request.get("command").and_then(Value::as_str) {
        Some("history") => Ok(ChatCommand::History(scope)),
        Some("clear") => Ok(ChatCommand::Clear(scope)),
        Some("ask") => Ok(ChatCommand::Ask {
            scope,
            question: request
                .get("question")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            connection_id: request
                .get("connection_id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .map(str::to_owned),
        }),
        _ => Err(invalid("the command must be history, ask or clear")),
    }
}

impl From<ChatError> for CommandError {
    fn from(error: ChatError) -> Self {
        let code = match &error {
            ChatError::EmptyQuestion => "empty",
            ChatError::QuestionTooLong => "too_long",
            ChatError::NothingToChatAbout => "nothing_to_chat",
            ChatError::ContextTooSmall => "context_too_small",
            ChatError::Llm(_) => "",
        };
        match error {
            ChatError::Llm(error) => error.into(),
            other => Self::new(code, other.to_string()),
        }
    }
}

/// A storage failure as a command error.
fn storage(error: &dyn std::fmt::Display) -> CommandError {
    CommandError::new(
        "storage_failed",
        format!("the notes could not be read: {error}"),
    )
}

/// A message as JSON.
pub fn message_json(message: &ChatMessage) -> Value {
    json!({
        "role": message.role.as_str(),
        "text": message.text,
        "created_at": message.created_at.to_rfc3339(),
        "included": message.included,
        "omitted": message.omitted,
        "connection": message.connection_label,
    })
}

/// A chat's messages as the `history` answer.
pub fn history_json(messages: &[ChatMessage]) -> Value {
    json!({"ok": true, "messages": messages.iter().map(message_json).collect::<Vec<_>>()})
}

/// One meeting read for a chat.
struct MeetingText {
    title: String,
    started_at: DateTime<Utc>,
    lines: Vec<SpokenLine>,
    notes: Option<String>,
    summary: Option<String>,
}

impl MeetingText {
    /// `Meeting "Title", 2026-11-09 10:00 UTC`.
    fn label(&self) -> String {
        format!(
            "Meeting \"{}\", {}",
            self.title,
            self.started_at.format("%Y-%m-%d %H:%M UTC")
        )
    }

    /// The summary and notes, or `None` when it has neither.
    fn short(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(summary) = self
            .summary
            .as_deref()
            .filter(|text| !text.trim().is_empty())
        {
            parts.push(format!("Summary:\n{}", summary.trim()));
        }
        if let Some(notes) = self.notes.as_deref().filter(|text| !text.trim().is_empty()) {
            parts.push(format!("Notes:\n{}", notes.trim()));
        }
        (!parts.is_empty()).then(|| parts.join("\n\n"))
    }

    /// The summary, notes and transcript.
    fn full(&self) -> String {
        let transcript = render_transcript(&self.lines);
        let mut text = self.short().unwrap_or_default();
        if !transcript.trim().is_empty() {
            if !text.is_empty() {
                text.push_str("\n\n");
            }
            text.push_str("Transcript:\n");
            text.push_str(&transcript);
        }
        text
    }

    fn item(&self) -> ContextItem {
        let full = self.full();
        let short = self.short().filter(|short| short.len() < full.len());
        ContextItem {
            label: self.label(),
            full,
            short,
        }
    }
}

/// Reads a meeting's title, transcript (with saved people's names), notes and summary.
fn read_meeting<Repository, Voices>(
    repository: &Repository,
    voices: &Voices,
    meeting_id: &MeetingId,
) -> Result<MeetingText, CommandError>
where
    Repository: ProjectCommandRepository + ?Sized,
    Voices: VoiceVectorDB + ?Sized,
{
    let writer = ExportWriter::new(Vec::new(), std::env::temp_dir());
    let document = ExportManager::new(repository, repository, voices, &writer)
        .extract_meeting(
            meeting_id,
            &ExportOptions::new(ExportFormat::Markdown, true, false, None),
        )
        .map_err(|error| storage(&error))?;
    Ok(MeetingText {
        title: document.title().to_owned(),
        started_at: document.started_at(),
        lines: document
            .transcript_segments()
            .iter()
            .map(|segment| SpokenLine {
                speaker: segment.speaker().to_owned(),
                start_ms: segment.start_time_ms(),
                text: segment.text().to_owned(),
            })
            .collect(),
        notes: MeetingCrdtService::new(repository)
            .load_note_text(meeting_id)
            .map_err(|error| storage(&error))?,
        summary: load_summary(repository, meeting_id)
            .map_err(|error| storage(&error))?
            .map(|summary| summary.text),
    })
}

/// Whether a meeting is shown (not folded into another, #88) and exists.
fn listed_meetings<Repository>(repository: &Repository) -> Result<Vec<MeetingId>, StorageError>
where
    Repository: ProjectCommandRepository + ?Sized,
{
    let listing = MeetingListing::of(&repository.meeting_recordings()?);
    Ok(repository
        .list_meeting_ids()?
        .into_iter()
        .filter(|meeting_id| listing.is_listed(meeting_id))
        .collect())
}

/// A project file's text as an item, when it has text.
fn file_item(
    files: Option<&ProjectFiles>,
    resource: &crate::domain::models::Attachment,
) -> Option<ContextItem> {
    let text = files?.searchable_text(resource)?;
    (!text.trim().is_empty()).then(|| ContextItem {
        label: format!("File {}", resource.shown_file_name()),
        full: text,
        short: None,
    })
}

/// Reads an image file the model can take.
fn read_image(resource: &crate::domain::models::Attachment) -> Option<LlmImage> {
    let size = std::fs::metadata(resource.file_path()).ok()?.len();
    if size == 0 || size > MAX_LLM_IMAGE_BYTES as u64 {
        return None;
    }
    LlmImage::new(
        resource.mime_type(),
        std::fs::read(resource.file_path()).ok()?,
    )
}

/// Reads what a chat is about.
///
/// Inputs: The repository, the voice library (for speakers' names), the project files (for
/// their text; `None` leaves files out), the scope and the question (to order material for
/// everything). Outputs: The material, or `meeting_not_found`, `project_not_found` or
/// `storage_failed`. Side effects: Reads (and unseals) meetings and files.
pub fn gather_material<Repository, Voices>(
    repository: &Repository,
    voices: &Voices,
    files: Option<&ProjectFiles>,
    scope: &ChatScope,
    question: &str,
) -> Result<ChatMaterial, CommandError>
where
    Repository: ProjectCommandRepository + ?Sized,
    Voices: VoiceVectorDB + ?Sized,
{
    match scope {
        ChatScope::Meeting(meeting_id) => {
            if repository
                .load_meeting_metadata(meeting_id)
                .map_err(|error| storage(&error))?
                .is_none()
            {
                return Err(CommandError::new(
                    "meeting_not_found",
                    "there is no such meeting",
                ));
            }
            let meeting = read_meeting(repository, voices, meeting_id)?;
            // The summary's material holds the meeting's photos and slides, already checked.
            let images = prepare_summary(repository, voices, meeting_id, &[], "")
                .map(|prepared| prepared.material.images)
                .unwrap_or_default();
            Ok(ChatMaterial {
                description: format!("the meeting \"{}\"", meeting.title),
                items: vec![ContextItem {
                    short: None,
                    ..meeting.item()
                }],
                images,
            })
        }
        ChatScope::Project(project_id) => project_material(repository, voices, files, *project_id),
        ChatScope::All => everything_material(repository, voices, files, question),
    }
}

/// A project's files, then its meetings newest first, and its newest images.
fn project_material<Repository, Voices>(
    repository: &Repository,
    voices: &Voices,
    files: Option<&ProjectFiles>,
    project_id: ProjectId,
) -> Result<ChatMaterial, CommandError>
where
    Repository: ProjectCommandRepository + ?Sized,
    Voices: VoiceVectorDB + ?Sized,
{
    let Some(project) = repository
        .load_project(project_id)
        .map_err(|error| storage(&error))?
    else {
        return Err(CommandError::new(
            "project_not_found",
            "there is no such project",
        ));
    };
    let mut items = Vec::new();
    let mut resources = repository
        .project_resources(project_id)
        .map_err(|error| storage(&error))?;
    resources.sort_by_key(|resource| resource.original_timestamp());
    items.extend(
        resources
            .iter()
            .filter_map(|resource| file_item(files, resource)),
    );
    let images: Vec<LlmImage> = resources
        .iter()
        .rev()
        .filter(|resource| resource.mime_type().starts_with("image/"))
        .filter_map(read_image)
        .take(MAX_PROJECT_IMAGES)
        .collect();
    let listed = listed_meetings(repository).map_err(|error| storage(&error))?;
    let mut meetings = Vec::new();
    for meeting_id in repository
        .project_meeting_ids(project_id)
        .map_err(|error| storage(&error))?
    {
        if listed.contains(&meeting_id) {
            meetings.push(read_meeting(repository, voices, &meeting_id)?);
        }
    }
    meetings.sort_by_key(|meeting| std::cmp::Reverse(meeting.started_at));
    items.extend(meetings.iter().map(MeetingText::item));
    Ok(ChatMaterial {
        description: format!("the project \"{}\"", project.name().as_str()),
        items,
        images,
    })
}

/// A list of every meeting and project, then the meetings and files that best match the
/// question (newest first among equals).
fn everything_material<Repository, Voices>(
    repository: &Repository,
    voices: &Voices,
    files: Option<&ProjectFiles>,
    question: &str,
) -> Result<ChatMaterial, CommandError>
where
    Repository: ProjectCommandRepository + ?Sized,
    Voices: VoiceVectorDB + ?Sized,
{
    let projects = repository
        .list_projects()
        .map_err(|error| storage(&error))?;
    let project_names: HashMap<ProjectId, String> = projects
        .iter()
        .map(|summary| {
            (
                summary.project.project_id(),
                summary.project.name().as_str().to_owned(),
            )
        })
        .collect();
    let meeting_projects: HashMap<MeetingId, ProjectId> = repository
        .meeting_projects()
        .map_err(|error| storage(&error))?
        .into_iter()
        .collect();
    let mut meetings = Vec::new();
    for meeting_id in listed_meetings(repository).map_err(|error| storage(&error))? {
        let meeting = read_meeting(repository, voices, &meeting_id)?;
        let project = meeting_projects
            .get(&meeting_id)
            .and_then(|project_id| project_names.get(project_id))
            .cloned();
        meetings.push((meeting, project));
    }
    meetings.sort_by_key(|meeting| std::cmp::Reverse(meeting.0.started_at));

    let mut index = String::from("Meetings, newest first:\n");
    for (meeting, project) in &meetings {
        index.push_str(&format!(
            "- {} \"{}\"{}{}\n",
            meeting.started_at.format("%Y-%m-%d %H:%M UTC"),
            meeting.title,
            project
                .as_deref()
                .map(|name| format!(", project \"{name}\""))
                .unwrap_or_default(),
            if meeting.summary.is_some() {
                ""
            } else {
                ", no summary"
            },
        ));
    }
    index.push_str("\nProjects:\n");
    for summary in &projects {
        index.push_str(&format!(
            "- \"{}\": {} meetings, {} files\n",
            summary.project.name().as_str(),
            summary.meeting_count,
            summary.resource_count
        ));
    }

    // (score, newest first, item)
    let mut ranked: Vec<(usize, DateTime<Utc>, ContextItem)> = meetings
        .iter()
        .map(|(meeting, project)| {
            let mut item = meeting.item();
            if let Some(project) = project {
                item.label.push_str(&format!(", project \"{project}\""));
            }
            let score = relevance(question, &format!("{}\n{}", meeting.title, item.full));
            (score, meeting.started_at, item)
        })
        .collect();
    for summary in &projects {
        for resource in repository
            .project_resources(summary.project.project_id())
            .map_err(|error| storage(&error))?
        {
            if let Some(mut item) = file_item(files, &resource) {
                let score = relevance(question, &format!("{}\n{}", item.label, item.full));
                // Files come in only when they match the question.
                if score > 0 {
                    item.label.push_str(&format!(
                        " in project \"{}\"",
                        summary.project.name().as_str()
                    ));
                    ranked.push((score, resource.original_timestamp(), item));
                }
            }
        }
    }
    ranked.sort_by(|left, right| right.0.cmp(&left.0).then(right.1.cmp(&left.1)));
    let mut items = vec![ContextItem {
        label: String::from("List of all meetings and projects"),
        full: index,
        short: None,
    }];
    items.extend(ranked.into_iter().map(|(_, _, item)| item));
    Ok(ChatMaterial {
        description: String::from("all of their meetings, summaries and projects"),
        items,
        images: Vec::new(),
    })
}

/// Asks a question and keeps it with the answer.
///
/// Inputs: The services, the connection (the default when `None`), the question, `gather`
/// (reads the material and the chat so far, holding the host's store) and `store` (adds the
/// question and answer to the chat). Outputs: `{"ok":true,"question","answer"}` or an error
/// answer. Side effects: Sends the question and the material that fits to the connection's
/// server, then stores both messages. Blocks for as long as the model takes.
pub fn ask_with(
    services: &LlmServices,
    connection_id: Option<&str>,
    question: &str,
    gather: impl FnOnce() -> Result<(ChatMaterial, Vec<ChatMessage>), CommandError>,
    store: impl FnOnce(&[ChatMessage]) -> Result<(), CommandError>,
) -> Value {
    let run = || -> Result<Value, CommandError> {
        if question.trim().is_empty() {
            return Err(ChatError::EmptyQuestion.into());
        }
        let settings = services.settings();
        let connection = match connection_id {
            Some(id) => settings.connection(id).ok_or_else(|| {
                CommandError::new("connection_not_found", "there is no such connection")
            })?,
            None => settings.default_connection().ok_or_else(|| {
                CommandError::new(
                    "no_connection",
                    "add an AI connection in Settings, or choose a default one",
                )
            })?,
        };
        let engine = services.engine(connection)?;
        let (material, history) = gather()?;
        let asked_at = Utc::now();
        let reply = ask(engine.as_ref(), connection, material, &history, question)?;
        let messages = [
            ChatMessage::question(question.trim(), asked_at),
            ChatMessage {
                role: ChatRole::Assistant,
                text: reply.text,
                created_at: Utc::now(),
                included: reply.included,
                omitted: reply.omitted,
                connection_label: Some(connection.label().to_owned()),
            },
        ];
        store(&messages)?;
        Ok(json!({
            "ok": true,
            "question": message_json(&messages[0]),
            "answer": message_json(&messages[1]),
            "truncated": reply.truncated,
            "images_sent": reply.images_sent,
        }))
    };
    run().unwrap_or_else(|error| error.to_json())
}

/// Runs `history` or `clear` against a chat store.
///
/// Outputs: The JSON answer. Side effects: `clear` deletes the chat.
pub fn run_chat_store_command<Store>(store: &Store, command: &ChatCommand) -> Value
where
    Store: ChatHistoryRepository + ?Sized,
{
    match command {
        ChatCommand::History(scope) => match store.chat_history(scope) {
            Ok(messages) => history_json(&messages),
            Err(error) => storage(&error).to_json(),
        },
        ChatCommand::Clear(scope) => match store.clear_chat(scope) {
            Ok(cleared) => json!({"ok": true, "cleared": cleared}),
            Err(error) => storage(&error).to_json(),
        },
        ChatCommand::Ask { .. } => {
            CommandError::new("invalid_request", "ask needs a language model").to_json()
        }
    }
}

/// Reads the material and the chat so far for `ask_with`'s `gather` step.
///
/// Side effects: Reads (and unseals) the material and the chat.
pub fn gather_for_ask<Repository, Voices>(
    repository: &Repository,
    voices: &Voices,
    files: Option<&ProjectFiles>,
    scope: &ChatScope,
    question: &str,
) -> Result<(ChatMaterial, Vec<ChatMessage>), CommandError>
where
    Repository: ProjectCommandRepository + ChatHistoryRepository + ?Sized,
    Voices: VoiceVectorDB + ?Sized,
{
    let material = gather_material(repository, voices, files, scope, question)?;
    let history = repository
        .chat_history(scope)
        .map_err(|error| storage(&error))?;
    Ok((material, history))
}

/// Adds a question and its answer to the chat, for `ask_with`'s `store` step.
pub fn store_messages<Store>(
    store: &Store,
    scope: &ChatScope,
    messages: &[ChatMessage],
) -> Result<(), CommandError>
where
    Store: ChatHistoryRepository + ?Sized,
{
    store
        .append_chat_messages(scope, messages)
        .map_err(|error| CommandError::new("storage_failed", error.to_string()))
}
