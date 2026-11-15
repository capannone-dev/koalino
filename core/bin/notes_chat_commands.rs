//! Chat with notes (#97) for the hosts: the app over the C ABI, the daemon's routes and the CLI.
//!
//! A request is JSON tagged by `command`; the scope is `"scope": "meeting" | "project" | "all"`
//! with `"id"` for a meeting or a project:
//!
//! | `command` | Fields | Answer |
//! | --- | --- | --- |
//! | `history` | `scope`, `id`? | `messages` |
//! | `ask` | `scope`, `id`?, `question`, `connection_id`?, `on_paired_computer`? | `question`, `answer` (messages) |
//! | `clear` | `scope`, `id`? | `cleared` |
//!
//! A message is `{"role","text","created_at","included","omitted","connection"}`. Error codes:
//! `invalid_request`, `empty`, `too_long`, `meeting_not_found`, `project_not_found`,
//! `nothing_to_chat`, `context_too_small`, `no_connection`, `connection_not_found`,
//! the `llm_*` codes of `llm_commands`, and `storage_failed`. With `on_paired_computer` (#107),
//! a paired computer answers: `no_paired_computer`, `paired_computer_timeout`,
//! `paired_computer_left`, `rate_limited`, `busy`, `p2p_unavailable`, or the computer's own code.
//!
//! `gather_material` reads what a chat is about: one meeting's transcript, summary, notes and
//! photos and files' text; one project's files (text, e-mails, PDF and office documents, #128),
//! images and meetings, newest first, each meeting followed by its files; or
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
        meeting_summary::{AttachedDocument, SpokenLine, render_transcript},
        notes_chat::{ChatError, ChatMaterial, ContextItem, ask, relevance},
        paired_chat_exchange::{
            PairedChatAnswer, PairedChatOutcome, PairedChatRequest, PairedChatTurn,
        },
        paired_chat_relay::PairedChatResult,
    },
    domain::{
        chat::{ChatMessage, ChatRole, ChatScope, ChatSource, ChatTarget},
        errors::StorageError,
        llm::LlmImage,
        models::{ExportFormat, ExportOptions, MeetingId},
        projects::ProjectId,
    },
    llm_commands::{CommandError, LlmServices},
    meeting_summaries::{MAX_LLM_DOCUMENTS, load_summary, prepare_summary, read_document},
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
        /// Ask a paired computer instead of a connection on this device (#107).
        on_paired_computer: bool,
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
            on_paired_computer: request
                .get("on_paired_computer")
                .and_then(Value::as_bool)
                .unwrap_or(false),
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
        "sources": message.sources.iter().map(source_json).collect::<Vec<_>>(),
    })
}

/// One cited place: `{"number","label","at_ms","meeting_id"}` for a meeting, or
/// `{"number","label","at_ms","project_id","resource_id"}` for a file (#110).
fn source_json(source: &ChatSource) -> Value {
    let mut value = json!({
        "number": source.number,
        "label": source.label,
        "at_ms": source.at_ms,
    });
    match &source.target {
        Some(ChatTarget::Meeting { meeting_id }) => {
            value["meeting_id"] = json!(meeting_id);
        }
        Some(ChatTarget::File {
            project_id,
            resource_id,
        }) => {
            value["project_id"] = json!(project_id);
            value["resource_id"] = json!(resource_id);
        }
        None => {}
    }
    value
}

/// A chat's messages as the `history` answer.
pub fn history_json(messages: &[ChatMessage]) -> Value {
    json!({"ok": true, "messages": messages.iter().map(message_json).collect::<Vec<_>>()})
}

/// One meeting read for a chat.
struct MeetingText {
    /// The meeting's ID, which a citation opens (#110).
    meeting_id: String,
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
            target: Some(ChatTarget::Meeting {
                meeting_id: self.meeting_id.clone(),
            }),
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
        meeting_id: meeting_id.as_str().to_owned(),
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

/// Reads a meeting for the assistant's MCP tools (#112).
///
/// Inputs: The repository, the voice library (for saved people's names), the meeting's ID, and
/// whether to include the transcript. Outputs: `{"meeting_id","title","started_at","summary",
/// "notes","transcript"}` (absent parts are null; the transcript as `[hh:mm:ss] Speaker: text`
/// turns), or `meeting_not_found` / `storage_failed`. Side effects: Reads (and unseals) the
/// meeting.
pub fn meeting_json<Repository, Voices>(
    repository: &Repository,
    voices: &Voices,
    meeting_id: &MeetingId,
    include_transcript: bool,
) -> Result<Value, CommandError>
where
    Repository: ProjectCommandRepository + ?Sized,
    Voices: VoiceVectorDB + ?Sized,
{
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
    let transcript = include_transcript.then(|| render_transcript(&meeting.lines));
    Ok(json!({
        "meeting_id": meeting.meeting_id,
        "title": meeting.title,
        "started_at": meeting.started_at.to_rfc3339(),
        "summary": meeting.summary,
        "notes": meeting.notes,
        "transcript": transcript,
    }))
}

/// Whether a meeting is shown (not folded into another, #88) and exists.
pub fn listed_meetings<Repository>(repository: &Repository) -> Result<Vec<MeetingId>, StorageError>
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
        target: Some(ChatTarget::File {
            project_id: resource
                .project_id()
                .map(|project_id| project_id.to_string()),
            resource_id: resource.attachment_id().to_string(),
        }),
    })
}

/// A meeting's file as chat material, which a citation opens through the meeting (#128).
///
/// Inputs: The meeting, the file's text, and whether to name the meeting in the label (in a
/// project's chat). Side effects: None.
fn meeting_file_item(
    meeting: &MeetingText,
    document: AttachedDocument,
    name_the_meeting: bool,
) -> ContextItem {
    let label = if name_the_meeting {
        format!("File {}, meeting \"{}\"", document.name, meeting.title)
    } else {
        format!("File {}", document.name)
    };
    ContextItem {
        label,
        full: document.text,
        short: None,
        target: Some(ChatTarget::Meeting {
            meeting_id: meeting.meeting_id.clone(),
        }),
    }
}

/// The text of a meeting's stored files, oldest first, at most `MAX_LLM_DOCUMENTS` (#128).
///
/// Side effects: Reads and decrypts the files.
fn meeting_documents<Repository>(
    repository: &Repository,
    attachments: &[crate::domain::models::Attachment],
) -> Vec<AttachedDocument>
where
    Repository: ProjectCommandRepository + ?Sized,
{
    let mut readable: Vec<_> = attachments.iter().collect();
    readable.sort_by_key(|attachment| attachment.original_timestamp());
    readable
        .into_iter()
        .filter_map(|attachment| {
            read_document(
                repository,
                std::path::Path::new(attachment.file_path()),
                attachment.mime_type(),
                &attachment.shown_file_name(),
            )
        })
        .take(MAX_LLM_DOCUMENTS)
        .collect()
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
            // The summary's material holds the meeting's photos and slides, already checked,
            // and its files' text (#128).
            let (images, documents) =
                prepare_summary(repository, voices, meeting_id, &[], "", None)
                    .map(|prepared| (prepared.material.images, prepared.material.documents))
                    .unwrap_or_default();
            let mut items = vec![ContextItem {
                short: None,
                ..meeting.item()
            }];
            items.extend(
                documents
                    .into_iter()
                    .map(|document| meeting_file_item(&meeting, document, false)),
            );
            Ok(ChatMaterial {
                description: format!("the meeting \"{}\"", meeting.title),
                items,
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
        .filter_map(|resource| {
            crate::meeting_summaries::read_image(
                repository,
                std::path::Path::new(resource.file_path()),
                resource.mime_type(),
            )
        })
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
    for meeting in &meetings {
        items.push(meeting.item());
        // Each meeting's files follow it (#128).
        let attachments = repository
            .get_attachments_for_meeting(&MeetingId::new(meeting.meeting_id.as_str()))
            .map_err(|error| storage(&error))?;
        items.extend(
            meeting_documents(repository, &attachments)
                .into_iter()
                .map(|document| meeting_file_item(meeting, document, true)),
        );
    }
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
        target: None,
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
                sources: reply.sources,
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

/// Reports whether this computer answers its paired devices' chat questions (#107): the owner
/// chose a connection under "Summaries for my other devices".
///
/// Side effects: Reads the settings file.
pub fn offers_paired_chat(services: &LlmServices) -> bool {
    services.settings().paired_summary_connection().is_some()
}

/// Answers a paired device's chat question from this computer's own copy of the notes (#107).
///
/// Inputs: The services, the question with its scope and recent turns, and `gather` (reads the
/// material for a scope and question, holding the host's store only while it reads).
/// Outputs: `Answered`, labelled "<connection> on <this computer>", or `Declined` with an error
/// code: `not_offered` when no connection is chosen for paired devices, else the `ask` codes
/// (`nothing_to_chat`, `llm_unreachable`, ...). Side effects: Sends the question, the turns and
/// the material that fits to the chosen connection's server; blocks for as long as the model
/// takes. Nothing is stored here: the asking device keeps its own chat. Nothing is logged.
pub fn answer_for_paired_device(
    services: &LlmServices,
    request: &PairedChatRequest,
    gather: impl FnOnce(&ChatScope, &str) -> Result<ChatMaterial, CommandError>,
) -> PairedChatOutcome {
    let declined = |error: CommandError| PairedChatOutcome::Declined {
        error_code: error.code.to_owned(),
    };
    let settings = services.settings();
    let Some(connection) = settings.paired_summary_connection() else {
        return PairedChatOutcome::Declined {
            error_code: "not_offered".to_owned(),
        };
    };
    let engine = match services.engine(connection) {
        Ok(engine) => engine,
        Err(error) => return declined(error),
    };
    let material = match gather(&request.scope, &request.question) {
        Ok(material) => material,
        Err(error) => return declined(error),
    };
    let asked_at = Utc::now();
    let history: Vec<ChatMessage> = request
        .history
        .iter()
        .map(|turn| ChatMessage {
            role: turn.role,
            ..ChatMessage::question(&turn.text, asked_at)
        })
        .collect();
    match ask(
        engine.as_ref(),
        connection,
        material,
        &history,
        &request.question,
    ) {
        Ok(reply) => PairedChatOutcome::Answered(PairedChatAnswer {
            text: reply.text,
            included: reply.included,
            omitted: reply.omitted,
            sources: reply.sources,
            connection_label: format!("{} on {}", connection.label(), services.device_name()),
            truncated: reply.truncated,
            images_sent: u32::try_from(reply.images_sent).unwrap_or(u32::MAX),
        }),
        Err(error) => declined(error.into()),
    }
}

/// Asks a paired computer and keeps its answer in this device's chat (#107).
///
/// Inputs: The question, `history` (reads this device's chat so far), `ask` (sends the trimmed
/// question and the turns to the paired computers and waits; see `P2pHandle::ask_paired_computer`)
/// and `store` (adds the question and answer to this device's chat).
/// Outputs: `{"ok":true,"question","answer"}` as `ask_with`, or `{"ok":false,"error_code","error"}`.
/// Side effects: Sends the question and the turns, sealed, to a paired computer, which reads its
/// own copy of the notes; on an answer, stores both messages here. Blocks for minutes at most.
pub fn ask_paired_computer_with(
    question: &str,
    history: impl FnOnce() -> Result<Vec<ChatMessage>, CommandError>,
    ask: impl FnOnce(String, Vec<PairedChatTurn>) -> PairedChatResult,
    store: impl FnOnce(&[ChatMessage]) -> Result<(), CommandError>,
) -> Value {
    let question = question.trim();
    if question.is_empty() {
        return CommandError::from(ChatError::EmptyQuestion).to_json();
    }
    let turns = match history() {
        Ok(messages) => messages
            .into_iter()
            .map(|message| PairedChatTurn {
                role: message.role,
                text: message.text,
            })
            .collect(),
        Err(error) => return error.to_json(),
    };
    let asked_at = Utc::now();
    let answer = match ask(question.to_owned(), turns) {
        Ok(answer) => answer,
        Err(code) => {
            return json!({"ok": false, "error": paired_chat_problem(&code), "error_code": code});
        }
    };
    let messages = [
        ChatMessage::question(question, asked_at),
        ChatMessage {
            role: ChatRole::Assistant,
            text: answer.text,
            created_at: Utc::now(),
            included: answer.included,
            omitted: answer.omitted,
            connection_label: Some(answer.connection_label),
            sources: answer.sources,
        },
    ];
    if let Err(error) = store(&messages) {
        return error.to_json();
    }
    json!({
        "ok": true,
        "question": message_json(&messages[0]),
        "answer": message_json(&messages[1]),
        "truncated": answer.truncated,
        "images_sent": answer.images_sent,
    })
}

/// A message for people about a paired computer's error code.
fn paired_chat_problem(code: &str) -> String {
    match code {
        "no_paired_computer" => String::from(
            "No paired computer answered. Turn on \"Summaries for my other devices\" in a computer's AI settings, and keep it on and connected.",
        ),
        "paired_computer_timeout" => String::from("The computer took too long to answer."),
        "paired_computer_left" => String::from("The computer disconnected before answering."),
        "rate_limited" => {
            String::from("The computer answers at most 30 questions an hour; try again later.")
        }
        "busy" => String::from("The computer is answering other questions; try again shortly."),
        "p2p_unavailable" => String::from("Paired devices are off on this device."),
        other => format!("The computer could not answer ({other})."),
    }
}
