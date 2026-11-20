//! AI connections and meeting summaries over the C ABI (#95).
//!
//! `note_taker_llm_command` runs the commands of `llm_commands.rs` (settings, connections, the
//! template, automatic summaries, connection tests), so the app and the web API behave alike.
//! `note_taker_summarize_meeting` writes a meeting's summary. It blocks for as long as the model
//! takes, so the app calls it off the UI thread; the runtime lock is held only while the meeting
//! is read and the summary stored. Every function returns caller-freed JSON
//! (`note_taker_free_string`): `{"ok":true,...}` or `{"ok":false,"error_code","error"}`. Keys
//! are write-only and never returned or logged.
//!
//! `note_taker_handoff_meeting` writes a meeting's hand-off to the owner's own AI app (#114):
//! the message and the files the share sheet carries.
//!
//! `note_taker_request_paired_summary` asks a paired computer to write a meeting's summary with
//! its own connection, such as Claude Code or Codex signed in there (#107), or cancels the
//! request. `note_taker_get_meeting_summary` reports the request's state.
//!
//! `note_taker_export_summary` saves a meeting's summary as a PDF, text, Word or Markdown file
//! where the owner chose (#130).
//!
//! `start_auto_summaries` runs the app's automatic summaries (#101) once the runtime opens, and
//! a stopped recording wakes them.

use std::{
    ffi::{CString, c_char},
    path::PathBuf,
    sync::{Arc, OnceLock},
};

use serde_json::{Value, json};

use crate::{
    adapters::{
        audio::retained_audio_store::RetainedSpeechIndex,
        export::{pdf_fonts::PdfFontConfig, summary_export::SummaryExportFormat},
    },
    application::{
        combined_meetings::HeldSpeech,
        meeting_handoff::{HandoffDelivery, HandoffPurpose},
        meeting_summary::SummaryError,
        meeting_summary_document::read_summary_prompt,
    },
    auto_summaries::{AutoSummarizer, AutoSummaryHost, AutoSummaryWake},
    domain::{
        llm::{MeetingSummary, SummaryRequest},
        models::{DeviceId, MeetingId},
    },
    ffi::{
        ffi_string, runtime_recording_meeting, with_runtime_repositories,
        with_runtime_summary_sources,
    },
    ffi_settings::app_setting,
    llm_commands::{
        LlmServices, handoff_with, new_summary_request, run_llm_command, summarize_with,
        summary_json, summary_request_json,
    },
    llm_settings::{PlatformLlmKeyring, forget_sealed_api_keys, llm_settings_file_for},
    meeting_summaries::{
        MeetingPromptError, PreparedSummary, load_summary, load_summary_request, prepare_summary,
        set_meeting_prompt, store_answered_request, store_summary, store_summary_request,
        write_handoff,
    },
    ports::storage::{DocumentChanges, MeetingRecording, NoteRepository},
    project_handoff::{NotesHandoffScope, write_notes_handoff},
    summary_export_commands::export_summary_to_file,
    transcription_settings::TranscriptionPaths,
};

/// The services, built from the app's settings on first use.
static SERVICES: OnceLock<Arc<LlmServices>> = OnceLock::new();

/// Builds or borrows the services: settings beside the database, the glossary and this device's
/// name. The first build deletes sealed API key files an earlier version kept.
pub(crate) fn services() -> Arc<LlmServices> {
    Arc::clone(SERVICES.get_or_init(|| {
        let paths = TranscriptionPaths::from_lookup(&app_setting);
        let database = database_path();
        let key_directory = paths.key_directory.clone();
        // Sealed API keys an earlier version kept (Android) are deleted (#117).
        forget_sealed_api_keys(&key_directory);
        let device_name =
            app_setting("NOTE_TAKER_DEVICE_NAME").unwrap_or_else(|| String::from("this device"));
        Arc::new(
            LlmServices::new(llm_settings_file_for(&database), &device_name)
                .with_keyring(Arc::new(PlatformLlmKeyring::new(
                    key_directory.clone(),
                    crate::ffi::provided_encryption_key(),
                )))
                .with_transcription_settings(paths.settings_file)
                .with_saved_device_name(&database),
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
            with_runtime_summary_sources(|repository, voices, p2p| {
                prepare_summary(repository, voices, &meeting_id, glossary, device_name, p2p)
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

/// Writes a meeting's hand-off to the owner's own AI app (#114).
///
/// Inputs: A JSON object: `meeting_id` (or, for a chat, #126: `project_id` for a project's
/// meetings and files, or `"all": true` for every note), `purpose` (`summary` or `chat`),
/// `directory`, a folder
/// the app owns for hand-offs (it is emptied first), and optional `inline` (true on computers,
/// #126: the message carries the meeting, to paste into a website). Outputs: Caller-freed
/// `{"ok":true,"message","prompt","files":[{"path","mime_type"}],"skipped","document_attached"}`
/// (`document_attached`: the message does not carry the Markdown) or an error (`invalid_request`, `nothing_to_summarize`, `storage_failed`, `runtime_unavailable`).
/// Side effects: Writes the meeting as Markdown and plain copies of its attachments into the
/// folder; the app shares them and empties the folder again. Nothing is sent anywhere.
///
/// # Safety
/// `request_json` must be null or a valid NUL-terminated string that stays alive for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_handoff_meeting(request_json: *const c_char) -> *mut c_char {
    let Some(request) = (unsafe { ffi_string(request_json) })
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
    else {
        return into_c(failure(
            "invalid_request",
            "the request must be a JSON object",
        ));
    };
    let text = |field: &str| request.get(field).and_then(Value::as_str);
    let purpose = match text("purpose") {
        Some("summary") => HandoffPurpose::Summary,
        Some("chat") => HandoffPurpose::Chat,
        _ => {
            return into_c(failure(
                "invalid_request",
                "`purpose` must be summary or chat",
            ));
        }
    };
    // A project, or every note, goes to the owner's AI app for a chat (#126 follow-up).
    let notes_scope = match (
        text("project_id"),
        request.get("all").and_then(Value::as_bool),
    ) {
        (Some(project_id), _) => {
            let Some(crate::domain::chat::ChatScope::Project(project_id)) =
                crate::domain::chat::ChatScope::parse("project", Some(project_id))
            else {
                return into_c(failure(
                    "invalid_request",
                    "`project_id` is not a project ID",
                ));
            };
            Some(NotesHandoffScope::Project(project_id))
        }
        (None, Some(true)) => Some(NotesHandoffScope::Everything),
        _ => None,
    };
    if let (Some(scope), Some(directory)) = (notes_scope, text("directory")) {
        let delivery = if request.get("inline").and_then(Value::as_bool) == Some(true) {
            HandoffDelivery::Pasted
        } else {
            HandoffDelivery::Attached
        };
        let directory = PathBuf::from(directory);
        let answer = handoff_with(&services(), |_, _, _| {
            with_runtime_summary_sources(|repository, voices, _| {
                write_notes_handoff(repository, voices, scope, delivery, &directory)
            })
            .unwrap_or_else(|| {
                Err(SummaryError::Storage(String::from(
                    "the native runtime is unavailable",
                )))
            })
        });
        return into_c(answer.to_string());
    }
    let (Some(meeting_id), Some(directory)) = (text("meeting_id"), text("directory")) else {
        return into_c(failure(
            "invalid_request",
            "`meeting_id` and `directory` are required",
        ));
    };
    // A computer's owner pastes the message into a website, so it carries the meeting (#126).
    let delivery = if request.get("inline").and_then(Value::as_bool) == Some(true) {
        HandoffDelivery::Pasted
    } else {
        HandoffDelivery::Attached
    };
    let meeting_id = MeetingId::new(meeting_id);
    let directory = PathBuf::from(directory);
    let answer = handoff_with(&services(), |glossary, device_name, template| {
        with_runtime_summary_sources(|repository, voices, p2p| {
            write_handoff(
                repository,
                voices,
                &meeting_id,
                glossary,
                device_name,
                p2p,
                template,
                purpose,
                delivery,
                &directory,
            )
        })
        .unwrap_or_else(|| {
            Err(SummaryError::Storage(String::from(
                "the native runtime is unavailable",
            )))
        })
    });
    into_c(answer.to_string())
}

/// Reads a meeting's summary and its own prompt.
///
/// Inputs: The meeting ID. Outputs: Caller-freed `{"ok":true,"summary": {...} or null,
/// "meeting_prompt": text or null, "request": a paired computer's summary request (#107) or null}`
/// or an error. Side effects: Reads the meeting document.
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
        let request = load_summary_request(repository, &meeting_id)
            .ok()
            .flatten()
            .map(|request| summary_request_json(&request));
        match load_summary(repository, &meeting_id) {
            Ok(summary) => json!({
                "ok": true,
                "summary": summary.as_ref().map(summary_json),
                "meeting_prompt": prompt,
                "request": request,
            }),
            Err(_) => json!({"ok": false, "error_code": "storage_failed",
                             "error": "the summary could not be read"}),
        }
    });
    into_c(answer.map_or_else(runtime_unavailable, |answer| answer.to_string()))
}

/// Saves a meeting's summary as a file (#130).
///
/// Inputs: The meeting ID; the format (`pdf`, `txt`, `docx` or `md`); the file to write, which
/// the owner chose (an existing file is replaced).
/// Outputs: Caller-freed `{"ok":true,"file_path","size_bytes"}`, or `{"ok":false,"error_code","error"}` with
/// `invalid_request`, `meeting_not_found`, `no_summary`, `storage_error`, `export_failed` or
/// `runtime_unavailable`.
/// Side effects: Reads the meeting; may read font files; writes the file. It runs synchronously,
/// so the app calls it off the UI thread.
///
/// # Safety
/// Each pointer must be null or a valid NUL-terminated string that stays alive for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_export_summary(
    meeting_id: *const c_char,
    format: *const c_char,
    output_path: *const c_char,
) -> *mut c_char {
    let arguments = (
        unsafe { ffi_string(meeting_id) },
        unsafe { ffi_string(format) },
        unsafe { ffi_string(output_path) },
    );
    let (Some(meeting_id), Some(format), Some(output_path)) = arguments else {
        return into_c(failure(
            "invalid_request",
            "the meeting ID, format and path must be UTF-8 C strings",
        ));
    };
    let format = match format.parse::<SummaryExportFormat>() {
        Ok(format) => format,
        Err(error) => return into_c(failure("invalid_request", &error.to_string())),
    };
    if output_path.trim().is_empty() {
        return into_c(failure("invalid_request", "the file path is empty"));
    }
    let meeting_id = MeetingId::new(meeting_id);
    let answer = with_runtime_repositories(|repository, _| -> Value {
        match export_summary_to_file(
            repository,
            &meeting_id,
            format,
            std::path::Path::new(output_path),
            &PdfFontConfig::from_environment(),
        ) {
            Ok(saved_path) => json!({
                "ok": true,
                "file_path": saved_path.display().to_string(),
                "size_bytes": std::fs::metadata(&saved_path).map_or(0, |metadata| metadata.len()),
            }),
            Err(error) => json!({"ok": false, "error_code": error.code(),
                                 "error": error.to_string()}),
        }
    });
    into_c(answer.map_or_else(runtime_unavailable, |answer| answer.to_string()))
}

/// Asks a paired computer to write a meeting's summary (#107), or cancels the request.
///
/// Inputs: JSON `{"meeting_id", "cancel"?: bool}`. Outputs: Caller-freed `{"ok":true,"request":
/// the new request or null}`, or `invalid_request` / `storage_failed` / `runtime_unavailable`.
/// Side effects: Writes the request (this device, the time, and the owner's own template if they
/// wrote one) into the meeting document, which syncs to paired devices; a computer whose owner
/// chose a connection for paired devices answers it. A new request replaces an earlier one, such
/// as a failed one. Nothing is sent to an AI server from this device.
///
/// # Safety
/// `request_json` must be null or a valid NUL-terminated string that stays alive for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_request_paired_summary(
    request_json: *const c_char,
) -> *mut c_char {
    let Some(request) = (unsafe { ffi_string(request_json) })
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
    else {
        return into_c(failure(
            "invalid_request",
            "the request must be a JSON object",
        ));
    };
    let Some(meeting_id) = request
        .get("meeting_id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(MeetingId::new)
    else {
        return into_c(failure("invalid_request", "`meeting_id` is required"));
    };
    let cancel = request
        .get("cancel")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let services = services();
    let answer = with_runtime_repositories(|repository, _| {
        let summary_request = (!cancel).then(|| {
            new_summary_request(
                &services,
                repository.device_id().as_str(),
                chrono::Utc::now(),
            )
        });
        match store_summary_request(repository, &meeting_id, summary_request.as_ref()) {
            Ok(()) => json!({
                "ok": true,
                "request": summary_request.as_ref().map(summary_request_json),
            })
            .to_string(),
            Err(error) => failure("storage_failed", &error.to_string()),
        }
    });
    into_c(answer.unwrap_or_else(runtime_unavailable))
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

/// Wakes the automatic summaries once they run.
static AUTO_SUMMARY_WAKE: OnceLock<Arc<AutoSummaryWake>> = OnceLock::new();

/// The database path the app configured.
fn database_path() -> PathBuf {
    PathBuf::from(
        app_setting("NOTE_TAKER_DATABASE_PATH")
            .unwrap_or_else(|| String::from("note_taker.sqlite3")),
    )
}

/// Reads and writes meetings for the app's automatic summaries (#101), taking the runtime lock
/// only for one read or write at a time.
struct FfiAutoSummaryHost {
    retained_speech: RetainedSpeechIndex,
}

/// Words an unavailable runtime for a summary step.
fn runtime_gone() -> SummaryError {
    SummaryError::Storage(String::from("the native runtime is unavailable"))
}

impl AutoSummaryHost for FfiAutoSummaryHost {
    fn recordings(&self) -> Result<Vec<MeetingRecording>, String> {
        with_runtime_repositories(|repository, _| repository.meeting_recordings())
            .ok_or_else(|| String::from("the native runtime is unavailable"))?
            .map_err(|error| error.to_string())
    }

    fn has_summary(&self, meeting_id: &MeetingId) -> Result<bool, String> {
        with_runtime_repositories(|repository, _| load_summary(repository, meeting_id))
            .ok_or_else(|| String::from("the native runtime is unavailable"))?
            .map(|summary| summary.is_some())
            .map_err(|error| error.to_string())
    }

    fn is_recording(&self, meeting_id: &MeetingId) -> bool {
        runtime_recording_meeting().as_ref() == Some(meeting_id)
    }

    fn held_speech(&self, meeting_id: &MeetingId) -> Option<HeldSpeech> {
        self.retained_speech
            .held_speech(meeting_id, std::time::SystemTime::now())
    }

    fn prepare(
        &self,
        meeting_id: &MeetingId,
        glossary: &[String],
        device_name: &str,
    ) -> Result<PreparedSummary, SummaryError> {
        with_runtime_summary_sources(|repository, voices, p2p| {
            prepare_summary(repository, voices, meeting_id, glossary, device_name, p2p)
        })
        .unwrap_or_else(|| Err(runtime_gone()))
    }

    fn store(&self, meeting_id: &MeetingId, summary: &MeetingSummary) -> Result<(), SummaryError> {
        with_runtime_repositories(|repository, _| store_summary(repository, meeting_id, summary))
            .unwrap_or_else(|| Err(runtime_gone()))
    }

    fn summary_request(&self, meeting_id: &MeetingId) -> Result<Option<SummaryRequest>, String> {
        with_runtime_repositories(|repository, _| load_summary_request(repository, meeting_id))
            .unwrap_or_else(|| Err(runtime_gone()))
            .map_err(|error| error.to_string())
    }

    fn set_summary_request(
        &self,
        meeting_id: &MeetingId,
        request: &SummaryRequest,
    ) -> Result<(), String> {
        with_runtime_repositories(|repository, _| {
            store_summary_request(repository, meeting_id, Some(request))
        })
        .unwrap_or_else(|| Err(runtime_gone()))
        .map_err(|error| error.to_string())
    }

    fn document_changes_since(&self, after_revision: u64) -> Result<DocumentChanges, String> {
        with_runtime_repositories(|repository, _| repository.document_changes_since(after_revision))
            .ok_or_else(|| String::from("the native runtime is unavailable"))?
            .map_err(|error| error.to_string())
    }

    fn answer_request(
        &self,
        meeting_id: &MeetingId,
        summary: &MeetingSummary,
    ) -> Result<(), SummaryError> {
        with_runtime_repositories(|repository, _| {
            store_answered_request(repository, meeting_id, summary)
        })
        .unwrap_or_else(|| Err(runtime_gone()))
    }
}

/// Starts the app's automatic summaries, once per process (#101).
///
/// Inputs: This device's ID, which marks the meetings recorded here. Side effects: Spawns a
/// thread that summarizes due meetings with the default connection while the owner keeps
/// automatic summaries on; its first round waits for the runtime lock.
pub(crate) fn start_auto_summaries(device_id: DeviceId) {
    if AUTO_SUMMARY_WAKE.get().is_some() {
        return;
    }
    let wake = Arc::clone(AUTO_SUMMARY_WAKE.get_or_init(Arc::default));
    let host = FfiAutoSummaryHost {
        retained_speech: RetainedSpeechIndex::new(
            crate::refinement_worker::retained_audio_directory(&database_path()),
        ),
    };
    if let Err(error) = AutoSummarizer::new(services(), device_id).spawn(Box::new(host), wake) {
        tracing::warn!(error = %error, "automatic summaries could not start");
    }
}

/// Asks the automatic summaries to look for due meetings now, if they run.
pub(crate) fn wake_auto_summaries() {
    if let Some(wake) = AUTO_SUMMARY_WAKE.get() {
        wake.wake();
    }
}

#[cfg(test)]
mod tests {
    //! Checks the C entry points refuse bad input before touching the runtime or the network.

    use std::ffi::{CStr, CString};

    use super::{
        note_taker_handoff_meeting, note_taker_llm_command, note_taker_request_paired_summary,
        note_taker_summarize_meeting,
    };

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
        let refused = answer(unsafe { note_taker_request_paired_summary(std::ptr::null()) });
        assert_eq!(refused["error_code"], "invalid_request");
        for request in [r#"{"cancel":true}"#, r#"{"meeting_id":""}"#, "[]"] {
            let request = CString::new(request).unwrap_or_default();
            let refused = answer(unsafe { note_taker_request_paired_summary(request.as_ptr()) });
            assert_eq!(refused["error_code"], "invalid_request", "{request:?}");
        }
    }

    #[test]
    fn a_hand_off_needs_a_purpose_a_meeting_and_a_folder() {
        let refused = answer(unsafe { note_taker_handoff_meeting(std::ptr::null()) });
        assert_eq!(refused["error_code"], "invalid_request");
        for request in [
            r#"{"meeting_id":"m","directory":"/tmp/x","purpose":"poem"}"#,
            r#"{"purpose":"chat","directory":"/tmp/x"}"#,
            r#"{"purpose":"summary","meeting_id":"m"}"#,
        ] {
            let request = CString::new(request).unwrap_or_default();
            let refused = answer(unsafe { note_taker_handoff_meeting(request.as_ptr()) });
            assert_eq!(refused["error_code"], "invalid_request", "{request:?}");
        }
    }
}
