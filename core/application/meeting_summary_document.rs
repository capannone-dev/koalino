//! Keeps a meeting's summary and its own summary prompt in the meeting's Automerge document
//! (#95), so both sync to paired devices and are sealed at rest with the rest of the document.
//!
//! | Root key | Value |
//! | --- | --- |
//! | `summary` | map `{ text: str, connection_label: str, model: str, created_at_ms: int, prompt: str, edited: bool, truncated: bool }` |
//! | `summary_prompt` | string, the template this meeting uses instead of the owner's default; absent for the default |
//!
//! A new summary replaces the whole map, so two devices summarizing at once converge on one
//! complete summary (Automerge's winner) rather than a mix of both.

use automerge::{
    AutoCommit, ObjId, ObjType, ROOT, ReadDoc, ScalarValue, Value, transaction::Transactable,
};
use chrono::{TimeZone, Utc};

use crate::domain::{errors::StorageError, llm::MeetingSummary};

/// Root key of the summary map.
const SUMMARY_KEY: &str = "summary";
/// Root key of the meeting's own summary prompt.
const SUMMARY_PROMPT_KEY: &str = "summary_prompt";

/// Stores a summary, replacing any earlier one.
///
/// Inputs: The serialized document and the summary. Outputs: The updated document, or a storage
/// error for a corrupt document. Side effects: None.
pub fn with_summary(document: &[u8], summary: &MeetingSummary) -> Result<Vec<u8>, StorageError> {
    let mut automerge_document = load(document)?;
    let object = automerge_document
        .put_object(ROOT, SUMMARY_KEY, ObjType::Map)
        .map_err(|error| crdt_error("unable to create the summary", error))?;
    let fields: [(&str, ScalarValue); 7] = [
        ("text", ScalarValue::Str(summary.text.as_str().into())),
        (
            "connection_label",
            ScalarValue::Str(summary.connection_label.as_str().into()),
        ),
        ("model", ScalarValue::Str(summary.model.as_str().into())),
        (
            "created_at_ms",
            ScalarValue::Int(summary.created_at.timestamp_millis()),
        ),
        ("prompt", ScalarValue::Str(summary.prompt.as_str().into())),
        ("edited", ScalarValue::Boolean(summary.edited)),
        ("truncated", ScalarValue::Boolean(summary.truncated)),
    ];
    for (field, value) in fields {
        automerge_document
            .put(&object, field, value)
            .map_err(|error| crdt_error("unable to write the summary", error))?;
    }
    Ok(automerge_document.save())
}

/// Removes the summary, as when the owner undoes the first one an app gave (#114).
///
/// Inputs: The serialized document. Outputs: The updated document (unchanged without a
/// summary), or a storage error. Side effects: None.
pub fn without_summary(document: &[u8]) -> Result<Vec<u8>, StorageError> {
    let mut automerge_document = load(document)?;
    if automerge_document
        .get(ROOT, SUMMARY_KEY)
        .map_err(|error| crdt_error("unable to read the summary", error))?
        .is_some()
    {
        automerge_document
            .delete(ROOT, SUMMARY_KEY)
            .map_err(|error| crdt_error("unable to remove the summary", error))?;
    }
    Ok(automerge_document.save())
}

/// Reads the summary.
///
/// Inputs: The serialized document. Outputs: The summary; `None` when there is none or it is
/// malformed; a storage error for a corrupt document. Side effects: None.
pub fn read_summary(document: &[u8]) -> Result<Option<MeetingSummary>, StorageError> {
    let automerge_document = load(document)?;
    let Ok(Some((Value::Object(ObjType::Map), object))) = automerge_document.get(ROOT, SUMMARY_KEY)
    else {
        return Ok(None);
    };
    let text = |field: &str| read_string(&automerge_document, &object, field);
    let flag = |field: &str| {
        matches!(
            read_scalar(&automerge_document, &object, field),
            Some(ScalarValue::Boolean(true))
        )
    };
    let created_at = match read_scalar(&automerge_document, &object, "created_at_ms") {
        Some(ScalarValue::Int(milliseconds)) => Utc.timestamp_millis_opt(milliseconds).single(),
        _ => None,
    };
    Ok((|| {
        Some(MeetingSummary {
            text: text("text")?,
            connection_label: text("connection_label").unwrap_or_default(),
            model: text("model").unwrap_or_default(),
            created_at: created_at?,
            prompt: text("prompt").unwrap_or_default(),
            edited: flag("edited"),
            truncated: flag("truncated"),
        })
    })())
}

/// Sets the meeting's own summary prompt, or clears it with `None`.
///
/// Inputs: The serialized document and the checked template text. Outputs: The updated
/// document, or a storage error. Side effects: None.
pub fn with_summary_prompt(document: &[u8], prompt: Option<&str>) -> Result<Vec<u8>, StorageError> {
    let mut automerge_document = load(document)?;
    match prompt {
        Some(prompt) => automerge_document
            .put(ROOT, SUMMARY_PROMPT_KEY, prompt)
            .map_err(|error| crdt_error("unable to write the summary prompt", error))?,
        None => {
            if automerge_document
                .get(ROOT, SUMMARY_PROMPT_KEY)
                .map_err(|error| crdt_error("unable to read the summary prompt", error))?
                .is_some()
            {
                automerge_document
                    .delete(ROOT, SUMMARY_PROMPT_KEY)
                    .map_err(|error| crdt_error("unable to clear the summary prompt", error))?;
            }
        }
    }
    Ok(automerge_document.save())
}

/// Reads the meeting's own summary prompt.
///
/// Outputs: The text, `None` when the meeting uses the default, or a storage error.
/// Side effects: None.
pub fn read_summary_prompt(document: &[u8]) -> Result<Option<String>, StorageError> {
    let automerge_document = load(document)?;
    Ok(read_string(&automerge_document, &ROOT, SUMMARY_PROMPT_KEY))
}

/// Reads a scalar field, or `None` when absent or not a scalar.
fn read_scalar(
    automerge_document: &AutoCommit,
    object: &ObjId,
    field: &str,
) -> Option<ScalarValue> {
    match automerge_document.get(object, field) {
        Ok(Some((Value::Scalar(value), _))) => Some(value.into_owned()),
        _ => None,
    }
}

/// Reads a string field, or `None` when absent or not a string.
fn read_string(automerge_document: &AutoCommit, object: &ObjId, field: &str) -> Option<String> {
    match read_scalar(automerge_document, object, field)? {
        ScalarValue::Str(text) => Some(text.to_string()),
        _ => None,
    }
}

/// Deserializes a stored document.
fn load(document: &[u8]) -> Result<AutoCommit, StorageError> {
    AutoCommit::load(document).map_err(|error| crdt_error("meeting document is corrupt", error))
}

/// Converts an Automerge failure into a portable storage error.
fn crdt_error(operation: &str, error: automerge::AutomergeError) -> StorageError {
    StorageError::Rejected {
        reason: format!("{operation}: {error}"),
    }
}
