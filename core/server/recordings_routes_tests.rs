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

/// Sends a GET with a `Range` header; returns the status, the headers and the body.
async fn get_range(
    router: axum::Router,
    uri: &str,
    range: &str,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = router
        .oneshot(
            Request::builder()
                .uri(uri)
                .header("range", range)
                .body(Body::empty())
                .unwrap_or_else(|error| panic!("{error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    (status, headers, bytes.to_vec())
}

/// Browsers move in a long recording by asking for the part they need (#197).
#[tokio::test]
async fn a_recording_is_served_in_parts_for_seeking() {
    let scratch = Scratch::new();
    let uri = "/api/meetings/m-1/recording";
    let (_, _, whole) = send(scratch.router(), "GET", uri, None).await;
    let total = whole.len();

    let (status, headers, body) = get_range(scratch.router(), uri, "bytes=100-199").await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(body, whole[100..200]);
    assert_eq!(
        headers.get("content-range").and_then(|v| v.to_str().ok()),
        Some(format!("bytes 100-199/{total}").as_str())
    );
    assert_eq!(
        headers.get("accept-ranges").and_then(|v| v.to_str().ok()),
        Some("bytes")
    );

    let (status, _, body) = get_range(scratch.router(), uri, "bytes=1000-").await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(body, whole[1000..]);

    let (status, _, body) = get_range(scratch.router(), uri, "bytes=-44").await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(body, whole[total - 44..], "the last 44 bytes");

    let (status, headers, _) = get_range(scratch.router(), uri, &format!("bytes={total}-")).await;
    assert_eq!(status, StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(
        headers.get("content-range").and_then(|v| v.to_str().ok()),
        Some(format!("bytes */{total}").as_str())
    );

    let (status, _, body) = get_range(scratch.router(), uri, "bytes=0-1,5-9").await;
    assert_eq!(status, StatusCode::OK, "several ranges get the whole file");
    assert_eq!(body.len(), total);
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
