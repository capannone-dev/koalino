//! Defines the Rust core's public module boundary and its initial contract tests.
//!
//! This file exports the hexagonal layers. Production implementations are added in
//! their respective modules; tests below define the expected behavior first.

/// Contains infrastructure implementations and operational concerns.
pub mod adapters;
/// Contains use-case orchestration that only depends on domain types and ports.
pub mod application;
/// Exposes the dependency-injected command-line driving adapter.
pub mod cli;
/// Opens the encrypted SQLite stores for the CLI, daemon, and FFI composition roots.
/// Runs foreground CLI recordings and coordinates `record stop` across processes.
#[path = "bin/cli_recording.rs"]
pub mod cli_recording;
/// Resolves the acquisition device ID shared by the CLI, daemon, and FFI composition roots.
#[path = "bin/device_identity.rs"]
pub mod device_identity;
/// Contains pure business types and business failures.
pub mod domain;
/// Exposes the C-compatible Flutter bridge without leaking Rust domain types across FFI.
#[path = "bin/ffi.rs"]
pub mod ffi;
/// JNI entry points the Android app's Kotlin code calls (the Keystore key hand-over).
#[cfg(target_os = "android")]
#[path = "bin/ffi_android.rs"]
pub mod ffi_android;
/// Keeps the owner's default transcription language.
#[path = "bin/ffi_language.rs"]
pub mod ffi_language;
/// The app's log level and log export over the C ABI (Issue #46).
#[path = "bin/ffi_logging.rs"]
pub mod ffi_logging;
/// Turns the Flutter Devices screen's JSON requests into P2P commands.
#[path = "bin/ffi_p2p.rs"]
pub mod ffi_p2p;
/// Holds the settings the Flutter app passes to `note_taker_configure`, over the environment.
#[path = "bin/ffi_settings.rs"]
pub mod ffi_settings;
#[path = "bin/p2p_composition.rs"]
pub mod p2p_composition;
/// Composes the live meeting pipeline from environment configuration.
#[path = "bin/pipeline_composition.rs"]
pub mod pipeline_composition;
/// Contains dependency-inversion traits for external capabilities.
pub mod ports;
/// Defines wire protocols with external processes (the Teams bot bridge).
pub mod protocol;
/// Drives live recordings from raw PCM through the meeting pipeline worker.
#[path = "bin/recording_controller.rs"]
pub mod recording_controller;
/// Samples local screen shares beside a recording, on its own thread (Issue #49).
#[path = "bin/screen_share_watcher.rs"]
pub mod screen_share_watcher;
/// Exposes the headless Axum API composition without leaking HTTP details into application services.
pub mod server;
#[path = "bin/storage_composition.rs"]
pub mod storage_composition;
/// Composes the Teams bot bridge (WebSocket server, buffer, and meeting pump) for the daemon.
#[path = "bin/teams_bridge_composition.rs"]
pub mod teams_bridge_composition;
/// Composes and runs the daemon's P2P pairing and sync task (Issue #43).
#[path = "bin/transcription_host.rs"]
pub mod transcription_host;

#[cfg(test)]
mod scaffold_contract_tests {
    use crate::{
        application::meeting_recorder::MeetingRecorder,
        domain::models::{
            AudioRecording, DeviceId, MeetingId, RecordingId, Transcript, TranscriptionContext,
        },
        ports::{ai::TranscriptionEngine, storage::NoteRepository},
    };

    struct StubTranscriptionEngine;

    impl TranscriptionEngine for StubTranscriptionEngine {
        fn transcribe(
            &self,
            audio_recording: &AudioRecording,
            _transcription_context: &TranscriptionContext,
        ) -> Result<Transcript, crate::domain::errors::TranscriptionError> {
            Ok(Transcript::new(
                audio_recording.meeting_id().clone(),
                "Discussed offline synchronization.",
            ))
        }
    }

    #[derive(Default)]
    struct InMemoryNoteRepository;

    impl NoteRepository for InMemoryNoteRepository {
        fn create_meeting(
            &self,
            _meeting_id: &MeetingId,
            _initial_crdt_document: &[u8],
        ) -> Result<(), crate::domain::errors::StorageError> {
            Ok(())
        }

        fn save_transcript(
            &self,
            transcript: &Transcript,
        ) -> Result<(), crate::domain::errors::StorageError> {
            assert_eq!(transcript.content(), "Discussed offline synchronization.");
            Ok(())
        }

        fn load_transcript(
            &self,
            _meeting_id: &MeetingId,
        ) -> Result<Option<Transcript>, crate::domain::errors::StorageError> {
            Ok(None)
        }

        fn save_crdt_document(
            &self,
            _meeting_id: &MeetingId,
            _crdt_document: &[u8],
        ) -> Result<(), crate::domain::errors::StorageError> {
            Ok(())
        }

        fn load_crdt_document(
            &self,
            _meeting_id: &MeetingId,
        ) -> Result<Vec<u8>, crate::domain::errors::StorageError> {
            Ok(Vec::new())
        }

        fn save_unassigned_clip(
            &self,
            _clip: crate::domain::models::UnassignedVoiceClip,
        ) -> Result<(), crate::domain::errors::StorageError> {
            Ok(())
        }

        fn get_unassigned_clips(
            &self,
            _meeting_id: &MeetingId,
        ) -> Result<
            Vec<crate::domain::models::UnassignedVoiceClip>,
            crate::domain::errors::StorageError,
        > {
            Ok(Vec::new())
        }
    }

    /// Verifies that application orchestration depends on AI and storage ports rather than adapters.
    #[test]
    fn recorder_transcribes_and_persists_through_ports() {
        let meeting_identifier = MeetingId::new("meeting-42");
        let audio_recording = AudioRecording::new(
            RecordingId::new("recording-42"),
            meeting_identifier.clone(),
            DeviceId::new("device-rome"),
        );
        let meeting_recorder =
            MeetingRecorder::new(StubTranscriptionEngine, InMemoryNoteRepository);

        let persisted_transcript = meeting_recorder
            .process_recording(&audio_recording)
            .unwrap_or_else(|error| panic!("the port stubs should succeed: {error}"));

        assert_eq!(persisted_transcript.meeting_id(), &meeting_identifier);
        assert_eq!(
            persisted_transcript.content(),
            "Discussed offline synchronization."
        );
    }
}
