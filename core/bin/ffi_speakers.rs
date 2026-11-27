//! The meeting screen's speaker labels (#186): which voice a transcript segment belongs to, and
//! renaming a saved person from the transcript.
//!
//! Double-clicking or long-pressing a label names that voice. A saved person is renamed
//! everywhere (`rename_person`); an unknown voice is named through `note_taker_assign_cluster`,
//! as in Voices. Names are personal data: they travel only in these answers and are never logged.

use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    application::person_rename::{PersonRenameError, rename_person},
    domain::models::TranscriptSegment,
    ports::voice::VoiceVectorDB,
};

/// The voice of one transcript segment, for the meeting screen.
///
/// Inputs: The segment, and whether an identity is a person saved on this device.
/// Outputs: `{"kind":"person","id"}` for a saved person; else `{"kind":"unknown","id"}` with the
/// unknown voice's cluster; else `null` (no voice, so the label cannot be renamed).
/// Side effects: None.
pub fn segment_voice_json(segment: &TranscriptSegment, is_saved: impl Fn(Uuid) -> bool) -> Value {
    if let Some(person) = segment.speaker_identity_id().filter(|id| is_saved(*id)) {
        return json!({"kind": "person", "id": person.to_string()});
    }
    segment.unknown_cluster_id().map_or(
        Value::Null,
        |cluster| json!({"kind": "unknown", "id": cluster.to_string()}),
    )
}

/// Renames a saved person and answers in JSON.
///
/// Inputs: The voice store, the person's ID as a UUID string, and the typed name.
/// Outputs: `{"ok":true,"identity_id","identity_name"}`, or `{"ok":false,"error_code","error"}`
/// with `invalid_argument`, `invalid_name`, `person_not_found`, `name_taken` or `rename_failed`.
/// Side effects: See `rename_person`.
pub fn rename_person_json<VectorDatabase>(
    voice_store: &VectorDatabase,
    identity_id: &str,
    name: &str,
) -> String
where
    VectorDatabase: VoiceVectorDB + ?Sized,
{
    let Ok(identity_id) = Uuid::parse_str(identity_id) else {
        return error_json("invalid_argument", "identity_id must be a UUID");
    };
    match rename_person(voice_store, identity_id, name) {
        Ok(renamed) => json!({
            "ok": true,
            "identity_id": renamed.identity_id().to_string(),
            "identity_name": renamed.new_name(),
        })
        .to_string(),
        Err(error) => {
            let code = match &error {
                PersonRenameError::InvalidName => "invalid_name",
                PersonRenameError::PersonNotFound => "person_not_found",
                PersonRenameError::NameTaken => "name_taken",
                PersonRenameError::Storage(_) => "rename_failed",
            };
            error_json(code, &error.to_string())
        }
    }
}

/// An error answer.
fn error_json(code: &str, message: &str) -> String {
    json!({"ok": false, "error_code": code, "error": message}).to_string()
}

#[cfg(test)]
#[path = "ffi_speakers_tests.rs"]
mod tests;
