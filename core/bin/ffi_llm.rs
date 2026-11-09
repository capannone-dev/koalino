//! AI connections and meeting summaries over the C ABI (#95).
//!
//! `note_taker_llm_command` runs the commands of `llm_commands.rs` (settings, connections, the
//! template, automatic summaries, connection tests), so the app and the web API behave alike.
//! `note_taker_summarize_meeting` writes a meeting's summary. It blocks for as long as the model
//! takes, so the app calls it off the UI thread; the runtime lock is held only while the meeting
//! is read and the summary stored. Every function returns caller-freed JSON
//! (`note_taker_free_string`): `{"ok":true,...}` or `{"ok":false,"error_code","error"}`. Keys
//! are write-only and never returned or logged.

use std::{
    ffi::{CString, c_char},
    path::PathBuf,
    sync::{Arc, OnceLock},
};

use serde_json::{Value, json};

use crate::{
    application::{meeting_summary::SummaryError, meeting_summary_document::read_summary_prompt},
    domain::models::MeetingId,
    ffi::{ffi_string, provided_encryption_key, with_runtime_repositories},
    ffi_settings::app_setting,
    llm_commands::{LlmServices, run_llm_command, summarize_with, summary_json},
    llm_settings::{PlatformLlmKeyring, llm_settings_file_for},
    meeting_summaries::{
        MeetingPromptError, load_summary, prepare_summary, set_meeting_prompt, store_summary,
    },
    ports::storage::NoteRepository,
    transcription_settings::TranscriptionPaths,
};

/// The services, built from the app's settings on first use.
static SERVICES: OnceLock<Arc<LlmServices>> = OnceLock::new();

/// Builds or borrows the services: settings beside the database, keys in the keychain (sealed
/// under the host key on Android), the glossary and this device's name.
fn services() -> Arc<LlmServices> {
    Arc::clone(SERVICES.get_or_init(|| {
        let paths = TranscriptionPaths::from_lookup(&app_setting);
        let database = PathBuf::from(
            app_setting("NOTE_TAKER_DATABASE_PATH")
                .unwrap_or_else(|| String::from("note_taker.sqlite3")),
        );
        let key_directory = paths
            .sealed_key_file
            .parent()
            .map_or_else(PathBuf::new, std::path::Path::to_path_buf);
        let device_name =
            app_setting("NOTE_TAKER_DEVICE_NAME").unwrap_or_else(|| String::from("this device"));
        Arc::new(
            LlmServices::new(
                llm_settings_file_for(&database),
                Arc::new(PlatformLlmKeyring::new(
                    key_directory,
                    provided_encryption_key(),
                )),
                &device_name,
            )
            .with_transcription_settings(paths.settings_file),
        )
    }))
}

/// Hands a JSON answer to C.
fn into_c(answer: String) -> *mut c_char {
    CString::new(answer)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Words a failure.
fn failure(error_code: &str, error: &str) -> String {
    json!({"ok": false, "error_code": error_code, "error": error}).to_string()
}

/// The answer when the runtime cannot be reached.
fn runtime_unavailable() -> String {
    failure("runtime_unavailable", "the native runtime is unavailable")
}

/// Runs one AI settings command.
///
/// Inputs: A NUL-terminated UTF-8 JSON object tagged by `command` (see `llm_commands.rs`).
/// Outputs: Caller-freed JSON. Side effects: As the command says; `test_connection` waits on the
/// network, so call it off the UI thread.
///
/// # Safety
/// `request_json` must be null or a valid NUL-terminated string that stays alive for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_llm_command(request_json: *const c_char) -> *mut c_char {
    into_c(match unsafe { ffi_string(request_json) } {
        None => failure("invalid_request", "the request must be a UTF-8 C string"),
        Some(request_json) => run_llm_command(&services(), request_json),
    })
}

/// Writes a meeting's summary.
///
/// Inputs: The meeting ID, and a connection ID or null for the default connection.
/// Outputs: Caller-freed `{"ok":true,"summary","parts","images_sent","images_skipped"}` or an
/// error. Side effects: Sends the meeting's transcript, notes and photos to the connection's
/// server and stores the summary in the meeting document, which syncs to paired devices. Blocks
/// for as long as the model takes.
///
/// # Safety
/// Both pointers must be null or valid NUL-terminated strings that stay alive for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_summarize_meeting(
    meeting_id: *const c_char,
    connection_id: *const c_char,
) -> *mut c_char {
    let Some(meeting_id) = (unsafe { ffi_string(meeting_id) }).map(MeetingId::new) else {
        return into_c(failure(
            "invalid_request",
            "the meeting ID must be a UTF-8 C string",
        ));
    };
    let connection_id = unsafe { ffi_string(connection_id) }.filter(|id| !id.is_empty());
    let exists = with_runtime_repositories(|repository, _| {
        repository
            .load_meeting_metadata(&meeting_id)
            .map(|metadata| metadata.is_some())
    });
    match exists {
        None => return into_c(runtime_unavailable()),
        Some(Ok(false)) => return into_c(failure("meeting_not_found", "there is no such meeting")),
        Some(Err(_)) => return into_c(failure("storage_failed", "the meeting could not be read")),
        Some(Ok(true)) => {}
    }
    let unavailable = || SummaryError::Storage(String::from("the native runtime is unavailable"));
    let answer = summarize_with(
        &services(),
        connection_id,
        |glossary, device_name| {
            with_runtime_repositories(|repository, voices| {
                prepare_summary(repository, voices, &meeting_id, glossary, device_name)
            })
            .unwrap_or_else(|| Err(unavailable()))
        },
        |summary| {
            with_runtime_repositories(|repository, _| {
                store_summary(repository, &meeting_id, summary)
            })
            .unwrap_or_else(|| Err(unavailable()))
        },
    );
    into_c(answer.to_string())
}

/// Reads a meeting's summary and its own prompt.
///
/// Inputs: The meeting ID. Outputs: Caller-freed `{"ok":true,"summary": {...} or null,
/// "meeting_prompt": text or null}` or an error. Side effects: Reads the meeting document.
///
/// # Safety
/// `meeting_id` must be null or a valid NUL-terminated string that stays alive for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_get_meeting_summary(meeting_id: *const c_char) -> *mut c_char {
    let Some(meeting_id) = (unsafe { ffi_string(meeting_id) }).map(MeetingId::new) else {
        return into_c(failure(
            "invalid_request",
            "the meeting ID must be a UTF-8 C string",
        ));
    };
    let answer = with_runtime_repositories(|repository, _| -> Value {
        let prompt = repository
            .load_crdt_document(&meeting_id)
            .ok()
            .and_then(|document| read_summary_prompt(&document).ok().flatten());
        match load_summary(repository, &meeting_id) {
            Ok(summary) => json!({
                "ok": true,
                "summary": summary.as_ref().map(summary_json),
                "meeting_prompt": prompt,
            }),
            Err(_) => json!({"ok": false, "error_code": "storage_failed",
                             "error": "the summary could not be read"}),
        }
    });
    into_c(answer.map_or_else(runtime_unavailable, |answer| answer.to_string()))
}

/// Sets or clears a meeting's own summary prompt.
///
/// Inputs: The meeting ID, and the template text or null for the default. Outputs: Caller-freed
/// `{"ok":true,"meeting_prompt"}` or `invalid_template` / `storage_failed`. Side effects: Writes
/// the meeting document, which syncs to paired devices.
///
/// # Safety
/// Both pointers must be null or valid NUL-terminated strings that stay alive for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_set_meeting_summary_prompt(
    meeting_id: *const c_char,
    prompt: *const c_char,
) -> *mut c_char {
    let Some(meeting_id) = (unsafe { ffi_string(meeting_id) }).map(MeetingId::new) else {
        return into_c(failure(
            "invalid_request",
            "the meeting ID must be a UTF-8 C string",
        ));
    };
    let prompt = unsafe { ffi_string(prompt) }.filter(|prompt| !prompt.trim().is_empty());
    let answer = with_runtime_repositories(|repository, _| {
        match set_meeting_prompt(repository, &meeting_id, prompt) {
            Ok(()) => json!({"ok": true, "meeting_prompt": prompt}).to_string(),
            Err(MeetingPromptError::Template(error)) => {
                failure("invalid_template", &error.to_string())
            }
            Err(error) => failure("storage_failed", &error.to_string()),
        }
    });
    into_c(answer.unwrap_or_else(runtime_unavailable))
}

#[cfg(test)]
mod tests {
    //! Checks the C entry points refuse bad input before touching the runtime or the network.

    use std::ffi::{CStr, CString};

    use super::{note_taker_llm_command, note_taker_summarize_meeting};

    fn answer(pointer: *mut std::ffi::c_char) -> serde_json::Value {
        assert!(!pointer.is_null());
        let text = unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned();
        drop(unsafe { CString::from_raw(pointer) });
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("{error}"))
    }

    #[test]
    fn null_or_malformed_requests_are_refused() {
        let refused = answer(unsafe { note_taker_llm_command(std::ptr::null()) });
        assert_eq!(refused["error_code"], "invalid_request");
        let not_json = CString::new("not json").unwrap_or_default();
        let refused = answer(unsafe { note_taker_llm_command(not_json.as_ptr()) });
        assert_eq!(refused["error_code"], "invalid_request");
        let refused =
            answer(unsafe { note_taker_summarize_meeting(std::ptr::null(), std::ptr::null()) });
        assert_eq!(refused["error_code"], "invalid_request");
    }
}
