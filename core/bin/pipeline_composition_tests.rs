//! Specifies environment-driven pipeline composition without loading models or calling networks.
//! Configuration is read through an injected lookup, so the process environment is never used.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use uuid::Uuid;

use crate::{
    adapters::audio::chunk_audio_store::ChunkAudioStore,
    application::{
        compute_router::ComputePreference, crypto::voice_encryptor::ephemeral_cipher,
        transcription_routing::RemoteTranscriptionGateway,
    },
    domain::{
        errors::TranscriptionError,
        models::{DeviceId, DeviceManifest, TranscriptionModel},
        transcription_language::TranscriptionLanguage,
    },
};

use super::{
    PipelineConfig, local_model_label, select_diarization_engine, select_transcription_engine,
    spawn_recording_controller,
};

/// A paired device that would transcribe, so a controller composes without a local model.
struct PairedDeviceOnly;

impl RemoteTranscriptionGateway for PairedDeviceOnly {
    fn route(&self) -> ComputePreference {
        ComputePreference::Automatic
    }
    fn local_manifest(&self) -> DeviceManifest {
        DeviceManifest::new(DeviceId::new("device-test"), 2, 0, Vec::new())
    }
    fn connected_manifests(&self) -> Vec<DeviceManifest> {
        Vec::new()
    }
    fn transcribe_on_peer(
        &self,
        _peer: &DeviceId,
        _samples: Vec<i16>,
        _previous_text: Option<String>,
        _language: Option<TranscriptionLanguage>,
    ) -> Result<String, TranscriptionError> {
        Err(TranscriptionError::Unavailable)
    }
}

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

/// Verifies the defaults: VAD on, diarization off.
#[test]
fn defaults_have_vad_and_no_diarization() {
    let config = valid_config(&[]);

    assert!(config.vad_enabled());
    assert!(!config.diarization_requested());
}

/// Verifies values parse case-insensitively.
#[test]
fn explicit_values_are_parsed() {
    let config = valid_config(&[
        ("NOTE_TAKER_VAD_ENABLED", "false"),
        ("NOTE_TAKER_ENABLE_DIARIZATION", "1"),
    ]);

    assert!(!config.vad_enabled());
    assert!(config.diarization_requested());
}

/// Verifies invalid values name the offending variable instead of silently defaulting.
#[test]
fn invalid_values_are_rejected_with_the_variable_name() {
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

/// A code Whisper does not know is refused at start-up, and a valid one is offered typed so
/// recordings can carry it.
#[test]
fn local_whisper_language_must_be_one_whisper_knows() {
    let unknown_error = config_from(&[("NOTE_TAKER_WHISPER_LANGUAGE", "xx")])
        .err()
        .unwrap_or_else(|| panic!("xx must be rejected"));
    assert!(format!("{unknown_error:#}").contains("NOTE_TAKER_WHISPER_LANGUAGE"));
    assert_eq!(
        valid_config(&[("NOTE_TAKER_WHISPER_LANGUAGE", "IT")])
            .transcription_language()
            .as_str(),
        "it"
    );
    assert_eq!(valid_config(&[]).transcription_language().as_str(), "auto");
}

/// Verifies a missing engine is reported with the variable that would configure one.
#[test]
fn missing_engines_are_reported_with_their_configuration_variables() {
    let store = Arc::new(ChunkAudioStore::new());

    let error = select_transcription_engine(&valid_config(&[]), &store)
        .err()
        .unwrap_or_else(|| panic!("no model must fail"));
    assert!(format!("{error:#}").contains("NOTE_TAKER_WHISPER_MODEL"));

    // An OpenAI key an earlier setup left in the environment selects nothing (#117).
    assert!(
        select_transcription_engine(&valid_config(&[("OPENAI_API_KEY", "sk-test")]), &store)
            .is_err()
    );
}

/// Verifies `local` fails clearly when the build has no local inference.
#[cfg(not(feature = "local-ai"))]
#[test]
fn local_provider_requires_the_local_ai_build() {
    let store = Arc::new(ChunkAudioStore::new());
    let error = select_transcription_engine(
        &valid_config(&[("NOTE_TAKER_WHISPER_MODEL", "/models/ggml-base.bin")]),
        &store,
    )
    .err()
    .unwrap_or_else(|| panic!("local without local-ai must fail"));

    assert!(format!("{error:#}").contains("local-ai"));
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
        valid_config(&[("NOTE_TAKER_ENABLE_DIARIZATION", "true")]),
        Some(Arc::new(PairedDeviceOnly)),
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
        None,
    );

    assert!(result.is_err());
    let _ = std::fs::remove_file(&database_path);
}

/// The manifest advertises only what the pipeline could build: local Whisper with a model (in
/// local-ai builds), and nothing else; an OpenAI key in the environment adds nothing (#117).
#[test]
fn advertised_models_follow_the_configured_engines() {
    let whisper = if cfg!(feature = "local-ai") {
        vec![TranscriptionModel::Whisper]
    } else {
        Vec::new()
    };
    let model = ("NOTE_TAKER_WHISPER_MODEL", "/models/ggml-base.bin");
    let key = ("OPENAI_API_KEY", "sk-test");

    assert!(
        valid_config(&[key])
            .advertised_transcription_models()
            .is_empty()
    );
    assert_eq!(
        valid_config(&[model, key]).advertised_transcription_models(),
        whisper
    );
}

/// Phones run the after-meeting pass from the charging job (#87 step 2), so it is on by default on
/// every platform; Settings or `NOTE_TAKER_TRANSCRIPT_PASS=false` turn it off.
#[test]
fn the_after_meeting_pass_is_on_by_default_everywhere() {
    assert!(valid_config(&[]).transcript_pass());
    assert!(!valid_config(&[("NOTE_TAKER_TRANSCRIPT_PASS", "false")]).transcript_pass());
}

/// Verifies the after-meeting pass runs on the NPU when the live engine does and no separate pass
/// model was chosen, with the live whisper.cpp model as its fallback (#75).
#[test]
fn the_after_meeting_pass_follows_the_npu_unless_it_has_its_own_model() {
    use super::{PassEngineChoice, pass_engine_choice};
    let live = PathBuf::from("/models/ggml-base.bin");
    let npu = (
        PathBuf::from("/data/npu"),
        PathBuf::from("/models/ggml-base.bin"),
    );
    let base = valid_config(&[("NOTE_TAKER_WHISPER_MODEL", "/models/ggml-base.bin")]);

    assert_eq!(
        pass_engine_choice(&base),
        Some(PassEngineChoice::WhisperCpp(live.clone())),
        "without the NPU, the live model"
    );
    assert_eq!(
        pass_engine_choice(&base.clone().with_npu_whisper(Some(npu.clone()))),
        Some(PassEngineChoice::Npu {
            directory: npu.0.clone(),
            vocabulary_model: npu.1.clone(),
            fallback_model: live,
        })
    );
    let own = PathBuf::from("/models/ggml-large-v3-turbo.bin");
    assert_eq!(
        pass_engine_choice(
            &base
                .clone()
                .with_npu_whisper(Some(npu))
                .with_transcript_pass(None, Some(own.clone()))
        ),
        Some(PassEngineChoice::WhisperCpp(own)),
        "a model chosen for the pass wins"
    );
    assert_eq!(
        pass_engine_choice(&valid_config(&[])),
        None,
        "no model at all"
    );
}
