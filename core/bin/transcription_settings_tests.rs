//! Tests the owner's saved transcription choices.

use std::path::PathBuf;

use super::transcription_settings::{
    KeyTestResult, SavedProvider, SavedTranscriptionSettings, TranscriptionPaths,
    apply_saved_settings, load_settings, save_settings, validated_key,
};
use crate::pipeline_composition::{PipelineConfig, TranscriptionProvider};

/// Creates a clean data folder holding a database path and a shipped model.
fn paths(name: &str) -> TranscriptionPaths {
    let folder = std::env::temp_dir().join(format!(
        "note-taker-transcription-{name}-{}",
        std::process::id()
    ));
    drop(std::fs::remove_dir_all(&folder));
    std::fs::create_dir_all(folder.join("shipped")).unwrap_or_else(|e| panic!("{e}"));
    let shipped = folder.join("shipped").join("ggml-base.bin");
    std::fs::write(&shipped, b"model").unwrap_or_else(|e| panic!("{e}"));
    let database = folder.join("note_taker.sqlite3");
    TranscriptionPaths::from_lookup(&|name| match name {
        "NOTE_TAKER_DATABASE_PATH" => Some(database.display().to_string()),
        "NOTE_TAKER_WHISPER_MODEL" => Some(shipped.display().to_string()),
        _ => None,
    })
}

fn config(paths: &TranscriptionPaths) -> PipelineConfig {
    let shipped = paths
        .shipped_model
        .as_ref()
        .map(|p| p.display().to_string());
    PipelineConfig::from_lookup(|name| match name {
        "NOTE_TAKER_WHISPER_MODEL" => shipped.clone(),
        _ => None,
    })
    .unwrap_or_else(|e| panic!("{e}"))
}

fn install(paths: &TranscriptionPaths, file: &str) -> PathBuf {
    std::fs::create_dir_all(&paths.models_directory).unwrap_or_else(|e| panic!("{e}"));
    let path = paths.models_directory.join(file);
    std::fs::write(&path, b"model").unwrap_or_else(|e| panic!("{e}"));
    path
}

/// Verifies saving only a key keeps audio local.
#[test]
fn a_saved_key_alone_stays_local() {
    let paths = paths("key");
    let applied = apply_saved_settings(
        config(&paths),
        &SavedTranscriptionSettings::default(),
        &paths,
        Some("sk-test".to_owned()),
    );
    assert_eq!(
        applied.transcription_provider(),
        TranscriptionProvider::Local
    );
}

/// Verifies OpenAI is used only when chosen.
#[test]
fn choosing_openai_selects_it_and_enables_the_fallback() {
    let paths = paths("openai");
    let saved = SavedTranscriptionSettings {
        provider: Some(SavedProvider::Openai),
        ..Default::default()
    };
    let applied = apply_saved_settings(config(&paths), &saved, &paths, Some("sk-test".to_owned()));
    assert_eq!(
        applied.transcription_provider(),
        TranscriptionProvider::OpenAi
    );
    assert!(applied.cloud_fallback());
}

/// Verifies a chosen model replaces the shipped one, and a missing one keeps it.
#[test]
fn a_chosen_model_applies_only_when_it_exists() {
    let paths = paths("model");
    let saved = SavedTranscriptionSettings {
        whisper_model: Some("ggml-small.bin".to_owned()),
        ..Default::default()
    };
    let before = apply_saved_settings(config(&paths), &saved, &paths, None);
    assert_eq!(before.local_model_path(), paths.shipped_model.as_deref());
    let installed = install(&paths, "ggml-small.bin");
    let after = apply_saved_settings(config(&paths), &saved, &paths, None);
    assert_eq!(after.local_model_path(), Some(installed.as_path()));
}

/// Verifies a damaged or unknown name can't reach the file system.
#[test]
fn only_catalog_models_can_be_located_or_removed() {
    let paths = paths("names");
    install(&paths, "ggml-tiny.bin");
    assert!(paths.locate_model("../ggml-tiny.bin").is_none());
    assert!(paths.locate_model("custom.bin").is_none());
    assert!(paths.is_removable("ggml-tiny.bin"));
    assert!(!paths.is_removable("ggml-base.bin"));
    assert!(paths.locate_model("ggml-base.bin").is_some());
}

/// Verifies choices survive a restart and damage is harmless, and the key is never in the file.
#[test]
fn settings_round_trip_without_the_key() {
    let paths = paths("file");
    let saved = SavedTranscriptionSettings {
        provider: Some(SavedProvider::Local),
        whisper_model: Some("ggml-small.bin".to_owned()),
        openai_model: None,
        efficient_gpu: false,
        dismissed_proposals: vec!["ggml-medium.bin".to_owned()],
        transcript_pass: Some(false),
        transcript_pass_model: Some("ggml-large-v3-turbo.bin".to_owned()),
        glossary: vec!["Marco Rossi".to_owned()],
    };
    assert_eq!(load_settings(&paths.settings_file), Default::default());
    save_settings(&paths.settings_file, &saved).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(load_settings(&paths.settings_file), saved);
    let text = std::fs::read_to_string(&paths.settings_file).unwrap_or_else(|e| panic!("{e}"));
    assert!(!text.contains("sk-"));
    std::fs::write(&paths.settings_file, b"{broken").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(load_settings(&paths.settings_file), Default::default());
}

/// Verifies a key's form is checked before saving.
#[test]
fn keys_are_trimmed_and_malformed_ones_refused() {
    assert_eq!(validated_key("  sk-abc\n"), Some("sk-abc"));
    assert_eq!(validated_key("   "), None);
    assert_eq!(validated_key("sk abc"), None);
    assert_eq!(validated_key(&"x".repeat(600)), None);
}

/// Verifies the plain errors the owner sees.
#[test]
fn key_test_statuses_map_to_plain_results() {
    assert_eq!(KeyTestResult::from_status(200), KeyTestResult::Valid);
    assert_eq!(KeyTestResult::from_status(401), KeyTestResult::Invalid);
    assert_eq!(KeyTestResult::from_status(429), KeyTestResult::QuotaUsed);
    assert_eq!(KeyTestResult::from_status(500), KeyTestResult::Unexpected);
}

/// Efficient mode is off unless chosen, and old settings files still load.
#[test]
fn efficient_mode_defaults_to_off_for_older_settings_files() {
    let settings_file = paths("efficient").settings_file;
    std::fs::write(
        &settings_file,
        br#"{"provider":"local","whisper_model":"ggml-base.bin"}"#,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let mut saved = load_settings(&settings_file);
    assert!(!saved.efficient_gpu);
    saved.efficient_gpu = true;
    save_settings(&settings_file, &saved).unwrap_or_else(|e| panic!("{e}"));
    assert!(load_settings(&settings_file).efficient_gpu);
}

#[test]
fn the_glossary_is_saved_with_the_settings_and_read_fresh_for_each_recording() {
    use super::transcription_settings::{glossary_source, save_glossary, settings_file_for};
    use crate::domain::transcription_glossary::TranscriptionGlossary;
    let paths = paths("glossary");
    let database = paths
        .settings_file
        .to_string_lossy()
        .trim_end_matches(".transcription-settings.json")
        .to_owned();
    assert_eq!(
        settings_file_for(std::path::Path::new(&database)),
        paths.settings_file
    );
    let source = glossary_source(paths.settings_file.clone());
    assert_eq!(source(), None, "no glossary before one is saved");

    let glossary = TranscriptionGlossary::parse(["Marco Rossi", "OKR"])
        .unwrap_or_else(|error| panic!("{error}"));
    save_glossary(&paths.settings_file, &glossary).unwrap_or_else(|error| panic!("{error}"));
    let mut saved = load_settings(&paths.settings_file);
    saved.efficient_gpu = true;
    save_settings(&paths.settings_file, &saved).unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(source(), Some(glossary));
    assert!(
        load_settings(&paths.settings_file).efficient_gpu,
        "other choices are kept"
    );
    save_glossary(&paths.settings_file, &TranscriptionGlossary::default())
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(source(), None);
}

#[test]
fn a_glossary_edited_into_something_invalid_is_ignored() {
    use super::transcription_settings::glossary_source;
    let paths = paths("glossary-invalid");
    let saved = SavedTranscriptionSettings {
        glossary: vec!["line\nbreak".to_owned()],
        ..SavedTranscriptionSettings::default()
    };
    save_settings(&paths.settings_file, &saved).unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(glossary_source(paths.settings_file.clone())(), None);
}
