//! Defines feature-gated contract tests for the local whisper.cpp transcription adapter.

use std::{env, path::PathBuf, sync::Arc};

use crate::{
    domain::{
        models::{AudioRecording, DeviceId, MeetingId, RecordingId, TranscriptionContext},
        transcription_language::TranscriptionLanguage,
    },
    ports::ai::TranscriptionEngine,
};

use super::local_whisper_cpp::{
    LocalWhisperCppConfig, LocalWhisperCppTranscriptionEngine, RecordingPcmSource,
    is_non_speech_marker,
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
        .transcribe(&audio_recording, &TranscriptionContext::default())
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

/// Verifies whisper.cpp's non-speech markers are recognized, so they never reach the transcript
/// (Issue #51), while real text, even with brackets inside, is kept.
#[test]
fn non_speech_markers_are_recognized() {
    for marker in [
        "[BLANK_AUDIO]",
        " [Music] ",
        "(silence)",
        "[ Silence ]",
        "*applause*",
    ] {
        assert!(is_non_speech_marker(marker), "{marker}");
    }
    for text in [
        "Hello team.",
        "[Laughs] that was funny",
        "We said (quietly) hello.",
        "",
    ] {
        assert!(!is_non_speech_marker(text), "{text}");
    }
}

/// Supplies a 16 kHz mono PCM16 WAV file's samples.
#[cfg(feature = "local-ai")]
struct WavFileSource(PathBuf);

#[cfg(feature = "local-ai")]
impl RecordingPcmSource for WavFileSource {
    /// Reads the file, skipping its 44-byte header.
    fn load_pcm_16khz_mono(
        &self,
        _audio_recording: &AudioRecording,
    ) -> Result<Vec<f32>, crate::domain::errors::TranscriptionError> {
        let bytes = std::fs::read(&self.0)
            .unwrap_or_else(|error| panic!("the speech file must be readable: {error}"));
        let (samples, _) = bytes[44..].as_chunks::<2>();
        Ok(samples
            .iter()
            .map(|sample| f32::from(i16::from_le_bytes(*sample)) / 32_768.0)
            .collect())
    }
}

/// Verifies with real speech that a chunk's language overrides the engine's configuration. Needs
/// `NOTE_TAKER_WHISPER_MODEL` and `NOTE_TAKER_TEST_ENGLISH_SPEECH_WAV`.
#[test]
#[ignore = "needs a Whisper model and an English speech file"]
#[cfg(feature = "local-ai")]
fn a_chunks_language_overrides_the_configured_one() {
    let path = |variable: &str| {
        PathBuf::from(env::var(variable).unwrap_or_else(|_| panic!("set {variable}")))
    };
    let configuration = LocalWhisperCppConfig::new(
        path("NOTE_TAKER_WHISPER_MODEL"),
        4,
        false,
        Some("it".to_owned()),
    )
    .unwrap_or_else(|error| panic!("the configuration must be valid: {error}"));
    let engine = LocalWhisperCppTranscriptionEngine::new(
        configuration,
        Arc::new(WavFileSource(path("NOTE_TAKER_TEST_ENGLISH_SPEECH_WAV"))),
    )
    .unwrap_or_else(|error| panic!("the model must load: {error}"));
    let recording = AudioRecording::new(
        RecordingId::new("language-recording"),
        MeetingId::new("language-meeting"),
        DeviceId::new("language-device"),
    );
    let transcribe = |language: Option<&str>| {
        let language = language.map(|text| {
            TranscriptionLanguage::parse(text)
                .unwrap_or_else(|error| panic!("{text} is valid: {error}"))
        });
        let context = TranscriptionContext::new(None).with_language(language);
        engine
            .transcribe(&recording, &context)
            .unwrap_or_else(|error| panic!("the speech must transcribe: {error}"))
            .content()
            .to_lowercase()
    };

    // Forced to Italian by the configuration, the English words do not come out.
    let configured = transcribe(None);
    assert!(!configured.contains("americans"), "{configured}");
    for language in ["en", "auto"] {
        let text = transcribe(Some(language));
        assert!(text.contains("americans"), "{language}: {text}");
    }
}
