//! Times a short public clip on this device to confirm Automatic's model (#109).
//!
//! `whisper_suggestion` picks Automatic's model from memory, GPU and core speed, which on an unknown
//! device is only an estimate. Until real recordings measure it (`whisper_proposal`), the device
//! times a clip with that model; when it falls behind, the next smaller model on the device is
//! timed too, until one keeps up. Results are kept per hardware and set of models, so a change of
//! either checks again. This module only decides; the worker runs the clip and saves the timings
//! to telemetry, where the #68 rule reads them like any recording's.

use serde::{Deserialize, Serialize};

use super::whisper_proposal::{FALLING_BEHIND_FACTOR, model_cost};

/// How many times the clip is transcribed, each saved as one chunk: with the 15 s clip, the
/// 120 s and five chunks the #68 rule needs before it counts a model as measured.
pub const CHECK_RUNS: usize = 8;

/// One model's timing.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CheckedModel {
    /// The catalog file name, such as `ggml-small.bin`.
    pub file_name: String,
    /// Processing time over audio time; under `FALLING_BEHIND_FACTOR` keeps up.
    pub real_time_factor: f32,
}

/// The checks made on this device, stored beside the transcription settings.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SpeedCheckRecord {
    /// Describes the hardware and the models on the device when the checks ran.
    pub hardware_key: String,
    /// The models timed, in the order they were.
    pub models: Vec<CheckedModel>,
}

impl SpeedCheckRecord {
    /// The results that apply to `hardware_key`: none when the hardware or models changed.
    fn results_for(&self, hardware_key: &str) -> &[CheckedModel] {
        if self.hardware_key == hardware_key {
            &self.models
        } else {
            &[]
        }
    }

    /// One model's factor on `hardware_key`, if it was timed.
    fn factor(&self, hardware_key: &str, file_name: &str) -> Option<f32> {
        self.results_for(hardware_key)
            .iter()
            .find(|model| model.file_name == file_name)
            .map(|model| model.real_time_factor)
    }
}

/// Processing time over audio time.
///
/// Inputs: The audio's length and how long transcription took. Outputs: The factor; infinite
/// for no audio. Side effects: None.
pub fn real_time_factor(audio_ms: u64, processing_ms: u64) -> f32 {
    if audio_ms == 0 {
        return f32::INFINITY;
    }
    processing_ms as f32 / audio_ms as f32
}

/// Adds one model's timing.
///
/// Inputs: The record, the current hardware key, the model and its factor.
/// Outputs: The record with the result, replacing an earlier one for that model; results for
/// other hardware are dropped. Side effects: None.
#[must_use]
pub fn with_result(
    record: SpeedCheckRecord,
    hardware_key: &str,
    file_name: &str,
    real_time_factor: f32,
) -> SpeedCheckRecord {
    let mut models: Vec<CheckedModel> = record
        .results_for(hardware_key)
        .iter()
        .filter(|model| model.file_name != file_name)
        .cloned()
        .collect();
    models.push(CheckedModel {
        file_name: file_name.to_owned(),
        real_time_factor,
    });
    SpeedCheckRecord {
        hardware_key: hardware_key.to_owned(),
        models,
    }
}

/// The smaller models on the device, most accurate first.
fn smaller_models_on_device<'a>(
    file_name: &str,
    on_device: &'a dyn Fn(&str) -> bool,
) -> impl Iterator<Item = &'static str> + 'a {
    let cost = model_cost(file_name);
    super::whisper_proposal::ladder_files()
        .rev()
        .filter(move |(_, rung_cost)| cost.is_some_and(|cost| *rung_cost < cost))
        .map(|(file, _)| file)
        .filter(move |file| on_device(file))
}

/// Decides which model to time next, if any.
///
/// Inputs: Automatic's model; the record; the current hardware key; whether recordings already
/// measured a model here (`whisper_proposal`'s rule); whether a model is on the device.
/// Outputs: Automatic's model while it is neither timed nor measured; after it fell behind, the
/// most accurate smaller model on the device not yet timed, until one keeps up; else `None`.
/// Side effects: None.
pub fn next_model_to_check(
    automatic_file: &str,
    record: &SpeedCheckRecord,
    hardware_key: &str,
    is_measured: &dyn Fn(&str) -> bool,
    on_device: &dyn Fn(&str) -> bool,
) -> Option<String> {
    let Some(factor) = record.factor(hardware_key, automatic_file) else {
        return (!is_measured(automatic_file)).then(|| automatic_file.to_owned());
    };
    if factor <= FALLING_BEHIND_FACTOR {
        return None;
    }
    for smaller in smaller_models_on_device(automatic_file, on_device) {
        match record.factor(hardware_key, smaller) {
            Some(factor) if factor <= FALLING_BEHIND_FACTOR => return None,
            Some(_) => {}
            None => return Some(smaller.to_owned()),
        }
    }
    None
}

/// Words the result for Settings, beside the Automatic row.
///
/// Inputs: The record, the current hardware key, Automatic's model, and whether a smaller model is
/// still to be timed (`next_model_to_check` names one).
/// Outputs: The sentence, or `None` before Automatic's model was timed on this hardware.
/// Side effects: None.
pub fn speed_check_message(
    record: &SpeedCheckRecord,
    hardware_key: &str,
    automatic_file: &str,
    checking_next: bool,
) -> Option<String> {
    let factor = record.factor(hardware_key, automatic_file)?;
    let start = format!("Checked on this device: {factor:.1}× real time");
    if factor <= FALLING_BEHIND_FACTOR {
        return Some(format!("{start}, so it keeps up with recordings."));
    }
    let slow = format!("{start}, too slow to keep up with recordings.");
    if checking_next {
        return Some(format!("{slow} A smaller model is being checked."));
    }
    let keeps_up = record
        .results_for(hardware_key)
        .iter()
        .filter(|model| model.real_time_factor <= FALLING_BEHIND_FACTOR)
        .filter(|model| {
            model_cost(&model.file_name)
                .zip(model_cost(automatic_file))
                .is_some_and(|(cost, automatic_cost)| cost < automatic_cost)
        })
        .max_by(|a, b| {
            model_cost(&a.file_name)
                .partial_cmp(&model_cost(&b.file_name))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    Some(match keeps_up {
        Some(model) => format!(
            "{slow} {} keeps up ({:.1}× real time).",
            model_name(&model.file_name),
            model.real_time_factor
        ),
        None => format!("{slow} No smaller model keeps up here either."),
    })
}

/// A model's short name: `ggml-base.bin` is `base`.
fn model_name(file_name: &str) -> &str {
    let stem = file_name.strip_suffix(".bin").unwrap_or(file_name);
    stem.strip_prefix("ggml-").unwrap_or(stem)
}
