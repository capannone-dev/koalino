//! Stores which meetings are another device's recording of a meeting recorded here too (#88).
//!
//! The meeting's CRDT document says whether it was folded into another meeting (`combined_into`)
//! and which meetings the owner split from it (`kept_apart_from`); these columns of `meetings` are
//! the index of that, written by the projection in the same transaction as the document:
//!
//! | Column | Value |
//! | --- | --- |
//! | `combined_into` | the meeting it was folded into, or `NULL` |
//! | `combined_offset_ms` | where it sits on that meeting's timeline |
//! | `combine_on` | the device that combines their audio |
//! | `kept_apart_from` | IDs of meetings split from it, one per line, or `NULL` |
//! | `recorded_on_phone` | 1 when a phone recorded it, 0 when not, `NULL` when not yet said |
//! | `combined_with` | IDs of recordings its transcript was built from, one per line, or `NULL` |

use crate::adapters::storage::sqlite_adapter::WriteTransaction;
use rusqlite::{Connection, OptionalExtension, params};

use super::{
    SqliteNoteRepository, apply_projection, missing_meeting_error, parse_sqlite_timestamp,
    read_document, sqlite_error, write_document,
};
use crate::{
    domain::{
        errors::StorageError,
        models::{DeviceId, MeetingId},
    },
    ports::{
        meeting_document::{MeetingCombination, MeetingProjection},
        storage::MeetingRecording,
    },
};

/// Adds the combination columns to databases created before #88.
///
/// Side effects: Alters the `meetings` table once.
pub(super) fn initialize_combination_schema(connection: &Connection) -> Result<(), StorageError> {
    for (column, kind) in [
        ("combined_into", "TEXT"),
        ("combined_offset_ms", "INTEGER"),
        ("combine_on", "TEXT"),
        ("kept_apart_from", "TEXT"),
        ("recorded_on_phone", "INTEGER"),
        ("combined_with", "TEXT"),
    ] {
        let present: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('meetings') WHERE name = ?1",
                params![column],
                |row| row.get(0),
            )
            .map_err(|error| sqlite_error("unable to inspect meetings schema", error))?;
        if present == 0 {
            connection
                .execute_batch(&format!("ALTER TABLE meetings ADD COLUMN {column} {kind}"))
                .map_err(|error| sqlite_error("unable to add a combination column", error))?;
        }
    }
    Ok(())
}

/// Writes the document's combination and split meetings into the index.
///
/// Side effects: Updates one row; callers provide the transaction.
pub(super) fn project_combination(
    connection: &Connection,
    meeting_id: &MeetingId,
    projection: &MeetingProjection,
) -> Result<(), StorageError> {
    let combination = projection.combined_into.as_ref();
    let kept_apart = joined_lines(&projection.kept_apart_from);
    let combined_with = joined_lines(&projection.combined_with);
    connection
        .execute(
            "UPDATE meetings SET combined_into = ?1, combined_offset_ms = ?2, combine_on = ?3,
                 kept_apart_from = ?4, recorded_on_phone = ?5, combined_with = ?6
             WHERE meeting_id = ?7",
            params![
                combination.map(|combination| combination.into.as_str()),
                combination.map(|combination| combination.offset_ms),
                combination.map(|combination| combination.combine_on.as_str()),
                kept_apart,
                projection.recorded_on_phone,
                combined_with,
                meeting_id.as_str(),
            ],
        )
        .map_err(|error| sqlite_error("unable to project the meeting's combination", error))?;
    Ok(())
}

/// Joins meeting IDs one per line, or `None` for none.
fn joined_lines(meetings: &[MeetingId]) -> Option<String> {
    (!meetings.is_empty()).then(|| {
        meetings
            .iter()
            .map(MeetingId::as_str)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

/// Splits meeting IDs stored one per line.
fn split_lines(stored: Option<&str>) -> Vec<MeetingId> {
    stored
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.is_empty())
        .map(MeetingId::new)
        .collect()
}

impl SqliteNoteRepository {
    /// Lists every meeting as a recording, oldest first (see `NoteRepository::meeting_recordings`).
    pub(super) fn read_meeting_recordings(&self) -> Result<Vec<MeetingRecording>, StorageError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT meeting_id, created_at_utc, acquisition_device_id, combined_into,
                        combined_offset_ms, combine_on, kept_apart_from, recorded_on_phone,
                        combined_with
                 FROM meetings ORDER BY created_at_utc, meeting_id",
            )
            .map_err(|error| sqlite_error("unable to list meeting recordings", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<bool>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                ))
            })
            .map_err(|error| sqlite_error("unable to list meeting recordings", error))?;
        let mut recordings = Vec::new();
        for row in rows {
            let (
                meeting_id,
                created_at,
                device_id,
                into,
                offset_ms,
                combine_on,
                kept_apart,
                recorded_on_phone,
                combined_with,
            ) = row.map_err(|error| sqlite_error("unable to read a meeting recording", error))?;
            let combined_into = match (into, offset_ms, combine_on) {
                (Some(into), Some(offset_ms), Some(combine_on)) => Some(MeetingCombination {
                    into: MeetingId::new(into),
                    offset_ms,
                    combine_on: DeviceId::new(combine_on),
                }),
                _ => None,
            };
            recordings.push(MeetingRecording {
                meeting_id: MeetingId::new(meeting_id),
                created_at: parse_sqlite_timestamp(&created_at)?,
                acquisition_device_id: DeviceId::new(device_id),
                combined_into,
                kept_apart_from: split_lines(kept_apart.as_deref()),
                recorded_on_phone,
                combined_with: split_lines(combined_with.as_deref()),
            });
        }
        Ok(recordings)
    }

    /// Writes a combination change into a meeting's document and index.
    ///
    /// Inputs: The meeting and the change to make to its document bytes.
    /// Outputs: Whether the document changed, or a storage error (a rejection for an unknown
    /// meeting, `Unavailable` without a document codec).
    /// Side effects: One transaction rewriting the document and its index row.
    pub(super) fn rewrite_combination(
        &self,
        meeting_id: &MeetingId,
        change: impl FnOnce(
            &dyn crate::ports::meeting_document::MeetingDocumentCodec,
            &[u8],
        ) -> Result<Vec<u8>, StorageError>,
    ) -> Result<bool, StorageError> {
        let Some(document_codec) = self.document_codec.as_deref() else {
            return Err(StorageError::Unavailable);
        };
        let transaction = self
            .connection
            .write_transaction()
            .map_err(|error| sqlite_error("unable to begin combining meetings", error))?;
        let document = read_document(&transaction, self.embedding_cipher.as_ref(), meeting_id)?;
        let changed = change(document_codec, &document)?;
        let differs = changed != document;
        if differs {
            write_document(
                &transaction,
                self.embedding_cipher.as_ref(),
                meeting_id,
                &changed,
            )?;
            apply_projection(
                &transaction,
                self.embedding_cipher.as_ref(),
                meeting_id,
                &document_codec.project(meeting_id, &changed)?,
            )?;
        }
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit combining meetings", error))?;
        Ok(differs)
    }

    /// Reads the combination a meeting's index row records.
    ///
    /// Outputs: It, `None` when the meeting stands alone, or a rejection for an unknown meeting.
    pub(super) fn read_combination(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Option<MeetingId>, StorageError> {
        self.connection
            .query_row(
                "SELECT combined_into FROM meetings WHERE meeting_id = ?1",
                params![meeting_id.as_str()],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(|error| sqlite_error("unable to read the meeting's combination", error))?
            .ok_or_else(|| missing_meeting_error(meeting_id))
            .map(|into| into.map(MeetingId::new))
    }
}
