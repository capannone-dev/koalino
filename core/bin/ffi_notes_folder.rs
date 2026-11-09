//! The notes folder (#98) over the C ABI: its Settings commands and the app's rounds.
//!
//! `note_taker_notes_folder` takes one JSON command (`get`, `set`, `sync_now`; see
//! `notes_folder_commands`) and returns caller-freed JSON (`note_taker_free_string`).
//! `start_notes_folder` runs rounds on a thread once the runtime opens; while the owner keeps
//! the folder off, a round only reads the settings file. Rounds take the runtime lock, so they
//! wait for recordings' short calls like any other command. Nothing here logs names or text.

use std::{
    ffi::{CString, c_char},
    path::PathBuf,
    sync::{Arc, OnceLock},
};

use serde_json::json;

use crate::{
    auto_summaries::AutoSummaryWake,
    ffi::{ffi_string, run_notes_folder_round},
    ffi_settings::app_setting,
    notes_folder_commands::NotesFolderKeeper,
};

/// The keeper for the app's database, built on first use.
static KEEPER: OnceLock<Arc<NotesFolderKeeper>> = OnceLock::new();

/// Whether the rounds' thread runs.
static WAKE: OnceLock<Arc<AutoSummaryWake>> = OnceLock::new();

/// The owner's home folder, for the suggested location.
fn home_directory() -> Option<PathBuf> {
    ["HOME", "USERPROFILE"]
        .into_iter()
        .find_map(|name| std::env::var_os(name).filter(|value| !value.is_empty()))
        .map(PathBuf::from)
}

/// Builds or borrows the keeper beside the configured database.
fn keeper() -> Arc<NotesFolderKeeper> {
    Arc::clone(KEEPER.get_or_init(|| {
        let database = PathBuf::from(
            app_setting("NOTE_TAKER_DATABASE_PATH")
                .unwrap_or_else(|| String::from("note_taker.sqlite3")),
        );
        Arc::new(NotesFolderKeeper::for_database(&database, home_directory()))
    }))
}

/// Starts the app's notes folder rounds, once per process.
///
/// Side effects: Spawns a thread whose rounds take the runtime lock; the first waits for the
/// runtime the caller is opening.
pub(crate) fn start_notes_folder() {
    if WAKE.get().is_some() {
        return;
    }
    let wake = Arc::clone(WAKE.get_or_init(Arc::default));
    if let Err(error) = keeper().spawn(wake, Box::new(run_notes_folder_round)) {
        tracing::warn!(error = %error, "notes folder rounds could not start");
    }
}

/// Runs one notes folder Settings command.
///
/// Inputs: `request_json`, a NUL-terminated UTF-8 JSON object tagged by `command`.
/// Outputs: An owned JSON answer (free with `note_taker_free_string`); null only if allocation
/// fails. Side effects: `set` saves the settings beside the database; `set` turning the folder
/// on and `sync_now` run a round, which blocks until it is done, so the app calls this off the
/// UI thread.
///
/// # Safety
/// `request_json` must be null or a valid NUL-terminated C string that stays alive for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_notes_folder(request_json: *const c_char) -> *mut c_char {
    let answer = match unsafe { ffi_string(request_json) } {
        None => json!({"ok": false, "error_code": "invalid_request",
                       "error": "the request must be a UTF-8 C string"}),
        Some(request_json) => keeper().command(request_json, &mut run_notes_folder_round),
    };
    CString::new(answer.to_string())
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

#[cfg(test)]
mod tests {
    //! Checks the C entry point refuses bad input before touching the runtime.

    use std::ffi::{CStr, CString};

    use super::note_taker_notes_folder;

    #[test]
    fn null_or_malformed_requests_are_refused() {
        for request in [None, Some("not json"), Some(r#"{"command":"drop"}"#)] {
            let owned = request.map(|text| CString::new(text).unwrap_or_default());
            let pointer = unsafe {
                note_taker_notes_folder(
                    owned
                        .as_ref()
                        .map_or(std::ptr::null(), |text| text.as_ptr()),
                )
            };
            assert!(!pointer.is_null());
            let text = unsafe { CStr::from_ptr(pointer) }
                .to_string_lossy()
                .into_owned();
            drop(unsafe { CString::from_raw(pointer) });
            let answer: serde_json::Value =
                serde_json::from_str(&text).unwrap_or_else(|error| panic!("{error}"));
            assert_eq!(answer["error_code"], "invalid_request", "{request:?}");
        }
    }
}
