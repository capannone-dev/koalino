//! Defines the Rust core's public module boundary and its initial contract tests.
//!
//! This file exports the hexagonal layers. Production implementations are added in
//! their respective modules; tests below define the expected behavior first.

/// Contains infrastructure implementations and operational concerns.
pub mod adapters;
/// Contains use-case orchestration that only depends on domain types and ports.
pub mod application;
/// Summarizes finished meetings automatically with the default AI connection (#101).
#[path = "bin/auto_summaries.rs"]
pub mod auto_summaries;
#[cfg(test)]
#[path = "bin/auto_summaries_tests.rs"]
mod auto_summaries_tests;
/// Notices calls for the desktop app: Settings and polls (#111).
#[path = "bin/call_watch_commands.rs"]
pub mod call_watch_commands;
#[cfg(test)]
#[path = "bin/call_watch_commands_tests.rs"]
mod call_watch_commands_tests;
/// Clear copies of sealed attachments for the app's viewers (#99).
#[path = "bin/clear_copies.rs"]
pub mod clear_copies;
#[cfg(test)]
#[path = "bin/clear_copies_tests.rs"]
mod clear_copies_tests;
/// Exposes the dependency-injected command-line driving adapter.
pub mod cli;
/// Opens the encrypted SQLite stores for the CLI, daemon, and FFI composition roots.
/// Runs foreground CLI recordings and coordinates `record stop` across processes.
#[path = "bin/cli_recording.rs"]
pub mod cli_recording;
/// Resolves the acquisition device ID shared by the CLI, daemon, and FFI composition roots.
#[path = "bin/device_identity.rs"]
pub mod device_identity;
/// Keeps the name the owner gave this device, shown to nearby and paired devices (#121).
#[path = "bin/device_name_settings.rs"]
pub mod device_name_settings;
#[cfg(test)]
#[path = "bin/device_name_settings_tests.rs"]
mod device_name_settings_tests;
/// Reads free disk space and total memory.
#[path = "bin/device_resources.rs"]
pub mod device_resources;
/// Contains pure business types and business failures.
pub mod domain;
/// Exposes the C-compatible Flutter bridge without leaking Rust domain types across FFI.
#[path = "bin/ffi.rs"]
pub mod ffi;
/// JNI entry points the Android app's Kotlin code calls (the Keystore key hand-over).
#[cfg(target_os = "android")]
#[path = "bin/ffi_android.rs"]
pub mod ffi_android;
/// The assistant access setting over the C ABI (#112).
#[path = "bin/ffi_assistant_access.rs"]
pub mod ffi_assistant_access;
/// The call watch over the C ABI (#111).
#[path = "bin/ffi_call_watch.rs"]
pub mod ffi_call_watch;
/// Chat with notes over the C ABI (#97).
#[path = "bin/ffi_chat.rs"]
pub mod ffi_chat;
/// The owner's edits to a meeting over the C ABI (#96).
#[path = "bin/ffi_edits.rs"]
pub mod ffi_edits;
#[cfg(test)]
#[path = "bin/ffi_edits_tests.rs"]
mod ffi_edits_tests;
/// Keeps the owner's default transcription language.
#[path = "bin/ffi_language.rs"]
pub mod ffi_language;
/// AI connections and meeting summaries over the C ABI (#95).
#[path = "bin/ffi_llm.rs"]
pub mod ffi_llm;
/// The app's log level and log export over the C ABI (Issue #46).
#[path = "bin/ffi_logging.rs"]
pub mod ffi_logging;
/// The notes folder's Settings and rounds over the C ABI (#98).
#[path = "bin/ffi_notes_folder.rs"]
pub mod ffi_notes_folder;
/// Turns the Flutter Devices screen's JSON requests into P2P commands.
#[path = "bin/ffi_p2p.rs"]
pub mod ffi_p2p;
/// Holds the settings the Flutter app passes to `note_taker_configure`, over the environment.
#[path = "bin/ffi_settings.rs"]
pub mod ffi_settings;
/// The desktop app's Teams bridge for the browser extension (#123).
#[path = "bin/ffi_teams_bridge.rs"]
pub mod ffi_teams_bridge;
/// Settings' Whisper model controls over the C ABI.
#[path = "bin/ffi_transcription.rs"]
pub mod ffi_transcription;
/// Runs the language model commands shared by the app and the web API (#95).
#[path = "bin/llm_commands.rs"]
pub mod llm_commands;
#[cfg(test)]
#[path = "bin/llm_commands_tests.rs"]
mod llm_commands_tests;
/// Keeps the owner's language model connections, summary template and their keys (#95).
#[path = "bin/llm_settings.rs"]
pub mod llm_settings;
#[cfg(test)]
#[path = "bin/llm_settings_tests.rs"]
mod llm_settings_tests;
/// Answers the owner's AI assistant over MCP, read-only and off until turned on (#112).
#[path = "bin/mcp_server.rs"]
pub mod mcp_server;
#[cfg(test)]
#[path = "bin/mcp_server_tests.rs"]
mod mcp_server_tests;
/// Prepares, writes and keeps meeting summaries for the app, web API and CLI (#95).
#[path = "bin/meeting_summaries.rs"]
pub mod meeting_summaries;
#[cfg(test)]
#[path = "bin/meeting_summaries_tests.rs"]
mod meeting_summaries_tests;
/// Chat with notes for the app, the daemon and the CLI (#97).
#[path = "bin/notes_chat_commands.rs"]
pub mod notes_chat_commands;
#[cfg(test)]
#[path = "bin/notes_chat_commands_tests.rs"]
mod notes_chat_commands_tests;
/// Keeps the opt-in notes folder: plain files mirroring projects and meetings (#98).
#[path = "bin/notes_folder.rs"]
pub mod notes_folder;
/// Runs the notes folder for the app and the daemon: settings commands and rounds (#98).
#[path = "bin/notes_folder_commands.rs"]
pub mod notes_folder_commands;
#[cfg(test)]
#[path = "bin/notes_folder_commands_tests.rs"]
mod notes_folder_commands_tests;
#[cfg(test)]
#[path = "bin/notes_folder_tests.rs"]
mod notes_folder_tests;
#[path = "bin/p2p_composition.rs"]
pub mod p2p_composition;
#[path = "bin/p2p_network_routes.rs"]
pub mod p2p_network_routes;
/// Answers paired devices' chat questions on a thread of their own (#107).
#[path = "bin/paired_chat_host.rs"]
pub mod paired_chat_host;
#[cfg(test)]
#[path = "bin/paired_chat_host_tests.rs"]
mod paired_chat_host_tests;
/// Composes the live meeting pipeline from environment configuration.
#[path = "bin/pipeline_composition.rs"]
pub mod pipeline_composition;
/// Contains dependency-inversion traits for external capabilities.
pub mod ports;
/// Drives live recordings from raw PCM through the meeting pipeline worker.
#[path = "bin/project_commands.rs"]
pub mod project_commands;
#[cfg(test)]
#[path = "bin/project_commands_tests.rs"]
mod project_commands_tests;
/// Defines wire protocols with external processes (the Teams bot bridge).
pub mod protocol;
#[path = "bin/recording_controller.rs"]
pub mod recording_controller;
/// Runs the after-meeting transcript pass on its own thread (#87).
#[path = "bin/refinement_worker.rs"]
pub mod refinement_worker;
#[cfg(test)]
#[path = "bin/refinement_worker_tests.rs"]
mod refinement_worker_tests;
/// Samples local screen shares beside a recording, on its own thread (Issue #49).
#[path = "bin/screen_share_watcher.rs"]
pub mod screen_share_watcher;
/// The search answer shared by the app, the web and the CLI (#100).
#[path = "bin/search_commands.rs"]
pub mod search_commands;
#[cfg(test)]
#[path = "bin/search_commands_tests.rs"]
mod search_commands_tests;
/// Exposes the headless Axum API composition without leaking HTTP details into application services.
pub mod server;
/// Reads who is speaking from call windows beside a recording (#113).
#[path = "bin/speaker_name_watcher.rs"]
pub mod speaker_name_watcher;
/// Times a public clip with Automatic's Whisper model on this device (#109).
#[path = "bin/speed_check.rs"]
pub mod speed_check;
#[cfg(test)]
#[path = "bin/speed_check_tests.rs"]
mod speed_check_tests;
#[path = "bin/storage_composition.rs"]
pub mod storage_composition;
/// Composes the Teams bot bridge (WebSocket server, buffer, and meeting pump) for the daemon.
#[path = "bin/teams_bridge_composition.rs"]
pub mod teams_bridge_composition;
/// Composes and runs the daemon's P2P pairing and sync task (Issue #43).
#[path = "bin/transcription_host.rs"]
pub mod transcription_host;
/// Keeps the owner's transcription choices.
#[path = "bin/transcription_settings.rs"]
pub mod transcription_settings;
#[cfg(test)]
#[path = "bin/transcription_settings_tests.rs"]
mod transcription_settings_tests;

/// Times speed checks by thread CPU time, so load does not fail them (#81).
#[cfg(test)]
mod cpu_stopwatch_for_tests;

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
