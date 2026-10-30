//! Specifies environment-driven pipeline composition without loading models or calling networks.
//! Configuration is read through an injected lookup, so the process environment is never used.

use std::{collections::HashMap, sync::Arc};

use uuid::Uuid;

use crate::{
    adapters::audio::chunk_audio_store::ChunkAudioStore,
    application::{crypto::voice_encryptor::ephemeral_cipher, pipeline::task_metrics::TaskProfile},
    domain::models::{DeviceId, TaskType, TranscriptionModel},
};

use super::{
    CLOUD_TRANSCRIPTION_POWER_MILLIWATTS, PipelineConfig, SelectedTranscriptionEngine,
    TranscriptionProvider, local_model_label, select_diarization_engine,
    select_transcription_engine, spawn_recording_controller, transcription_task_profile,
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

/// Verifies local Whisper auto-detects the language by default.
///
/// whisper.cpp's own default is English, which transcribes Italian meetings as broken English
/// (requirement 4.2), so the composition must always pass an explicit language.
#[test]
fn local_whisper_language_defaults_to_auto_detection() {
    assert_eq!(valid_config(&[]).whisper_language(), "auto");
    assert_eq!(
        valid_config(&[("NOTE_TAKER_WHISPER_LANGUAGE", "  ")]).whisper_language(),
        "auto"
    );
}

/// Verifies an explicit local Whisper language is normalized to a lowercase language code.
#[test]
fn local_whisper_language_accepts_language_codes() {
    assert_eq!(
        valid_config(&[("NOTE_TAKER_WHISPER_LANGUAGE", " IT ")]).whisper_language(),
        "it"
    );
    assert_eq!(
        valid_config(&[("NOTE_TAKER_WHISPER_LANGUAGE", "en")]).whisper_language(),
        "en"
    );
    assert_eq!(
        valid_config(&[("NOTE_TAKER_WHISPER_LANGUAGE", "Auto")]).whisper_language(),
        "auto"
    );
}

/// Verifies malformed languages are rejected with the variable name instead of reaching whisper.
#[test]
fn local_whisper_language_rejects_malformed_values() {
    for malformed_language in ["italian", "e", "en-US", "i\u{0}t", "1t"] {
        let language_error = config_from(&[("NOTE_TAKER_WHISPER_LANGUAGE", malformed_language)])
            .err()
            .unwrap_or_else(|| panic!("{malformed_language:?} must be rejected"));
        assert!(format!("{language_error:#}").contains("NOTE_TAKER_WHISPER_LANGUAGE"));
    }
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

/// Verifies cloud transcriptions are recorded as `cloud_transcription` with the OpenAI model.
#[test]
fn openai_transcriptions_are_recorded_as_cloud_work() {
    let config = valid_config(&[
        ("OPENAI_API_KEY", "sk-test"),
        ("NOTE_TAKER_OPENAI_WHISPER_MODEL", "gpt-4o-transcribe"),
    ]);
    let engine = select_transcription_engine(&config, &Arc::new(ChunkAudioStore::new()))
        .unwrap_or_else(|error| panic!("a key should compose: {error:#}"));

    assert_eq!(
        transcription_task_profile(&engine, &config),
        TaskProfile::new(
            TaskType::CloudTranscription,
            "gpt-4o-transcribe",
            CLOUD_TRANSCRIPTION_POWER_MILLIWATTS
        )
    );
}

/// Verifies a local model is labelled by its file name without the folder or extension, so no
/// path (which can name the user) reaches telemetry.
#[test]
fn local_models_are_labelled_by_file_stem() {
    assert_eq!(
        local_model_label(std::path::Path::new("/home/someone/models/ggml-small.bin")),
        "ggml-small"
    );
    assert_eq!(
        local_model_label(std::path::Path::new("C:\\models\\ggml-base.en.bin")),
        "ggml-base.en"
    );
    assert_eq!(local_model_label(std::path::Path::new("")), "whisper.cpp");
}

/// Verifies no engine is composed unless diarization is requested and both models are named, so
/// transcription keeps working (unattributed) on a device without models.
#[test]
fn diarization_engine_needs_a_request_and_both_models() {
    let unrequested = valid_config(&[
        (
            "NOTE_TAKER_DIARIZATION_SEGMENTATION_MODEL",
            "/missing/s.onnx",
        ),
        ("NOTE_TAKER_DIARIZATION_EMBEDDING_MODEL", "/missing/e.onnx"),
    ]);
    let without_embedding = valid_config(&[
        ("NOTE_TAKER_ENABLE_DIARIZATION", "true"),
        (
            "NOTE_TAKER_DIARIZATION_SEGMENTATION_MODEL",
            "/missing/s.onnx",
        ),
    ]);

    for config in [&unrequested, &without_embedding] {
        let engine = select_diarization_engine(config)
            .unwrap_or_else(|error| panic!("nothing should load: {error:#}"));
        assert!(engine.is_none());
    }
}

/// Verifies named models that cannot load are an error, not silently unattributed meetings.
#[cfg(feature = "diarization")]
#[test]
fn unloadable_diarization_models_are_an_error() {
    let config = valid_config(&[
        ("NOTE_TAKER_ENABLE_DIARIZATION", "true"),
        (
            "NOTE_TAKER_DIARIZATION_SEGMENTATION_MODEL",
            "/missing/s.onnx",
        ),
        ("NOTE_TAKER_DIARIZATION_EMBEDDING_MODEL", "/missing/e.onnx"),
    ]);

    assert!(select_diarization_engine(&config).is_err());
}

/// Verifies the composed controller opens its own stores on the database and reports that a
/// voice library is available while diarization is not (no models are configured).
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
        None,
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
        None,
    );

    assert!(result.is_err());
    let _ = std::fs::remove_file(&database_path);
}

/// The manifest advertises only engines the pipeline could build: local Whisper with a model
/// (in local-ai builds), OpenAI with a key, both under `auto`, and nothing without either.
#[test]
fn advertised_models_follow_the_configured_engines() {
    let whisper = || {
        if cfg!(feature = "local-ai") {
            vec![TranscriptionModel::Whisper]
        } else {
            Vec::new()
        }
    };
    let openai = TranscriptionModel::ThirdParty(String::from("openai"));
    let model = ("NOTE_TAKER_WHISPER_MODEL", "/models/ggml-base.bin");
    let key = ("OPENAI_API_KEY", "sk-test");

    assert!(
        valid_config(&[])
            .advertised_transcription_models()
            .is_empty()
    );
    assert_eq!(
        valid_config(&[model]).advertised_transcription_models(),
        whisper()
    );
    assert_eq!(
        valid_config(&[key]).advertised_transcription_models(),
        vec![openai.clone()]
    );
    let mut both = whisper();
    both.push(openai.clone());
    assert_eq!(
        valid_config(&[model, key]).advertised_transcription_models(),
        both
    );
    assert_eq!(
        valid_config(&[model, key, ("NOTE_TAKER_TRANSCRIPTION_PROVIDER", "local")])
            .advertised_transcription_models(),
        whisper()
    );
    assert_eq!(
        valid_config(&[model, key, ("NOTE_TAKER_TRANSCRIPTION_PROVIDER", "openai")])
            .advertised_transcription_models(),
        vec![openai]
    );
}
