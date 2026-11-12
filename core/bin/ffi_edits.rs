//! The owner's edits to a meeting over the C ABI (#96): notes, the summary's text and single
//! transcript segments, so the app edits them as the web does.
//!
//! `note_taker_meeting_edit` takes one JSON object tagged by `command` and returns caller-freed
//! JSON (`note_taker_free_string`): `{"ok":true,...}` or `{"ok":false,"error_code","error"}`.
//!
//! | `command` | Fields | Answer |
//! | --- | --- | --- |
//! | `notes` | `meeting_id` | `notes`: text or null |
//! | `set_notes` | `meeting_id`, `text` | `notes` |
//! | `edit_summary` | `meeting_id`, `text` | `summary`, marked `edited` |
//! | `edit_segment` | `meeting_id`, `start_time_ms`, `end_time_ms`, `text` | `segment` or null when blank text removed it |
//! | `keep_answer` | `meeting_id`, `text`, `source`, optional `prompt` | `summary`, `previous` (null when none) |
//! | `restore_summary` | `meeting_id`, `summary` (as `previous` gave it, or null) | nothing |
//!
//! `keep_answer` keeps an answer from the owner's own AI app as the summary (#114), and
//! `restore_summary` undoes it.
//!
//! Error codes: `invalid_request`, `meeting_not_found`, `segment_not_found`, `empty`,
//! `too_long`, `storage_failed` and `runtime_unavailable`. Note text is never logged.

use std::ffi::{CString, c_char};

use serde_json::{Value, json};

use crate::{
    application::{
        meeting_edits::{EditError, edit_segment, edit_summary, load_notes, set_notes},
        meeting_handoff::{restore_summary, store_external_summary},
    },
    domain::{llm::MeetingSummary, models::MeetingId},
    ffi::{ffi_string, with_runtime_repositories},
    llm_commands::summary_json,
    ports::storage::NoteRepository,
};

/// Runs one edit command against a repository.
///
/// Inputs: The repository and the request JSON. Outputs: The JSON answer. Side effects: As the
/// command says; edits write the meeting document, which syncs to paired devices.
pub fn run_edit_command<Repository>(repository: &Repository, request_json: &str) -> Value
where
    Repository: NoteRepository + ?Sized,
{
    let Ok(request) = serde_json::from_str::<Value>(request_json) else {
        return failure("invalid_request", "the request is not JSON");
    };
    let text = |field: &str| request.get(field).and_then(Value::as_str);
    let Some(meeting_id) = text("meeting_id").map(MeetingId::new) else {
        return failure("invalid_request", "`meeting_id` is required");
    };
    let missing_text = || failure("invalid_request", "`text` is required");
    let answer = match request.get("command").and_then(Value::as_str) {
        Some("notes") => load_notes(repository, &meeting_id).map(|notes| json!({"notes": notes})),
        Some("set_notes") => {
            let Some(notes) = text("text") else {
                return missing_text();
            };
            set_notes(repository, &meeting_id, notes).map(|()| json!({"notes": notes}))
        }
        Some("edit_summary") => {
            let Some(summary) = text("text") else {
                return missing_text();
            };
            edit_summary(repository, &meeting_id, summary, chrono::Utc::now())
                .map(|summary| json!({"summary": summary_json(&summary)}))
        }
        Some("keep_answer") => {
            let (Some(answer), Some(source)) = (text("text"), text("source")) else {
                return failure("invalid_request", "`text` and `source` are required");
            };
            store_external_summary(
                repository,
                &meeting_id,
                answer,
                source,
                text("prompt").unwrap_or_default(),
                chrono::Utc::now(),
            )
            .map(|kept| {
                json!({
                    "summary": summary_json(&kept.summary),
                    "previous": kept.previous.as_ref().map(summary_json),
                })
            })
        }
        Some("restore_summary") => {
            let previous = match request.get("summary") {
                None | Some(Value::Null) => None,
                Some(value) => match summary_from_json(value) {
                    Some(summary) => Some(summary),
                    None => return failure("invalid_request", "`summary` is malformed"),
                },
            };
            restore_summary(repository, &meeting_id, previous.as_ref()).map(|()| json!({}))
        }
        Some("edit_segment") => {
            let time = |field: &str| request.get(field).and_then(Value::as_u64);
            let (Some(start), Some(end), Some(segment_text)) =
                (time("start_time_ms"), time("end_time_ms"), text("text"))
            else {
                return failure(
                    "invalid_request",
                    "`start_time_ms`, `end_time_ms` and `text` are required",
                );
            };
            edit_segment(repository, &meeting_id, start, end, segment_text).map(|segment| {
                json!({"segment": segment.map(|segment| json!({
                    "start_time_ms": segment.start_time_ms(),
                    "end_time_ms": segment.end_time_ms(),
                    "text": segment.text(),
                }))})
            })
        }
        _ => return failure("invalid_request", "unknown command"),
    };
    match answer {
        Ok(Value::Object(mut fields)) => {
            fields.insert(String::from("ok"), Value::Bool(true));
            Value::Object(fields)
        }
        Ok(_) => failure("storage_failed", "unexpected answer"),
        Err(error) => edit_failure(&error),
    }
}

/// Reads a summary as `summary_json` writes it, or `None` when a field is missing or wrong.
fn summary_from_json(value: &Value) -> Option<MeetingSummary> {
    let text = |field: &str| value.get(field).and_then(Value::as_str).map(str::to_owned);
    let flag = |field: &str| value.get(field).and_then(Value::as_bool);
    Some(MeetingSummary {
        text: text("text")?,
        connection_label: text("connection_label")?,
        model: text("model")?,
        created_at: chrono::DateTime::parse_from_rfc3339(&text("created_at")?)
            .ok()?
            .with_timezone(&chrono::Utc),
        prompt: text("prompt")?,
        edited: flag("edited")?,
        truncated: flag("truncated")?,
    })
}

/// Maps an edit failure to its code.
fn edit_failure(error: &EditError) -> Value {
    match error {
        EditError::MeetingNotFound => failure("meeting_not_found", &error.to_string()),
        EditError::SegmentNotFound => failure("segment_not_found", &error.to_string()),
        EditError::Empty => failure("empty", &error.to_string()),
        EditError::TooLong { .. } => failure("too_long", &error.to_string()),
        EditError::Storage(_) => failure("storage_failed", "the meeting could not be saved"),
    }
}

/// Words a failure.
fn failure(error_code: &str, error: &str) -> Value {
    json!({"ok": false, "error_code": error_code, "error": error})
}

/// Runs one edit command for the app.
///
/// Inputs: A NUL-terminated UTF-8 JSON request (see the module header). Outputs: Caller-freed
/// JSON. Side effects: As the command says; the runtime lock is held for the edit only.
///
/// # Safety
/// `request_json` must be null or a valid NUL-terminated string that stays alive for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_meeting_edit(request_json: *const c_char) -> *mut c_char {
    let answer =
        match unsafe { ffi_string(request_json) } {
            None => failure("invalid_request", "the request must be a UTF-8 C string"),
            Some(request_json) => with_runtime_repositories(|repository, _| {
                run_edit_command(repository, request_json)
            })
            .unwrap_or_else(|| failure("runtime_unavailable", "the native runtime is unavailable")),
        };
    CString::new(answer.to_string())
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::run_edit_command;
    use crate::{
        adapters::storage::sqlite_adapter::SqliteNoteRepository,
        application::{
            crypto::voice_encryptor::ephemeral_cipher, meeting_crdt::MeetingCrdtService,
            meeting_document::AutomergeMeetingDocumentCodec,
        },
        domain::models::{MeetingId, TranscriptSegment},
        ports::storage::NoteRepository,
    };

    #[test]
    fn the_app_edits_notes_summaries_and_segments() {
        let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
            .and_then(|repository| {
                repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
            })
            .unwrap_or_else(|error| panic!("{error}"));
        MeetingCrdtService::new(&repository)
            .create_meeting(&MeetingId::new("m-1"))
            .unwrap_or_else(|error| panic!("{error}"));
        repository
            .save_transcript_segments(
                &MeetingId::new("m-1"),
                &[TranscriptSegment::new(
                    MeetingId::new("m-1"),
                    0,
                    900,
                    None,
                    "ciao",
                )],
            )
            .unwrap_or_else(|error| panic!("{error}"));
        let run = |request: serde_json::Value| run_edit_command(&repository, &request.to_string());

        let set = run(json!({"command": "set_notes", "meeting_id": "m-1", "text": "Agenda"}));
        assert_eq!(set["ok"], true, "{set}");
        let notes = run(json!({"command": "notes", "meeting_id": "m-1"}));
        assert_eq!(notes["notes"], "Agenda");
        let summary = run(json!({"command": "edit_summary", "meeting_id": "m-1", "text": "Done."}));
        assert_eq!(summary["summary"]["edited"], true, "{summary}");
        let segment = run(json!({"command": "edit_segment", "meeting_id": "m-1",
                                 "start_time_ms": 0, "end_time_ms": 900, "text": "Ciao a tutti."}));
        assert_eq!(segment["segment"]["text"], "Ciao a tutti.", "{segment}");
        let missing = run(json!({"command": "notes", "meeting_id": "nope"}));
        assert_eq!(missing["error_code"], "meeting_not_found");
        let unknown = run(json!({"command": "edit_segment", "meeting_id": "m-1",
                                 "start_time_ms": 1, "end_time_ms": 2, "text": "x"}));
        assert_eq!(unknown["error_code"], "segment_not_found");
    }
}
