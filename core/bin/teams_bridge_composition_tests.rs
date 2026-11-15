//! Specifies the Teams bridge configuration and, end to end, the path from a mock bot over the
//! WebSocket through the pump into `MeetingSession` and the slide store.
//!
//! The transcription engine is a mock that reads (and so deletes) chunk audio from the shared
//! store; databases and slide directories live in unique temp paths that are removed afterwards.

use std::{path::PathBuf, sync::Arc, time::Duration};

use futures::SinkExt;
use protobuf::Message as _;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

use super::{TeamsBridgeConfig, TeamsBridgePorts, start_teams_bridge};
use crate::{
    adapters::{
        audio::chunk_audio_store::ChunkAudioStore,
        network::teams_bridge::message_decoder::local_meeting_uuid,
        storage::{
            slide_attachment_store::SlideAttachmentStore, sqlite_adapter::SqliteNoteRepository,
            sqlite_voice_vector_db::SqliteVoiceVectorDB,
        },
    },
    application::{
        crypto::voice_encryptor::ephemeral_cipher, pipeline::pipeline_factory::PipelineFactory,
        recording_manager::RecordingManager, teams_processor::TeamsIdentityMapper,
    },
    domain::{
        errors::TranscriptionError,
        models::{AudioRecording, DeviceId, MeetingId, Transcript, TranscriptionContext},
    },
    ports::{
        ai::TranscriptionEngine,
        crypto::EmbeddingCipher,
        storage::{AttachmentRepository, NoteRepository},
    },
    protocol::teams_bridge::{
        AudioChunk, ExternalParticipant, MeetingDocument, MeetingEnded, MeetingMetadata,
        TeamsBridgeMessage, VideoFrame,
    },
};

/// Builds a lookup over fixed variable values.
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

/// Verifies the defaults: disabled, port 8765, 10 s buffer, no secret.
#[test]
fn configuration_defaults_match_the_issue() {
    let config = TeamsBridgeConfig::from_lookup(lookup_of(&[]))
        .unwrap_or_else(|error| panic!("defaults should parse: {error}"));
    assert!(!config.enabled());
    assert_eq!(config.port(), 8765);
    assert_eq!(config.buffer_duration(), Duration::from_secs(10));
    assert!(!config.requires_authentication());
    assert_eq!(config.slide_directory(), None);
}

/// Verifies every variable is read, and an empty secret counts as unset.
#[test]
fn configuration_reads_every_variable() {
    let config = TeamsBridgeConfig::from_lookup(lookup_of(&[
        ("NOTE_TAKER_TEAMS_BRIDGE_ENABLED", "true"),
        ("NOTE_TAKER_TEAMS_BRIDGE_PORT", "9123"),
        ("NOTE_TAKER_TEAMS_BRIDGE_SECRET", "hunter2-but-longer"),
        ("NOTE_TAKER_TEAMS_BRIDGE_BUFFER_SECONDS", "20"),
        (
            "NOTE_TAKER_TEAMS_BRIDGE_SLIDE_DIR",
            "/var/lib/note-taker/slides",
        ),
    ]))
    .unwrap_or_else(|error| panic!("config should parse: {error}"));
    assert!(config.enabled());
    assert_eq!(config.port(), 9123);
    assert!(config.requires_authentication());
    assert_eq!(config.buffer_duration(), Duration::from_secs(20));
    assert_eq!(
        config.slide_directory(),
        Some(PathBuf::from("/var/lib/note-taker/slides").as_path())
    );
    assert!(
        !format!("{config:?}").contains("hunter2"),
        "the secret must be redacted"
    );

    let blank_secret =
        TeamsBridgeConfig::from_lookup(lookup_of(&[("NOTE_TAKER_TEAMS_BRIDGE_SECRET", "  ")]))
            .unwrap_or_else(|error| panic!("config should parse: {error}"));
    assert!(!blank_secret.requires_authentication());
}

/// Verifies invalid values are rejected with the variable's name.
#[test]
fn configuration_rejects_invalid_values() {
    for (name, value) in [
        ("NOTE_TAKER_TEAMS_BRIDGE_ENABLED", "maybe"),
        ("NOTE_TAKER_TEAMS_BRIDGE_PORT", "0"),
        ("NOTE_TAKER_TEAMS_BRIDGE_PORT", "70000"),
        ("NOTE_TAKER_TEAMS_BRIDGE_BUFFER_SECONDS", "0"),
        ("NOTE_TAKER_TEAMS_BRIDGE_BUFFER_SECONDS", "301"),
        ("NOTE_TAKER_TEAMS_BRIDGE_BUFFER_SECONDS", "ten"),
    ] {
        match TeamsBridgeConfig::from_lookup(lookup_of(&[(name, value)])) {
            Ok(_) => panic!("{name}={value} should be rejected"),
            Err(error) => assert!(error.to_string().contains(name), "{error}"),
        }
    }
}

/// A keychain holding a fixed bridge secret.
struct FixedSecretKeychain(&'static str);

impl crate::ports::crypto::KeychainStore for FixedSecretKeychain {
    /// Returns the secret's bytes.
    fn load_key(
        &self,
    ) -> Result<Option<zeroize::Zeroizing<Vec<u8>>>, crate::domain::errors::CryptoError> {
        Ok(Some(zeroize::Zeroizing::new(self.0.as_bytes().to_vec())))
    }

    /// Refuses writes.
    fn store_key(&self, _key_bytes: &[u8]) -> Result<(), crate::domain::errors::CryptoError> {
        Err(crate::domain::errors::CryptoError::Keychain {
            reason: "read-only".to_owned(),
        })
    }

    /// Names the entry.
    fn location(&self) -> &str {
        "fixed"
    }
}

/// Verifies the keychain's secret (#102) is used only when the environment sets none.
#[test]
fn the_keychain_secret_applies_only_without_an_environment_secret() {
    let without_secret = TeamsBridgeConfig::from_lookup(lookup_of(&[]))
        .unwrap_or_else(|error| panic!("config should parse: {error}"))
        .with_keychain_secret(&FixedSecretKeychain("from-keychain"));
    assert!(without_secret.requires_authentication());

    let with_secret = TeamsBridgeConfig::from_lookup(lookup_of(&[(
        "NOTE_TAKER_TEAMS_BRIDGE_SECRET",
        "from-environment",
    )]))
    .unwrap_or_else(|error| panic!("config should parse: {error}"))
    .with_keychain_secret(&FixedSecretKeychain("from-keychain"));
    assert!(with_secret.requires_authentication());
    assert!(
        !format!("{with_secret:?}").contains("from-"),
        "never printed"
    );
}

/// Transcribes by reading the chunk's audio from the shared store, as real engines do.
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
            "teams words",
        ))
    }
}

/// A temporary database file and slide directory, removed on drop.
struct TemporaryInstallation {
    database_path: PathBuf,
    slide_directory: PathBuf,
    cipher: Arc<dyn EmbeddingCipher>,
}

impl TemporaryInstallation {
    /// Reserves unique paths.
    fn new() -> Self {
        let unique = Uuid::new_v4();
        Self {
            database_path: std::env::temp_dir().join(format!("note-taker-teams-{unique}.sqlite3")),
            slide_directory: std::env::temp_dir().join(format!("note-taker-teams-slides-{unique}")),
            cipher: ephemeral_cipher(),
        }
    }

    /// Opens a note repository on the database.
    fn note_repository(&self) -> SqliteNoteRepository {
        SqliteNoteRepository::open(
            &self.database_path,
            DeviceId::new("device-test"),
            Arc::clone(&self.cipher),
        )
        .unwrap_or_else(|error| panic!("repository should open: {error}"))
    }
}

impl Drop for TemporaryInstallation {
    /// Removes the database and slides.
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.database_path);
        let _ = std::fs::remove_dir_all(&self.slide_directory);
    }
}

/// Serializes a message into a binary frame.
fn frame(message: &TeamsBridgeMessage) -> Message {
    Message::binary(
        message
            .write_to_bytes()
            .unwrap_or_else(|error| panic!("message should encode: {error}")),
    )
}

/// Sends `message` from the mock bot, panicking on transport errors.
async fn send_frame<Socket>(bot: &mut Socket, message: Message)
where
    Socket: futures::Sink<Message> + Unpin,
    Socket::Error: std::fmt::Display,
{
    bot.send(message)
        .await
        .unwrap_or_else(|error| panic!("bot should send: {error}"));
}

/// Builds a participant.
fn participant(external_id: &str, display_name: &str) -> ExternalParticipant {
    let mut participant = ExternalParticipant::new();
    participant.external_id = external_id.to_owned();
    participant.display_name = display_name.to_owned();
    participant
}

/// Verifies five bot audio chunks become five Known segments, the slide and a captions document
/// are attached, the meeting end closes the meeting, and no raw audio is left behind.
#[tokio::test]
async fn five_audio_chunks_are_processed_by_the_meeting_session() {
    let installation = TemporaryInstallation::new();
    let chunk_audio_store = Arc::new(ChunkAudioStore::new());
    let engine_audio_store = Arc::clone(&chunk_audio_store);
    let worker_repository = installation.note_repository();
    let recording_manager = Arc::new(
        RecordingManager::spawn(move || -> Result<_, String> {
            Ok(PipelineFactory::new(
                StoreReadingEngine {
                    chunk_audio_store: engine_audio_store,
                },
                worker_repository,
            ))
        })
        .unwrap_or_else(|error| panic!("manager should start: {error}")),
    );
    let voice_store = SqliteVoiceVectorDB::open(
        &installation.database_path,
        Arc::clone(&installation.cipher),
    )
    .unwrap_or_else(|error| panic!("voice store should open: {error}"));
    let config = TeamsBridgeConfig::from_lookup(lookup_of(&[
        ("NOTE_TAKER_TEAMS_BRIDGE_ENABLED", "true"),
        ("NOTE_TAKER_TEAMS_BRIDGE_SECRET", "bridge-test-secret"),
    ]))
    .unwrap_or_else(|error| panic!("config should parse: {error}"))
    .with_port(0);
    let service = start_teams_bridge(
        &config,
        TeamsBridgePorts {
            pipeline: super::BridgePipeline::ready(
                Arc::clone(&recording_manager),
                Arc::clone(&chunk_audio_store),
            ),
            identity_mapper: Arc::new(TeamsIdentityMapper::new(std::sync::Mutex::new(voice_store))),
            slide_workflow: {
                let note_repository = installation.note_repository();
                let sealed_files = note_repository.sealed_files();
                SlideAttachmentStore::new(
                    installation.slide_directory.clone(),
                    note_repository,
                    DeviceId::new("device-test"),
                    sealed_files,
                )
            },
            device_id: DeviceId::new("device-test"),
        },
    )
    .await
    .unwrap_or_else(|error| panic!("bridge should start: {error}"));

    let (mut bot, _response) = connect_async(format!("ws://{}", service.local_addr()))
        .await
        .unwrap_or_else(|error| panic!("bot should connect: {error}"));
    send_frame(&mut bot, Message::text(r#"{"auth":"bridge-test-secret"}"#)).await;
    let mut metadata = MeetingMetadata::new();
    metadata.meeting_id = "19:integration@thread.v2".to_owned();
    metadata.meeting_title = "  Weekly sync ".to_owned();
    metadata
        .participants
        .push(participant("teams-user-1", "Ada Lovelace"));
    metadata
        .participants
        .push(participant("teams-user-2", "Alan Turing"));
    let mut metadata_message = TeamsBridgeMessage::new();
    metadata_message.set_metadata(metadata.clone());
    send_frame(&mut bot, frame(&metadata_message)).await;
    // A roster change re-sends the metadata; it must not replace the stored title.
    metadata.meeting_title = "Renamed by Teams".to_owned();
    let mut resent_metadata_message = TeamsBridgeMessage::new();
    resent_metadata_message.set_metadata(metadata);
    send_frame(&mut bot, frame(&resent_metadata_message)).await;
    for chunk_index in 0..5_u64 {
        let mut audio = AudioChunk::new();
        let speaker = if chunk_index % 2 == 0 {
            "teams-user-1"
        } else {
            "teams-user-2"
        };
        audio.participant = protobuf::MessageField::some(participant(speaker, speaker));
        audio.audio_data = vec![0; 32_000];
        audio.timestamp_ms = chunk_index * 1_000;
        audio.sample_rate = 16_000;
        let mut audio_message = TeamsBridgeMessage::new();
        audio_message.set_audio(audio);
        send_frame(&mut bot, frame(&audio_message)).await;
    }
    let mut video = VideoFrame::new();
    video.image_data = b"\x89PNG\r\n\x1a\nslide".to_vec();
    video.timestamp_ms = 2_500;
    video.source = "screen_share".to_owned();
    let mut video_message = TeamsBridgeMessage::new();
    video_message.set_video(video);
    send_frame(&mut bot, frame(&video_message)).await;
    // The browser extension's captions and meeting end (#102).
    let mut document = MeetingDocument::new();
    document.kind = "teams_captions".to_owned();
    document.media_type = "text/vtt".to_owned();
    document.content = b"WEBVTT\n".to_vec();
    let mut document_message = TeamsBridgeMessage::new();
    document_message.set_document(document);
    send_frame(&mut bot, frame(&document_message)).await;
    let mut ended = MeetingEnded::new();
    ended.meeting_id = "19:integration@thread.v2".to_owned();
    let mut ended_message = TeamsBridgeMessage::new();
    ended_message.set_ended(ended);
    send_frame(&mut bot, frame(&ended_message)).await;

    let meeting_id = MeetingId::new(local_meeting_uuid("19:integration@thread.v2").to_string());
    let mut stored_segments = Vec::new();
    for _attempt in 0..200 {
        let polled_manager = Arc::clone(&recording_manager);
        let polled_meeting = meeting_id.clone();
        stored_segments = tokio::task::spawn_blocking(move || {
            polled_manager.with_note_repository(move |note_repository| {
                note_repository.load_transcript_segments(&polled_meeting)
            })
        })
        .await
        .unwrap_or_else(|error| panic!("poll should not panic: {error}"))
        .unwrap_or_else(|error| panic!("worker should answer: {error}"))
        .unwrap_or_else(|error| panic!("segments should load: {error}"));
        if stored_segments.len() >= 5 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    drop(bot);
    let statistics = service.shutdown().await;

    assert_eq!(stored_segments.len(), 5, "every chunk becomes one segment");
    let identities: Vec<_> = stored_segments
        .iter()
        .map(|segment| segment.speaker_identity_id())
        .collect();
    assert!(
        identities.iter().all(Option::is_some),
        "segments are Known: {identities:?}"
    );
    assert_eq!(identities[0], identities[2]);
    assert_ne!(identities[0], identities[1]);
    assert_eq!(statistics.processed_audio_chunks, 5);
    assert_eq!(statistics.stored_slides, 1);
    assert_eq!(statistics.stored_documents, 1);
    assert_eq!(statistics.meetings_ended, 1);
    assert!(
        chunk_audio_store.is_empty(),
        "raw audio must not outlive transcription"
    );
    let attachments = installation
        .note_repository()
        .get_attachments_for_meeting(&meeting_id)
        .unwrap_or_else(|error| panic!("attachments should load: {error}"));
    assert_eq!(attachments.len(), 2, "the slide and the captions");
    assert!(
        attachments
            .iter()
            .any(|attachment| attachment.mime_type() == "text/vtt")
    );
    let stored_title = installation
        .note_repository()
        .load_meeting_metadata(&meeting_id)
        .unwrap_or_else(|error| panic!("metadata should load: {error}"))
        .and_then(|meeting_metadata| {
            meeting_metadata
                .title()
                .map(|title| title.as_str().to_owned())
        });
    assert_eq!(stored_title.as_deref(), Some("Weekly sync"));
}
