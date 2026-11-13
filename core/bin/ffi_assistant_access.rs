//! Assistant access (#112) over the C ABI: the Settings switch that lets the owner's AI
//! assistant read the notes through the `note_taker_mcp` server, and how to connect it.
//!
//! `note_taker_assistant_access` takes `{"command":"get"}` or `{"command":"set","enabled":bool}`
//! and returns caller-freed JSON (`note_taker_free_string`) as `mcp_server::access_command`
//! describes. The server binary is looked for beside the app's executable; where it is missing
//! (Android, or a build without it), `server_path` is null and the app hides the setting.
//! Nothing here logs paths or notes.

use std::{
    ffi::{CString, c_char},
    path::PathBuf,
};

use serde_json::json;

use crate::{
    application::crypto::key_manager::PASSPHRASE_VARIABLE,
    ffi::ffi_string,
    ffi_settings::app_setting,
    mcp_server::{SERVER_FILE_NAME, access_command},
};

/// The server binary beside the running executable, when installed there.
///
/// Side effects: Reads the executable's path and checks for the file.
fn installed_server() -> Option<PathBuf> {
    let server = std::env::current_exe()
        .ok()?
        .parent()?
        .join(SERVER_FILE_NAME);
    server.is_file().then_some(server)
}

/// Runs one assistant access Settings command.
///
/// Inputs: `request_json`, a NUL-terminated UTF-8 JSON object tagged by `command`.
/// Outputs: An owned JSON answer (free with `note_taker_free_string`); null only if allocation
/// fails. Side effects: `set` saves `<db>.assistant-access.json` beside the app's database.
///
/// # Safety
/// `request_json` must be null or a valid NUL-terminated C string that stays alive for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_assistant_access(request_json: *const c_char) -> *mut c_char {
    let answer = match unsafe { ffi_string(request_json) } {
        None => json!({"ok": false, "error_code": "invalid_request",
                       "error": "the request must be a UTF-8 C string"}),
        Some(request_json) => {
            let database = PathBuf::from(
                app_setting("NOTE_TAKER_DATABASE_PATH")
                    .unwrap_or_else(|| String::from("note_taker.sqlite3")),
            );
            // A relative path would mean nothing to the assistant, started elsewhere.
            let database = std::path::absolute(&database).unwrap_or(database);
            let passphrase_needed = app_setting(PASSPHRASE_VARIABLE)
                .is_some_and(|passphrase| !passphrase.trim().is_empty());
            access_command(
                &database,
                request_json,
                installed_server().as_deref(),
                passphrase_needed,
            )
        }
    };
    CString::new(answer.to_string())
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}
