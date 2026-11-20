//! Specifies the web routes for the owner's kept meeting recordings (#140): the switch, playing a
//! meeting's recording as WAV, and wiping them all.

use std::{path::PathBuf, sync::Arc};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::{
    adapters::{
        audio::retained_audio_store::RetainedAudioStore,
        storage::{
            in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
        },
    },
    application::{
        crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
        transcript_refinement::ChunkSpan,
    },
    domain::models::MeetingId,
    kept_recordings::{RecordingsServices, kept_recordings_directory},
    ports::crypto::EmbeddingCipher,
    server::api::{ServerAppState, build_router},
};

/// A database folder with one kept recording of meeting `m-1`.
struct Scratch {
    folder: PathBuf,
    database: PathBuf,
    cipher: Arc<dyn EmbeddingCipher>,
}

impl Scratch {
    fn new() -> Self {
        let folder = std::env::temp_dir().join(format!("nt-recordings-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
        let scratch = Self {
            database: folder.join("db.sqlite3"),
            folder,
            cipher: Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([5; 32]))),
        };
        RetainedAudioStore::new(
            kept_recordings_directory(&scratch.database),
            Arc::clone(&scratch.cipher),
        )
        .append_chunk(
            &MeetingId::new("m-1"),
            ChunkSpan {
                start_ms: 500,
                end_ms: 1_500,
            },
            None,
            &[700; 16_000],
        )
        .unwrap_or_else(|error| panic!("{error}"));
        scratch
    }

    fn router(&self) -> axum::Router {
        let repository = SqliteNoteRepository::open_in_memory(Arc::clone(&self.cipher))
            .unwrap_or_else(|error| panic!("{error}"));
        build_router(
            ServerAppState::new(repository, InMemoryVoiceVectorDB::new()).with_recordings(
                Arc::new(RecordingsServices::for_database(
                    &self.database,
                    Arc::clone(&self.cipher),
                    self.folder.join("playback"),
                )),
            ),
        )
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.folder);
    }
}

async fn send(
    router: axum::Router,
    method: &str,
    uri: &str,
    body: Option<&str>,
) -> (StatusCode, Option<String>, Vec<u8>) {
    let mut request = Request::builder().method(method).uri(uri);
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    let response = router
        .oneshot(
            request
                .body(Body::from(body.unwrap_or_default().to_owned()))
                .unwrap_or_else(|error| panic!("{error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    (status, content_type, bytes.to_vec())
}

fn json(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_slice(bytes).unwrap_or_else(|error| panic!("{error}"))
}

#[tokio::test]
async fn the_switch_is_read_and_saved() {
    let scratch = Scratch::new();
    let (status, _, body) = send(scratch.router(), "GET", "/api/settings/recordings", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json(&body)["keep"], false);
    assert_eq!(json(&body)["count"], 1);

    let (status, _, body) = send(
        scratch.router(),
        "PUT",
        "/api/settings/recordings",
        Some(r#"{"keep":true}"#),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json(&body)["keep"], true);
}

#[tokio::test]
async fn a_meeting_recording_plays_as_wav_and_a_missing_one_is_404() {
    let scratch = Scratch::new();
    let (_, _, body) = send(
        scratch.router(),
        "GET",
        "/api/meetings/m-1/recording/available",
        None,
    )
    .await;
    assert_eq!(json(&body)["available"], true);
    let (_, _, body) = send(
        scratch.router(),
        "GET",
        "/api/meetings/x/recording/available",
        None,
    )
    .await;
    assert_eq!(json(&body)["available"], false);
    let (status, content_type, body) =
        send(scratch.router(), "GET", "/api/meetings/m-1/recording", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type.as_deref(), Some("audio/wav"));
    assert_eq!(&body[..4], b"RIFF");
    assert_eq!(
        body.len(),
        44 + 1_500 * 16 * 2,
        "silence to 0.5 s, then 1 s of speech"
    );

    let (status, _, body) = send(
        scratch.router(),
        "GET",
        "/api/meetings/other/recording",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json(&body)["error_code"], "not_found");
}

#[tokio::test]
async fn wiping_deletes_every_recording() {
    let scratch = Scratch::new();
    let (status, _, body) = send(scratch.router(), "DELETE", "/api/recordings", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json(&body)["deleted"], 1);
    let (_, _, body) = send(scratch.router(), "GET", "/api/settings/recordings", None).await;
    assert_eq!(json(&body)["count"], 0);
}
