//! Specifies what the speed check (#109) times and how Settings shows its result: Automatic's model
//! on this device, nothing under OpenAI or with a hand-picked model, and a result only for the
//! hardware and models it was timed on.

use super::{efficient_gpu_status, speed_check_message_in, speed_check_target_in};
use crate::{
    domain::whisper_speed_check::{SpeedCheckRecord, with_result},
    transcription_settings::{SavedTranscriptionSettings, TranscriptionPaths, save_settings},
};

/// A data folder whose only model is the shipped `base`, so Automatic runs `base` on any host.
fn paths(name: &str) -> TranscriptionPaths {
    let folder = std::env::temp_dir().join(format!(
        "note-taker-speed-target-{name}-{}",
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

#[test]
fn automatic_on_this_device_is_the_target() {
    let paths = paths("automatic");
    let target = speed_check_target_in(&paths).unwrap_or_else(|| panic!("a target"));
    assert_eq!(target.automatic_file, "ggml-base.bin");
    assert_eq!(
        target.models.keys().collect::<Vec<_>>(),
        vec!["ggml-base.bin"]
    );
    assert!(target.hardware_key.ends_with("; ggml-base.bin"));
}

#[test]
fn a_hand_picked_model_has_no_target() {
    let paths = paths("no-target");
    let hand_picked = SavedTranscriptionSettings {
        whisper_model: Some("ggml-base.bin".to_owned()),
        ..SavedTranscriptionSettings::default()
    };
    save_settings(&paths.settings_file, &hand_picked).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(speed_check_target_in(&paths), None);
}

#[test]
fn settings_show_the_result_for_this_hardware_only() {
    let paths = paths("message");
    let efficient = efficient_gpu_status().0.is_some();
    assert_eq!(
        speed_check_message_in(&paths, "ggml-base.bin", efficient),
        None
    );

    let target = speed_check_target_in(&paths).unwrap_or_else(|| panic!("a target"));
    let record = with_result(
        SpeedCheckRecord::default(),
        &target.hardware_key,
        "ggml-base.bin",
        0.12,
    );
    std::fs::write(
        &paths.speed_check_file,
        serde_json::to_vec(&record).unwrap_or_default(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        speed_check_message_in(&paths, "ggml-base.bin", efficient).as_deref(),
        Some("Checked on this device: 0.1× real time, so it keeps up with recordings.")
    );

    let other = with_result(SpeedCheckRecord::default(), "other", "ggml-base.bin", 0.12);
    std::fs::write(
        &paths.speed_check_file,
        serde_json::to_vec(&other).unwrap_or_default(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        speed_check_message_in(&paths, "ggml-base.bin", efficient),
        None
    );
}

#[test]
fn the_pass_model_is_a_model_on_this_device_or_the_live_one() {
    use super::{STATUS_INVALID, STATUS_OK, set_pass_model_in};
    use crate::transcription_settings::load_settings;
    let paths = paths("pass-model");
    assert_eq!(set_pass_model_in(&paths, Some("ggml-base.bin")), STATUS_OK);
    assert_eq!(
        load_settings(&paths.settings_file)
            .transcript_pass_model
            .as_deref(),
        Some("ggml-base.bin")
    );
    // Not downloaded here, or not a catalog model: refused, the choice kept.
    assert_eq!(
        set_pass_model_in(&paths, Some("ggml-large-v3-turbo.bin")),
        STATUS_INVALID
    );
    assert_eq!(set_pass_model_in(&paths, Some("../x.bin")), STATUS_INVALID);
    assert_eq!(
        load_settings(&paths.settings_file)
            .transcript_pass_model
            .as_deref(),
        Some("ggml-base.bin")
    );
    // None: the pass uses the live model again.
    assert_eq!(set_pass_model_in(&paths, None), STATUS_OK);
    assert_eq!(
        load_settings(&paths.settings_file).transcript_pass_model,
        None
    );
}

/// Verifies what Settings learns about the NPU (#75): whether this build and phone can use it,
/// where its files go, whether they are all there, and the owner's choice.
#[test]
fn the_npu_section_reports_support_files_and_choice() {
    use super::npu_json;
    let paths = paths("npu");
    let saved = SavedTranscriptionSettings {
        npu_whisper: true,
        ..SavedTranscriptionSettings::default()
    };
    let on_a_supported_phone = npu_json(&paths, &saved, true, Some("SM8550"));
    assert_eq!(on_a_supported_phone["available"], true);
    assert_eq!(on_a_supported_phone["supported"], true);
    assert_eq!(on_a_supported_phone["installed"], false);
    assert_eq!(on_a_supported_phone["enabled"], true);
    assert_eq!(
        on_a_supported_phone["directory"],
        paths.npu_directory.display().to_string()
    );
    let elsewhere = npu_json(&paths, &saved, true, Some("SM8650"));
    assert_eq!(elsewhere["supported"], false, "another chip");
    let without_the_feature = npu_json(&paths, &saved, false, Some("SM8550"));
    assert_eq!(without_the_feature["supported"], false);
    assert_eq!(npu_json(&paths, &saved, true, None)["supported"], false);
}
