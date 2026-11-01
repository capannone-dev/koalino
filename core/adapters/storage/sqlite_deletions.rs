//! Deletes meetings and unknown voices, and keeps the records that stop them coming back.
//!
//! Automerge documents and cluster shares merge by union, so a paired device that still holds a
//! deleted item would otherwise restore it. `deleted_items` keeps one row per deleted meeting or
//! unknown voice, forever. A record holds only an ID and a time, and deletions always win over
//! anything a peer sends, as for saved voices (#56). The cost is that a deleted ID is never reused;
//! meetings and clusters get new UUIDs.
//!
//! This child module of `sqlite_adapter` removes database rows only. Attachment files and review
//! audio live in folders the composition roots own, so they remove those.

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use super::{SqliteNoteRepository, sqlite_error};
use crate::domain::{
    errors::StorageError,
    models::{DeletionRecords, MeetingId},
};

/// Marks a deleted meeting's row.
const MEETING_KIND: &str = "meeting";
/// Marks a deleted unknown voice's row.
const CLUSTER_KIND: &str = "unknown_voice";

/// Creates the deletion records' table and the revision floor.
///
/// Side effects: Creates `deleted_items` and `document_revision_floor` when absent.
pub(super) fn initialize_deletion_schema(connection: &Connection) -> Result<(), StorageError> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS deleted_items (
                item_kind TEXT NOT NULL,
                item_id TEXT NOT NULL,
                deleted_at TEXT NOT NULL,
                PRIMARY KEY (item_kind, item_id)
            );
            CREATE TABLE IF NOT EXISTS document_revision_floor (
                only_row INTEGER PRIMARY KEY CHECK (only_row = 1),
                revision INTEGER NOT NULL
            );",
        )
        .map_err(|error| sqlite_error("unable to initialize deletion records", error))
}

/// Records that an item was deleted, keeping the earliest time when it already was.
///
/// Side effects: May insert one row; callers provide the transaction.
fn record_deletion(
    connection: &Connection,
    item_kind: &str,
    item_id: &str,
) -> Result<(), StorageError> {
    connection
        .execute(
            "INSERT OR IGNORE INTO deleted_items (item_kind, item_id, deleted_at)
             VALUES (?1, ?2, ?3)",
            params![item_kind, item_id, Utc::now().to_rfc3339()],
        )
        .map(|_| ())
        .map_err(|error| sqlite_error("unable to record a deletion", error))
}

/// Tells whether an item has a deletion record.
fn is_recorded(
    connection: &Connection,
    item_kind: &str,
    item_id: &str,
) -> Result<bool, StorageError> {
    connection
        .query_row(
            "SELECT 1 FROM deleted_items WHERE item_kind = ?1 AND item_id = ?2",
            params![item_kind, item_id],
            |_| Ok(()),
        )
        .optional()
        .map(|found| found.is_some())
        .map_err(|error| sqlite_error("unable to read a deletion record", error))
}

/// Tells whether a meeting has a deletion record.
pub(super) fn is_meeting_recorded(
    connection: &Connection,
    meeting_id: &MeetingId,
) -> Result<bool, StorageError> {
    is_recorded(connection, MEETING_KIND, meeting_id.as_str())
}

/// Tells whether an unknown voice has a deletion record.
pub(super) fn is_cluster_recorded(
    connection: &Connection,
    cluster_id: Uuid,
) -> Result<bool, StorageError> {
    is_recorded(connection, CLUSTER_KIND, &cluster_id.to_string())
}

/// Removes a meeting's rows and leaves its deletion record.
///
/// Outputs: Whether the meeting existed; an unknown meeting is recorded only when
/// `record_when_absent` is set, so a peer's deletion also covers a meeting yet to arrive.
/// Side effects: Deletes the meeting, its transcript segments, attachment rows, clusters, member
/// embeddings and review audio. Keeps the deleted revision as a floor, so the next document write
/// still takes a higher one and watchers notice it. Callers provide the transaction.
fn remove_meeting(
    connection: &Connection,
    meeting_id: &MeetingId,
    record_when_absent: bool,
) -> Result<bool, StorageError> {
    let deleted_revision: Option<i64> = connection
        .query_row(
            "SELECT document_revision FROM meetings WHERE meeting_id = ?1",
            params![meeting_id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| sqlite_error("unable to read a meeting", error))?;
    let Some(deleted_revision) = deleted_revision else {
        if record_when_absent {
            record_deletion(connection, MEETING_KIND, meeting_id.as_str())?;
        }
        return Ok(false);
    };
    for statement in [
        "DELETE FROM cluster_chunks WHERE cluster_id IN
            (SELECT cluster_id FROM unknown_speaker_clusters WHERE meeting_id = ?1)",
        "DELETE FROM unassigned_voice_clips WHERE meeting_id = ?1",
        "DELETE FROM unknown_speaker_clusters WHERE meeting_id = ?1",
        "DELETE FROM transcript_segments WHERE meeting_id = ?1",
        "DELETE FROM attachments WHERE meeting_id = ?1",
        "DELETE FROM meetings WHERE meeting_id = ?1",
    ] {
        connection
            .execute(statement, params![meeting_id.as_str()])
            .map_err(|error| sqlite_error("unable to delete a meeting", error))?;
    }
    connection
        .execute(
            "INSERT INTO document_revision_floor (only_row, revision) VALUES (1, ?1)
             ON CONFLICT(only_row) DO UPDATE SET revision = MAX(revision, excluded.revision)",
            params![deleted_revision],
        )
        .map_err(|error| sqlite_error("unable to keep the document revision", error))?;
    record_deletion(connection, MEETING_KIND, meeting_id.as_str())?;
    Ok(true)
}

/// Removes an unknown voice's rows and leaves its deletion record.
///
/// Outputs: Whether the voice existed; an absent one is still recorded when `record_when_absent`
/// is set. Side effects: Deletes its member embeddings and review audio. Callers provide the
/// transaction.
fn remove_cluster(
    connection: &Connection,
    cluster_id: Uuid,
    record_when_absent: bool,
) -> Result<bool, StorageError> {
    let cluster_key = cluster_id.to_string();
    for statement in [
        "DELETE FROM unassigned_voice_clips WHERE cluster_id = ?1",
        "DELETE FROM cluster_chunks WHERE cluster_id = ?1",
    ] {
        connection
            .execute(statement, params![cluster_key])
            .map_err(|error| sqlite_error("unable to delete an unknown voice", error))?;
    }
    let removed_rows = connection
        .execute(
            "DELETE FROM unknown_speaker_clusters WHERE cluster_id = ?1",
            params![cluster_key],
        )
        .map_err(|error| sqlite_error("unable to delete an unknown voice", error))?;
    if removed_rows > 0 || record_when_absent {
        record_deletion(connection, CLUSTER_KIND, &cluster_key)?;
    }
    Ok(removed_rows > 0)
}

impl SqliteNoteRepository {
    /// Implements `NoteRepository::delete_meeting` in one transaction.
    pub(super) fn delete_meeting_rows(&self, meeting_id: &MeetingId) -> Result<bool, StorageError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| sqlite_error("unable to begin meeting deletion", error))?;
        let existed = remove_meeting(&transaction, meeting_id, false)?;
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit meeting deletion", error))?;
        Ok(existed)
    }

    /// Implements `NoteRepository::delete_unknown_voice` in one transaction.
    pub(super) fn delete_unknown_voice_rows(&self, cluster_id: Uuid) -> Result<bool, StorageError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| sqlite_error("unable to begin voice deletion", error))?;
        let existed = remove_cluster(&transaction, cluster_id, false)?;
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit voice deletion", error))?;
        Ok(existed)
    }

    /// Implements `NoteRepository::deletion_records`, sorted so every device lists them alike.
    pub(super) fn read_deletion_records(&self) -> Result<DeletionRecords, StorageError> {
        let read_ids = |item_kind: &str| -> Result<Vec<String>, StorageError> {
            let mut statement = self
                .connection
                .prepare_cached(
                    "SELECT item_id FROM deleted_items WHERE item_kind = ?1 ORDER BY item_id",
                )
                .map_err(|error| sqlite_error("unable to list deletion records", error))?;
            statement
                .query_map(params![item_kind], |row| row.get(0))
                .map_err(|error| sqlite_error("unable to list deletion records", error))?
                .collect::<Result<Vec<String>, _>>()
                .map_err(|error| sqlite_error("unable to read deletion records", error))
        };
        Ok(DeletionRecords {
            meeting_ids: read_ids(MEETING_KIND)?
                .into_iter()
                .map(MeetingId::new)
                .collect(),
            cluster_ids: read_ids(CLUSTER_KIND)?
                .iter()
                .map(|stored_id| {
                    Uuid::parse_str(stored_id).map_err(|_| StorageError::Rejected {
                        reason: String::from("a stored deletion record is not a UUID"),
                    })
                })
                .collect::<Result<Vec<Uuid>, StorageError>>()?,
        })
    }

    /// Implements `NoteRepository::apply_deletion_records` in one transaction.
    pub(super) fn apply_deletion_rows(
        &self,
        records: &DeletionRecords,
    ) -> Result<Vec<MeetingId>, StorageError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| sqlite_error("unable to begin applying deletions", error))?;
        let mut removed_meetings = Vec::new();
        for meeting_id in &records.meeting_ids {
            if remove_meeting(&transaction, meeting_id, true)? {
                removed_meetings.push(meeting_id.clone());
            }
        }
        for cluster_id in &records.cluster_ids {
            remove_cluster(&transaction, *cluster_id, true)?;
        }
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit applied deletions", error))?;
        Ok(removed_meetings)
    }
}
