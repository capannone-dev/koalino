//! Stores projects, indexes their meetings and resources, and merges paired devices' project
//! lists (Issue #85).
//!
//! Tables: `projects` holds the live projects, and `project_tombstones` one row per deleted project,
//! kept forever so a paired device that missed the deletion cannot bring it back. The meeting's
//! CRDT document names its project; `meetings.project_id` is the index of that, written by the
//! projection, and `attachments.project_id` marks a project's resources. A meeting or resource
//! naming a project this device does not have (deleted, or not synced yet) is simply not shown in
//! any project. Times are stored as RFC 3339 text with milliseconds. This child module of
//! `sqlite_adapter` never logs names.

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use super::{
    AttachmentOwnerFilter, SqliteNoteRepository, apply_projection, parse_sqlite_timestamp,
    read_document, sqlite_error, write_document,
};
use crate::{
    domain::{
        errors::StorageError,
        models::{Attachment, MeetingId},
        projects::{Project, ProjectId, ProjectLibrary, ProjectName, ProjectTombstone},
    },
    ports::projects::{
        ProjectLibraryImport, ProjectLibrarySync, ProjectRepository, ProjectSummary,
    },
};

/// Creates the project tables and the project columns of meetings and attachments.
///
/// Outputs: Success (also when they exist) or a storage error.
/// Side effects: May create tables and an index, and run `ALTER TABLE`.
pub(super) fn initialize_project_schema(connection: &Connection) -> Result<(), StorageError> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
                project_id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                created_at_utc TEXT NOT NULL,
                updated_at_utc TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS project_tombstones (
                project_id TEXT PRIMARY KEY NOT NULL,
                deleted_at_utc TEXT NOT NULL
            );",
        )
        .map_err(|error| sqlite_error("unable to create the project tables", error))?;
    for table in ["meetings", "attachments"] {
        let existing: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = 'project_id'",
                params![table],
                |row| row.get(0),
            )
            .map_err(|error| sqlite_error("unable to inspect a table", error))?;
        if existing == 0 {
            connection
                .execute_batch(&format!("ALTER TABLE {table} ADD COLUMN project_id TEXT"))
                .map_err(|error| sqlite_error("unable to add a project column", error))?;
        }
    }
    connection
        .execute_batch(
            "CREATE INDEX IF NOT EXISTS meetings_project_index ON meetings(project_id);
             CREATE INDEX IF NOT EXISTS attachments_project_index ON attachments(project_id);",
        )
        .map_err(|error| sqlite_error("unable to index projects", error))
}

/// Drops precision below a millisecond, which stored times do not keep, so a returned project
/// equals the stored one.
fn whole_milliseconds(instant: DateTime<Utc>) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(instant.timestamp_millis()).unwrap_or(instant)
}

/// Writes an instant as RFC 3339 UTC text with milliseconds.
fn time_text(instant: DateTime<Utc>) -> String {
    instant.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Reads an instant written by `time_text`.
fn parse_time(text: &str) -> Result<DateTime<Utc>, StorageError> {
    DateTime::parse_from_rfc3339(text)
        .map(|instant| instant.with_timezone(&Utc))
        .map_err(|_| StorageError::Rejected {
            reason: String::from("a stored project time is invalid"),
        })
}

/// Builds a project from its stored columns.
///
/// Outputs: The project, or a rejection for a corrupt row. Side effects: None.
fn project_from_columns(
    project_text: &str,
    name: &str,
    created_text: &str,
    updated_text: &str,
) -> Result<Project, StorageError> {
    let project_id = ProjectId::parse(project_text).ok_or_else(|| StorageError::Rejected {
        reason: String::from("a stored project ID is invalid"),
    })?;
    let name = ProjectName::parse(name).map_err(|_| StorageError::Rejected {
        reason: String::from("a stored project name is invalid"),
    })?;
    Ok(Project::new(
        project_id,
        name,
        parse_time(created_text)?,
        parse_time(updated_text)?,
    ))
}

/// Reads one live project inside `connection`.
fn read_project(
    connection: &Connection,
    project_id: ProjectId,
) -> Result<Option<Project>, StorageError> {
    let row = connection
        .query_row(
            "SELECT name, created_at_utc, updated_at_utc FROM projects WHERE project_id = ?1",
            params![project_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|error| sqlite_error("unable to load a project", error))?;
    row.map(|(name, created, updated)| {
        project_from_columns(&project_id.to_string(), &name, &created, &updated)
    })
    .transpose()
}

/// Tells whether `project_id` was deleted.
fn is_tombstoned(connection: &Connection, project_id: ProjectId) -> Result<bool, StorageError> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM project_tombstones WHERE project_id = ?1)",
            params![project_id.to_string()],
            |row| row.get(0),
        )
        .map_err(|error| sqlite_error("unable to check a project deletion", error))
}

/// Deletes a project's row and resource rows, and records the deletion.
///
/// Outputs: Whether the project or any of its resources existed here.
/// Side effects: Writes rows; callers provide the transaction.
fn remove_project(
    connection: &Connection,
    project_id: ProjectId,
    deleted_at: DateTime<Utc>,
) -> Result<bool, StorageError> {
    let project_text = project_id.to_string();
    let removed_projects = connection
        .execute(
            "DELETE FROM projects WHERE project_id = ?1",
            params![project_text],
        )
        .map_err(|error| sqlite_error("unable to delete a project", error))?;
    let removed_resources = connection
        .execute(
            "DELETE FROM attachments WHERE project_id = ?1",
            params![project_text],
        )
        .map_err(|error| sqlite_error("unable to delete a project's resources", error))?;
    connection
        .execute(
            "INSERT OR IGNORE INTO project_tombstones (project_id, deleted_at_utc) VALUES (?1, ?2)",
            params![project_text, time_text(deleted_at)],
        )
        .map_err(|error| sqlite_error("unable to record a project deletion", error))?;
    Ok(removed_projects + removed_resources > 0)
}

impl ProjectRepository for SqliteNoteRepository {
    fn create_project(&self, name: &ProjectName) -> Result<Project, StorageError> {
        let now = whole_milliseconds(Utc::now());
        let project = Project::new(ProjectId::new_random(), name.clone(), now, now);
        self.connection
            .execute(
                "INSERT INTO projects (project_id, name, created_at_utc, updated_at_utc) VALUES (?1, ?2, ?3, ?3)",
                params![project.project_id().to_string(), name.as_str(), time_text(now)],
            )
            .map_err(|error| sqlite_error("unable to create a project", error))?;
        tracing::info!(project_id = %project.project_id(), "Project created");
        Ok(project)
    }

    fn rename_project(
        &self,
        project_id: ProjectId,
        name: &ProjectName,
    ) -> Result<Project, StorageError> {
        let Some(current) = read_project(&self.connection, project_id)? else {
            return Err(StorageError::Rejected {
                reason: String::from("the project does not exist"),
            });
        };
        // A rename must win over the version it replaces, even if the clock went back.
        let updated_at = whole_milliseconds(Utc::now())
            .max(current.updated_at() + chrono::Duration::milliseconds(1));
        self.connection
            .execute(
                "UPDATE projects SET name = ?1, updated_at_utc = ?2 WHERE project_id = ?3",
                params![name.as_str(), time_text(updated_at), project_id.to_string()],
            )
            .map_err(|error| sqlite_error("unable to rename a project", error))?;
        tracing::info!(%project_id, "Project renamed");
        Ok(Project::new(
            project_id,
            name.clone(),
            current.created_at(),
            updated_at,
        ))
    }

    fn delete_project(&self, project_id: ProjectId) -> Result<bool, StorageError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| sqlite_error("unable to begin a project deletion", error))?;
        let existed = remove_project(&transaction, project_id, Utc::now())?;
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit a project deletion", error))?;
        tracing::info!(%project_id, existed, "Project deleted");
        Ok(existed)
    }

    fn list_projects(&self) -> Result<Vec<ProjectSummary>, StorageError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT p.project_id, p.name, p.created_at_utc, p.updated_at_utc,
                        (SELECT COUNT(*) FROM meetings m WHERE m.project_id = p.project_id),
                        (SELECT COUNT(*) FROM attachments a WHERE a.project_id = p.project_id),
                        (SELECT MAX(m.created_at_utc) FROM meetings m WHERE m.project_id = p.project_id)
                 FROM projects p ORDER BY p.name COLLATE NOCASE, p.project_id",
            )
            .map_err(|error| sqlite_error("unable to list projects", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            })
            .map_err(|error| sqlite_error("unable to list projects", error))?;
        rows.map(|row| {
            let (project_text, name, created, updated, meetings, resources, latest) =
                row.map_err(|error| sqlite_error("unable to read a project", error))?;
            Ok(ProjectSummary {
                project: project_from_columns(&project_text, &name, &created, &updated)?,
                meeting_count: usize::try_from(meetings).unwrap_or(0),
                resource_count: usize::try_from(resources).unwrap_or(0),
                latest_meeting_at: latest.as_deref().map(parse_sqlite_timestamp).transpose()?,
            })
        })
        .collect()
    }

    fn load_project(&self, project_id: ProjectId) -> Result<Option<Project>, StorageError> {
        read_project(&self.connection, project_id)
    }

    fn project_meeting_ids(&self, project_id: ProjectId) -> Result<Vec<MeetingId>, StorageError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT m.meeting_id FROM meetings m JOIN projects p ON p.project_id = m.project_id
                 WHERE m.project_id = ?1 ORDER BY m.created_at_utc DESC, m.meeting_id DESC",
            )
            .map_err(|error| sqlite_error("unable to list a project's meetings", error))?;
        statement
            .query_map(params![project_id.to_string()], |row| {
                row.get::<_, String>(0)
            })
            .map_err(|error| sqlite_error("unable to list a project's meetings", error))?
            .map(|row| {
                row.map(MeetingId::new)
                    .map_err(|error| sqlite_error("unable to read a meeting ID", error))
            })
            .collect()
    }

    fn meeting_projects(&self) -> Result<Vec<(MeetingId, ProjectId)>, StorageError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT m.meeting_id, m.project_id FROM meetings m
                 JOIN projects p ON p.project_id = m.project_id ORDER BY m.meeting_id",
            )
            .map_err(|error| sqlite_error("unable to list meeting projects", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| sqlite_error("unable to list meeting projects", error))?;
        let mut pairs = Vec::new();
        for row in rows {
            let (meeting_text, project_text) =
                row.map_err(|error| sqlite_error("unable to read a meeting project", error))?;
            if let Some(project_id) = ProjectId::parse(&project_text) {
                pairs.push((MeetingId::new(meeting_text), project_id));
            }
        }
        Ok(pairs)
    }

    fn move_meeting_to_project(
        &self,
        meeting_id: &MeetingId,
        project_id: Option<ProjectId>,
    ) -> Result<(), StorageError> {
        let Some(document_codec) = self.document_codec.as_deref() else {
            return Err(StorageError::Unavailable);
        };
        if let Some(project_id) = project_id
            && read_project(&self.connection, project_id)?.is_none()
        {
            return Err(StorageError::Rejected {
                reason: String::from("the project does not exist"),
            });
        }
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| sqlite_error("unable to begin moving a meeting", error))?;
        let document = read_document(&transaction, meeting_id)?;
        let moved = document_codec.with_project(&document, project_id)?;
        if moved != document {
            write_document(&transaction, meeting_id, &moved)?;
            apply_projection(
                &transaction,
                meeting_id,
                &document_codec.project(meeting_id, &moved)?,
            )?;
        }
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit moving a meeting", error))?;
        self.log_operation("move_meeting_to_project", meeting_id);
        Ok(())
    }

    fn project_resources(&self, project_id: ProjectId) -> Result<Vec<Attachment>, StorageError> {
        self.load_attachments(AttachmentOwnerFilter::Project(project_id))
    }
}

impl SqliteNoteRepository {
    /// Lists every deletion record, by project ID.
    fn read_project_tombstones(&self) -> Result<Vec<ProjectTombstone>, StorageError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT project_id, deleted_at_utc FROM project_tombstones ORDER BY project_id",
            )
            .map_err(|error| sqlite_error("unable to list project deletions", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| sqlite_error("unable to list project deletions", error))?;
        rows.map(|row| {
            let (project_text, deleted_text) =
                row.map_err(|error| sqlite_error("unable to read a project deletion", error))?;
            Ok(ProjectTombstone {
                project_id: ProjectId::parse(&project_text).ok_or_else(|| {
                    StorageError::Rejected {
                        reason: String::from("a stored project deletion is invalid"),
                    }
                })?,
                deleted_at: parse_time(&deleted_text)?,
            })
        })
        .collect()
    }
}

impl ProjectLibrarySync for SqliteNoteRepository {
    fn project_library_digest(&self) -> Result<[u8; 32], StorageError> {
        let library = self.export_project_library()?;
        let mut hasher = Sha256::new();
        for project in &library.projects {
            for field in [
                project.project_id().to_string(),
                project.name().as_str().to_owned(),
                time_text(project.created_at()),
                time_text(project.updated_at()),
            ] {
                hasher.update(field.as_bytes());
                hasher.update([0]);
            }
        }
        hasher.update([0xFF]);
        for tombstone in &library.tombstones {
            hasher.update(tombstone.project_id.to_string().as_bytes());
            hasher.update([0]);
        }
        Ok(hasher.finalize().into())
    }

    fn export_project_library(&self) -> Result<ProjectLibrary, StorageError> {
        let mut projects: Vec<Project> = self
            .list_projects()?
            .into_iter()
            .map(|summary| summary.project)
            .collect();
        projects.sort_by_key(Project::project_id);
        Ok(ProjectLibrary {
            projects,
            tombstones: self.read_project_tombstones()?,
        })
    }

    fn import_project_library(
        &self,
        library: &ProjectLibrary,
    ) -> Result<ProjectLibraryImport, StorageError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| sqlite_error("unable to begin a project merge", error))?;
        let mut outcome = ProjectLibraryImport::default();
        for tombstone in &library.tombstones {
            if is_tombstoned(&transaction, tombstone.project_id)? {
                continue;
            }
            remove_project(&transaction, tombstone.project_id, tombstone.deleted_at)?;
            outcome.changed = true;
            outcome.deleted_project_ids.push(tombstone.project_id);
        }
        for incoming in &library.projects {
            if is_tombstoned(&transaction, incoming.project_id())? {
                continue;
            }
            let write = match read_project(&transaction, incoming.project_id())? {
                None => true,
                Some(stored) => incoming.supersedes(&stored),
            };
            if !write {
                continue;
            }
            transaction
                .execute(
                    "INSERT INTO projects (project_id, name, created_at_utc, updated_at_utc)
                     VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(project_id) DO UPDATE SET name = excluded.name,
                         updated_at_utc = excluded.updated_at_utc",
                    params![
                        incoming.project_id().to_string(),
                        incoming.name().as_str(),
                        time_text(incoming.created_at()),
                        time_text(incoming.updated_at()),
                    ],
                )
                .map_err(|error| sqlite_error("unable to merge a project", error))?;
            outcome.changed = true;
        }
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit a project merge", error))?;
        if outcome.changed {
            tracing::info!(
                deleted = outcome.deleted_project_ids.len(),
                "Projects merged from a paired device"
            );
        }
        Ok(outcome)
    }
}
