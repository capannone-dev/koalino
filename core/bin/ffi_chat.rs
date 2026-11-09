//! Chat with notes (#97) over the C ABI.
//!
//! `note_taker_chat` takes one JSON command (`history`, `ask`, `clear`; see
//! `notes_chat_commands`) and returns caller-freed JSON (`note_taker_free_string`). `ask` blocks
//! for as long as the model takes, so the app calls it off the UI thread; the runtime lock is
//! held only while the material and the chat are read and the new messages stored. Questions
//! and answers are never logged.

use std::ffi::{CString, c_char};

use crate::{
    ffi::{ffi_string, with_runtime_chat_sources, with_runtime_repositories},
    ffi_llm::services,
    llm_commands::CommandError,
    notes_chat_commands::{
        ChatCommand, ask_with, gather_for_ask, parse_chat_request, run_chat_store_command,
        store_messages,
    },
};

/// The answer when the runtime cannot be opened.
fn runtime_unavailable() -> CommandError {
    CommandError::new("runtime_unavailable", "the native runtime is unavailable")
}

/// Runs one chat command.
///
/// Inputs: `request_json`, a NUL-terminated UTF-8 JSON object tagged by `command`.
/// Outputs: An owned JSON answer (free with `note_taker_free_string`); null only if allocation
/// fails. Side effects: `ask` sends the question and material to the connection's server and
/// stores both messages; `clear` deletes the chat.
///
/// # Safety
/// `request_json` must be null or a valid NUL-terminated C string that stays alive for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_chat(request_json: *const c_char) -> *mut c_char {
    let answer = match unsafe { ffi_string(request_json) }.map(parse_chat_request) {
        None => {
            CommandError::new("invalid_request", "the request must be a UTF-8 C string").to_json()
        }
        Some(Err(error)) => error.to_json(),
        Some(Ok(ChatCommand::Ask {
            scope,
            question,
            connection_id,
        })) => ask_with(
            &services(),
            connection_id.as_deref(),
            &question,
            || {
                with_runtime_chat_sources(|repository, voices, files| {
                    gather_for_ask(repository, voices, Some(files), &scope, &question)
                })
                .unwrap_or_else(|| Err(runtime_unavailable()))
            },
            |messages| {
                with_runtime_repositories(|repository, _| {
                    store_messages(repository, &scope, messages)
                })
                .unwrap_or_else(|| Err(runtime_unavailable()))
            },
        ),
        Some(Ok(command)) => {
            with_runtime_repositories(|repository, _| run_chat_store_command(repository, &command))
                .unwrap_or_else(|| runtime_unavailable().to_json())
        }
    };
    CString::new(answer.to_string())
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

#[cfg(test)]
mod tests {
    //! Checks the C entry point refuses bad input before touching the runtime.

    use std::ffi::{CStr, CString};

    use super::note_taker_chat;

    #[test]
    fn null_or_malformed_requests_are_refused() {
        for request in [
            None,
            Some("not json"),
            Some(r#"{"command":"ask","scope":"meeting"}"#),
        ] {
            let owned = request.map(|text| CString::new(text).unwrap_or_default());
            let pointer = unsafe {
                note_taker_chat(
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
