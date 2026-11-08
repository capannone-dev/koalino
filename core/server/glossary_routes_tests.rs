//! Specifies the web Settings page's glossary routes: reading and replacing the names and terms
//! transcription is prompted with, stored in the transcription settings file beside the database.

use std::path::PathBuf;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::{
    adapters::storage::{
        in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
    },
    application::crypto::voice_encryptor::ephemeral_cipher,
    server::api::{ServerAppState, build_router},
};

/// A settings file in a fresh folder, removed on drop.
struct ScratchSettings(PathBuf);

impl ScratchSettings {
    fn new() -> Self {
        let folder =
            std::env::temp_dir().join(format!("note-taker-glossary-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
        Self(folder.join("notes.sqlite3.transcription-settings.json"))
    }
}

impl Drop for ScratchSettings {
    fn drop(&mut self) {
        if let Some(folder) = self.0.parent() {
            let _ = std::fs::remove_dir_all(folder);
        }
    }
}

fn router(settings: Option<&ScratchSettings>) -> axum::Router {
    let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .unwrap_or_else(|error| panic!("{error}"));
    let state = ServerAppState::new(repository, InMemoryVoiceVectorDB::new());
    build_router(match settings {
        Some(settings) => state.with_settings_file(settings.0.clone()),
        None => state,
    })
}

async fn send(
    router: axum::Router,
    method: &str,
    body: Option<&str>,
) -> (StatusCode, serde_json::Value) {
    let mut request = Request::builder()
        .method(method)
        .uri("/api/settings/glossary");
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    let response = router
        .oneshot(
            request
                .body(body.map_or_else(Body::empty, |text| Body::from(text.to_owned())))
                .unwrap_or_else(|error| panic!("{error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("{error}")),
    )
}

#[tokio::test]
async fn the_glossary_is_replaced_and_read_back_cleaned() {
    let settings = ScratchSettings::new();
    let router = router(Some(&settings));

    let (empty_status, empty) = send(router.clone(), "GET", None).await;
    let (status, saved) = send(
        router.clone(),
        "PUT",
        Some(r#"{"terms":[" Marco Rossi ","","OKR","okr"]}"#),
    )
    .await;
    let (_, after) = send(router, "GET", None).await;

    assert_eq!(empty_status, StatusCode::OK);
    assert_eq!(empty, serde_json::json!({ "terms": [] }));
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        saved,
        serde_json::json!({ "terms": ["Marco Rossi", "OKR"] })
    );
    assert_eq!(after, saved);
}

#[tokio::test]
async fn an_invalid_glossary_is_refused_and_nothing_changes() {
    let settings = ScratchSettings::new();
    let router = router(Some(&settings));
    let long_term = "x".repeat(61);

    let (status, body) = send(
        router.clone(),
        "PUT",
        Some(&format!(r#"{{"terms":["{long_term}"]}}"#)),
    )
    .await;
    let (_, after) = send(router, "GET", None).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error_code"], "invalid_glossary");
    assert_eq!(after, serde_json::json!({ "terms": [] }));
}

#[tokio::test]
async fn without_a_settings_file_the_routes_are_unavailable() {
    let (status, body) = send(router(None), "GET", None).await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error_code"], "unavailable");
}
