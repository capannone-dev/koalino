//! Settings' Whisper model controls over the C ABI (#67).
//!
//! `note_taker_transcription_settings` returns everything Settings shows as JSON. Transcription
//! runs on the device: the app keeps no API keys (#117). Changes take effect at the next recording, because the runtime
//! rebuilds the pipeline when a recording starts after a change.

use std::{
    collections::BTreeMap,
    ffi::{CStr, CString, c_char, c_int},
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
};

use serde_json::json;

use crate::{
    adapters::ai::{
        compute_resources::core_max_speeds,
        gpu_attempt_guard::{clear_gpu_block, gpu_is_blocked},
        npu_whisper::install::{NpuInstall, supported_soc, this_soc},
        whisper_model_download::{
            HuggingFaceModelSource, ModelDownloadError, download_whisper_model,
        },
    },
    device_resources::{available_disk_bytes, total_memory_bytes},
    domain::{
        whisper_catalog::{WHISPER_MODELS, WhisperModelEntry, find_whisper_model},
        whisper_speed_check::{next_model_to_check, speed_check_message, stepped_down_model},
        whisper_suggestion::{
            AutomaticModel, DeviceGpu, DeviceProfile, WhisperSuggestion, automatic_model,
            suggest_whisper_model,
        },
    },
    ffi_settings::app_setting,
    pipeline_composition::{efficient_gpu_status, local_gpu_megabytes},
    speed_check::{CheckTarget, load_record},
    transcription_settings::{
        SavedTranscriptionSettings, TranscriptionPaths, load_settings, save_settings,
        saved_keep_for, with_automatic_model,
    },
};
use crate::{
    application::combined_meetings::{MAX_KEEP_SPEECH_DAYS, MIN_KEEP_SPEECH_DAYS, keep_for_days},
    domain::transcription_glossary::TranscriptionGlossary,
    transcription_settings::{
        computer_sound_enabled, keep_speech_days, save_computer_sound, save_glossary,
        save_keep_speech_days, save_transcript_pass, transcript_pass_enabled,
    },
};

const STATUS_OK: c_int = 0;
const STATUS_INVALID: c_int = 1;
const STATUS_FAILED: c_int = 2;
/// Another download is running.
const STATUS_BUSY: c_int = 5;
/// The file would not fit on disk.
const STATUS_NO_SPACE: c_int = 6;
/// The model is in use or shipped with the app.
const STATUS_NOT_REMOVABLE: c_int = 7;

/// Share of total memory a model may use before Settings warns about it.
const MEMORY_WARNING_PERCENT: u64 = 70;
/// Space kept free beyond the file itself.
const DISK_MARGIN_BYTES: u64 = 200 * 1024 * 1024;

/// Set when a choice changes, so the next recording rebuilds the pipeline.
static PIPELINE_STALE: AtomicBool = AtomicBool::new(false);

/// Reports and clears whether the pipeline must be rebuilt. Side effects: Clears the flag.
pub fn take_pipeline_stale() -> bool {
    PIPELINE_STALE.swap(false, Ordering::Relaxed)
}

/// The progress of the current or last download.
#[derive(Clone, Default)]
struct DownloadStatus {
    file_name: Option<String>,
    received_bytes: u64,
    total_bytes: u64,
    /// `downloading`, `done`, `cancelled` or `failed`.
    state: &'static str,
    /// `no_network`, `server`, `corrupted` or `storage`.
    error: Option<&'static str>,
}

/// The single download slot.
fn download_slot() -> &'static (Mutex<DownloadStatus>, Arc<AtomicBool>) {
    static SLOT: OnceLock<(Mutex<DownloadStatus>, Arc<AtomicBool>)> = OnceLock::new();
    SLOT.get_or_init(|| {
        (
            Mutex::new(DownloadStatus::default()),
            Arc::new(AtomicBool::new(false)),
        )
    })
}

/// Reads the app's paths.
fn paths() -> TranscriptionPaths {
    TranscriptionPaths::from_lookup(&app_setting)
}

/// Whether this build can record what the computer plays (#108): the `computer-sound` feature on
/// Windows or Linux.
pub(crate) fn computer_sound_available() -> bool {
    cfg!(all(
        feature = "computer-sound",
        any(target_os = "windows", target_os = "linux")
    ))
}

/// Whether the next start or resume should also record the computer's sound (#108).
///
/// Side effects: Reads the settings file.
pub(crate) fn computer_sound_wanted() -> bool {
    computer_sound_available() && computer_sound_enabled(&load_settings(&paths().settings_file))
}

/// Reads how long speech waits to be combined on this device (#93).
///
/// Side effects: Reads the settings file.
pub(crate) fn speech_keep_for() -> std::time::Duration {
    saved_keep_for(&load_settings(&paths().settings_file))
}

/// Reads, changes and stores the choices, then marks the pipeline stale.
fn update_settings(change: impl FnOnce(&mut SavedTranscriptionSettings)) -> c_int {
    let paths = paths();
    let mut saved = load_settings(&paths.settings_file);
    change(&mut saved);
    match save_settings(&paths.settings_file, &saved) {
        Ok(()) => {
            PIPELINE_STALE.store(true, Ordering::Relaxed);
            STATUS_OK
        }
        Err(error) => {
            tracing::warn!(error = %error, "unable to store the transcription settings");
            STATUS_FAILED
        }
    }
}

/// Reads a C string argument.
///
/// # Safety
/// `text` must be null or a valid NUL-terminated C string for this call.
unsafe fn text_argument<'a>(text: *const c_char) -> Option<&'a str> {
    if text.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(text) }.to_str().ok()
}

/// Builds one model's row.
fn model_row(
    model: &WhisperModelEntry,
    paths: &TranscriptionPaths,
    total_memory: Option<u64>,
) -> serde_json::Value {
    json!({
        "file_name": model.file_name(),
        "name": model.name(),
        "family": model.family(),
        "size_bytes": model.size_bytes(),
        "english_only": model.is_english_only(),
        "on_device": paths.locate_model(model.file_name()).is_some(),
        "removable": paths.is_removable(model.file_name()),
        "too_large_for_memory": total_memory.is_some_and(|total| {
            model.estimated_memory_bytes() > total / 100 * MEMORY_WARNING_PERCENT
        }),
    })
}

/// Reads what this device offers, for the suggested model.
///
/// Inputs: Whether efficient mode can run here, and whether the GPU is blocked because attempts
/// on it never finished: transcription then runs on the CPU, so the profile has no GPU and
/// Automatic picks (and the speed check times) a model the CPU keeps up with.
/// Outputs: The profile.
/// Side effects: Reads system files; may start whisper.cpp's backends.
pub(crate) fn device_profile(efficient_gpu_available: bool, gpu_blocked: bool) -> DeviceProfile {
    let graphics_card_gib = if gpu_blocked {
        0
    } else {
        u64::from(local_gpu_megabytes()) / 1024
    };
    DeviceProfile {
        is_phone: cfg!(target_os = "android"),
        total_memory_bytes: total_memory_bytes(),
        gpu: if gpu_blocked {
            DeviceGpu::None
        } else if efficient_gpu_available {
            DeviceGpu::EfficientPhoneGpu
        } else if graphics_card_gib > 0 {
            DeviceGpu::Discrete {
                memory_gib: graphics_card_gib,
            }
        } else {
            DeviceGpu::None
        },
        fastest_core_khz: core_max_speeds().into_iter().max(),
    }
}

/// Resolves "Automatic" (#106) on this device.
///
/// Inputs: The paths, to see which models are here.
/// Outputs: The device's suggestion and what Automatic runs, `None` when no model is here.
/// Side effects: Reads system files; may start whisper.cpp's backends to find a GPU.
fn automatic_on_this_device(
    paths: &TranscriptionPaths,
) -> (WhisperSuggestion, Option<AutomaticModel<'_>>) {
    let suggestion = suggest_whisper_model(&device_profile(
        efficient_gpu_status().0.is_some(),
        gpu_is_blocked(&paths.gpu_attempt_marker),
    ));
    let automatic = automatic_model(
        suggestion,
        |file_name| paths.locate_model(file_name).is_some(),
        paths.shipped_file_name(),
    );
    (suggestion, automatic)
}

/// What Automatic runs on this device (#106): its own model, or the smaller one the speed check
/// (#109) found keeps up when its own falls behind on this hardware (#181).
///
/// Inputs: The paths, and Automatic's model as `automatic_on_this_device` resolves it.
/// Outputs: The model to run; unchanged before the check, when Automatic's model keeps up, when no
/// smaller model kept up, or when the timings are from other hardware or another set of models.
/// Side effects: Reads the check's record; may start whisper.cpp's backends to find a GPU.
fn automatic_to_run<'a>(
    paths: &TranscriptionPaths,
    automatic: Option<AutomaticModel<'a>>,
) -> Option<AutomaticModel<'a>> {
    let automatic = automatic?;
    let models = models_on_device(paths);
    let key = hardware_key(
        efficient_gpu_status().0.is_some(),
        gpu_is_blocked(&paths.gpu_attempt_marker),
        &models,
    );
    let record = load_record(&paths.speed_check_file);
    Some(
        match stepped_down_model(&record, &key, automatic.file_name)
            .filter(|smaller| models.contains_key(*smaller))
        {
            Some(smaller) => AutomaticModel {
                file_name: smaller,
                ..automatic
            },
            None => automatic,
        },
    )
}

/// The models on the device, by file name: the catalog's and the shipped one.
fn models_on_device(paths: &TranscriptionPaths) -> BTreeMap<String, PathBuf> {
    let mut models: BTreeMap<String, PathBuf> = WHISPER_MODELS
        .iter()
        .filter_map(|model| {
            paths
                .locate_model(model.file_name())
                .map(|path| (model.file_name().to_owned(), path))
        })
        .collect();
    if let Some(shipped) = paths.shipped_file_name()
        && let Some(path) = paths.locate_model(shipped)
    {
        models.entry(shipped.to_owned()).or_insert(path);
    }
    models
}

/// Describes the hardware and the models on it, so a change of either checks again (#109).
fn hardware_key(
    efficient_gpu_available: bool,
    gpu_blocked: bool,
    models: &BTreeMap<String, PathBuf>,
) -> String {
    let names: Vec<&str> = models.keys().map(String::as_str).collect();
    format!(
        "{:?}; {}",
        device_profile(efficient_gpu_available, gpu_blocked),
        names.join(", ")
    )
}

/// What the speed check (#109) times: Automatic's model on this device.
///
/// Inputs: The paths. Outputs: The target, or `None` with a hand-picked model on the device, or
/// with no model. Side effects: Reads files; may start whisper.cpp's backends to
/// find a GPU.
fn speed_check_target_in(paths: &TranscriptionPaths) -> Option<CheckTarget> {
    let saved = load_settings(&paths.settings_file);
    let hand_picked = saved
        .whisper_model
        .as_deref()
        .is_some_and(|file_name| paths.locate_model(file_name).is_some());
    if hand_picked {
        return None;
    }
    let automatic = automatic_on_this_device(paths).1?;
    let models = models_on_device(paths);
    Some(CheckTarget {
        automatic_file: automatic.file_name.to_owned(),
        efficient: automatic.efficient,
        hardware_key: hardware_key(
            efficient_gpu_status().0.is_some(),
            gpu_is_blocked(&paths.gpu_attempt_marker),
            &models,
        ),
        models,
    })
}

/// The speed check's target for the after-meeting worker of `database_path`.
///
/// Side effects: As `speed_check_target_in`.
pub fn speed_check_target(database_path: &std::path::Path) -> Option<CheckTarget> {
    let database = database_path.display().to_string();
    let paths = TranscriptionPaths::from_lookup(&|name| match name {
        "NOTE_TAKER_DATABASE_PATH" => Some(database.clone()),
        _ => app_setting(name),
    });
    speed_check_target_in(&paths)
}

/// Words the speed check's result for Automatic's model, for Settings.
///
/// Outputs: The sentence, or `None` before the model was timed on this hardware.
/// Side effects: Reads the record.
fn speed_check_message_in(
    paths: &TranscriptionPaths,
    automatic_file: &str,
    efficient_gpu_available: bool,
) -> Option<String> {
    let record = load_record(&paths.speed_check_file);
    let models = models_on_device(paths);
    let key = hardware_key(
        efficient_gpu_available,
        gpu_is_blocked(&paths.gpu_attempt_marker),
        &models,
    );
    // Automatic's own result is in the record when there is one, so only on-device matters here.
    let checking_next = next_model_to_check(automatic_file, &record, &key, &|_| false, &|file| {
        models.contains_key(file)
    })
    .is_some();
    speed_check_message(&record, &key, automatic_file, checking_next)
}

/// The saved choices with "Automatic" applied, as the next recording uses them.
///
/// Inputs: The saved choices and the paths. Outputs: The choices; a hand-picked model that is
/// no longer on the device counts as Automatic.
/// Side effects: See `automatic_on_this_device`, only when Automatic applies.
pub(crate) fn effective_settings(
    saved: &SavedTranscriptionSettings,
    paths: &TranscriptionPaths,
) -> SavedTranscriptionSettings {
    let mut saved = saved.clone();
    saved.whisper_model = saved
        .whisper_model
        .filter(|file_name| paths.locate_model(file_name).is_some());
    if saved.whisper_model.is_some() {
        return saved;
    }
    with_automatic_model(
        &saved,
        automatic_to_run(paths, automatic_on_this_device(paths).1),
    )
}

/// Describes the NPU engine for Settings (#75).
///
/// Inputs: The paths, the saved choices, whether this build has the `npu-whisper` feature, and
/// this phone's system-on-chip. Outputs: `available` (the build can), `supported` (and so can
/// this chip), `directory` (where the host downloads the files), `installed` (they are all
/// there) and `enabled` (the owner's choice). Side effects: Checks files.
pub(crate) fn npu_json(
    paths: &TranscriptionPaths,
    saved: &SavedTranscriptionSettings,
    available: bool,
    soc_model: Option<&str>,
) -> serde_json::Value {
    let install = NpuInstall::new(&paths.npu_directory);
    json!({
        "available": available,
        "supported": available && soc_model.is_some_and(supported_soc),
        "directory": paths.npu_directory.display().to_string(),
        "installed": install.is_installed(),
        "enabled": saved.npu_whisper,
    })
}

/// Builds the whole Settings view. Side effects: Reads files.
fn settings_json() -> String {
    let paths = paths();
    let saved = load_settings(&paths.settings_file);
    let total_memory = total_memory_bytes();
    // A hand-picked model on this device; `None` is Automatic (#106).
    let selected = saved
        .whisper_model
        .clone()
        .filter(|file| paths.locate_model(file).is_some());
    let download = download_slot()
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let disk_probe = paths.models_directory.parent().map(PathBuf::from);
    let (efficient_gpu_name, gpu_in_use) = efficient_gpu_status();
    let suggestion = suggest_whisper_model(&device_profile(
        efficient_gpu_name.is_some(),
        gpu_is_blocked(&paths.gpu_attempt_marker),
    ));
    let automatic = automatic_model(
        suggestion,
        |file_name| paths.locate_model(file_name).is_some(),
        paths.shipped_file_name(),
    );
    json!({
        "selected_model": selected,
        "shipped_model": paths.shipped_file_name(),
        "available_disk_bytes": disk_probe.as_deref().and_then(available_disk_bytes),
        "suggestion": {
            "file_name": suggestion.file_name,
            "efficient": suggestion.efficient,
            "reason": suggestion.reason,
        },
        "glossary": saved.glossary,
        "keep_speech": {
            "days": keep_speech_days(saved_keep_for(&saved)),
            "min_days": MIN_KEEP_SPEECH_DAYS,
            "max_days": MAX_KEEP_SPEECH_DAYS,
        },
        "automatic": {
            "speed_check": automatic.and_then(|choice| speed_check_message_in(
                &paths,
                choice.file_name,
                efficient_gpu_name.is_some(),
            )),
            "enabled": selected.is_none(),
            // The model Automatic runs, after any step down; the check's message above is about
            // Automatic's own model.
            "file_name": automatic_to_run(&paths, automatic).map(|choice| choice.file_name),
            "efficient": automatic.is_some_and(|choice| choice.efficient),
            "waiting_for_download": automatic.and_then(|choice| choice.waiting_for_download),
        },
        "after_meeting_pass": transcript_pass_enabled(&saved),
        // The pass's own model when it is still on this device; null re-transcribes with the
        // live model (#87).
        "after_meeting_pass_model": saved
            .transcript_pass_model
            .as_deref()
            .filter(|file| paths.locate_model(file).is_some()),
        "computer_sound": {
            "available": computer_sound_available(),
            "enabled": computer_sound_enabled(&saved),
        },
        "npu": npu_json(
            &paths,
            &saved,
            cfg!(feature = "npu-whisper"),
            this_soc().as_deref(),
        ),
        "efficient": {
            "gpu_name": efficient_gpu_name,
            "enabled": saved.efficient_gpu,
            "gpu_blocked": gpu_is_blocked(&paths.gpu_attempt_marker),
            "gpu_in_use": gpu_in_use,
        },
        "models": WHISPER_MODELS.iter()
            .map(|model| model_row(model, &paths, total_memory))
            .collect::<Vec<_>>(),
        "download": {
            "file_name": download.file_name,
            "received_bytes": download.received_bytes,
            "total_bytes": download.total_bytes,
            "state": download.state,
            "error": download.error,
        },
    })
    .to_string()
}

/// Returns what Settings shows as JSON; free it with `note_taker_free_string`.
///
/// Outputs: The JSON, or null if it can't be built. The key is never included.
/// Side effects: Reads files and the key store, which may prompt to unlock.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_transcription_settings() -> *mut c_char {
    CString::new(settings_json())
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Records that the owner doesn't want a proposed model, which changes nothing else.
///
/// Inputs: A catalog file name.
/// Outputs: 0, or 1 when unknown. Side effects: Stores the dismissal.
///
/// # Safety
/// `file_name` must be null or a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_dismiss_whisper_proposal(file_name: *const c_char) -> c_int {
    let Some(file_name) = (unsafe { text_argument(file_name) }) else {
        return STATUS_INVALID;
    };
    if find_whisper_model(file_name).is_none() {
        return STATUS_INVALID;
    }
    update_settings(|saved| {
        if !saved
            .dismissed_proposals
            .iter()
            .any(|name| name == file_name)
        {
            saved.dismissed_proposals.push(file_name.to_owned());
        }
    })
}

/// Reads what a proposal depends on from the saved settings.
///
/// Outputs: The model in use and the dismissed files, or `None` without a model.
/// Side effects: Reads the settings file.
pub(crate) fn proposal_settings() -> Option<(String, Vec<String>)> {
    let paths = paths();
    let saved = load_settings(&paths.settings_file);
    let current = effective_settings(&saved, &paths)
        .whisper_model
        .or_else(|| paths.shipped_file_name().map(str::to_owned))?;
    Some((current, saved.dismissed_proposals))
}

/// Chooses the Whisper model for the next recording.
///
/// Inputs: A catalog file name that is on this device.
/// Outputs: 0, or 1 when unknown or not on this device. Side effects: Stores the choice.
///
/// # Safety
/// `file_name` must be null or a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_set_whisper_model(file_name: *const c_char) -> c_int {
    let Some(file_name) = (unsafe { text_argument(file_name) }) else {
        return STATUS_INVALID;
    };
    if paths().locate_model(file_name).is_none() {
        return STATUS_INVALID;
    }
    update_settings(|saved| saved.whisper_model = Some(file_name.to_owned()))
}

/// Saves the glossary of names and terms Whisper is prompted with, from the next recording on.
///
/// Inputs: The terms, one per line (blank lines are ignored; an empty text clears it).
/// Outputs: 0, 1 when a term is too long or the terms together are (see
/// `TranscriptionGlossary::parse`), or 2 when the file can't be written.
/// Side effects: Stores the glossary. The pipeline is not rebuilt: each recording reads it when it
/// starts. The terms are user content and are never logged.
///
/// # Safety
/// `terms` must be null or a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_set_glossary(terms: *const c_char) -> c_int {
    let Some(terms) = (unsafe { text_argument(terms) }) else {
        return STATUS_INVALID;
    };
    let Ok(glossary) = TranscriptionGlossary::parse(terms.lines()) else {
        return STATUS_INVALID;
    };
    match save_glossary(&paths().settings_file, &glossary) {
        Ok(()) => STATUS_OK,
        Err(error) => {
            tracing::warn!(error = %error, "unable to store the glossary");
            STATUS_FAILED
        }
    }
}

/// Sets how many days a recording's speech may wait to be combined with another device's (#93).
///
/// Inputs: The days, from `MIN_KEEP_SPEECH_DAYS` to `MAX_KEEP_SPEECH_DAYS`.
/// Outputs: 0, 1 when out of range (nothing is saved), or 2 when the file can't be written.
/// Side effects: Stores the window; the after-meeting worker reads it at its next round and
/// deletes speech older than a shorter window then. The pipeline is not rebuilt.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_set_keep_speech_days(days: c_int) -> c_int {
    let Some(days) = u16::try_from(days)
        .ok()
        .filter(|days| keep_for_days(*days).is_some())
    else {
        return STATUS_INVALID;
    };
    match save_keep_speech_days(&paths().settings_file, days) {
        Ok(()) => STATUS_OK,
        Err(error) => {
            tracing::warn!(error = %error, "unable to store the speech retention window");
            STATUS_FAILED
        }
    }
}

/// Chooses "Automatic" (#106): the next recording uses the model suggested for this device.
///
/// Outputs: 0, or 2 when the choice can't be stored. Side effects: Forgets the hand-picked model;
/// downloaded models stay on the device.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_use_automatic_model() -> c_int {
    update_settings(|saved| saved.whisper_model = None)
}

/// Turns recording the computer's sound (#108) on or off from the next start or resume.
///
/// Inputs: Non-zero to turn it on. Outputs: 0, or 2 when the choice can't be stored.
/// Side effects: Stores the choice; a recording in progress keeps its capture until paused.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_set_computer_sound(enabled: c_int) -> c_int {
    match save_computer_sound(&paths().settings_file, enabled != 0) {
        Ok(()) => STATUS_OK,
        Err(error) => {
            tracing::warn!(error = %error, "unable to store the computer sound choice");
            STATUS_FAILED
        }
    }
}

/// Turns the after-meeting pass (#87) on or off from the next recording.
///
/// Inputs: Non-zero to turn it on. Outputs: 0, or 2 when the choice can't be stored.
/// Side effects: Stores the choice and rebuilds the pipeline before the next recording. Speech
/// already kept is still processed and deleted.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_set_after_meeting_pass(enabled: c_int) -> c_int {
    match save_transcript_pass(&paths().settings_file, enabled != 0) {
        Ok(()) => {
            PIPELINE_STALE.store(true, Ordering::Relaxed);
            STATUS_OK
        }
        Err(error) => {
            tracing::warn!(error = %error, "unable to store the after-meeting pass choice");
            STATUS_FAILED
        }
    }
}

/// Chooses the model the after-meeting pass (#87) re-transcribes with, from the next recording.
///
/// Inputs: A catalog file name that is on this device, or null (or empty) for the live model.
/// Outputs: 0, 1 when the model is unknown or not on this device, or 2 when the choice can't be
/// stored. Side effects: Stores the choice; the pipeline is rebuilt before the next recording.
///
/// # Safety
/// `file_name` must be null or a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_set_after_meeting_pass_model(
    file_name: *const c_char,
) -> c_int {
    let file_name = unsafe { text_argument(file_name) }.filter(|name| !name.is_empty());
    set_pass_model_in(&paths(), file_name)
}

/// Stores the after-meeting pass's model in a given data folder (#87).
///
/// Inputs: The folder's paths and a model file name on this device, or `None` for the live model.
/// Outputs and side effects: As `note_taker_set_after_meeting_pass_model`.
fn set_pass_model_in(paths: &TranscriptionPaths, file_name: Option<&str>) -> c_int {
    if file_name.is_some_and(|name| paths.locate_model(name).is_none()) {
        return STATUS_INVALID;
    }
    let mut saved = load_settings(&paths.settings_file);
    saved.transcript_pass_model = file_name.map(str::to_owned);
    match save_settings(&paths.settings_file, &saved) {
        Ok(()) => {
            PIPELINE_STALE.store(true, Ordering::Relaxed);
            STATUS_OK
        }
        Err(error) => {
            tracing::warn!(error = %error, "unable to store the after-meeting pass model");
            STATUS_FAILED
        }
    }
}

/// Turns efficient mode on or off for the next recording.
///
/// Inputs: Non-zero to turn it on, which needs a GPU it supports; turning it off always works.
/// Outputs: 0, or 1 when this device has no such GPU. Side effects: Stores the choice.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_set_efficient_gpu(enabled: c_int) -> c_int {
    let enabled = enabled != 0;
    if enabled && efficient_gpu_status().0.is_none() {
        return STATUS_INVALID;
    }
    update_settings(|saved| saved.efficient_gpu = enabled)
}

/// Turns Whisper on the NPU on or off for the next recording (#75).
///
/// Inputs: Non-zero to turn it on, which needs a supported phone and every downloaded file;
/// turning it off always works. Outputs: 0, or 1 when the NPU can't be used here yet.
/// Side effects: Stores the choice and marks the pipeline stale.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_set_npu_whisper(enabled: c_int) -> c_int {
    let enabled = enabled != 0;
    if enabled {
        let paths = paths();
        let usable = cfg!(feature = "npu-whisper")
            && this_soc().as_deref().is_some_and(supported_soc)
            && NpuInstall::new(&paths.npu_directory).is_installed();
        if !usable {
            return STATUS_INVALID;
        }
    }
    update_settings(|saved| saved.npu_whisper = enabled)
}

/// Lets the GPU be tried again after a crash stopped it, for example after a system update.
///
/// Outputs: 0, or 2 when the record can't be removed. Side effects: Removes the record, and the
/// next recording tries the GPU.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_retry_gpu() -> c_int {
    if clear_gpu_block(&paths().gpu_attempt_marker).is_err() {
        return STATUS_FAILED;
    }
    PIPELINE_STALE.store(true, Ordering::Relaxed);
    STATUS_OK
}

/// Starts downloading a model in the background.
///
/// Inputs: A catalog file name. Outputs: 0, 1 when unknown, 5 when another is running, or 6 when
/// it won't fit. Side effects: Spawns a thread that contacts Hugging Face and writes the file;
/// progress appears in `note_taker_transcription_settings`.
///
/// # Safety
/// `file_name` must be null or a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_download_whisper_model(file_name: *const c_char) -> c_int {
    let Some(model) = (unsafe { text_argument(file_name) }).and_then(find_whisper_model) else {
        return STATUS_INVALID;
    };
    let paths = paths();
    if std::fs::create_dir_all(&paths.models_directory).is_err() {
        return STATUS_FAILED;
    }
    if available_disk_bytes(&paths.models_directory)
        .is_some_and(|free| free < model.size_bytes() + DISK_MARGIN_BYTES)
    {
        return STATUS_NO_SPACE;
    }
    let (status, cancel) = download_slot();
    {
        let mut status = status.lock().unwrap_or_else(PoisonError::into_inner);
        if status.state == "downloading" {
            return STATUS_BUSY;
        }
        *status = DownloadStatus {
            file_name: Some(model.file_name().to_owned()),
            received_bytes: 0,
            total_bytes: model.size_bytes(),
            state: "downloading",
            error: None,
        };
    }
    cancel.store(false, Ordering::Relaxed);
    let thread_cancel = Arc::clone(cancel);
    let spawned = std::thread::Builder::new()
        .name("whisper-model-download".to_owned())
        .spawn(move || {
            let result = HuggingFaceModelSource::new().and_then(|source| {
                download_whisper_model(
                    model,
                    &source,
                    &paths.models_directory,
                    &thread_cancel,
                    &mut |received| {
                        download_slot()
                            .0
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .received_bytes = received;
                    },
                )
            });
            let mut status = download_slot()
                .0
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            (status.state, status.error) = match &result {
                Ok(_) => ("done", None),
                Err(ModelDownloadError::Cancelled) => ("cancelled", None),
                Err(ModelDownloadError::Network) => ("failed", Some("no_network")),
                Err(ModelDownloadError::Server(_)) => ("failed", Some("server")),
                Err(ModelDownloadError::Corrupted) => ("failed", Some("corrupted")),
                Err(ModelDownloadError::Storage(_)) => ("failed", Some("storage")),
            };
            if let Err(error) = &result {
                tracing::warn!(error = %error, "Whisper model download did not finish");
            }
        });
    if spawned.is_err() {
        status.lock().unwrap_or_else(PoisonError::into_inner).state = "failed";
        return STATUS_FAILED;
    }
    STATUS_OK
}

/// Cancels the running download, if any. Side effects: The partial file gets removed.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_cancel_whisper_download() {
    download_slot().1.store(true, Ordering::Relaxed);
}

/// Removes a downloaded model, returning to the shipped one if it was chosen.
///
/// Inputs: A catalog file name. Outputs: 0, 7 when shipped or absent, or 2.
/// Side effects: Deletes the file and may change the choice. A recording that already loaded the
/// model keeps it in memory until it stops.
///
/// # Safety
/// `file_name` must be null or a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_remove_whisper_model(file_name: *const c_char) -> c_int {
    let Some(file_name) = (unsafe { text_argument(file_name) }) else {
        return STATUS_INVALID;
    };
    let paths = paths();
    if !paths.is_removable(file_name) {
        return STATUS_NOT_REMOVABLE;
    }
    if std::fs::remove_file(paths.models_directory.join(file_name)).is_err() {
        return STATUS_FAILED;
    }
    update_settings(|saved| {
        if saved.whisper_model.as_deref() == Some(file_name) {
            saved.whisper_model = None;
        }
    })
}

#[cfg(test)]
#[path = "ffi_transcription_tests.rs"]
mod tests;
