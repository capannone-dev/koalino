//! Specifies the `/api/projects` routes the web Projects pages use (Issue #85): JSON shapes, status
//! codes, uploads, serving a resource's bytes safely, and the meeting list naming each meeting's
//! project.

use std::{path::PathBuf, sync::Arc};

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

use crate::{
    adapters::storage::{
        in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
    },
    application::{
        crypto::voice_encryptor::ephemeral_cipher, meeting_crdt::MeetingCrdtService,
        meeting_document::AutomergeMeetingDocumentCodec,
    },
    domain::models::{DeviceId, MeetingId},
    project_commands::ProjectFiles,
    server::api::{ServerAppState, build_router},
};

/// A router over an in-memory database (with one meeting) and a scratch attachment folder.
struct Fixture {
    directory: PathBuf,
    router: axum::Router,
}

impl Fixture {
    /// Builds the router, with project files unless `without_files`.
    fn new(with_files: bool) -> Self {
        let directory = std::env::temp_dir().join(format!("project-routes-{}", Uuid::new_v4()));
        let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
            .and_then(|repository| {
                repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
            })
            .unwrap_or_else(|error| panic!("repository must open: {error}"));
        MeetingCrdtService::new(&repository)
            .create_meeting(&MeetingId::new("meeting-1"))
            .unwrap_or_else(|error| panic!("meeting must be created: {error}"));
        let sealed_files = repository.sealed_files();
        let mut state = ServerAppState::new(repository, InMemoryVoiceVectorDB::new());
        if with_files {
            state = state.with_project_files(Arc::new(ProjectFiles::new(
                directory.join("attachments"),
                DeviceId::new("server"),
                sealed_files,
            )));
        }
        Self {
            directory,
            router: build_router(state),
        }
    }

    /// Sends one request and returns the status, the headers and the raw body.
    async fn raw(
        &self,
        method: &str,
        uri: &str,
        content_type: Option<&str>,
        body: Vec<u8>,
    ) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
        let mut request = Request::builder().method(method).uri(uri);
        if let Some(content_type) = content_type {
            request = request.header(header::CONTENT_TYPE, content_type);
        }
        let response = self
            .router
            .clone()
            .oneshot(
                request
                    .body(Body::from(body))
                    .unwrap_or_else(|error| panic!("request must build: {error}")),
            )
            .await
            .unwrap_or_else(|error| panic!("router must answer: {error}"));
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap_or_else(|error| panic!("body must read: {error}"));
        (status, headers, bytes.to_vec())
    }

    /// Sends one request with an optional JSON body and returns the status and JSON (`Null` when
    /// empty).
    async fn send(&self, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
        let (status, _, bytes) = self
            .raw(
                method,
                uri,
                body.as_ref().map(|_| "application/json"),
                body.map(|body| body.to_string().into_bytes())
                    .unwrap_or_default(),
            )
            .await;
        let json = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|error| panic!("body must be JSON: {error}"))
        };
        (status, json)
    }

    /// Creates a project and returns its ID.
    async fn create(&self, name: &str) -> String {
        let (status, project) = self
            .send("POST", "/api/projects", Some(json!({"name": name})))
            .await;
        assert_eq!(status, StatusCode::CREATED, "{project}");
        project["project_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[tokio::test]
async fn projects_are_created_listed_renamed_and_deleted() {
    let fixture = Fixture::new(true);
    let project_id = fixture.create("website").await;

    let (renamed_status, renamed) = fixture
        .send(
            "PATCH",
            &format!("/api/projects/{project_id}"),
            Some(json!({"name": "Website"})),
        )
        .await;
    let (_, listed) = fixture.send("GET", "/api/projects", None).await;
    let (blank_status, _) = fixture
        .send("POST", "/api/projects", Some(json!({"name": " "})))
        .await;
    let (deleted_status, _) = fixture
        .send("DELETE", &format!("/api/projects/{project_id}"), None)
        .await;
    let (again_status, _) = fixture
        .send("DELETE", &format!("/api/projects/{project_id}"), None)
        .await;
    let (bad_id_status, _) = fixture.send("GET", "/api/projects/not-a-uuid", None).await;

    assert_eq!(renamed_status, StatusCode::OK);
    assert_eq!(renamed["name"], "Website");
    assert_eq!(listed[0]["name"], "Website");
    assert_eq!(blank_status, StatusCode::BAD_REQUEST);
    assert_eq!(deleted_status, StatusCode::NO_CONTENT);
    assert_eq!(again_status, StatusCode::NOT_FOUND);
    assert_eq!(bad_id_status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_meeting_moves_into_a_project_and_the_meeting_list_says_so() {
    let fixture = Fixture::new(true);
    let project_id = fixture.create("Hiring").await;

    let (moved_status, _) = fixture
        .send(
            "PUT",
            "/api/meetings/meeting-1/project",
            Some(json!({"project_id": project_id})),
        )
        .await;
    let (_, meetings) = fixture.send("GET", "/api/meetings", None).await;
    let (_, detail) = fixture
        .send("GET", &format!("/api/projects/{project_id}"), None)
        .await;
    let (ghost_status, _) = fixture
        .send(
            "PUT",
            "/api/meetings/meeting-ghost/project",
            Some(json!({"project_id": project_id})),
        )
        .await;
    let (out_status, _) = fixture
        .send(
            "PUT",
            "/api/meetings/meeting-1/project",
            Some(json!({"project_id": null})),
        )
        .await;
    let (_, meetings_after) = fixture.send("GET", "/api/meetings", None).await;

    assert_eq!(moved_status, StatusCode::NO_CONTENT);
    assert_eq!(meetings[0]["project_id"], project_id.as_str());
    assert_eq!(detail["meetings"][0]["meeting_id"], "meeting-1");
    assert_eq!(ghost_status, StatusCode::NOT_FOUND);
    assert_eq!(out_status, StatusCode::NO_CONTENT);
    assert_eq!(meetings_after[0]["project_id"], Value::Null);
}

#[tokio::test]
async fn uploads_and_notes_become_resources_served_safely_and_deletable() {
    let fixture = Fixture::new(true);
    let project_id = fixture.create("Hiring").await;
    let email = b"From: Anna <anna@example.com>\r\nSubject: Offer\r\nContent-Type: text/plain\r\n\r\nYes!\r\n";

    let (upload_status, _, upload_body) = fixture
        .raw(
            "POST",
            &format!("/api/projects/{project_id}/resources?name=offer.eml"),
            Some("application/octet-stream"),
            email.to_vec(),
        )
        .await;
    let (any_status, _, any_body) = fixture
        .raw(
            "POST",
            &format!("/api/projects/{project_id}/resources?name=site.dwg"),
            Some("application/octet-stream"),
            b"AC1032".to_vec(),
        )
        .await;
    let any: Value = serde_json::from_slice(&any_body).unwrap_or_default();
    let any_id = any["added"][0]["attachment_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let (_, any_headers, _) = fixture
        .raw(
            "GET",
            &format!("/api/projects/{project_id}/resources/{any_id}/file"),
            None,
            Vec::new(),
        )
        .await;
    let (note_status, note) = fixture
        .send(
            "POST",
            &format!("/api/projects/{project_id}/notes"),
            Some(json!({"title": "Plan", "text": "Call Anna"})),
        )
        .await;
    let uploaded: Value = serde_json::from_slice(&upload_body).unwrap_or_default();
    let email_id = uploaded["added"][0]["attachment_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let note_id = note["attachment_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();

    let (_, read) = fixture
        .send(
            "GET",
            &format!("/api/projects/{project_id}/resources/{email_id}/email"),
            None,
        )
        .await;
    let (file_status, file_headers, file_body) = fixture
        .raw(
            "GET",
            &format!("/api/projects/{project_id}/resources/{note_id}/file"),
            None,
            Vec::new(),
        )
        .await;
    let (deleted_status, _) = fixture
        .send(
            "DELETE",
            &format!("/api/projects/{project_id}/resources/{note_id}"),
            None,
        )
        .await;
    let (gone_status, _, _) = fixture
        .raw(
            "GET",
            &format!("/api/projects/{project_id}/resources/{note_id}/file"),
            None,
            Vec::new(),
        )
        .await;

    assert_eq!(upload_status, StatusCode::CREATED);
    assert_eq!(uploaded["added"][0]["kind"], "email");
    assert!(
        uploaded["added"][0].get("file_path").is_none(),
        "{uploaded}"
    );
    assert_eq!(
        any_status,
        StatusCode::CREATED,
        "any file type is accepted (#94)"
    );
    assert_eq!(any["added"][0]["kind"], "document");
    assert_eq!(
        any_headers[header::CONTENT_TYPE],
        "application/octet-stream"
    );
    assert!(
        any_headers[header::CONTENT_DISPOSITION]
            .to_str()
            .is_ok_and(|disposition| disposition.starts_with("attachment;")),
        "a file the browser cannot show safely is only downloaded"
    );
    assert_eq!(note_status, StatusCode::CREATED);
    assert_eq!(read["subject"], "Offer");
    assert_eq!(file_status, StatusCode::OK);
    assert_eq!(file_body, b"# Plan\n\nCall Anna\n");
    assert_eq!(
        file_headers[header::CONTENT_TYPE],
        "text/markdown; charset=utf-8"
    );
    assert_eq!(file_headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert!(
        file_headers[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .is_ok_and(|policy| policy.contains("sandbox"))
    );
    assert_eq!(deleted_status, StatusCode::NO_CONTENT);
    assert_eq!(gone_status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn without_project_files_file_routes_are_unavailable() {
    let fixture = Fixture::new(false);
    let project_id = fixture.create("Hiring").await;

    let (status, _) = fixture
        .send(
            "POST",
            &format!("/api/projects/{project_id}/notes"),
            Some(json!({"title": "Plan", "text": ""})),
        )
        .await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}
