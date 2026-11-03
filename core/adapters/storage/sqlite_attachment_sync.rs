//! Lists and stores the attachments exchanged with paired devices (Issue #57).
//!
//! A linked attachment gets a content hash and size once, recorded in its row, so listing them for
//! a manifest never rereads files. Only rows linked to a meeting, or project resources (Issue #85),
//! are ever listed or looked up by hash. This child module of `sqlite_adapter` reads metadata only and never logs names.

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use super::{SqliteNoteRepository, sqlite_error};
use crate::domain::{
    errors::StorageError,
    models::{AttachmentOwner, MeetingId, SharedAttachment},
    projects::ProjectId,
};

/// The SQL condition matching rows that are shared: linked to a meeting or owned by a project.
const SHARED_ROW: &str = "(meeting_id IS NOT NULL OR project_id IS NOT NULL)";

/// Adds the nullable hash and size columns to older databases.
///
/// Outputs: Success (also when they exist) or a storage error.
/// Side effects: May run `ALTER TABLE`; existing rows are hashed later, one at a time.
pub(super) fn add_attachment_content_columns(connection: &Connection) -> Result<(), StorageError> {
    for (column, definition) in [
        ("content_hash", "content_hash TEXT"),
        ("size_bytes", "size_bytes INTEGER"),
    ] {
        let existing: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('attachments') WHERE name = ?1",
                params![column],
                |row| row.get(0),
            )
            .map_err(|error| sqlite_error("unable to inspect attachments schema", error))?;
        if existing == 0 {
            connection
                .execute(
                    &format!("ALTER TABLE attachments ADD COLUMN {definition}"),
                    [],
                )
                .map_err(|error| sqlite_error("unable to add an attachment column", error))?;
        }
    }
    Ok(())
}

impl SqliteNoteRepository {
    /// Lists linked attachments and project resources without a content hash.
    ///
    /// Outputs: (attachment ID, file path) pairs, or a storage error. Side effects: Reads rows.
    pub fn attachments_awaiting_hash(&self) -> Result<Vec<(String, String)>, StorageError> {
        let mut statement = self
            .connection
            .prepare(&format!(
                "SELECT attachment_id, file_path FROM attachments
                 WHERE {SHARED_ROW} AND content_hash IS NULL ORDER BY rowid"
            ))
            .map_err(|error| sqlite_error("unable to prepare the attachment hash query", error))?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|error| sqlite_error("unable to list unhashed attachments", error))?;
        rows.map(|row| row.map_err(|error| sqlite_error("unable to read an attachment", error)))
            .collect()
    }

    /// Records a file's content hash and size.
    ///
    /// Outputs: Success or a storage error. Side effects: Updates one row.
    pub fn record_attachment_hash(
        &self,
        attachment_id: &str,
        content_hash: &str,
        size_bytes: u64,
    ) -> Result<(), StorageError> {
        self.connection
            .execute(
                "UPDATE attachments SET content_hash = ?1, size_bytes = ?2 WHERE attachment_id = ?3",
                params![
                    content_hash,
                    i64::try_from(size_bytes).unwrap_or(i64::MAX),
                    attachment_id
                ],
            )
            .map_err(|error| sqlite_error("unable to record an attachment hash", error))?;
        Ok(())
    }

    /// Lists linked, hashed attachments and project resources with their local paths.
    ///
    /// Outputs: (metadata, local path) pairs ordered by ID; the metadata's file name is the
    /// path's last part without the leading `<attachment ID>-`. Or a storage error.
    /// Side effects: Reads rows.
    pub fn shared_attachment_rows(&self) -> Result<Vec<SharedAttachment>, StorageError> {
        let mut statement = self
            .connection
            .prepare(&format!(
                "SELECT attachment_id, meeting_id, file_path, mime_type, content_hash, size_bytes,
                        original_timestamp_utc, original_device_id, meeting_time_ms, project_id
                 FROM attachments
                 WHERE {SHARED_ROW} AND content_hash IS NOT NULL
                 ORDER BY attachment_id"
            ))
            .map_err(|error| {
                sqlite_error("unable to prepare the shared attachment query", error)
            })?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                ))
            })
            .map_err(|error| sqlite_error("unable to list shared attachments", error))?;
        rows.map(|row| {
            let (
                id,
                meeting_id,
                path,
                mime_type,
                content_hash,
                size,
                timestamp,
                device,
                time,
                project,
            ) = row.map_err(|error| sqlite_error("unable to read a shared attachment", error))?;
            // A project resource never has a meeting; a stored project ID is always valid text.
            let owner = match (project.as_deref().and_then(ProjectId::parse), meeting_id) {
                (Some(project_id), _) => AttachmentOwner::Project(project_id),
                (None, Some(meeting_id)) => AttachmentOwner::Meeting(MeetingId::new(meeting_id)),
                (None, None) => {
                    return Err(StorageError::Rejected {
                        reason: String::from("a shared attachment has no usable owner"),
                    });
                }
            };
            let attachment_id = Uuid::parse_str(&id).map_err(|error| StorageError::Rejected {
                reason: format!("stored attachment has an invalid UUID: {error}"),
            })?;
            let original_timestamp = DateTime::parse_from_rfc3339(&timestamp)
                .map(|parsed| parsed.with_timezone(&Utc))
                .map_err(|error| StorageError::Rejected {
                    reason: format!("stored attachment has an invalid time: {error}"),
                })?;
            let file_name = std::path::Path::new(&path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let file_name = file_name
                .strip_prefix(&format!("{id}-"))
                .map(str::to_owned)
                .unwrap_or(file_name);
            Ok(SharedAttachment {
                attachment_id,
                owner,
                file_name,
                mime_type,
                content_hash,
                size_bytes: u64::try_from(size).unwrap_or(0),
                original_timestamp,
                original_device_id: device,
                meeting_time_ms: time.and_then(|value| u64::try_from(value).ok()),
            })
        })
        .collect()
    }

    /// Tells whether an attachment row exists, linked or not.
    ///
    /// Outputs: Whether it exists, or a storage error. Side effects: Reads one row.
    pub fn attachment_exists(&self, attachment_id: Uuid) -> Result<bool, StorageError> {
        self.connection
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM attachments WHERE attachment_id = ?1)",
                params![attachment_id.to_string()],
                |row| row.get(0),
            )
            .map_err(|error| sqlite_error("unable to look up an attachment", error))
    }

    /// Tells whether a meeting exists here.
    ///
    /// Outputs: Whether it exists, or a storage error. Side effects: Reads one row.
    pub fn meeting_exists(&self, meeting_id: &MeetingId) -> Result<bool, StorageError> {
        self.connection
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM meetings WHERE meeting_id = ?1)",
                params![meeting_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|error| sqlite_error("unable to look up a meeting", error))
    }

    /// Tells whether a project exists here (and is not deleted).
    ///
    /// Outputs: Whether it exists, or a storage error. Side effects: Reads one row.
    pub fn project_exists(&self, project_id: ProjectId) -> Result<bool, StorageError> {
        self.connection
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM projects WHERE project_id = ?1)",
                params![project_id.to_string()],
                |row| row.get(0),
            )
            .map_err(|error| sqlite_error("unable to look up a project", error))
    }

    /// Tells whether the meeting or project an attachment belongs to exists here.
    ///
    /// Outputs: Whether it exists, or a storage error. Side effects: Reads one row.
    pub fn attachment_owner_exists(&self, owner: &AttachmentOwner) -> Result<bool, StorageError> {
        match owner {
            AttachmentOwner::Meeting(meeting_id) => self.meeting_exists(meeting_id),
            AttachmentOwner::Project(project_id) => self.project_exists(*project_id),
        }
    }

    /// Finds the file of a linked attachment or project resource by content hash.
    ///
    /// Outputs: Its local path, `None` when no linked attachment has the hash, or a storage error.
    /// Side effects: Reads one row.
    pub fn shared_attachment_path(
        &self,
        content_hash: &str,
    ) -> Result<Option<String>, StorageError> {
        self.connection
            .query_row(
                &format!(
                    "SELECT file_path FROM attachments
                     WHERE content_hash = ?1 AND {SHARED_ROW} ORDER BY rowid LIMIT 1"
                ),
                params![content_hash],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| sqlite_error("unable to look up a shared attachment", error))
    }

    /// Stores the row of a verified file from a paired device.
    ///
    /// Inputs: The metadata and the file's local path.
    /// Outputs: Success, or a rejection when the meeting or project is unknown, the attachment was
    /// deleted here, or the row cannot be written.
    /// Side effects: Inserts one row; an existing row is kept unchanged.
    pub fn insert_received_attachment(
        &self,
        attachment: &SharedAttachment,
        file_path: &str,
    ) -> Result<(), StorageError> {
        let (meeting_id, project_id) = match &attachment.owner {
            AttachmentOwner::Meeting(meeting_id) => (Some(meeting_id.as_str().to_owned()), None),
            AttachmentOwner::Project(project_id) => (None, Some(project_id.to_string())),
        };
        let inserted = self
            .connection
            .execute(
                "INSERT OR IGNORE INTO attachments (attachment_id, meeting_id, file_path, mime_type, original_timestamp_utc, original_device_id, meeting_time_ms, content_hash, size_bytes, project_id)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10
                 WHERE (EXISTS (SELECT 1 FROM meetings WHERE meeting_id = ?2)
                        OR EXISTS (SELECT 1 FROM projects WHERE project_id = ?10))
                   AND NOT EXISTS (SELECT 1 FROM deleted_items
                                   WHERE item_kind = 'attachment' AND item_id = ?1)",
                params![
                    attachment.attachment_id.to_string(),
                    meeting_id,
                    file_path,
                    attachment.mime_type,
                    attachment.original_timestamp.to_rfc3339(),
                    attachment.original_device_id,
                    attachment
                        .meeting_time_ms
                        .map(|time| i64::try_from(time).unwrap_or(i64::MAX)),
                    attachment.content_hash,
                    i64::try_from(attachment.size_bytes).unwrap_or(i64::MAX),
                    project_id,
                ],
            )
            .map_err(|error| sqlite_error("unable to store a received attachment", error))?;
        if inserted == 0 {
            return Err(StorageError::Rejected {
                reason: String::from("the received attachment was not stored"),
            });
        }
        Ok(())
    }
}
