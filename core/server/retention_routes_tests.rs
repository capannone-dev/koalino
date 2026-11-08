//! Specifies the web Settings page's speech retention route (#93): reading and changing how many
//! days a recording's speech may wait to be combined with another device's recording.

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
            std::env::temp_dir().join(format!("note-taker-retention-{}", uuid::Uuid::new_v4()));
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
        .uri("/api/settings/retention");
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
async fn the_window_starts_at_three_weeks_and_can_be_changed() {
    let settings = ScratchSettings::new();
    let router = router(Some(&settings));

    let (status, initial) = send(router.clone(), "GET", None).await;
    let (saved_status, saved) = send(router.clone(), "PUT", Some(r#"{"days":7}"#)).await;
    let (_, after) = send(router, "GET", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        initial,
        serde_json::json!({ "days": 21, "min_days": 1, "max_days": 60 })
    );
    assert_eq!(saved_status, StatusCode::OK);
    assert_eq!(saved["days"], 7);
    assert_eq!(after, saved);
}

#[tokio::test]
async fn a_window_out_of_range_is_refused_and_nothing_changes() {
    let settings = ScratchSettings::new();
    let router = router(Some(&settings));

    let (status, body) = send(router.clone(), "PUT", Some(r#"{"days":0}"#)).await;
    let (_, after) = send(router.clone(), "GET", None).await;
    let (unavailable, _) = send(self::router(None), "GET", None).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error_code"], "invalid_window");
    assert_eq!(after["days"], 21);
    assert_eq!(unavailable, StatusCode::SERVICE_UNAVAILABLE);
}
