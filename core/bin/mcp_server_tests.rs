//! Specifies the read-only MCP server (#112): the JSON-RPC exchange, the tools and what they
//! read, and the switch that keeps it off until the owner turns it on.

use std::{cell::Cell, path::PathBuf, sync::Arc};

use serde_json::{Value, json};

use super::{
    mcp_server::{AssistantAccessSettings, McpServer, access_command, serve},
    project_commands::ProjectFiles,
};
use crate::{
    adapters::storage::{
        in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
    },
    application::{
        crypto::voice_encryptor::ephemeral_cipher, meeting_crdt::MeetingCrdtService,
        meeting_document::AutomergeMeetingDocumentCodec, meeting_edits::set_notes,
    },
    domain::{
        models::{DeviceId, MeetingId, MeetingTitle, TranscriptSegment},
        projects::ProjectName,
    },
    ports::{projects::ProjectRepository, storage::NoteRepository},
};

struct Fixture {
    repository: SqliteNoteRepository,
    voices: InMemoryVoiceVectorDB,
    files: ProjectFiles,
    folder: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.folder);
    }
}

fn fixture() -> Fixture {
    let folder = std::env::temp_dir().join(format!("mcp-{}", uuid::Uuid::new_v4()));
    let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .and_then(|repository| {
            repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
        })
        .unwrap_or_else(|error| panic!("{error}"));
    let files = ProjectFiles::new(
        folder.join("attachments"),
        DeviceId::new("device"),
        repository.sealed_files(),
    );
    Fixture {
        repository,
        voices: InMemoryVoiceVectorDB::new(),
        files,
        folder,
    }
}

fn meeting(fixture: &Fixture, id: &str, title: &str, said: &str) -> MeetingId {
    let meeting_id = MeetingId::new(id);
    MeetingCrdtService::new(&fixture.repository)
        .create_titled_meeting(
            &meeting_id,
            &MeetingTitle::parse(title).unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    fixture
        .repository
        .save_transcript_segments(
            &meeting_id,
            &[TranscriptSegment::new(
                meeting_id.clone(),
                61_000,
                64_000,
                None,
                said,
            )],
        )
        .unwrap_or_else(|error| panic!("{error}"));
    meeting_id
}

fn server(
    fixture: &Fixture,
    enabled: bool,
) -> McpServer<'_, SqliteNoteRepository, InMemoryVoiceVectorDB> {
    McpServer::new(
        &fixture.repository,
        &fixture.voices,
        &fixture.files,
        Box::new(move || enabled),
    )
}

/// Sends one request and returns the reply as JSON.
fn exchange(
    server: &McpServer<'_, SqliteNoteRepository, InMemoryVoiceVectorDB>,
    request: Value,
) -> Value {
    let reply = server
        .handle(&request.to_string())
        .unwrap_or_else(|| panic!("no reply to {request}"));
    serde_json::from_str(&reply).unwrap_or_else(|error| panic!("{error}"))
}

/// Calls a tool and returns its result object.
fn call(
    server: &McpServer<'_, SqliteNoteRepository, InMemoryVoiceVectorDB>,
    tool: &str,
    arguments: Value,
) -> Value {
    exchange(
        server,
        json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call",
               "params": {"name": tool, "arguments": arguments}}),
    )["result"]
        .clone()
}

/// The JSON a successful tool call returned as its text.
fn tool_json(result: &Value) -> Value {
    assert_eq!(result["isError"], false, "{result}");
    serde_json::from_str(result["content"][0]["text"].as_str().unwrap_or_default())
        .unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn initialize_answers_the_clients_version_and_offers_tools() {
    let fixture = fixture();
    let server = server(&fixture, true);
    let reply = exchange(
        &server,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
               "params": {"protocolVersion": "2025-03-26", "capabilities": {},
                          "clientInfo": {"name": "test", "version": "1"}}}),
    );
    assert_eq!(reply["jsonrpc"], "2.0");
    assert_eq!(reply["id"], 1);
    assert_eq!(reply["result"]["protocolVersion"], "2025-03-26");
    assert!(reply["result"]["capabilities"]["tools"].is_object());
    assert_eq!(reply["result"]["serverInfo"]["name"], "koalino");
}

#[test]
fn an_unknown_version_gets_the_latest_one() {
    let fixture = fixture();
    let server = server(&fixture, true);
    let reply = exchange(
        &server,
        json!({"jsonrpc": "2.0", "id": "a", "method": "initialize",
               "params": {"protocolVersion": "1999-01-01"}}),
    );
    assert_eq!(reply["id"], "a");
    assert_eq!(reply["result"]["protocolVersion"], "2025-06-18");
}

#[test]
fn notifications_get_no_reply_and_ping_gets_an_empty_one() {
    let fixture = fixture();
    let server = server(&fixture, true);
    assert_eq!(
        server.handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
        None
    );
    let reply = exchange(
        &server,
        json!({"jsonrpc": "2.0", "id": 2, "method": "ping"}),
    );
    assert_eq!(reply["result"], json!({}));
}

#[test]
fn bad_json_and_unknown_methods_are_json_rpc_errors() {
    let fixture = fixture();
    let server = server(&fixture, true);
    let reply: Value = serde_json::from_str(&server.handle("{not json").unwrap_or_default())
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(reply["error"]["code"], -32700);
    assert_eq!(reply["id"], Value::Null);
    let reply = exchange(
        &server,
        json!({"jsonrpc": "2.0", "id": 3, "method": "resources/list"}),
    );
    assert_eq!(reply["error"]["code"], -32601);
}

#[test]
fn every_tool_is_listed_as_read_only() {
    let fixture = fixture();
    let server = server(&fixture, true);
    let reply = exchange(
        &server,
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/list"}),
    );
    let tools = reply["result"]["tools"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert_eq!(
        names,
        [
            "list_meetings",
            "read_meeting",
            "search",
            "list_projects",
            "read_project",
            "read_project_file"
        ]
    );
    for tool in &tools {
        assert_eq!(tool["annotations"]["readOnlyHint"], true, "{tool}");
        assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
    }
}

#[test]
fn with_access_off_every_tool_says_how_to_turn_it_on_and_reads_nothing() {
    let fixture = fixture();
    meeting(&fixture, "m-1", "Budget", "We agreed on the budget.");
    let server = server(&fixture, false);
    let result = call(&server, "list_meetings", json!({}));
    assert_eq!(result["isError"], true);
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    assert!(text.contains("Settings"), "{text}");
    assert!(!text.contains("Budget"), "{text}");
}

#[test]
fn the_switch_is_read_at_every_call() {
    let fixture = fixture();
    let enabled = Cell::new(false);
    let server = McpServer::new(
        &fixture.repository,
        &fixture.voices,
        &fixture.files,
        Box::new(|| enabled.get()),
    );
    assert_eq!(call(&server, "list_projects", json!({}))["isError"], true);
    enabled.set(true);
    assert_eq!(call(&server, "list_projects", json!({}))["isError"], false);
}

#[test]
fn list_meetings_gives_the_newest_first_and_filters_by_project() {
    let fixture = fixture();
    let project = fixture
        .repository
        .create_project(&ProjectName::parse("Launch").unwrap_or_else(|error| panic!("{error}")))
        .unwrap_or_else(|error| panic!("{error}"))
        .project_id();
    let older = meeting(&fixture, "m-old", "Kick-off", "Hello.");
    // Start times are kept to the second.
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    meeting(&fixture, "m-new", "Review", "Bye.");
    fixture
        .repository
        .move_meeting_to_project(&older, Some(project))
        .unwrap_or_else(|error| panic!("{error}"));
    let server = server(&fixture, true);

    let all = tool_json(&call(&server, "list_meetings", json!({})));
    let ids: Vec<&str> = all["meetings"]
        .as_array()
        .map(|meetings| {
            meetings
                .iter()
                .filter_map(|meeting| meeting["meeting_id"].as_str())
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(ids, ["m-new", "m-old"]);
    assert_eq!(all["meetings"][1]["title"], "Kick-off");
    assert_eq!(all["meetings"][1]["project_id"], project.to_string());
    assert_eq!(all["meetings"][0]["project_id"], Value::Null);

    let limited = tool_json(&call(&server, "list_meetings", json!({"limit": 1})));
    assert_eq!(limited["meetings"].as_array().map(Vec::len), Some(1));
    assert_eq!(limited["total"], 2);

    let in_project = tool_json(&call(
        &server,
        "list_meetings",
        json!({"project_id": project.to_string()}),
    ));
    assert_eq!(in_project["meetings"].as_array().map(Vec::len), Some(1));
    assert_eq!(in_project["meetings"][0]["meeting_id"], "m-old");
}

#[test]
fn read_meeting_gives_its_title_notes_and_transcript() {
    let fixture = fixture();
    let meeting_id = meeting(&fixture, "m-1", "Budget", "We agreed on the budget.");
    set_notes(&fixture.repository, &meeting_id, "Send the figures.")
        .unwrap_or_else(|error| panic!("{error}"));
    let server = server(&fixture, true);

    let read = tool_json(&call(&server, "read_meeting", json!({"meeting_id": "m-1"})));
    assert_eq!(read["title"], "Budget");
    assert_eq!(read["notes"], "Send the figures.");
    let transcript = read["transcript"].as_str().unwrap_or_default();
    assert!(transcript.starts_with("[00:01:01] "), "{transcript}");
    assert!(
        transcript.ends_with(": We agreed on the budget."),
        "{transcript}"
    );

    let without = tool_json(&call(
        &server,
        "read_meeting",
        json!({"meeting_id": "m-1", "include_transcript": false}),
    ));
    assert_eq!(without["transcript"], Value::Null);
}

#[test]
fn a_missing_meeting_or_argument_is_a_tool_error() {
    let fixture = fixture();
    let server = server(&fixture, true);
    let missing = call(&server, "read_meeting", json!({"meeting_id": "nope"}));
    assert_eq!(missing["isError"], true);
    assert!(
        missing["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("meeting_not_found")
    );
    let no_argument = call(&server, "read_meeting", json!({}));
    assert_eq!(no_argument["isError"], true);
    let unknown = exchange(
        &server,
        json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call",
               "params": {"name": "delete_meeting", "arguments": {}}}),
    );
    assert_eq!(unknown["error"]["code"], -32602);
}

#[test]
fn search_finds_words_in_transcripts() {
    let fixture = fixture();
    meeting(&fixture, "m-1", "Budget", "We agreed on the budget.");
    meeting(&fixture, "m-2", "Lunch", "Pasta again.");
    let server = server(&fixture, true);
    let found = tool_json(&call(&server, "search", json!({"query": "budget agreed"})));
    let hits = found["hits"].as_array().cloned().unwrap_or_default();
    assert!(!hits.is_empty());
    assert!(hits.iter().all(|hit| hit["meeting"]["meeting_id"] == "m-1"));
    let too_short = call(&server, "search", json!({"query": ""}));
    assert_eq!(too_short["isError"], true);
}

#[test]
fn projects_their_details_and_their_text_files_can_be_read() {
    let fixture = fixture();
    let project = fixture
        .repository
        .create_project(&ProjectName::parse("Launch").unwrap_or_else(|error| panic!("{error}")))
        .unwrap_or_else(|error| panic!("{error}"))
        .project_id();
    let note = fixture
        .files
        .add_note(&fixture.repository, project, "Plan", "Ship on Friday.")
        .unwrap_or_else(|error| panic!("{error}"));
    let server = server(&fixture, true);

    let projects = tool_json(&call(&server, "list_projects", json!({})));
    assert_eq!(projects["projects"][0]["name"], "Launch");

    let detail = tool_json(&call(
        &server,
        "read_project",
        json!({"project_id": project.to_string()}),
    ));
    let resource = &detail["resources"][0];
    assert_eq!(resource["attachment_id"], note.attachment_id().to_string());
    assert_eq!(resource.get("file_path"), None, "no file paths: {resource}");

    let file = tool_json(&call(
        &server,
        "read_project_file",
        json!({"project_id": project.to_string(),
               "attachment_id": note.attachment_id().to_string()}),
    ));
    assert_eq!(file["text"], "# Plan\n\nShip on Friday.\n");

    let elsewhere = call(
        &server,
        "read_project_file",
        json!({"project_id": uuid::Uuid::new_v4().to_string(),
               "attachment_id": note.attachment_id().to_string()}),
    );
    assert_eq!(elsewhere["isError"], true);
}

#[test]
fn serve_answers_each_line_and_skips_notifications() {
    let fixture = fixture();
    let server = server(&fixture, true);
    let input = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
        "\n",
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        "\n\n",
        r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
        "\n"
    );
    let mut output = Vec::new();
    serve(&server, input.as_bytes(), &mut output).unwrap_or_else(|error| panic!("{error}"));
    let text = String::from_utf8(output).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(lines[0].contains(r#""id":1"#));
    assert!(lines[1].contains(r#""id":2"#));
}

#[test]
fn access_is_off_until_saved_on() {
    let folder = std::env::temp_dir().join(format!("mcp-settings-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
    let file = AssistantAccessSettings::file_for(&folder.join("notes.sqlite3"));
    assert!(
        file.to_string_lossy()
            .ends_with("notes.sqlite3.assistant-access.json")
    );
    assert!(!AssistantAccessSettings::load(&file).enabled);
    AssistantAccessSettings { enabled: true }
        .save(&file)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(AssistantAccessSettings::load(&file).enabled);
    std::fs::write(&file, "garbage").unwrap_or_else(|error| panic!("{error}"));
    assert!(
        !AssistantAccessSettings::load(&file).enabled,
        "unreadable means off"
    );
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn the_settings_command_turns_access_on_and_shows_how_to_connect() {
    let folder = std::env::temp_dir().join(format!("mcp-command-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
    let database = folder.join("my notes.sqlite3");
    let binary = folder.join("note_taker_mcp");
    std::fs::write(&binary, "").unwrap_or_else(|error| panic!("{error}"));

    let shown = access_command(&database, r#"{"command":"get"}"#, Some(&binary), false);
    assert_eq!(shown["ok"], true);
    assert_eq!(shown["enabled"], false);
    assert_eq!(shown["server_path"], binary.to_string_lossy().as_ref());
    let server = &shown["config"]["mcpServers"]["koalino"];
    assert_eq!(server["command"], binary.to_string_lossy().as_ref());
    assert_eq!(
        server["env"]["NOTE_TAKER_DATABASE_PATH"],
        database.to_string_lossy().as_ref()
    );
    let claude = shown["claude_code"].as_str().unwrap_or_default();
    assert!(
        claude.starts_with("claude mcp add koalino --env "),
        "{claude}"
    );
    assert!(
        claude.contains(&format!(
            "\"NOTE_TAKER_DATABASE_PATH={}\"",
            database.display()
        )),
        "a path with spaces is quoted: {claude}"
    );
    assert_eq!(shown["passphrase_needed"], false);

    let turned_on = access_command(
        &database,
        r#"{"command":"set","enabled":true}"#,
        Some(&binary),
        false,
    );
    assert_eq!(turned_on["enabled"], true);
    assert!(AssistantAccessSettings::load(&AssistantAccessSettings::file_for(&database)).enabled);

    let without_binary = access_command(&database, r#"{"command":"get"}"#, None, true);
    assert_eq!(without_binary["server_path"], Value::Null);
    assert_eq!(without_binary["config"], Value::Null);
    assert_eq!(without_binary["passphrase_needed"], true);

    let bad = access_command(&database, r#"{"command":"delete"}"#, None, false);
    assert_eq!(bad["ok"], false);
    assert_eq!(bad["error_code"], "invalid_request");
    let _ = std::fs::remove_dir_all(&folder);
}
