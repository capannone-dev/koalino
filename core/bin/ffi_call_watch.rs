//! The call watch (#111) over the C ABI, for the desktop app.
//!
//! `note_taker_call_watch` takes one JSON request (`poll`, `get`, `set`; see
//! `call_watch_commands`) and returns caller-freed JSON (`note_taker_free_string`). The app polls
//! every few seconds; a poll runs `pactl` or `reg`, so the app calls it off the UI thread. It
//! needs no open runtime. Nothing here logs the apps seen.

use std::{
    ffi::{CString, c_char},
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

use serde_json::json;

use crate::{
    adapters::audio::microphone_users::SystemMicrophoneUsers,
    call_watch_commands::{CallWatchSettings, CallWatcher},
    ffi::ffi_string,
    ffi_settings::app_setting,
};

/// The app's watcher, built on first use.
static WATCHER: OnceLock<Mutex<CallWatcher>> = OnceLock::new();

/// Runs one call watch request.
///
/// Inputs: `request_json`, a NUL-terminated UTF-8 JSON object tagged by `command`.
/// Outputs: An owned JSON answer (free with `note_taker_free_string`); null only if allocation
/// fails. Side effects: `poll` runs the system's audio tool; `set` saves the settings beside the
/// app's database.
///
/// # Safety
/// `request_json` must be null or a valid NUL-terminated C string that stays alive for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_call_watch(request_json: *const c_char) -> *mut c_char {
    let answer = match unsafe { ffi_string(request_json) } {
        None => json!({"ok": false, "error_code": "invalid_request",
                       "error": "the request must be a UTF-8 C string"}),
        Some(request_json) => {
            let watcher = WATCHER.get_or_init(|| {
                let database = PathBuf::from(
                    app_setting("NOTE_TAKER_DATABASE_PATH")
                        .unwrap_or_else(|| String::from("note_taker.sqlite3")),
                );
                Mutex::new(CallWatcher::new(CallWatchSettings::file_for(&database)))
            });
            match watcher.lock() {
                Ok(mut watcher) => {
                    watcher.command(request_json, &mut || SystemMicrophoneUsers.current())
                }
                Err(_) => json!({"ok": false, "error_code": "runtime_unavailable",
                                 "error": "the call watch is unavailable"}),
            }
        }
    };
    CString::new(answer.to_string())
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}
