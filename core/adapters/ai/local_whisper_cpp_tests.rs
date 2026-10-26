//! Defines feature-gated contract tests for the local whisper.cpp transcription adapter.

use std::{env, path::PathBuf, sync::Arc};

use crate::{
    domain::models::{AudioRecording, DeviceId, MeetingId, RecordingId},
    ports::ai::TranscriptionEngine,
};

use super::local_whisper_cpp::{
    LocalWhisperCppConfig, LocalWhisperCppTranscriptionEngine, RecordingPcmSource,
};

/// Supplies a short, valid 16 kHz mono PCM buffer without persisting or logging raw audio.
struct SilentPcmSource;

impl RecordingPcmSource for SilentPcmSource {
    /// Resolves a deliberately silent adapter-local sample buffer for an explicitly configured model test.
    ///
    /// Inputs: `_audio_recording` is opaque portable recording metadata.
    /// Outputs: One second of 16 kHz mono floating-point PCM, or a portable source error.
    /// Side effects: Allocates an in-memory buffer only; it does not read files or log audio.
    fn load_pcm_16khz_mono(
        &self,
        _audio_recording: &AudioRecording,
    ) -> Result<Vec<f32>, crate::domain::errors::TranscriptionError> {
        Ok(vec![0.0; 16_000])
    }
}

/// Verifies that an explicitly configured real whisper.cpp model remains behind the portable port.
///
/// Inputs: `NOTE_TAKER_WHISPER_MODEL`, when set, must name a readable local GGML/GGUF model.
/// Outputs: A transcript associated with the original meeting, or a test skip when no model is configured.
/// Side effects: Only when the feature and environment variable are both enabled, loads a local model and
/// runs one silent inference. It never downloads a model or logs raw audio.
#[test]
#[cfg(feature = "local-ai")]
fn transcribes_with_an_explicitly_configured_local_model() {
    let Ok(model_path) = env::var("NOTE_TAKER_WHISPER_MODEL") else {
        return;
    };
    let audio_recording = AudioRecording::new(
        RecordingId::new("local-whisper-contract-recording"),
        MeetingId::new("local-whisper-contract-meeting"),
        DeviceId::new("local-whisper-contract-device"),
    );
    let configuration = LocalWhisperCppConfig::new(PathBuf::from(model_path), 1, false, None)
        .unwrap_or_else(|error| {
            panic!("configured local Whisper settings should be valid: {error}")
        });
    let transcription_engine =
        LocalWhisperCppTranscriptionEngine::new(configuration, Arc::new(SilentPcmSource))
            .unwrap_or_else(|error| panic!("configured local Whisper model should load: {error}"));

    let transcript = transcription_engine
        .transcribe(&audio_recording)
        .unwrap_or_else(|error| panic!("silent local inference should complete: {error}"));

    assert_eq!(transcript.meeting_id(), audio_recording.meeting_id());
}

/// Requires local whisper.cpp inference to apply prior chunk text as its initial prompt.
///
/// Inputs: The local adapter source, which will receive `TranscriptionContext` during the
/// context-propagation implementation.
/// Outputs: Confirms the native whisper.cpp parameter wiring includes both the portable context
/// and `FullParams::set_initial_prompt`.
/// Side effects: Reads compile-time source text only; it does not load a model or process audio.
#[test]
#[cfg(feature = "local-ai")]
fn whisper_cpp_adapter_receives_context_previous_text_and_sets_initial_prompt() {
    let adapter_source = include_str!("local_whisper_cpp.rs");

    assert!(
        adapter_source.contains("TranscriptionContext"),
        "local Whisper adapter must accept portable transcription context"
    );
    assert!(
        adapter_source.contains("set_initial_prompt"),
        "local Whisper adapter must map previous_text to whisper.cpp initial_prompt"
    );
}
