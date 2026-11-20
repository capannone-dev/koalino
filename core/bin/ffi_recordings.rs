//! The app's kept meeting recordings over the C ABI (#140).
//!
//! `note_taker_recordings_command` takes one JSON object tagged by `command`, as
//! `kept_recordings::run_recordings_command` describes: `status`, `set_keep` (`keep`), `wipe`,
//! `has` (`meeting_id`; answers `available`, without decrypting), `prepare` (`meeting_id`;
//! answers the decrypted WAV's `path` and `duration_ms`) and `release`.
//! The runtime lock is held only to find the meeting's recordings; decrypting runs after it, so
//! Flutter should call this from a background isolate.

use std::ffi::{CString, c_char};

use serde_json::{Value, json};

use crate::{
    domain::models::MeetingId,
    ffi::{ffi_string, runtime_kept_recordings},
    kept_recordings::run_recordings_command,
};

/// Runs one recordings command and returns caller-freed JSON.
///
/// Inputs: `request_json` is a non-null NUL-terminated UTF-8 JSON object.
/// Outputs: An owned JSON string (free with `note_taker_free_string`): the command's answer, or
/// `{"ok":false,"error_code"}` with `invalid_argument` or `runtime_unavailable`. Null only if
/// the answer cannot be allocated.
/// Side effects: As the command: writes the settings file, decrypts a recording to the playback
/// file, or deletes recordings.
///
/// # Safety
/// `request_json` must be null or point to a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_recordings_command(request_json: *const c_char) -> *mut c_char {
    let answer = match unsafe { ffi_string(request_json) } {
        Some(request) => answer(request),
        None => {
            json!({"ok": false, "error_code": "invalid_argument", "error": "the request must be UTF-8"})
        }
    };
    CString::new(answer.to_string())
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Answers a request with the runtime's services; `prepare` plays the meeting's own recording,
/// or one folded into it (#88), shifted onto its timeline.
fn answer(request: &str) -> Value {
    let parsed: Value = serde_json::from_str(request).unwrap_or(Value::Null);
    let meeting_id = parsed
        .get("meeting_id")
        .and_then(Value::as_str)
        .map(MeetingId::new);
    let Some((services, sources)) = runtime_kept_recordings(meeting_id.as_ref()) else {
        return json!({"ok": false, "error_code": "runtime_unavailable", "error": "the native runtime is unavailable"});
    };
    if parsed.get("command").and_then(Value::as_str) == Some("has") && meeting_id.is_some() {
        return json!({"ok": true, "available": !sources.is_empty()});
    }
    if parsed.get("command").and_then(Value::as_str) == Some("prepare") && meeting_id.is_some() {
        return match services.prepare_playback(&sources) {
            Ok(Value::Object(mut fields)) => {
                fields.insert("ok".to_owned(), Value::Bool(true));
                Value::Object(fields)
            }
            Ok(_) => json!({"ok": true}),
            Err(error) => json!({
                "ok": false,
                "error_code": crate::kept_recordings::playback_code(&error),
                "error": error.to_string(),
            }),
        };
    }
    run_recordings_command(&services, None, request)
}
