//! Tests the owner's saved transcription choices.

use std::path::PathBuf;

use super::transcription_settings::{
    SavedTranscriptionSettings, TranscriptionPaths, apply_saved_settings, load_settings,
    save_settings,
};
use crate::pipeline_composition::PipelineConfig;

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

/// Verifies a chosen model replaces the shipped one, and a missing one keeps it.
#[test]
fn a_chosen_model_applies_only_when_it_exists() {
    let paths = paths("model");
    let saved = SavedTranscriptionSettings {
        whisper_model: Some("ggml-small.bin".to_owned()),
        ..Default::default()
    };
    let before = apply_saved_settings(config(&paths), &saved, &paths);
    assert_eq!(before.local_model_path(), paths.shipped_model.as_deref());
    let installed = install(&paths, "ggml-small.bin");
    let after = apply_saved_settings(config(&paths), &saved, &paths);
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

/// Verifies choices survive a restart and damage is harmless.
#[test]
fn settings_round_trip() {
    let paths = paths("file");
    let saved = SavedTranscriptionSettings {
        whisper_model: Some("ggml-small.bin".to_owned()),
        efficient_gpu: false,
        dismissed_proposals: vec!["ggml-medium.bin".to_owned()],
        transcript_pass: Some(false),
        auto_download: Some(false),
        transcript_pass_model: Some("ggml-large-v3-turbo.bin".to_owned()),
        glossary: vec!["Marco Rossi".to_owned()],
        keep_speech_days: Some(14),
        keep_meeting_voices: true,
        computer_sound: Some(false),
        npu_whisper: true,
        keep_recordings: true,
    };
    assert_eq!(load_settings(&paths.settings_file), Default::default());
    save_settings(&paths.settings_file, &saved).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(load_settings(&paths.settings_file), saved);
    std::fs::write(&paths.settings_file, b"{broken").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(load_settings(&paths.settings_file), Default::default());
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

#[test]
fn the_speech_retention_window_is_saved_in_days_and_read_fresh() {
    use super::transcription_settings::{keep_for_source, save_keep_speech_days};
    use crate::application::combined_meetings::KEEP_FOR_COMBINING;
    const DAY: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);
    let paths = paths("keep-speech");
    let source = keep_for_source(paths.settings_file.clone());
    assert_eq!(source(), KEEP_FOR_COMBINING, "three weeks until chosen");

    save_keep_speech_days(&paths.settings_file, 7).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(source(), 7 * DAY);
    assert_eq!(
        load_settings(&paths.settings_file).keep_speech_days,
        Some(7)
    );

    assert!(save_keep_speech_days(&paths.settings_file, 0).is_err());
    assert!(save_keep_speech_days(&paths.settings_file, 61).is_err());
    assert_eq!(source(), 7 * DAY, "a refused window keeps the saved one");
}

#[test]
fn a_retention_window_edited_out_of_range_falls_back_to_three_weeks() {
    use super::transcription_settings::keep_for_source;
    use crate::application::combined_meetings::KEEP_FOR_COMBINING;
    let paths = paths("keep-speech-invalid");
    let saved = SavedTranscriptionSettings {
        keep_speech_days: Some(400),
        ..SavedTranscriptionSettings::default()
    };
    save_settings(&paths.settings_file, &saved).unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(
        keep_for_source(paths.settings_file.clone())(),
        KEEP_FOR_COMBINING
    );
}

/// Verifies keeping online-meeting participants' voices is off until chosen, and read fresh (#103).
#[test]
fn keeping_meeting_voices_is_off_until_chosen() {
    use super::transcription_settings::{keeps_meeting_voices, save_keep_meeting_voices};
    let paths = paths("meeting-voices");
    assert!(
        !keeps_meeting_voices(&paths.settings_file),
        "off by default"
    );

    save_keep_meeting_voices(&paths.settings_file, true).unwrap_or_else(|error| panic!("{error}"));
    assert!(keeps_meeting_voices(&paths.settings_file));
    save_keep_meeting_voices(&paths.settings_file, false).unwrap_or_else(|error| panic!("{error}"));
    assert!(!keeps_meeting_voices(&paths.settings_file));
}

#[test]
fn the_after_meeting_pass_switch_is_saved_and_defaults_to_on() {
    use super::transcription_settings::{save_transcript_pass, transcript_pass_enabled};
    let paths = paths("transcript-pass");
    assert!(transcript_pass_enabled(&load_settings(
        &paths.settings_file
    )));

    save_transcript_pass(&paths.settings_file, false).unwrap_or_else(|error| panic!("{error}"));
    let saved = load_settings(&paths.settings_file);
    assert_eq!(saved.transcript_pass, Some(false));
    assert!(!transcript_pass_enabled(&saved));
    assert!(!apply_saved_settings(config(&paths), &saved, &paths).transcript_pass());

    save_transcript_pass(&paths.settings_file, true).unwrap_or_else(|error| panic!("{error}"));
    assert!(transcript_pass_enabled(&load_settings(
        &paths.settings_file
    )));
}

#[test]
fn automatic_fills_in_the_model_and_efficient_mode_only_when_none_was_chosen() {
    use super::transcription_settings::with_automatic_model;
    use crate::domain::whisper_suggestion::AutomaticModel;
    let automatic = AutomaticModel {
        file_name: "ggml-small.bin",
        efficient: true,
        waiting_for_download: None,
    };

    let chosen = with_automatic_model(&SavedTranscriptionSettings::default(), Some(automatic));
    assert_eq!(chosen.whisper_model.as_deref(), Some("ggml-small.bin"));
    assert!(chosen.efficient_gpu);

    let manual = SavedTranscriptionSettings {
        whisper_model: Some("ggml-base.bin".to_owned()),
        ..SavedTranscriptionSettings::default()
    };
    let kept = with_automatic_model(&manual, Some(automatic));
    assert_eq!(kept.whisper_model.as_deref(), Some("ggml-base.bin"));
    assert!(
        !kept.efficient_gpu,
        "a hand-picked model keeps its own efficient switch"
    );

    let nothing = with_automatic_model(&SavedTranscriptionSettings::default(), None);
    assert_eq!(nothing.whisper_model, None);
}

/// Verifies recording the computer's sound is on until turned off (#108).
#[test]
fn the_computer_sound_is_recorded_until_turned_off() {
    use super::transcription_settings::{computer_sound_enabled, save_computer_sound};
    let paths = paths("computer-sound");
    assert!(computer_sound_enabled(&load_settings(&paths.settings_file)));
    save_computer_sound(&paths.settings_file, false).unwrap_or_else(|error| panic!("{error}"));
    assert!(!computer_sound_enabled(&load_settings(
        &paths.settings_file
    )));
    save_computer_sound(&paths.settings_file, true).unwrap_or_else(|error| panic!("{error}"));
    assert!(computer_sound_enabled(&load_settings(&paths.settings_file)));
}

/// Verifies a file an earlier version wrote, choosing OpenAI with its model, still loads and
/// transcribes on the device, and the old choice is dropped when saved again (#117).
#[test]
fn an_old_openai_choice_is_ignored_and_dropped() {
    let paths = paths("old-openai");
    std::fs::write(
        &paths.settings_file,
        br#"{"provider":"openai","openai_model":"whisper-1","whisper_model":"ggml-base.bin"}"#,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let saved = load_settings(&paths.settings_file);
    assert_eq!(saved.whisper_model.as_deref(), Some("ggml-base.bin"));
    let applied = apply_saved_settings(config(&paths), &saved, &paths);
    assert_eq!(applied.local_model_path(), paths.shipped_model.as_deref());
    save_settings(&paths.settings_file, &saved).unwrap_or_else(|e| panic!("{e}"));
    let text = std::fs::read_to_string(&paths.settings_file).unwrap_or_else(|e| panic!("{e}"));
    assert!(!text.contains("openai"));
}

/// Verifies the NPU engine is chosen only when the owner turned it on and its files are complete,
/// and that the shipped multilingual model supplies its vocabulary (#75).
#[test]
fn the_npu_applies_only_when_turned_on_and_installed() {
    use crate::adapters::ai::npu_whisper::install::{
        MODEL_FILES, NpuInstall, QNN_COMMON_FILES, hexagon_files,
    };
    let paths = paths("npu");
    assert_eq!(
        paths.npu_directory,
        paths.models_directory.with_file_name("npu"),
        "beside the models folder"
    );
    let off = SavedTranscriptionSettings::default();
    assert!(!off.npu_whisper, "off until the owner turns it on");
    let on = SavedTranscriptionSettings {
        npu_whisper: true,
        ..Default::default()
    };
    assert_eq!(
        apply_saved_settings(config(&paths), &on, &paths).npu_directory(),
        None,
        "nothing downloaded yet"
    );
    let install = NpuInstall::new(&paths.npu_directory);
    for (folder, files) in [
        (install.model_dir(), MODEL_FILES.as_slice()),
        (install.qnn_dir(), QNN_COMMON_FILES.as_slice()),
    ] {
        std::fs::create_dir_all(&folder).unwrap_or_else(|e| panic!("{e}"));
        for file in files {
            std::fs::write(folder.join(file), b"x").unwrap_or_else(|e| panic!("{e}"));
        }
    }
    for file in hexagon_files(73) {
        std::fs::write(install.qnn_dir().join(file), b"x").unwrap_or_else(|e| panic!("{e}"));
    }
    let applied = apply_saved_settings(config(&paths), &on, &paths);
    assert_eq!(applied.npu_directory(), Some(paths.npu_directory.as_path()));
    assert_eq!(
        applied.npu_vocabulary_model(),
        paths.shipped_model.as_deref()
    );
    assert_eq!(
        apply_saved_settings(config(&paths), &off, &paths).npu_directory(),
        None,
        "turned off"
    );
}

/// Verifies turning the NPU on and off is stored and keeps the other choices.
#[test]
fn the_npu_choice_round_trips() {
    use super::transcription_settings::save_npu_whisper;
    let paths = paths("npu-save");
    let saved = SavedTranscriptionSettings {
        efficient_gpu: true,
        ..Default::default()
    };
    save_settings(&paths.settings_file, &saved).unwrap_or_else(|e| panic!("{e}"));
    save_npu_whisper(&paths.settings_file, true).unwrap_or_else(|e| panic!("{e}"));
    let loaded = load_settings(&paths.settings_file);
    assert!(loaded.npu_whisper);
    assert!(loaded.efficient_gpu, "other choices stay");
    save_npu_whisper(&paths.settings_file, false).unwrap_or_else(|e| panic!("{e}"));
    assert!(!load_settings(&paths.settings_file).npu_whisper);
}

#[test]
fn downloading_the_best_model_by_itself_is_saved_and_defaults_to_on() {
    use super::transcription_settings::{auto_download_enabled, save_auto_download};
    let paths = paths("auto-download");
    assert!(auto_download_enabled(&load_settings(&paths.settings_file)));

    save_auto_download(&paths.settings_file, false).unwrap_or_else(|error| panic!("{error}"));
    let saved = load_settings(&paths.settings_file);
    assert_eq!(saved.auto_download, Some(false));
    assert!(!auto_download_enabled(&saved));

    save_auto_download(&paths.settings_file, true).unwrap_or_else(|error| panic!("{error}"));
    assert!(auto_download_enabled(&load_settings(&paths.settings_file)));
}
