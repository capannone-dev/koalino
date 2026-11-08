//! Seals meeting text at rest (#92): the `meetings.transcript` and `transcript_segments.segment_text`
//! columns and the `meetings.automerge_document` CRDT blobs.
//!
//! Each value is sealed through the `EmbeddingCipher` port (AES-256-GCM under the database key)
//! and bound to its column and meeting, so a sealed value moved onto another meeting's row does not
//! open. Sealed values start with `SEALED_EMBEDDING_HEADER`; text columns hold them as BLOBs.
//! Values from before #92 (TEXT, or a document without the header) still read, and every
//! opening seals the ones left in one transaction, so a database is sealed after its first
//! opening by this build. Titles stay in clear: the meeting list shows them, and the owner chose
//! to keep them readable. Text and document bytes are never logged.

use rusqlite::{Connection, params, types::Value};

use super::sqlite_adapter::{WriteTransaction, sqlite_error};
use crate::{
    application::crypto::voice_encryptor::SEALED_EMBEDDING_HEADER,
    domain::{errors::StorageError, models::MeetingId},
    ports::crypto::EmbeddingCipher,
};

/// A sealed meeting field; each names its own sealing context.
#[derive(Clone, Copy, Debug)]
pub(super) enum NoteField {
    /// `meetings.transcript`.
    Transcript,
    /// `transcript_segments.segment_text`.
    SegmentText,
    /// `meetings.automerge_document`.
    Document,
}

impl NoteField {
    /// The context that binds a value to this field of `meeting_id`.
    fn context(self, meeting_id: &str) -> String {
        let field = match self {
            Self::Transcript => "meetings:transcript",
            Self::SegmentText => "transcript_segments:segment_text",
            Self::Document => "meetings:automerge_document",
        };
        format!("note-field:v1:{field}:{meeting_id}")
    }
}

/// Seals one field value.
///
/// Inputs: The database cipher, the field, its meeting, and the clear bytes.
/// Outputs: The sealed bytes, or a storage error. Side effects: Draws a random nonce.
pub(super) fn seal_field(
    cipher: &dyn EmbeddingCipher,
    field: NoteField,
    meeting_id: &MeetingId,
    clear: &[u8],
) -> Result<Vec<u8>, StorageError> {
    cipher
        .seal_secret(&field.context(meeting_id.as_str()), clear)
        .map_err(|error| StorageError::Rejected {
            reason: format!("unable to encrypt meeting text: {error}"),
        })
}

/// Opens one sealed field value.
///
/// Inputs: The cipher, the field, its meeting, and the stored bytes.
/// Outputs: The clear bytes, or a rejection for a wrong key, a wrong meeting, or tampering.
/// Side effects: None.
fn open_field(
    cipher: &dyn EmbeddingCipher,
    field: NoteField,
    meeting_id: &MeetingId,
    sealed: &[u8],
) -> Result<Vec<u8>, StorageError> {
    cipher
        .open_secret(&field.context(meeting_id.as_str()), sealed)
        .map(|clear| clear.to_vec())
        .map_err(|error| StorageError::Rejected {
            reason: format!("unable to decrypt meeting text: {error}"),
        })
}

/// Seals a text value for a TEXT column, stored as a BLOB.
///
/// Side effects: Draws a random nonce.
pub(super) fn seal_text(
    cipher: &dyn EmbeddingCipher,
    field: NoteField,
    meeting_id: &MeetingId,
    text: &str,
) -> Result<Vec<u8>, StorageError> {
    seal_field(cipher, field, meeting_id, text.as_bytes())
}

/// Reads a text column's value.
///
/// Inputs: The cipher, the field, its meeting, and the stored SQLite value.
/// Outputs: The text: a BLOB is opened, a TEXT from before #92 is taken as it is. Anything
/// else, or a BLOB that does not open or is not UTF-8, is a rejection. Side effects: None.
pub(super) fn open_text(
    cipher: &dyn EmbeddingCipher,
    field: NoteField,
    meeting_id: &MeetingId,
    stored: Value,
) -> Result<String, StorageError> {
    match stored {
        Value::Text(clear) => Ok(clear),
        Value::Blob(sealed) => String::from_utf8(open_field(cipher, field, meeting_id, &sealed)?)
            .map_err(|_| StorageError::Rejected {
                reason: String::from("stored meeting text is not UTF-8"),
            }),
        _ => Err(StorageError::Rejected {
            reason: String::from("stored meeting text has an unexpected type"),
        }),
    }
}

/// Reads a stored document.
///
/// Outputs: The document: sealed bytes are opened (a failure is a rejection); bytes without the
/// sealed header are a document from before #92. Side effects: None.
pub(super) fn open_document(
    cipher: &dyn EmbeddingCipher,
    meeting_id: &MeetingId,
    stored: Vec<u8>,
) -> Result<Vec<u8>, StorageError> {
    if stored.starts_with(SEALED_EMBEDDING_HEADER) {
        open_field(cipher, NoteField::Document, meeting_id, &stored)
    } else {
        Ok(stored)
    }
}

/// Seals every transcript, segment text and document left in clear.
///
/// Inputs: An open connection whose `meetings` and `transcript_segments` tables exist, and the
/// cipher. Outputs: How many values were sealed, or a storage error; on error nothing changes.
/// Side effects: Rewrites those values in one transaction. `secure_delete` (on for every
/// connection) overwrites the clear pages they leave.
pub(super) fn seal_clear_notes(
    connection: &Connection,
    cipher: &dyn EmbeddingCipher,
) -> Result<usize, StorageError> {
    let transaction = connection
        .write_transaction()
        .map_err(|error| sqlite_error("unable to begin sealing meeting text", error))?;
    let header_length = i64::try_from(SEALED_EMBEDDING_HEADER.len()).unwrap_or(i64::MAX);
    let clear_meetings = query_rows(
        &transaction,
        "SELECT rowid, meeting_id, transcript, automerge_document FROM meetings
         WHERE typeof(transcript) = 'text'
            OR substr(automerge_document, 1, ?1) IS NOT ?2",
        params![header_length, &SEALED_EMBEDDING_HEADER[..]],
    )?;
    let mut sealed_count = 0;
    for (row_id, meeting_id, transcript, document) in clear_meetings {
        let meeting_id = MeetingId::new(meeting_id);
        let transcript = match transcript {
            Value::Text(text) => {
                sealed_count += 1;
                Value::Blob(seal_text(
                    cipher,
                    NoteField::Transcript,
                    &meeting_id,
                    &text,
                )?)
            }
            other => other,
        };
        let document = match document {
            Value::Blob(bytes) if !bytes.starts_with(SEALED_EMBEDDING_HEADER) => {
                sealed_count += 1;
                Value::Blob(seal_field(
                    cipher,
                    NoteField::Document,
                    &meeting_id,
                    &bytes,
                )?)
            }
            other => other,
        };
        transaction
            .execute(
                "UPDATE meetings SET transcript = ?1, automerge_document = ?2 WHERE rowid = ?3",
                params![transcript, document, row_id],
            )
            .map_err(|error| sqlite_error("unable to seal a meeting's text", error))?;
    }
    let clear_segments = query_rows(
        &transaction,
        "SELECT rowid, meeting_id, segment_text, NULL FROM transcript_segments
         WHERE typeof(segment_text) = 'text'",
        [],
    )?;
    for (row_id, meeting_id, text, _) in clear_segments {
        let Value::Text(text) = text else { continue };
        let sealed = seal_text(
            cipher,
            NoteField::SegmentText,
            &MeetingId::new(meeting_id),
            &text,
        )?;
        transaction
            .execute(
                "UPDATE transcript_segments SET segment_text = ?1 WHERE rowid = ?2",
                params![sealed, row_id],
            )
            .map_err(|error| sqlite_error("unable to seal a transcript segment", error))?;
        sealed_count += 1;
    }
    transaction
        .commit()
        .map_err(|error| sqlite_error("unable to commit sealing meeting text", error))?;
    if sealed_count > 0 {
        tracing::info!(sealed_count, "sealed meeting text stored in clear");
    }
    Ok(sealed_count)
}

/// Runs a query whose rows are `(rowid, meeting_id, value, value)`.
///
/// Side effects: Reads rows.
fn query_rows(
    connection: &Connection,
    sql: &str,
    parameters: impl rusqlite::Params,
) -> Result<Vec<(i64, String, Value, Value)>, StorageError> {
    let mut statement = connection
        .prepare(sql)
        .map_err(|error| sqlite_error("unable to prepare sealing meeting text", error))?;
    statement
        .query_map(parameters, |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .and_then(Iterator::collect::<Result<Vec<_>, _>>)
        .map_err(|error| sqlite_error("unable to read meeting text to seal", error))
}
