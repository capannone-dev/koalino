//! Specifies the Teams bridge as the desktop app hosts it (#123): on by default, never without a
//! secret, and with a pipeline that is built only when the first meeting arrives, so an idle app
//! loads no model.

use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use futures::SinkExt;
use protobuf::Message as _;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;
use zeroize::Zeroizing;

use super::{BridgePipeline, app_bridge_config, open_teams_bridge_ports, start_teams_bridge};
use crate::{
    adapters::{
        audio::chunk_audio_store::ChunkAudioStore,
        network::teams_bridge::message_decoder::local_meeting_uuid,
        storage::sqlite_adapter::SqliteNoteRepository,
    },
    application::{
        crypto::voice_encryptor::ephemeral_cipher, pipeline::pipeline_factory::PipelineFactory,
        recording_manager::RecordingManager,
    },
    domain::{
        errors::{CryptoError, TranscriptionError},
        models::{AudioRecording, DeviceId, MeetingId, Transcript, TranscriptionContext},
    },
    ports::{
        ai::TranscriptionEngine,
        crypto::{EmbeddingCipher, KeychainStore},
    },
    protocol::teams_bridge::{
        AudioChunk, ExternalParticipant, MeetingMetadata, TeamsBridgeMessage,
    },
};

/// Builds a lookup over fixed values.
fn lookup_of(variables: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let variables: Vec<(String, String)> = variables
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    move |name| {
        variables
            .iter()
            .find(|(variable, _)| variable == name)
            .map(|(_, value)| value.clone())
    }
}

/// An in-memory keychain entry that can also refuse to work.
#[derive(Default)]
struct MemoryKeychain {
    stored: Mutex<Option<Vec<u8>>>,
    broken: bool,
}

impl KeychainStore for MemoryKeychain {
    /// Returns the stored bytes, or fails when broken.
    fn load_key(&self) -> Result<Option<Zeroizing<Vec<u8>>>, CryptoError> {
        if self.broken {
            return Err(CryptoError::Keychain {
                reason: "locked".to_owned(),
            });
        }
        Ok(self
            .stored
            .lock()
            .ok()
            .and_then(|stored| stored.clone())
            .map(Zeroizing::new))
    }

    /// Replaces the stored bytes.
    fn store_key(&self, key_bytes: &[u8]) -> Result<(), CryptoError> {
        if let Ok(mut stored) = self.stored.lock() {
            *stored = Some(key_bytes.to_vec());
        }
        Ok(())
    }

    /// Names the entry.
    fn location(&self) -> &str {
        "memory"
    }
}

/// Verifies the app's bridge is on without any setting, and makes the secret the browser host
/// will read when the keychain has none.
#[test]
fn the_app_bridge_is_on_by_default_and_creates_its_secret() {
    let keychain = MemoryKeychain::default();
    let config = app_bridge_config(&lookup_of(&[]), &keychain)
        .unwrap_or_else(|| panic!("the bridge should be on"));
    assert!(config.enabled());
    assert!(config.requires_authentication());
    let stored = keychain
        .load_key()
        .unwrap_or_else(|error| panic!("keychain should read: {error}"))
        .unwrap_or_else(|| panic!("a secret should be stored"));
    assert_eq!(stored.len(), 64, "32 random bytes in hex");

    let again = app_bridge_config(&lookup_of(&[]), &keychain);
    assert!(again.is_some());
    let unchanged = keychain
        .load_key()
        .unwrap_or_else(|error| panic!("keychain should read: {error}"))
        .unwrap_or_else(|| panic!("the secret should remain"));
    assert_eq!(stored, unchanged, "an existing secret is kept");
}

/// Verifies the owner can turn the bridge off, and a configured secret is used as it is.
#[test]
fn the_app_bridge_follows_its_settings() {
    let keychain = MemoryKeychain::default();
    assert!(
        app_bridge_config(
            &lookup_of(&[("NOTE_TAKER_TEAMS_BRIDGE_ENABLED", "false")]),
            &keychain
        )
        .is_none()
    );
    let configured = app_bridge_config(
        &lookup_of(&[("NOTE_TAKER_TEAMS_BRIDGE_SECRET", "vm-test-secret")]),
        &keychain,
    )
    .unwrap_or_else(|| panic!("the bridge should be on"));
    assert!(configured.requires_authentication());
    assert!(
        keychain
            .load_key()
            .unwrap_or_else(|error| panic!("keychain should read: {error}"))
            .is_none(),
        "a configured secret leaves the keychain alone"
    );
}

/// Verifies that without a secret the app keeps the bridge off, rather than accepting any local
/// program.
#[test]
fn the_app_bridge_stays_off_without_a_secret() {
    let keychain = MemoryKeychain {
        broken: true,
        ..MemoryKeychain::default()
    };
    assert!(app_bridge_config(&lookup_of(&[]), &keychain).is_none());
}

/// Returns fixed words after reading the chunk audio, as real engines do.
struct StoreReadingEngine {
    chunk_audio_store: Arc<ChunkAudioStore>,
}

impl TranscriptionEngine for StoreReadingEngine {
    /// Consumes the chunk audio and returns fixed words.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        _transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        self.chunk_audio_store.load_wav(audio_recording)?;
        Ok(Transcript::new(
            audio_recording.meeting_id().clone(),
            "browser words",
        ))
    }
}

/// A unique database path, removed on drop.
struct TemporaryDatabase {
    path: PathBuf,
    cipher: Arc<dyn EmbeddingCipher>,
}

impl TemporaryDatabase {
    /// Reserves a unique path.
    fn new() -> Self {
        Self {
            path: std::env::temp_dir()
                .join(format!("note-taker-app-bridge-{}.sqlite3", Uuid::new_v4())),
            cipher: ephemeral_cipher(),
        }
    }

    /// Opens a repository on it.
    fn note_repository(&self) -> SqliteNoteRepository {
        SqliteNoteRepository::open(
            &self.path,
            DeviceId::new("app-test"),
            Arc::clone(&self.cipher),
        )
        .unwrap_or_else(|error| panic!("repository should open: {error}"))
    }
}

impl Drop for TemporaryDatabase {
    /// Removes the database and the slides folder beside it.
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Sends one protobuf message as a binary frame.
async fn send<Socket>(socket: &mut Socket, message: &TeamsBridgeMessage)
where
    Socket: futures::Sink<Message> + Unpin,
    Socket::Error: std::fmt::Display,
{
    let bytes = message
        .write_to_bytes()
        .unwrap_or_else(|error| panic!("message should encode: {error}"));
    socket
        .send(Message::binary(bytes))
        .await
        .unwrap_or_else(|error| panic!("frame should send: {error}"));
}

/// Sends a meeting start and one second of audio for `meeting_id`.
async fn send_meeting_with_audio<Socket>(socket: &mut Socket, meeting_id: &str)
where
    Socket: futures::Sink<Message> + Unpin,
    Socket::Error: std::fmt::Display,
{
    let mut metadata = MeetingMetadata::new();
    metadata.meeting_id = meeting_id.to_owned();
    let mut start = TeamsBridgeMessage::new();
    start.set_metadata(metadata);
    send(socket, &start).await;
    let mut owner = ExternalParticipant::new();
    owner.external_id = "browser-owner".to_owned();
    owner.display_name = "Owner".to_owned();
    let mut audio = AudioChunk::new();
    audio.participant = protobuf::MessageField::some(owner);
    audio.audio_data = vec![0; 32_000];
    audio.sample_rate = 16_000;
    let mut chunk = TeamsBridgeMessage::new();
    chunk.set_audio(audio);
    send(socket, &chunk).await;
}

/// Verifies the pipeline is built only when a meeting starts, a failed build drops that
/// meeting's media and is retried at the next meeting, and the built pipeline then transcribes.
#[tokio::test]
async fn the_pipeline_is_built_when_the_first_meeting_arrives() {
    let database = TemporaryDatabase::new();
    let builds = Arc::new(AtomicUsize::new(0));
    let built = Arc::new(Mutex::new(None::<Arc<RecordingManager>>));
    let pipeline = {
        let builds = Arc::clone(&builds);
        let built = Arc::clone(&built);
        let worker_repository = Mutex::new(Some(database.note_repository()));
        BridgePipeline::on_first_meeting(Box::new(move || {
            // The first attempt fails, as when no model is configured yet.
            if builds.fetch_add(1, Ordering::SeqCst) == 0 {
                anyhow::bail!("no transcription model yet");
            }
            let chunk_audio_store = Arc::new(ChunkAudioStore::new());
            let engine_audio_store = Arc::clone(&chunk_audio_store);
            let repository = worker_repository
                .lock()
                .ok()
                .and_then(|mut slot| slot.take())
                .ok_or_else(|| anyhow::anyhow!("built twice"))?;
            let recording_manager = Arc::new(
                RecordingManager::spawn(move || -> Result<_, String> {
                    Ok(PipelineFactory::new(
                        StoreReadingEngine {
                            chunk_audio_store: engine_audio_store,
                        },
                        repository,
                    ))
                })
                .map_err(|error| anyhow::anyhow!("{error}"))?,
            );
            if let Ok(mut slot) = built.lock() {
                *slot = Some(Arc::clone(&recording_manager));
            }
            Ok((recording_manager, chunk_audio_store))
        }))
    };
    let ports = open_teams_bridge_ports(
        &database.path,
        DeviceId::new("app-test"),
        Arc::clone(&database.cipher),
        None,
        pipeline,
    )
    .unwrap_or_else(|error| panic!("ports should open: {error}"));
    let config = app_bridge_config(
        &lookup_of(&[("NOTE_TAKER_TEAMS_BRIDGE_SECRET", "app-test-secret")]),
        &MemoryKeychain::default(),
    )
    .unwrap_or_else(|| panic!("the bridge should be on"))
    .with_port(0);
    let service = start_teams_bridge(&config, ports)
        .await
        .unwrap_or_else(|error| panic!("bridge should start: {error}"));
    assert_eq!(
        builds.load(Ordering::SeqCst),
        0,
        "nothing is built while idle"
    );

    let (mut socket, _response) = connect_async(format!("ws://{}", service.local_addr()))
        .await
        .unwrap_or_else(|error| panic!("client should connect: {error}"));
    socket
        .send(Message::text(r#"{"auth":"app-test-secret"}"#))
        .await
        .unwrap_or_else(|error| panic!("auth should send: {error}"));
    send_meeting_with_audio(&mut socket, "browser-teams-first").await;
    send_meeting_with_audio(&mut socket, "browser-teams-second").await;

    let second = MeetingId::new(local_meeting_uuid("browser-teams-second").to_string());
    let mut segment_count = 0;
    for _attempt in 0..200 {
        let manager = built.lock().ok().and_then(|slot| slot.clone());
        if let Some(manager) = manager {
            let meeting = second.clone();
            segment_count = tokio::task::spawn_blocking(move || {
                manager.with_note_repository(move |repository| {
                    repository.load_transcript_segments(&meeting)
                })
            })
            .await
            .unwrap_or_else(|error| panic!("poll should not panic: {error}"))
            .ok()
            .and_then(Result::ok)
            .map_or(0, |segments| segments.len());
            if segment_count >= 1 {
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    drop(socket);
    let statistics = service.shutdown().await;

    assert_eq!(
        builds.load(Ordering::SeqCst),
        2,
        "one failed build, one retry"
    );
    assert_eq!(segment_count, 1, "the second meeting is transcribed");
    assert_eq!(
        statistics.failed_audio_chunks, 1,
        "the first meeting's audio had no pipeline"
    );
    assert_eq!(statistics.processed_audio_chunks, 1);
}

/// Verifies that with the keychain turned off the app never asks it for the bridge secret (which
/// would raise a keyring prompt): the bridge needs a configured secret then.
#[test]
fn with_the_keychain_off_the_app_bridge_leaves_the_keychain_alone() {
    let keychain = MemoryKeychain::default();

    assert!(
        app_bridge_config(
            &lookup_of(&[("NOTE_TAKER_KEYCHAIN_ENABLED", "false")]),
            &keychain
        )
        .is_none()
    );
    assert!(
        keychain
            .load_key()
            .unwrap_or_else(|error| panic!("keychain should read: {error}"))
            .is_none(),
        "no secret was created"
    );
    assert!(
        app_bridge_config(
            &lookup_of(&[
                ("NOTE_TAKER_KEYCHAIN_ENABLED", "false"),
                ("NOTE_TAKER_TEAMS_BRIDGE_SECRET", "vm-test-secret"),
            ]),
            &keychain
        )
        .is_some(),
        "a configured secret still turns it on"
    );
}
