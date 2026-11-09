//! Specifies the notes folder routes (#98): 503 without a keeper, off by default, turned on
//! with a folder that is written at once, and a relative folder refused.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
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
    notes_folder_commands::NotesFolderKeeper,
    project_commands::ProjectFiles,
    server::api::{ServerAppState, build_router},
};

fn state() -> ServerAppState {
    let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .and_then(|repository| {
            repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
        })
        .unwrap_or_else(|error| panic!("{error}"));
    MeetingCrdtService::new(&repository)
        .create_meeting(&MeetingId::new("m-1"))
        .unwrap_or_else(|error| panic!("{error}"));
    ServerAppState::new(repository, InMemoryVoiceVectorDB::new())
}

async fn call(
    router: &axum::Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(body.map_or_else(Body::empty, |body| Body::from(body.to_string())))
        .unwrap_or_else(|error| panic!("{error}"));
    let response = router
        .clone()
        .oneshot(request)
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn without_a_keeper_the_routes_are_unavailable() {
    let router = build_router(state());
    let (status, body) = call(&router, Method::GET, "/api/settings/notes-folder", None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error_code"], "unavailable");
}

#[tokio::test(flavor = "multi_thread")]
async fn turning_the_folder_on_writes_it() {
    let scratch = std::env::temp_dir().join(format!("notes-folder-routes-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&scratch).unwrap_or_else(|error| panic!("{error}"));
    let router = build_router(
        state()
            .with_project_files(Arc::new(ProjectFiles::new(
                scratch.join("attachments"),
                DeviceId::new("device"),
            )))
            .with_notes_folder(Arc::new(NotesFolderKeeper::for_database(
                &scratch.join("db.sqlite3"),
                None,
            ))),
    );

    let (status, body) = call(&router, Method::GET, "/api/settings/notes-folder", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["enabled"], false);

    let (status, body) = call(
        &router,
        Method::PUT,
        "/api/settings/notes-folder",
        Some(json!({"enabled": true, "path": "relative"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error_code"], "invalid_folder");

    let folder = scratch.join("Notes");
    let (status, body) = call(
        &router,
        Method::PUT,
        "/api/settings/notes-folder",
        Some(json!({"enabled": true, "path": folder})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["enabled"], true);
    assert!(folder.join("No project").is_dir(), "the folder was written");

    let (status, body) = call(
        &router,
        Method::POST,
        "/api/settings/notes-folder/sync",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["written"], 0);
    let _ = std::fs::remove_dir_all(&scratch);
}
