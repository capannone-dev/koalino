//! Specifies the on-device speed check (#109): which model to time next, how a result is kept per
//! hardware, and how Settings words it.

use super::whisper_speed_check::{
    SpeedCheckRecord, next_model_to_check, real_time_factor, speed_check_message, with_result,
};

const HARDWARE: &str = "phone 8 GB, 3.36 GHz; ggml-base, ggml-small";

/// Every model of the ladder up to `small` is on the device; nothing is measured yet.
fn on_device(file_name: &str) -> bool {
    matches!(
        file_name,
        "ggml-tiny.bin" | "ggml-base.bin" | "ggml-small.bin"
    )
}

fn unmeasured(_file_name: &str) -> bool {
    false
}

#[test]
fn an_unchecked_unmeasured_model_is_checked() {
    let record = SpeedCheckRecord::default();
    assert_eq!(
        next_model_to_check("ggml-small.bin", &record, HARDWARE, &unmeasured, &on_device),
        Some("ggml-small.bin".to_owned())
    );
}

#[test]
fn a_model_measured_in_real_recordings_needs_no_check() {
    let record = SpeedCheckRecord::default();
    assert_eq!(
        next_model_to_check(
            "ggml-small.bin",
            &record,
            HARDWARE,
            &|file| file == "ggml-small.bin",
            &on_device
        ),
        None
    );
}

#[test]
fn a_model_that_keeps_up_is_not_checked_again() {
    let record = with_result(
        SpeedCheckRecord::default(),
        HARDWARE,
        "ggml-small.bin",
        0.45,
    );
    assert_eq!(
        next_model_to_check("ggml-small.bin", &record, HARDWARE, &unmeasured, &on_device),
        None
    );
}

#[test]
fn a_model_too_slow_has_the_next_smaller_one_checked_until_one_keeps_up() {
    let record = with_result(SpeedCheckRecord::default(), HARDWARE, "ggml-small.bin", 1.4);
    assert_eq!(
        next_model_to_check("ggml-small.bin", &record, HARDWARE, &unmeasured, &on_device),
        Some("ggml-base.bin".to_owned())
    );
    let record = with_result(record, HARDWARE, "ggml-base.bin", 0.95);
    assert_eq!(
        next_model_to_check("ggml-small.bin", &record, HARDWARE, &unmeasured, &on_device),
        Some("ggml-tiny.bin".to_owned()),
        "base fell behind too"
    );
    let record = with_result(record, HARDWARE, "ggml-tiny.bin", 0.5);
    assert_eq!(
        next_model_to_check("ggml-small.bin", &record, HARDWARE, &unmeasured, &on_device),
        None
    );
}

#[test]
fn smaller_models_not_on_the_device_are_skipped() {
    let record = with_result(SpeedCheckRecord::default(), HARDWARE, "ggml-small.bin", 1.4);
    assert_eq!(
        next_model_to_check(
            "ggml-small.bin",
            &record,
            HARDWARE,
            &unmeasured,
            &|file| file == "ggml-small.bin" || file == "ggml-tiny.bin"
        ),
        Some("ggml-tiny.bin".to_owned())
    );
}

#[test]
fn new_hardware_or_models_start_the_check_again() {
    let record = with_result(
        SpeedCheckRecord::default(),
        HARDWARE,
        "ggml-small.bin",
        0.45,
    );
    let changed = "phone 8 GB, 3.36 GHz; ggml-base, ggml-medium, ggml-small";
    assert_eq!(
        next_model_to_check("ggml-small.bin", &record, changed, &unmeasured, &on_device),
        Some("ggml-small.bin".to_owned())
    );
    let record = with_result(record, changed, "ggml-base.bin", 0.2);
    assert_eq!(
        record.models.len(),
        1,
        "results for other hardware are dropped"
    );
}

#[test]
fn the_factor_is_processing_time_over_audio_time() {
    assert!((real_time_factor(120_000, 36_000) - 0.3).abs() < 1e-6);
    assert!(real_time_factor(0, 1_000).is_infinite());
}

#[test]
fn settings_words_the_result() {
    let none = SpeedCheckRecord::default();
    assert_eq!(
        speed_check_message(&none, HARDWARE, "ggml-small.bin", false),
        None
    );

    let fast = with_result(none.clone(), HARDWARE, "ggml-small.bin", 0.31);
    assert_eq!(
        speed_check_message(&fast, HARDWARE, "ggml-small.bin", false).as_deref(),
        Some("Checked on this device: 0.3× real time, so it keeps up with recordings.")
    );
    assert_eq!(
        speed_check_message(&fast, "other hardware", "ggml-small.bin", false),
        None,
        "a result for other hardware no longer applies"
    );

    let slow = with_result(none.clone(), HARDWARE, "ggml-small.bin", 1.42);
    assert_eq!(
        speed_check_message(&slow, HARDWARE, "ggml-small.bin", true).as_deref(),
        Some(
            "Checked on this device: 1.4× real time, too slow to keep up with recordings. \
             A smaller model is being checked."
        )
    );
    let base_keeps_up = with_result(slow.clone(), HARDWARE, "ggml-base.bin", 0.48);
    assert_eq!(
        speed_check_message(&base_keeps_up, HARDWARE, "ggml-small.bin", false).as_deref(),
        Some(
            "Checked on this device: 1.4× real time, too slow to keep up with recordings. \
             base keeps up (0.5× real time)."
        )
    );
    let nothing_keeps_up = with_result(
        with_result(slow, HARDWARE, "ggml-base.bin", 1.1),
        HARDWARE,
        "ggml-tiny.bin",
        0.97,
    );
    assert_eq!(
        speed_check_message(&nothing_keeps_up, HARDWARE, "ggml-small.bin", false).as_deref(),
        Some(
            "Checked on this device: 1.4× real time, too slow to keep up with recordings. \
             No smaller model keeps up here either."
        )
    );
}
