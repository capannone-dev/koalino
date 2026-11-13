//! A read-only MCP server over the local notes, for the owner's own AI assistant (#112).
//!
//! The Model Context Protocol is JSON-RPC 2.0, here over stdio: one message per line in, one
//! reply per line out. The assistant (Claude Code, Codex, Claude Desktop...) starts the
//! `note_taker_mcp` binary and asks it, through these tools:
//!
//! | Tool | Arguments | Answer |
//! | --- | --- | --- |
//! | `list_meetings` | `project_id`?, `limit`? (50, at most 500) | `meetings` (newest first), `total` |
//! | `read_meeting` | `meeting_id`, `include_transcript`? (true) | title, start, summary, notes, transcript |
//! | `search` | `query`, `scope`? (`meetings`/`projects`/`project`), `project_id`? | `search_commands`' hits |
//! | `list_projects` | none | `projects` |
//! | `read_project` | `project_id` | the project, its meetings and its files (no paths) |
//! | `read_project_file` | `project_id`, `attachment_id` | the file's `text` (text, Markdown, e-mail) |
//!
//! Nothing is written: no tool changes a note, and the server opens no network listener. It is
//! off until the owner turns it on (`<db>.assistant-access.json`), and the switch is read at
//! every call, so turning it off takes effect at once; while off, each call answers how to turn
//! it on. What the assistant reads may go to its online model, which the setting says. Logs name
//! the tool and whether it worked, never arguments or results.

use std::{
    fs,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    domain::{errors::StorageError, models::MeetingId},
    notes_chat_commands::{listed_meetings, meeting_json},
    ports::voice::VoiceVectorDB,
    project_commands::{
        ProjectCommandError, ProjectCommandRepository, ProjectFiles, existing_project,
        list_projects, project_detail,
    },
    search_commands::{HostSearchSources, SearchRequest, run_search, speaker_names},
};

/// The protocol versions this server speaks, newest first.
const PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];
/// Meetings listed when the assistant gives no limit.
const DEFAULT_MEETING_LIMIT: u64 = 50;
/// The most meetings one list returns.
const MAX_MEETING_LIMIT: u64 = 500;
/// What the assistant is told when access is off.
const ACCESS_OFF: &str = "Reading notes is turned off. The owner can turn it on in Koalino: \
Settings > AI assistant access.";

/// Whether the owner lets their AI assistant read the notes on this device.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AssistantAccessSettings {
    /// Off by default.
    #[serde(default)]
    pub enabled: bool,
}

impl AssistantAccessSettings {
    /// The settings file beside a database: `<db>.assistant-access.json`.
    ///
    /// Side effects: None.
    pub fn file_for(database_path: &Path) -> PathBuf {
        let mut name = database_path.as_os_str().to_owned();
        name.push(".assistant-access.json");
        PathBuf::from(name)
    }

    /// Reads the setting; a missing or unreadable file means off.
    ///
    /// Side effects: Reads the file.
    pub fn load(file: &Path) -> Self {
        fs::read(file)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Saves the setting.
    ///
    /// Outputs: An I/O error when the file cannot be written. Side effects: Replaces the file
    /// through a temporary one, so a crash never leaves half a file.
    pub fn save(&self, file: &Path) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        let mut temporary = file.as_os_str().to_owned();
        temporary.push(".tmp");
        fs::write(&temporary, bytes)?;
        fs::rename(&temporary, file)
    }
}

/// The server binary's file name, beside the app's executable.
pub const SERVER_FILE_NAME: &str = if cfg!(windows) {
    "note_taker_mcp.exe"
} else {
    "note_taker_mcp"
};

/// Runs one Settings command for assistant access (`get`, or `set` with `enabled`).
///
/// Inputs: The app's database, the JSON request, the server binary when it is installed, and
/// whether the app unlocks its key with a passphrase (which the server would need too, and which
/// is never written into an assistant's configuration). Outputs: `{"ok":true,"enabled",
/// "database_path","server_path","config","claude_code","passphrase_needed"}`, where `config` is
/// the `mcpServers` entry for Claude Desktop and similar clients and `claude_code` the matching
/// `claude mcp add` line (both null without the binary); or `{"ok":false,"error_code","error"}`
/// with `invalid_request` or `storage_failed`. Side effects: `set` saves the setting.
pub fn access_command(
    database_path: &Path,
    request_json: &str,
    server_path: Option<&Path>,
    passphrase_needed: bool,
) -> Value {
    let failure =
        |code: &str, message: &str| json!({"ok": false, "error_code": code, "error": message});
    let Ok(request) = serde_json::from_str::<Value>(request_json) else {
        return failure("invalid_request", "the request is not JSON");
    };
    let file = AssistantAccessSettings::file_for(database_path);
    match request.get("command").and_then(Value::as_str) {
        Some("get") => {}
        Some("set") => {
            let Some(enabled) = request.get("enabled").and_then(Value::as_bool) else {
                return failure("invalid_request", "`set` needs `enabled`");
            };
            if (AssistantAccessSettings { enabled }).save(&file).is_err() {
                return failure("storage_failed", "the setting could not be saved");
            }
            tracing::info!(enabled, "assistant access changed");
        }
        _ => return failure("invalid_request", "the command must be get or set"),
    }
    let database = database_path.to_string_lossy();
    let (config, claude_code) = match server_path {
        Some(server_path) => {
            let server = server_path.to_string_lossy();
            (
                json!({"mcpServers": {"koalino": {
                    "command": server,
                    "args": [],
                    "env": {"NOTE_TAKER_DATABASE_PATH": database},
                }}}),
                Value::String(format!(
                    "claude mcp add koalino --env {} -- {}",
                    quoted(&format!("NOTE_TAKER_DATABASE_PATH={database}")),
                    quoted(&server)
                )),
            )
        }
        None => (Value::Null, Value::Null),
    };
    json!({
        "ok": true,
        "enabled": AssistantAccessSettings::load(&file).enabled,
        "database_path": database,
        "server_path": server_path.map(|path| path.to_string_lossy().into_owned()),
        "config": config,
        "claude_code": claude_code,
        "passphrase_needed": passphrase_needed,
    })
}

/// Quotes a command-line word holding spaces.
fn quoted(word: &str) -> String {
    if word.contains(char::is_whitespace) {
        format!("\"{word}\"")
    } else {
        word.to_owned()
    }
}

/// A tool's failure: a stable code and a message free of note content.
struct ToolError {
    code: &'static str,
    message: String,
}

impl ToolError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl From<ProjectCommandError> for ToolError {
    fn from(error: ProjectCommandError) -> Self {
        Self::new(error.error_code, error.message)
    }
}

impl From<StorageError> for ToolError {
    fn from(_: StorageError) -> Self {
        Self::new("storage_failed", "the notes could not be read")
    }
}

/// A JSON-RPC failure: its code and message.
struct RpcError {
    code: i64,
    message: String,
}

impl RpcError {
    fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Answers MCP requests from one device's notes.
pub struct McpServer<'host, Repository: ?Sized, Voices: ?Sized> {
    repository: &'host Repository,
    voices: &'host Voices,
    files: &'host ProjectFiles,
    /// Whether access is on, asked at every tool call.
    access_enabled: Box<dyn Fn() -> bool + 'host>,
}

impl<'host, Repository, Voices> McpServer<'host, Repository, Voices>
where
    Repository: ProjectCommandRepository + ?Sized,
    Voices: VoiceVectorDB + ?Sized,
{
    /// Creates the server.
    ///
    /// Inputs: The note repository, the voice library (for saved people's names), the project
    /// files (for their text) and the access switch, asked at every tool call.
    /// Outputs: The server. Side effects: None.
    pub fn new(
        repository: &'host Repository,
        voices: &'host Voices,
        files: &'host ProjectFiles,
        access_enabled: Box<dyn Fn() -> bool + 'host>,
    ) -> Self {
        Self {
            repository,
            voices,
            files,
            access_enabled,
        }
    }

    /// Answers one JSON-RPC message.
    ///
    /// Inputs: One line of JSON. Outputs: The reply line, or `None` for a notification (a
    /// message without `id`). Side effects: Tool calls read (and unseal) notes when access is on.
    pub fn handle(&self, line: &str) -> Option<String> {
        let message: Value = match serde_json::from_str(line) {
            Ok(message) => message,
            Err(_) => {
                return Some(error_reply(
                    &Value::Null,
                    &RpcError::new(-32700, "the message is not JSON"),
                ));
            }
        };
        let id = message.get("id")?.clone();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let outcome = match message.get("method").and_then(Value::as_str) {
            Some("initialize") => Ok(initialize(&params)),
            Some("ping") => Ok(json!({})),
            Some("tools/list") => Ok(json!({ "tools": tools() })),
            Some("tools/call") => self.call_tool(&params),
            Some(_) => Err(RpcError::new(-32601, "unknown method")),
            None => Err(RpcError::new(-32600, "the request has no method")),
        };
        Some(match outcome {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string(),
            Err(error) => error_reply(&id, &error),
        })
    }

    /// Runs `tools/call`: a tool's failure is a result marked `isError`, an unknown tool a
    /// JSON-RPC error.
    fn call_tool(&self, params: &Value) -> Result<Value, RpcError> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| RpcError::new(-32602, "the call names no tool"))?;
        if !TOOL_NAMES.contains(&name) {
            return Err(RpcError::new(-32602, "unknown tool"));
        }
        if !(self.access_enabled)() {
            tracing::info!(tool = name, "assistant read refused: access is off");
            return Ok(tool_result(ACCESS_OFF, true));
        }
        let empty = json!({});
        let arguments = params.get("arguments").unwrap_or(&empty);
        let outcome = match name {
            "list_meetings" => self.list_meetings(arguments),
            "read_meeting" => self.read_meeting(arguments),
            "search" => self.search(arguments),
            "list_projects" => list_projects(self.repository)
                .map(|projects| json!({ "projects": projects }))
                .map_err(ToolError::from),
            "read_project" => required(arguments, "project_id").and_then(|project_id| {
                project_detail(self.repository, self.files, project_id, false)
                    .map_err(ToolError::from)
            }),
            _ => self.read_project_file(arguments),
        };
        tracing::info!(tool = name, ok = outcome.is_ok(), "assistant read");
        Ok(match outcome {
            Ok(value) => tool_result(
                &serde_json::to_string_pretty(&value).unwrap_or_default(),
                false,
            ),
            Err(error) => tool_result(&format!("{}: {}", error.code, error.message), true),
        })
    }

    /// Lists shown meetings, newest first, optionally of one project.
    fn list_meetings(&self, arguments: &Value) -> Result<Value, ToolError> {
        let limit = arguments
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_MEETING_LIMIT)
            .clamp(1, MAX_MEETING_LIMIT) as usize;
        let project_filter = match arguments.get("project_id").and_then(Value::as_str) {
            Some(project_id) => Some(existing_project(self.repository, project_id)?.project_id()),
            None => None,
        };
        let projects: std::collections::HashMap<MeetingId, String> = self
            .repository
            .meeting_projects()?
            .into_iter()
            .map(|(meeting_id, project_id)| (meeting_id, project_id.to_string()))
            .collect();
        let wanted_project = project_filter.map(|project_id| project_id.to_string());
        let mut meetings = Vec::new();
        for meeting_id in listed_meetings(self.repository)? {
            let project_id = projects.get(&meeting_id);
            if wanted_project.is_some() && project_id != wanted_project.as_ref() {
                continue;
            }
            if let Some(metadata) = self.repository.load_meeting_metadata(&meeting_id)? {
                meetings.push((metadata, project_id.cloned()));
            }
        }
        meetings.sort_by_key(|(metadata, _)| std::cmp::Reverse(metadata.created_at()));
        let total = meetings.len();
        let listed: Vec<Value> = meetings
            .into_iter()
            .take(limit)
            .map(|(metadata, project_id)| {
                json!({
                    "meeting_id": metadata.meeting_id().as_str(),
                    "title": metadata.display_title(),
                    "created_at": metadata.created_at().to_rfc3339(),
                    "project_id": project_id,
                })
            })
            .collect();
        Ok(json!({ "meetings": listed, "total": total }))
    }

    /// Reads one meeting.
    fn read_meeting(&self, arguments: &Value) -> Result<Value, ToolError> {
        let meeting_id = MeetingId::new(required(arguments, "meeting_id")?.trim());
        let include_transcript = arguments
            .get("include_transcript")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        meeting_json(
            self.repository,
            self.voices,
            &meeting_id,
            include_transcript,
        )
        .map_err(|error| ToolError::new(error.code, error.message))
    }

    /// Searches meetings or projects.
    fn search(&self, arguments: &Value) -> Result<Value, ToolError> {
        let request = SearchRequest {
            query: required(arguments, "query")?.to_owned(),
            scope: arguments
                .get("scope")
                .and_then(Value::as_str)
                .map(str::to_owned),
            project_id: arguments
                .get("project_id")
                .and_then(Value::as_str)
                .map(str::to_owned),
        };
        let sources = HostSearchSources::new(speaker_names(self.voices)?, Some(self.files));
        let answer = run_search(self.repository, &sources, &request);
        if answer["ok"] == false {
            return Err(ToolError::new(
                static_code(answer["error_code"].as_str()),
                answer["error"].as_str().unwrap_or("the search failed"),
            ));
        }
        Ok(answer)
    }

    /// Reads a project file's text.
    fn read_project_file(&self, arguments: &Value) -> Result<Value, ToolError> {
        let project = existing_project(self.repository, required(arguments, "project_id")?)?;
        let attachment_id = required(arguments, "attachment_id")?.trim();
        let resource = self
            .repository
            .project_resources(project.project_id())?
            .into_iter()
            .find(|resource| resource.attachment_id().to_string() == attachment_id)
            .ok_or_else(|| ToolError::new("resource_not_found", "the project has no such file"))?;
        let text = self.files.searchable_text(&resource).ok_or_else(|| {
            ToolError::new(
                "no_text",
                "the file has no text to read here (only text, Markdown and e-mail files do)",
            )
        })?;
        Ok(json!({
            "attachment_id": attachment_id,
            "mime_type": resource.mime_type(),
            "text": text,
        }))
    }
}

/// Answers each line read from `input` on `output`, until the input ends.
///
/// Inputs: The server, the client's messages (one per line; blank lines are skipped) and where
/// replies go. Outputs: An I/O error when reading or writing fails. Side effects: Writes and
/// flushes one line per reply.
pub fn serve<Repository, Voices>(
    server: &McpServer<'_, Repository, Voices>,
    input: impl BufRead,
    mut output: impl Write,
) -> io::Result<()>
where
    Repository: ProjectCommandRepository + ?Sized,
    Voices: VoiceVectorDB + ?Sized,
{
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = server.handle(&line) {
            writeln!(output, "{reply}")?;
            output.flush()?;
        }
    }
    Ok(())
}

/// The `initialize` result: the client's protocol version when spoken here, else the newest.
fn initialize(params: &Value) -> Value {
    let requested = params.get("protocolVersion").and_then(Value::as_str);
    let version = requested
        .filter(|version| PROTOCOL_VERSIONS.contains(version))
        .unwrap_or(PROTOCOL_VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "koalino", "version": env!("CARGO_PKG_VERSION") },
        "instructions": "Read-only access to the owner's Koalino meetings and projects: \
    transcripts, summaries, notes and project files. Start with search or list_meetings, then \
    read_meeting for one meeting. Nothing can be changed through these tools.",
    })
}

/// The tools' names, in the order they are listed.
const TOOL_NAMES: &[&str] = &[
    "list_meetings",
    "read_meeting",
    "search",
    "list_projects",
    "read_project",
    "read_project_file",
];

/// The tool list.
fn tools() -> Vec<Value> {
    let tool = |name: &str, title: &str, description: &str, properties: Value, needed: &[&str]| {
        json!({
            "name": name,
            "title": title,
            "description": description,
            "inputSchema": {
                "type": "object",
                "properties": properties,
                "required": needed,
                "additionalProperties": false,
            },
            "annotations": {
                "title": title,
                "readOnlyHint": true,
                "destructiveHint": false,
                "idempotentHint": true,
                "openWorldHint": false,
            },
        })
    };
    let text = |description: &str| json!({"type": "string", "description": description});
    vec![
        tool(
            "list_meetings",
            "List meetings",
            "Lists recorded meetings, newest first, with their IDs, titles, start times and \
projects.",
            json!({
                "project_id": text("Only the meetings of this project."),
                "limit": {"type": "integer", "minimum": 1, "maximum": MAX_MEETING_LIMIT,
                          "description": "How many to list (50 by default)."},
            }),
            &[],
        ),
        tool(
            "read_meeting",
            "Read a meeting",
            "Reads one meeting: its title, start time, summary, notes and transcript, as \
'[hh:mm:ss] Speaker: text' turns.",
            json!({
                "meeting_id": text("The meeting's ID, from list_meetings or search."),
                "include_transcript": {"type": "boolean",
                    "description": "Set false to read only the summary and notes."},
            }),
            &["meeting_id"],
        ),
        tool(
            "search",
            "Search notes",
            "Finds where words were said or written: meeting titles, summaries, notes and \
transcripts, or project names and files. Case and accents are ignored; every word must be in \
the same passage.",
            json!({
                "query": text("The words to find."),
                "scope": {"type": "string", "enum": ["meetings", "projects", "project"],
                          "description": "Where to search (meetings by default)."},
                "project_id": text("The project to search, for the project scope."),
            }),
            &["query"],
        ),
        tool(
            "list_projects",
            "List projects",
            "Lists the projects with their IDs, names and counts of meetings and files.",
            json!({}),
            &[],
        ),
        tool(
            "read_project",
            "Read a project",
            "Lists one project's meetings and files.",
            json!({ "project_id": text("The project's ID, from list_projects.") }),
            &["project_id"],
        ),
        tool(
            "read_project_file",
            "Read a project file",
            "Reads the text of a project's text, Markdown or e-mail file.",
            json!({
                "project_id": text("The project's ID."),
                "attachment_id": text("The file's attachment_id, from read_project."),
            }),
            &["project_id", "attachment_id"],
        ),
    ]
}

/// A string argument the tool needs.
fn required<'value>(arguments: &'value Value, name: &str) -> Result<&'value str, ToolError> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ToolError::new("invalid_request", format!("`{name}` is needed")))
}

/// A search error code as a static string.
fn static_code(code: Option<&str>) -> &'static str {
    match code {
        Some("empty") => "empty",
        Some("too_short") => "too_short",
        Some("too_long") => "too_long",
        Some("project_not_found") => "project_not_found",
        Some("invalid_request") => "invalid_request",
        _ => "storage_failed",
    }
}

/// A tool result holding one text.
fn tool_result(text: &str, is_error: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

/// A JSON-RPC error reply line.
fn error_reply(id: &Value, error: &RpcError) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": error.code, "message": error.message },
    })
    .to_string()
}
