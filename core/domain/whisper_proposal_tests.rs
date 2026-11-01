//! Specifies proposals from this device's own measurements: falling behind, plenty of headroom, too
//! little data, a dismissed proposal, and the limits on phones.

use super::whisper_proposal::{
    MIN_MEASURED_CHUNKS, TranscriptionMeasurement, WhisperProposal, propose_from_measurements,
};

const CHUNK_MS: u64 = 30_000;

/// Enough chunks of one model at a given multiple of audio length.
fn measured(model_label: &str, real_time_factor: f32) -> Vec<TranscriptionMeasurement> {
    (0..MIN_MEASURED_CHUNKS)
        .map(|_| TranscriptionMeasurement {
            model_label: model_label.to_owned(),
            audio_ms: CHUNK_MS,
            processing_ms: (CHUNK_MS as f32 * real_time_factor) as u64,
            power_mwh: CHUNK_MS as f32 * real_time_factor / 1_000.0,
        })
        .collect()
}

fn propose(
    current: &str,
    measurements: &[TranscriptionMeasurement],
    dismissed: &[String],
    is_phone: bool,
) -> Option<WhisperProposal> {
    propose_from_measurements(current, measurements, dismissed, is_phone)
}

#[test]
fn too_little_data_proposes_nothing() {
    let mut few = measured("ggml-small", 3.0);
    few.pop();
    assert_eq!(propose("ggml-small.bin", &few, &[], false), None);

    let mut short = measured("ggml-small", 3.0);
    for row in &mut short {
        row.audio_ms = 1_000;
    }
    assert_eq!(propose("ggml-small.bin", &short, &[], false), None);
}

/// A model falling behind gets a smaller one that should keep up, and it says this is an estimate.
#[test]
fn falling_behind_proposes_a_smaller_model() {
    let proposal = propose("ggml-medium.bin", &measured("ggml-medium", 2.0), &[], false)
        .unwrap_or_else(|| panic!("a proposal is expected"));

    assert_eq!(proposal.file_name, "ggml-small.bin");
    assert!(!proposal.measured);
    assert!(proposal.reason.contains("estimate"), "{}", proposal.reason);
}

/// A measured smaller model is trusted over the estimate, and its saving is stated.
#[test]
fn a_measured_smaller_model_is_preferred_and_its_saving_stated() {
    let mut rows = measured("ggml-medium", 2.0);
    rows.extend(measured("ggml-small", 0.9));
    rows.extend(measured("ggml-base", 0.3));

    let proposal = propose("ggml-medium.bin", &rows, &[], false)
        .unwrap_or_else(|| panic!("a proposal is expected"));

    assert_eq!(proposal.file_name, "ggml-base.bin");
    assert!(proposal.measured);
    assert!(proposal.reason.contains("85%"), "{}", proposal.reason);
}

/// Plenty of headroom gets the next larger model, when that should still keep up.
#[test]
fn headroom_proposes_a_larger_model() {
    let proposal = propose("ggml-base.bin", &measured("ggml-base", 0.1), &[], false)
        .unwrap_or_else(|| panic!("a proposal is expected"));
    assert_eq!(proposal.file_name, "ggml-small.bin");

    assert_eq!(
        propose("ggml-base.bin", &measured("ggml-base", 0.4), &[], false),
        None
    );
}

/// A larger model measured as too slow is not proposed from an estimate.
#[test]
fn a_larger_model_measured_as_too_slow_is_not_proposed() {
    let mut rows = measured("ggml-base", 0.1);
    rows.extend(measured("ggml-small", 0.8));

    assert_eq!(propose("ggml-base.bin", &rows, &[], false), None);
}

/// A dismissed proposal does not come back, and a smaller model after it can be offered instead.
#[test]
fn a_dismissed_proposal_does_not_return() {
    let rows = measured("ggml-medium", 2.0);
    let dismissed = vec!["ggml-small.bin".to_owned()];
    let next = propose("ggml-medium.bin", &rows, &dismissed, false);
    assert_eq!(
        next.map(|proposal| proposal.file_name),
        Some("ggml-base.bin")
    );

    let headroom = measured("ggml-base", 0.1);
    assert_eq!(propose("ggml-base.bin", &headroom, &dismissed, false), None);
}

/// Phones are not offered `medium`, which leaves no margin for live use there.
#[test]
fn a_phone_is_not_offered_medium() {
    assert_eq!(
        propose("ggml-small.bin", &measured("ggml-small", 0.05), &[], true),
        None
    );
    assert!(propose("ggml-small.bin", &measured("ggml-small", 0.05), &[], false).is_some());
}

/// A quantized model is compared with its family, and other rows don't count for it.
#[test]
fn a_quantized_model_belongs_to_its_family() {
    let proposal = propose(
        "ggml-medium-q5_0.bin",
        &measured("ggml-medium-q5_0", 2.0),
        &[],
        false,
    )
    .unwrap_or_else(|| panic!("a proposal is expected"));
    assert_eq!(proposal.file_name, "ggml-small.bin");

    assert_eq!(
        propose(
            "ggml-medium-q5_0.bin",
            &measured("ggml-medium", 2.0),
            &[],
            false
        ),
        None
    );
}

/// Older rows without audio length, and the smallest model, propose nothing.
#[test]
fn nothing_is_proposed_when_there_is_nothing_better() {
    let mut no_length = measured("ggml-small", 3.0);
    for row in &mut no_length {
        row.audio_ms = 0;
    }
    assert_eq!(propose("ggml-small.bin", &no_length, &[], false), None);
    assert_eq!(
        propose("ggml-tiny.bin", &measured("ggml-tiny", 2.0), &[], false),
        None
    );
}
