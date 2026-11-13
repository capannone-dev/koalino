//! Specifies the web Settings page's after-meeting pass route (#87): on or off, and its model.

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

/// A settings file in a fresh folder holding one downloaded model, removed on drop.
struct ScratchSettings(PathBuf);

impl ScratchSettings {
    fn new() -> Self {
        let folder = std::env::temp_dir().join(format!("note-taker-pass-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(folder.join("models")).unwrap_or_else(|error| panic!("{error}"));
        std::fs::write(folder.join("models").join("ggml-tiny.bin"), b"model")
            .unwrap_or_else(|error| panic!("{error}"));
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
        .uri("/api/settings/after-meeting-pass");
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
async fn the_pass_is_on_with_the_recording_model_and_can_be_changed() {
    let settings = ScratchSettings::new();
    let router = router(Some(&settings));

    let (status, initial) = send(router.clone(), "GET", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(initial["enabled"], true);
    assert_eq!(initial["model"], serde_json::Value::Null);
    assert!(
        initial["models"].as_array().is_some_and(|models| models
            .iter()
            .any(|model| model["file_name"] == "ggml-tiny.bin")),
        "{initial}"
    );

    let (_, model) = send(router.clone(), "PUT", Some(r#"{"model":"ggml-tiny.bin"}"#)).await;
    assert_eq!(model["model"], "ggml-tiny.bin");
    assert_eq!(model["enabled"], true);
    let (_, off) = send(router.clone(), "PUT", Some(r#"{"enabled":false}"#)).await;
    assert_eq!(off["enabled"], false);
    assert_eq!(off["model"], "ggml-tiny.bin", "a switch keeps the model");
    let (_, live) = send(router.clone(), "PUT", Some(r#"{"model":null}"#)).await;
    assert_eq!(live["model"], serde_json::Value::Null);
    let (_, after) = send(router, "GET", None).await;
    assert_eq!(after, live);
}

#[tokio::test]
async fn a_model_not_on_this_device_is_refused_and_nothing_changes() {
    let settings = ScratchSettings::new();
    let router = router(Some(&settings));

    let (status, body) = send(
        router.clone(),
        "PUT",
        Some(r#"{"enabled":false,"model":"ggml-large-v3.bin"}"#),
    )
    .await;
    let (_, after) = send(router, "GET", None).await;
    let (unavailable, _) = send(self::router(None), "GET", None).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error_code"], "invalid_model");
    assert_eq!(after["enabled"], true);
    assert_eq!(after["model"], serde_json::Value::Null);
    assert_eq!(unavailable, StatusCode::SERVICE_UNAVAILABLE);
}
