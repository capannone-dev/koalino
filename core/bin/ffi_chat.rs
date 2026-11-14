//! Chat with notes (#97) over the C ABI.
//!
//! `note_taker_chat` takes one JSON command (`history`, `ask`, `clear`; see
//! `notes_chat_commands`) and returns caller-freed JSON (`note_taker_free_string`). `ask` blocks
//! for as long as the model takes, so the app calls it off the UI thread; the runtime lock is
//! held only while the material and the chat are read and the new messages stored. Questions
//! and answers are never logged.

use std::ffi::{CString, c_char};

use crate::{
    domain::chat::ChatScope,
    ffi::{ffi_string, runtime_p2p, with_runtime_chat_sources, with_runtime_repositories},
    ffi_llm::services,
    llm_commands::CommandError,
    notes_chat_commands::{
        ChatCommand, answer_for_paired_device, ask_paired_computer_with, ask_with, gather_for_ask,
        gather_material, offers_paired_chat, parse_chat_request, run_chat_store_command,
        store_messages,
    },
    paired_chat_host::ThreadedPairedChatHost,
    ports::chat::ChatHistoryRepository,
};

/// Asks a paired computer and keeps its answer in this device's chat (#107).
///
/// Side effects: Reads this device's chat, sends the question sealed to the paired computers
/// through the P2P task, and stores both messages; blocks for minutes at most.
fn ask_paired_computer(scope: ChatScope, question: &str) -> serde_json::Value {
    let p2p = match runtime_p2p() {
        Ok(Some(p2p)) => p2p,
        Ok(None) => {
            return serde_json::json!({"ok": false, "error_code": "p2p_unavailable",
                "error": "Paired devices are off on this device."});
        }
        Err(_) => return runtime_unavailable().to_json(),
    };
    ask_paired_computer_with(
        question,
        || {
            with_runtime_repositories(|repository, _| {
                repository
                    .chat_history(&scope)
                    .map_err(|error| CommandError::new("storage_failed", error.to_string()))
            })
            .unwrap_or_else(|| Err(runtime_unavailable()))
        },
        |question, turns| p2p.ask_paired_computer(scope.clone(), question, turns),
        |messages| {
            with_runtime_repositories(|repository, _| store_messages(repository, &scope, messages))
                .unwrap_or_else(|| Err(runtime_unavailable()))
        },
    )
}

/// Answers paired devices' chat questions from this app's notes when the owner chose a connection
/// for them (#107).
///
/// Side effects: Starts the answering thread and attaches it to the P2P task.
pub(crate) fn answer_paired_chats(p2p: &crate::p2p_composition::P2pHandle) {
    let host = ThreadedPairedChatHost::spawn(
        || offers_paired_chat(&services()),
        |_peer: &crate::domain::models::DeviceId, request| {
            answer_for_paired_device(&services(), &request, |scope, question| {
                with_runtime_chat_sources(|repository, voices, files| {
                    gather_material(repository, voices, Some(files), scope, question)
                })
                .unwrap_or_else(|| Err(runtime_unavailable()))
            })
        },
    );
    match host {
        Ok(host) => p2p.answer_paired_chats_with(Box::new(host)),
        Err(error) => tracing::warn!(error = %error, "paired devices' chats cannot be answered"),
    }
}

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
            on_paired_computer: true,
            ..
        })) => ask_paired_computer(scope, &question),
        Some(Ok(ChatCommand::Ask {
            scope,
            question,
            connection_id,
            on_paired_computer: false,
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
