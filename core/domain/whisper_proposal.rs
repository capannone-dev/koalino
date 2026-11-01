//! Proposes a different Whisper model from how the current one runs on this device (#68).
//!
//! `whisper_suggestion` estimates from the hardware before anything is known. This uses the device's
//! own telemetry: a model that falls behind live speech gets a smaller one, and one with plenty of
//! headroom gets the next larger one. It is advice: nothing changes or downloads until the owner
//! accepts. Telemetry measures speed and energy, not accuracy, so accuracy comes from a fixed order.

/// Fewest chunks of a model that count as a measurement.
pub const MIN_MEASURED_CHUNKS: usize = 5;
/// Least audio that counts as a measurement, so that a few short chunks can't decide.
const MIN_MEASURED_AUDIO_MS: u64 = 120_000;
/// Only a model's newest chunks count, so a change of device conditions or mode shows up soon.
const NEWEST_CHUNKS_PER_MODEL: usize = 30;
/// Above this multiple of audio length, a model is falling behind. Below 1, because voice windows
/// and the rest of the recording need time too.
const FALLING_BEHIND_FACTOR: f32 = 0.9;
/// A proposed model must be expected to stay under this, to leave a real margin.
const COMFORTABLE_FACTOR: f32 = 0.6;

/// The full models from least to most accurate, with their cost relative to `base`. The costs come
/// from an S23 (`base` 0.14, `small` 0.45, `medium` 1.5 times audio length). `tiny` and turbo are
/// rough, so anything derived from them is presented as an estimate.
const MODEL_LADDER: [(&str, f32); 5] = [
    ("ggml-tiny.bin", 0.6),
    ("ggml-base.bin", 1.0),
    ("ggml-small.bin", 3.2),
    ("ggml-medium.bin", 10.7),
    ("ggml-large-v3-turbo.bin", 10.7),
];
/// The ladder's position of `small`, the largest model proposed on a phone.
const LARGEST_PHONE_RUNG: usize = 2;

/// One finished transcription, without any of its content.
#[derive(Clone, Debug, PartialEq)]
pub struct TranscriptionMeasurement {
    /// The model's file stem, such as `ggml-small`.
    pub model_label: String,
    /// The length of the audio; zero for records older than this field.
    pub audio_ms: u64,
    /// How long transcription took.
    pub processing_ms: u64,
    /// The estimated energy.
    pub power_mwh: f32,
}

/// A model worth trying.
#[derive(Clone, Debug, PartialEq)]
pub struct WhisperProposal {
    /// The catalog file name.
    pub file_name: &'static str,
    /// Why, in plain language for Settings.
    pub reason: String,
    /// True when the proposed model's speed was measured here, false for an estimate.
    pub measured: bool,
}

/// What a model's chunks add up to.
struct ModelSpeed {
    real_time_factor: f32,
    power_per_audio_ms: f32,
}

/// Adds up a model's newest chunks, if there are enough.
///
/// Inputs: The measurements, oldest first, and a file stem.
/// Outputs: The speed, or `None` with too little data. Side effects: None.
fn measured_speed(measurements: &[TranscriptionMeasurement], stem: &str) -> Option<ModelSpeed> {
    let newest: Vec<&TranscriptionMeasurement> = measurements
        .iter()
        .rev()
        .filter(|row| row.model_label == stem && row.audio_ms > 0)
        .take(NEWEST_CHUNKS_PER_MODEL)
        .collect();
    let audio_ms: u64 = newest.iter().map(|row| row.audio_ms).sum();
    if newest.len() < MIN_MEASURED_CHUNKS || audio_ms < MIN_MEASURED_AUDIO_MS {
        return None;
    }
    let processing_ms: u64 = newest.iter().map(|row| row.processing_ms).sum();
    let power_mwh: f32 = newest.iter().map(|row| row.power_mwh).sum();
    Some(ModelSpeed {
        real_time_factor: processing_ms as f32 / audio_ms as f32,
        power_per_audio_ms: power_mwh / audio_ms as f32,
    })
}

/// Finds a model's place on the ladder; `ggml-small-q5_1.bin` and `ggml-small.en.bin` are `small`.
///
/// Inputs: A catalog file name. Outputs: The position, or `None` for models above the ladder such as
/// `large-v3`. Side effects: None.
fn ladder_position(file_name: &str) -> Option<usize> {
    let stem = file_name.strip_suffix(".bin").unwrap_or(file_name);
    MODEL_LADDER.iter().rposition(|(rung, _)| {
        let rung_stem = rung.strip_suffix(".bin").unwrap_or(rung);
        stem == rung_stem
            || stem.starts_with(&format!("{rung_stem}-"))
            || stem.starts_with(&format!("{rung_stem}."))
    })
}

/// Proposes a model from this device's measurements.
///
/// Inputs: The current model's file name; measurements, oldest first; file names the owner dismissed;
/// and whether this is a phone.
/// Outputs: A proposal, or `None` with too little data or nothing clearly better. Side effects: None.
pub fn propose_from_measurements(
    current_file: &str,
    measurements: &[TranscriptionMeasurement],
    dismissed: &[String],
    is_phone: bool,
) -> Option<WhisperProposal> {
    let position = ladder_position(current_file)?;
    let current_stem = current_file.strip_suffix(".bin").unwrap_or(current_file);
    let current = measured_speed(measurements, current_stem)?;
    if current.real_time_factor > FALLING_BEHIND_FACTOR {
        propose_smaller(position, &current, measurements, dismissed)
    } else {
        propose_larger(position, &current, measurements, dismissed, is_phone)
    }
}

/// Proposes the most accurate smaller model expected to keep up, or else the smallest not dismissed.
///
/// Inputs: The current position and speed, the measurements, and dismissed files.
/// Outputs: The proposal, or `None` when no smaller model remains. Side effects: None.
fn propose_smaller(
    position: usize,
    current: &ModelSpeed,
    measurements: &[TranscriptionMeasurement],
    dismissed: &[String],
) -> Option<WhisperProposal> {
    let current_cost = MODEL_LADDER[position].1;
    let candidates = MODEL_LADDER[..position]
        .iter()
        .rev()
        .filter(|(file, _)| !dismissed.iter().any(|name| name == file));
    let mut last = None;
    for (file, cost) in candidates {
        let stem = file.strip_suffix(".bin").unwrap_or(file);
        let measured = measured_speed(measurements, stem);
        let factor = measured
            .as_ref()
            .map_or(current.real_time_factor * cost / current_cost, |speed| {
                speed.real_time_factor
            });
        let proposal = WhisperProposal {
            file_name: file,
            reason: smaller_reason(current, measured.as_ref()),
            measured: measured.is_some(),
        };
        if factor <= COMFORTABLE_FACTOR {
            return Some(proposal);
        }
        last = Some(proposal);
    }
    last
}

/// Words the reason for a smaller model.
///
/// Inputs: The current speed, and the proposed model's speed when measured.
/// Outputs: The reason. Side effects: None.
fn smaller_reason(current: &ModelSpeed, proposed: Option<&ModelSpeed>) -> String {
    let start = "Your current model is falling behind your recordings on this device.";
    match proposed {
        Some(speed) if current.power_per_audio_ms > 0.0 && speed.power_per_audio_ms > 0.0 => {
            let saved = 100.0 * (1.0 - speed.power_per_audio_ms / current.power_per_audio_ms);
            format!(
                "{start} This one kept up here, using about {saved:.0}% less battery, with more mistakes."
            )
        }
        Some(_) => format!("{start} This one kept up here, with more mistakes."),
        None => {
            format!("{start} This one should keep up, with more mistakes. That is an estimate.")
        }
    }
}

/// Proposes the next larger model when it should still keep up comfortably.
///
/// Inputs: The current position and speed, the measurements, dismissed files, and whether this is a
/// phone. Outputs: The proposal, or `None`. Side effects: None.
fn propose_larger(
    position: usize,
    current: &ModelSpeed,
    measurements: &[TranscriptionMeasurement],
    dismissed: &[String],
    is_phone: bool,
) -> Option<WhisperProposal> {
    let (file, cost) = MODEL_LADDER.get(position + 1)?;
    if (is_phone && position + 1 > LARGEST_PHONE_RUNG) || dismissed.iter().any(|name| name == file)
    {
        return None;
    }
    let stem = file.strip_suffix(".bin").unwrap_or(file);
    let measured = measured_speed(measurements, stem);
    let factor = measured.as_ref().map_or(
        current.real_time_factor * cost / MODEL_LADDER[position].1,
        |speed| speed.real_time_factor,
    );
    if factor > COMFORTABLE_FACTOR {
        return None;
    }
    let reason = if measured.is_some() {
        "Your device has time to spare, and this model kept up here. It makes fewer mistakes."
    } else {
        "Your device has time to spare, so this model should keep up and make fewer mistakes. That is an estimate."
    };
    Some(WhisperProposal {
        file_name: file,
        reason: reason.to_owned(),
        measured: measured.is_some(),
    })
}
