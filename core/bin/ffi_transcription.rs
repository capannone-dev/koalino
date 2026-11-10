//! Settings' Whisper model and OpenAI controls over the C ABI (#67).
//!
//! `note_taker_transcription_settings` returns everything Settings shows as JSON, and never the
//! key: only whether one is saved. Changes take effect at the next recording, because the runtime
//! rebuilds the pipeline when a recording starts after a change.

use std::{
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
        whisper_model_download::{
            HuggingFaceModelSource, ModelDownloadError, download_whisper_model,
        },
    },
    cloud_fallback::cloud_fallback_active,
    device_resources::{available_disk_bytes, total_memory_bytes},
    domain::{
        errors::CryptoError,
        whisper_catalog::{WHISPER_MODELS, WhisperModelEntry, find_whisper_model},
        whisper_suggestion::{
            AutomaticModel, DeviceGpu, DeviceProfile, WhisperSuggestion, automatic_model,
            suggest_whisper_model,
        },
    },
    ffi::provided_encryption_key,
    ffi_settings::app_setting,
    pipeline_composition::{OPENAI_API_KEY_VARIABLE, efficient_gpu_status, local_gpu_megabytes},
    transcription_settings::{
        OpenAiKeyStore, SavedProvider, SavedTranscriptionSettings, TranscriptionPaths,
        load_settings, save_settings, saved_keep_for, test_openai_key, validated_key,
        with_automatic_model,
    },
};
use crate::{
    application::combined_meetings::{MAX_KEEP_SPEECH_DAYS, MIN_KEEP_SPEECH_DAYS, keep_for_days},
    domain::transcription_glossary::TranscriptionGlossary,
    transcription_settings::{
        keep_speech_days, save_glossary, save_keep_speech_days, save_transcript_pass,
        transcript_pass_enabled,
    },
};

const STATUS_OK: c_int = 0;
const STATUS_INVALID: c_int = 1;
const STATUS_FAILED: c_int = 2;
/// The keychain is missing, so a key can't be saved (no plain-file fallback).
const STATUS_NO_KEYCHAIN: c_int = 3;
/// OpenAI was chosen with no key to use.
const STATUS_NO_KEY: c_int = 4;
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

/// Reads how long speech waits to be combined on this device (#93).
///
/// Side effects: Reads the settings file.
pub(crate) fn speech_keep_for() -> std::time::Duration {
    saved_keep_for(&load_settings(&paths().settings_file))
}

/// Opens the key store.
fn key_store(paths: &TranscriptionPaths) -> OpenAiKeyStore {
    OpenAiKeyStore::open(paths, provided_encryption_key())
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
/// Inputs: Whether efficient mode can run here. Outputs: The profile.
/// Side effects: Reads system files; may start whisper.cpp's backends.
fn device_profile(efficient_gpu_available: bool) -> DeviceProfile {
    let graphics_card_gib = u64::from(local_gpu_megabytes()) / 1024;
    DeviceProfile {
        is_phone: cfg!(target_os = "android"),
        total_memory_bytes: total_memory_bytes(),
        gpu: if efficient_gpu_available {
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
    let suggestion = suggest_whisper_model(&device_profile(efficient_gpu_status().0.is_some()));
    let automatic = automatic_model(
        suggestion,
        |file_name| paths.locate_model(file_name).is_some(),
        paths.shipped_file_name(),
    );
    (suggestion, automatic)
}

/// The saved choices with "Automatic" applied, as the next recording uses them.
///
/// Inputs: The saved choices and the paths. Outputs: The choices; a hand-picked model that is
/// no longer on the device counts as Automatic. Under OpenAI they are unchanged.
/// Side effects: See `automatic_on_this_device`, only when Automatic applies.
pub(crate) fn effective_settings(
    saved: &SavedTranscriptionSettings,
    paths: &TranscriptionPaths,
) -> SavedTranscriptionSettings {
    let mut saved = saved.clone();
    saved.whisper_model = saved
        .whisper_model
        .filter(|file_name| paths.locate_model(file_name).is_some());
    if saved.provider == Some(SavedProvider::Openai) || saved.whisper_model.is_some() {
        return saved;
    }
    with_automatic_model(&saved, automatic_on_this_device(paths).1)
}

/// Builds the whole Settings view. Side effects: Reads files and the key store.
fn settings_json() -> String {
    let paths = paths();
    let saved = load_settings(&paths.settings_file);
    let total_memory = total_memory_bytes();
    let (key_saved, keychain_available) = match key_store(&paths).load() {
        Ok(key) => (key.is_some(), true),
        Err(CryptoError::KeychainUnavailable { .. }) => (false, false),
        Err(_) => (false, true),
    };
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
    let suggestion = suggest_whisper_model(&device_profile(efficient_gpu_name.is_some()));
    let automatic = automatic_model(
        suggestion,
        |file_name| paths.locate_model(file_name).is_some(),
        paths.shipped_file_name(),
    );
    json!({
        "provider": match saved.provider {
            Some(SavedProvider::Openai) => "openai",
            _ => "local",
        },
        "openai_model": saved.openai_model.as_deref().unwrap_or("whisper-1"),
        "key_saved": key_saved,
        "environment_key": app_setting(OPENAI_API_KEY_VARIABLE).is_some(),
        "keychain_available": keychain_available,
        "selected_model": selected,
        "shipped_model": paths.shipped_file_name(),
        "available_disk_bytes": disk_probe.as_deref().and_then(available_disk_bytes),
        "using_local_fallback": cloud_fallback_active(),
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
            "enabled": selected.is_none(),
            "file_name": automatic.map(|choice| choice.file_name),
            "efficient": automatic.is_some_and(|choice| choice.efficient),
            "waiting_for_download": automatic.and_then(|choice| choice.waiting_for_download),
        },
        "after_meeting_pass": transcript_pass_enabled(&saved),
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
/// Outputs: The model in use and the dismissed files, or `None` when OpenAI transcribes, since this
/// device's speed then says nothing. Side effects: Reads the settings file.
pub(crate) fn proposal_settings() -> Option<(String, Vec<String>)> {
    let paths = paths();
    let saved = load_settings(&paths.settings_file);
    if saved.provider == Some(SavedProvider::Openai) {
        return None;
    }
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

/// Chooses where recordings are transcribed.
///
/// Inputs: `local` or `openai`; the app must have had the owner confirm that audio leaves the
/// device before passing `openai`.
/// Outputs: 0, 1 when unknown, or 4 when OpenAI has no key. Side effects: Stores the choice.
///
/// # Safety
/// `provider` must be null or a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_set_transcription_provider(provider: *const c_char) -> c_int {
    let provider = match unsafe { text_argument(provider) } {
        Some("local") => SavedProvider::Local,
        Some("openai") => SavedProvider::Openai,
        _ => return STATUS_INVALID,
    };
    if provider == SavedProvider::Openai
        && !matches!(key_store(&paths()).load(), Ok(Some(_)))
        && app_setting(OPENAI_API_KEY_VARIABLE).is_none()
    {
        return STATUS_NO_KEY;
    }
    update_settings(|saved| saved.provider = Some(provider))
}

/// Chooses the OpenAI model.
///
/// Inputs: A model name with no spaces. Outputs: 0 or 1. Side effects: Stores the choice.
///
/// # Safety
/// `model` must be null or a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_set_openai_model(model: *const c_char) -> c_int {
    let Some(model) = (unsafe { text_argument(model) }).and_then(validated_key) else {
        return STATUS_INVALID;
    };
    update_settings(|saved| saved.openai_model = Some(model.to_owned()))
}

/// Saves the OpenAI key without selecting OpenAI.
///
/// Inputs: The key. Outputs: 0, 1 when malformed, 3 when there is no keychain (nothing is
/// written anywhere else), or 2. Side effects: Writes the key store. The key is never logged.
///
/// # Safety
/// `key` must be null or a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_save_openai_key(key: *const c_char) -> c_int {
    let Some(key) = (unsafe { text_argument(key) }).and_then(validated_key) else {
        return STATUS_INVALID;
    };
    match key_store(&paths()).save(key) {
        Ok(()) => {
            PIPELINE_STALE.store(true, Ordering::Relaxed);
            STATUS_OK
        }
        Err(CryptoError::KeychainUnavailable { .. }) => STATUS_NO_KEYCHAIN,
        Err(_) => STATUS_FAILED,
    }
}

/// Removes the saved key and returns transcription to this device.
///
/// Outputs: 0, or 2 when the store refuses. Side effects: Writes the key store and the choices.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_remove_openai_key() -> c_int {
    if key_store(&paths()).remove().is_err() {
        return STATUS_FAILED;
    }
    update_settings(|saved| saved.provider = Some(SavedProvider::Local))
}

/// Tests the saved key without sending audio.
///
/// Outputs: `valid`, `invalid`, `quota_used`, `no_network`, `unexpected` or `no_key`; free it with
/// `note_taker_free_string`. Side effects: One HTTPS request to OpenAI. Blocks: call off the UI.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_test_openai_key() -> *mut c_char {
    let label = match key_store(&paths()).load() {
        Ok(Some(key)) => test_openai_key(&key).as_str(),
        _ => "no_key",
    };
    CString::new(label)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
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
