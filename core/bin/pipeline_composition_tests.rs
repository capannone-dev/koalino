//! Specifies environment-driven pipeline composition without loading models or calling networks.
//! Configuration is read through an injected lookup, so the process environment is never used.

use std::{collections::HashMap, sync::Arc};

use uuid::Uuid;

use crate::{
    adapters::audio::chunk_audio_store::ChunkAudioStore,
    application::crypto::voice_encryptor::ephemeral_cipher, domain::models::DeviceId,
};

use super::{
    PipelineConfig, SelectedTranscriptionEngine, TranscriptionProvider,
    select_transcription_engine, spawn_recording_controller,
};

/// Builds a config from literal variables, ignoring the real environment.
fn config_from(variables: &[(&str, &str)]) -> anyhow::Result<PipelineConfig> {
    let variables: HashMap<String, String> = variables
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    PipelineConfig::from_lookup(|name| variables.get(name).cloned())
}

/// Unwraps a configuration that must parse.
fn valid_config(variables: &[(&str, &str)]) -> PipelineConfig {
    config_from(variables).unwrap_or_else(|error| panic!("config should parse: {error:#}"))
}

/// Verifies the defaults: automatic provider, VAD on, diarization off.
#[test]
fn defaults_select_auto_provider_with_vad_and_no_diarization() {
    let config = valid_config(&[]);

    assert_eq!(config.transcription_provider(), TranscriptionProvider::Auto);
    assert!(config.vad_enabled());
    assert!(!config.diarization_requested());
}

/// Verifies values parse case-insensitively.
#[test]
fn explicit_values_are_parsed() {
    let config = valid_config(&[
        ("NOTE_TAKER_TRANSCRIPTION_PROVIDER", "OpenAI"),
        ("NOTE_TAKER_VAD_ENABLED", "false"),
        ("NOTE_TAKER_ENABLE_DIARIZATION", "1"),
    ]);

    assert_eq!(
        config.transcription_provider(),
        TranscriptionProvider::OpenAi
    );
    assert!(!config.vad_enabled());
    assert!(config.diarization_requested());
    assert_eq!(
        valid_config(&[("NOTE_TAKER_TRANSCRIPTION_PROVIDER", "local")]).transcription_provider(),
        TranscriptionProvider::Local
    );
}

/// Verifies invalid values name the offending variable instead of silently defaulting.
#[test]
fn invalid_values_are_rejected_with_the_variable_name() {
    let provider_error = config_from(&[("NOTE_TAKER_TRANSCRIPTION_PROVIDER", "whisperx")])
        .err()
        .unwrap_or_else(|| panic!("an unknown provider must be rejected"));
    assert!(format!("{provider_error:#}").contains("NOTE_TAKER_TRANSCRIPTION_PROVIDER"));

    let flag_error = config_from(&[("NOTE_TAKER_VAD_ENABLED", "maybe")])
        .err()
        .unwrap_or_else(|| panic!("an invalid boolean must be rejected"));
    assert!(format!("{flag_error:#}").contains("NOTE_TAKER_VAD_ENABLED"));
}

/// Verifies the OpenAI key never appears in debug output.
#[test]
fn debug_output_redacts_the_api_key() {
    let config = valid_config(&[("OPENAI_API_KEY", "sk-secret-value")]);

    assert!(!format!("{config:?}").contains("sk-secret-value"));
}

/// Verifies a missing engine is reported with the variables that would configure one.
#[test]
fn missing_engines_are_reported_with_their_configuration_variables() {
    let store = Arc::new(ChunkAudioStore::new());

    let openai_error = select_transcription_engine(
        &valid_config(&[("NOTE_TAKER_TRANSCRIPTION_PROVIDER", "openai")]),
        &store,
    )
    .err()
    .unwrap_or_else(|| panic!("openai without a key must fail"));
    assert!(format!("{openai_error:#}").contains("OPENAI_API_KEY"));

    let auto_error = select_transcription_engine(&valid_config(&[]), &store)
        .err()
        .unwrap_or_else(|| panic!("auto without any engine must fail"));
    let auto_message = format!("{auto_error:#}");
    assert!(auto_message.contains("OPENAI_API_KEY"));
    assert!(auto_message.contains("NOTE_TAKER_WHISPER_MODEL"));
}

/// Verifies `local` fails clearly when the build has no local inference.
#[cfg(not(feature = "local-ai"))]
#[test]
fn local_provider_requires_the_local_ai_build() {
    let store = Arc::new(ChunkAudioStore::new());
    let error = select_transcription_engine(
        &valid_config(&[
            ("NOTE_TAKER_TRANSCRIPTION_PROVIDER", "local"),
            ("NOTE_TAKER_WHISPER_MODEL", "/models/ggml-base.bin"),
        ]),
        &store,
    )
    .err()
    .unwrap_or_else(|| panic!("local without local-ai must fail"));

    assert!(format!("{error:#}").contains("local-ai"));
}

/// Verifies a configured key selects the cloud engine (constructed without any request), and
/// that `auto` falls back to it when no local model is available.
#[test]
fn an_api_key_selects_the_openai_engine() {
    let store = Arc::new(ChunkAudioStore::new());
    for provider in ["openai", "auto"] {
        let engine = select_transcription_engine(
            &valid_config(&[
                ("NOTE_TAKER_TRANSCRIPTION_PROVIDER", provider),
                ("OPENAI_API_KEY", "sk-test"),
            ]),
            &store,
        )
        .unwrap_or_else(|error| panic!("{provider} with a key should compose: {error:#}"));
        assert!(matches!(engine, SelectedTranscriptionEngine::OpenAi(_)));
    }
}

/// Verifies the composed controller opens its own stores on the database and reports that a
/// voice library is available while diarization is not (no adapter exists yet).
#[test]
fn spawned_controller_reports_pipeline_availability() {
    let database_path = std::env::temp_dir().join(format!(
        "note-taker-pipeline-composition-{}.sqlite3",
        Uuid::new_v4()
    ));
    let controller = spawn_recording_controller(
        &database_path,
        DeviceId::new("device-test"),
        ephemeral_cipher(),
        valid_config(&[
            ("OPENAI_API_KEY", "sk-test"),
            ("NOTE_TAKER_ENABLE_DIARIZATION", "true"),
        ]),
    )
    .unwrap_or_else(|error| panic!("controller should compose: {error:#}"));

    let availability = controller.recording_manager().availability();
    assert!(availability.voice_matching);
    assert!(!availability.diarization);
    drop(controller);
    let _ = std::fs::remove_file(&database_path);
}

/// Verifies composition fails before spawning a recording when no engine is configured.
#[test]
fn spawning_without_an_engine_fails() {
    let database_path = std::env::temp_dir().join(format!(
        "note-taker-pipeline-composition-{}.sqlite3",
        Uuid::new_v4()
    ));
    let result = spawn_recording_controller(
        &database_path,
        DeviceId::new("device-test"),
        ephemeral_cipher(),
        valid_config(&[]),
    );

    assert!(result.is_err());
    let _ = std::fs::remove_file(&database_path);
}
