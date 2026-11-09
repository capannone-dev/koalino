//! Specifies the Projects commands shared by the app (FFI) and the web API (Issue #85): every
//! answer is `{"ok":true,...}` or `{"ok":false,"error_code","error"}`, and the commands create,
//! list, rename and delete projects, move meetings, and add, read and delete resources.

use std::{path::PathBuf, sync::Arc};

use serde_json::{Value, json};
use uuid::Uuid;

use super::project_commands::{ProjectFiles, run_project_command};
use crate::{
    adapters::storage::sqlite_adapter::SqliteNoteRepository,
    application::{
        crypto::voice_encryptor::ephemeral_cipher, meeting_crdt::MeetingCrdtService,
        meeting_document::AutomergeMeetingDocumentCodec,
    },
    domain::models::{DeviceId, MeetingId},
};

/// A repository, its attachment folder and the command context, removed on drop.
struct Fixture {
    directory: PathBuf,
    repository: SqliteNoteRepository,
    files: ProjectFiles,
}

impl Fixture {
    /// Opens an empty database and attachment folder.
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("project-commands-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory)
            .unwrap_or_else(|error| panic!("scratch directory must be created: {error}"));
        let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
            .and_then(|repository| {
                repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
            })
            .unwrap_or_else(|error| panic!("repository must open: {error}"));
        let files = ProjectFiles::new(
            directory.join("attachments"),
            DeviceId::new("device"),
            repository.sealed_files(),
        );
        Self {
            directory,
            repository,
            files,
        }
    }

    /// Runs one command with paths included, as the app does.
    fn run(&self, request: Value) -> Value {
        let answer = run_project_command(&self.repository, &self.files, &request.to_string(), true);
        serde_json::from_str(&answer).unwrap_or_else(|error| panic!("answer must be JSON: {error}"))
    }

    /// Creates a project and returns its ID.
    fn create(&self, name: &str) -> String {
        let answer = self.run(json!({"command": "create", "name": name}));
        assert_eq!(answer["ok"], true, "{answer}");
        answer["project"]["project_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }

    /// Writes a source file outside the attachment folder.
    fn source(&self, name: &str, contents: &[u8]) -> String {
        let path = self.directory.join(name);
        std::fs::write(&path, contents)
            .unwrap_or_else(|error| panic!("source must be written: {error}"));
        path.to_string_lossy().into_owned()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// The error code of a failed answer.
fn error_code(answer: &Value) -> &str {
    assert_eq!(answer["ok"], false, "{answer}");
    answer["error_code"].as_str().unwrap_or_default()
}

#[test]
fn projects_are_created_renamed_listed_and_deleted() {
    let fixture = Fixture::new();
    let project_id = fixture.create("website");
    fixture.create("Budget");

    let renamed = fixture
        .run(json!({"command": "rename", "project_id": project_id, "name": "Website relaunch"}));
    let listed = fixture.run(json!({"command": "list"}));
    let deleted = fixture.run(json!({"command": "delete", "project_id": project_id}));
    let deleted_again = fixture.run(json!({"command": "delete", "project_id": project_id}));

    assert_eq!(renamed["project"]["name"], "Website relaunch");
    let names: Vec<&str> = listed["projects"]
        .as_array()
        .map(|projects| {
            projects
                .iter()
                .filter_map(|project| project["name"].as_str())
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(names, vec!["Budget", "Website relaunch"]);
    assert_eq!(listed["projects"][1]["meeting_count"], 0);
    assert_eq!(deleted["deleted"], true);
    assert_eq!(deleted_again["deleted"], false);
}

#[test]
fn bad_requests_and_names_are_refused() {
    let fixture = Fixture::new();

    assert_eq!(
        error_code(
            &serde_json::from_str(&run_project_command(
                &fixture.repository,
                &fixture.files,
                "{",
                true
            ))
            .unwrap_or_default()
        ),
        "invalid_request"
    );
    assert_eq!(
        error_code(&fixture.run(json!({"command": "explode"}))),
        "invalid_request"
    );
    assert_eq!(
        error_code(&fixture.run(json!({"command": "create", "name": "  "}))),
        "invalid_name"
    );
    assert_eq!(
        error_code(&fixture.run(json!({"command": "detail", "project_id": "not-a-uuid"}))),
        "invalid_request"
    );
    assert_eq!(
        error_code(&fixture.run(json!({"command": "detail", "project_id": Uuid::new_v4()}))),
        "project_not_found"
    );
}

#[test]
fn a_meeting_moves_into_a_project_and_shows_in_its_detail() {
    let fixture = Fixture::new();
    let project_id = fixture.create("Hiring");
    let meeting_id = MeetingId::new("meeting-1");
    MeetingCrdtService::new(&fixture.repository)
        .create_meeting(&meeting_id)
        .unwrap_or_else(|error| panic!("meeting must be created: {error}"));

    let moved = fixture.run(
        json!({"command": "move_meeting", "meeting_id": "meeting-1", "project_id": project_id}),
    );
    let detail = fixture.run(json!({"command": "detail", "project_id": project_id}));
    let memberships = fixture.run(json!({"command": "meeting_projects"}));
    let unknown_meeting = fixture.run(
        json!({"command": "move_meeting", "meeting_id": "meeting-ghost", "project_id": project_id}),
    );
    let moved_out = fixture
        .run(json!({"command": "move_meeting", "meeting_id": "meeting-1", "project_id": null}));

    assert_eq!(moved["ok"], true, "{moved}");
    assert_eq!(detail["project"]["meeting_count"], 1);
    assert_eq!(detail["meetings"][0]["meeting_id"], "meeting-1");
    assert_eq!(
        memberships["meeting_projects"]["meeting-1"],
        project_id.as_str()
    );
    assert_eq!(error_code(&unknown_meeting), "meeting_not_found");
    assert_eq!(moved_out["ok"], true);
    assert_eq!(
        fixture.run(json!({"command": "meeting_projects"}))["meeting_projects"],
        json!({})
    );
}

#[test]
fn files_notes_and_emails_become_resources_that_can_be_read_and_deleted() {
    let fixture = Fixture::new();
    let project_id = fixture.create("Hiring");
    let picture = fixture.source("board.png", b"\x89PNG");
    // Any file type is accepted (#94); a file that is gone is the one that fails.
    let program = fixture.source("setup.exe", b"MZ");
    let missing = format!("{program}.gone");
    let email = fixture.source(
        "offer.eml",
        b"From: Anna <anna@example.com>\r\nSubject: Offer\r\nContent-Type: text/plain\r\n\r\nYes!\r\n",
    );

    let added = fixture.run(json!({"command": "add_files", "project_id": project_id, "paths": [picture, program, missing, email]}));
    let note = fixture.run(json!({"command": "add_note", "project_id": project_id, "title": "Plan", "text": "Call Anna"}));
    let detail = fixture.run(json!({"command": "detail", "project_id": project_id}));

    assert_eq!(added["added"].as_array().map(Vec::len), Some(3), "{added}");
    assert_eq!(added["added"][1]["kind"], "document");
    assert_eq!(added["failed"][0]["path"], missing.as_str());
    assert_eq!(note["resource"]["file_name"], "Plan.md");
    assert_eq!(note["resource"]["kind"], "markdown");
    let resources = detail["resources"].as_array().cloned().unwrap_or_default();
    let kinds: Vec<&str> = resources
        .iter()
        .filter_map(|resource| resource["kind"].as_str())
        .collect();
    assert_eq!(kinds, vec!["image", "document", "email", "markdown"]);
    let note_path = note["resource"]["file_path"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(
        fixture
            .repository
            .sealed_files()
            .read_all(std::path::Path::new(&note_path))
            .ok()
            .map(|contents| String::from_utf8_lossy(&contents).into_owned())
            .as_deref(),
        Some("# Plan\n\nCall Anna\n")
    );

    let email_id = resources[2]["attachment_id"].clone();
    let read = fixture
        .run(json!({"command": "read_email", "project_id": project_id, "attachment_id": email_id}));
    assert_eq!(read["email"]["subject"], "Offer");
    assert_eq!(read["email"]["from"][0]["name"], "Anna");
    assert_eq!(
        read["email"]["body_text"].as_str().map(str::trim),
        Some("Yes!")
    );
    assert_eq!(
        error_code(&fixture.run(json!({"command": "read_email", "project_id": project_id, "attachment_id": resources[0]["attachment_id"]}))),
        "not_an_email"
    );

    let note_id = note["resource"]["attachment_id"].clone();
    let deleted = fixture.run(
        json!({"command": "delete_resource", "project_id": project_id, "attachment_id": note_id}),
    );
    assert_eq!(deleted["deleted"], true);
    assert!(!std::path::Path::new(&note_path).exists());
    assert_eq!(
        error_code(&fixture.run(json!({"command": "delete_resource", "project_id": project_id, "attachment_id": note_id}))),
        "resource_not_found"
    );
}

#[test]
fn deleting_a_project_removes_its_folder() {
    let fixture = Fixture::new();
    let project_id = fixture.create("Hiring");
    let picture = fixture.source("board.png", b"\x89PNG");
    fixture.run(json!({"command": "add_files", "project_id": project_id, "paths": [picture]}));
    let folder = fixture
        .directory
        .join(format!("attachments/project-{project_id}"));
    assert!(folder.exists());

    fixture.run(json!({"command": "delete", "project_id": project_id}));

    assert!(!folder.exists());
}

#[test]
fn paths_are_left_out_when_not_asked_for() {
    let fixture = Fixture::new();
    let project_id = fixture.create("Hiring");
    let note = run_project_command(
        &fixture.repository,
        &fixture.files,
        &json!({"command": "add_note", "project_id": project_id, "title": "Plan", "text": ""})
            .to_string(),
        false,
    );

    assert!(!note.contains("file_path"), "{note}");
}
