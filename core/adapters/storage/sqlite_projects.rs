//! Stores projects, indexes their meetings and resources, and merges paired devices' project
//! lists (Issue #85).
//!
//! Tables: `projects` holds the live projects, and `project_tombstones` one row per deleted project,
//! kept forever so a paired device that missed the deletion cannot bring it back. The meeting's
//! CRDT document names its project; `meetings.project_id` is the index of that, written by the
//! projection, and `attachments.project_id` marks a project's resources. A meeting or resource
//! naming a project this device does not have (deleted, or not synced yet) is simply not shown in
//! any project. Projects nest (#142): `projects.parent_project_id` and `moved_at_utc` hold each
//! one's parent and when it last moved (NULL: at the top level, placed when created), merged
//! separately from the name. Times are stored as RFC 3339 text with milliseconds. This child module of
//! `sqlite_adapter` never logs names.

use crate::adapters::storage::sqlite_adapter::WriteTransaction;
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
        projects::{
            Project, ProjectId, ProjectLibrary, ProjectName, ProjectTombstone, is_within,
            shown_parents,
        },
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
    // Nesting (#142): the parent and when it last changed (NULL: placed when created).
    for column in ["parent_project_id", "moved_at_utc"] {
        let existing: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('projects') WHERE name = ?1",
                params![column],
                |row| row.get(0),
            )
            .map_err(|error| sqlite_error("unable to inspect the project table", error))?;
        if existing == 0 {
            connection
                .execute_batch(&format!("ALTER TABLE projects ADD COLUMN {column} TEXT"))
                .map_err(|error| sqlite_error("unable to add a project nesting column", error))?;
        }
    }
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
    parent_text: Option<&str>,
    moved_text: Option<&str>,
) -> Result<Project, StorageError> {
    let project_id = ProjectId::parse(project_text).ok_or_else(|| StorageError::Rejected {
        reason: String::from("a stored project ID is invalid"),
    })?;
    let name = ProjectName::parse(name).map_err(|_| StorageError::Rejected {
        reason: String::from("a stored project name is invalid"),
    })?;
    let created_at = parse_time(created_text)?;
    let parent = parent_text
        .map(|text| {
            ProjectId::parse(text).ok_or_else(|| StorageError::Rejected {
                reason: String::from("a stored project parent is invalid"),
            })
        })
        .transpose()?;
    let moved_at = moved_text
        .map(parse_time)
        .transpose()?
        .unwrap_or(created_at);
    Ok(
        Project::new(project_id, name, created_at, parse_time(updated_text)?)
            .with_placement(parent, moved_at),
    )
}

/// The columns `project_from_columns` reads, in its order, after the ID.
const PROJECT_COLUMNS: &str =
    "name, created_at_utc, updated_at_utc, parent_project_id, moved_at_utc";

/// Reads one row selected with `project_id` first and then `PROJECT_COLUMNS`.
type ProjectRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
);

/// Builds a project from a `ProjectRow`.
fn project_from_row(row: &ProjectRow) -> Result<Project, StorageError> {
    project_from_columns(
        &row.0,
        &row.1,
        &row.2,
        &row.3,
        row.4.as_deref(),
        row.5.as_deref(),
    )
}

/// Reads a `ProjectRow` from columns starting at `first`.
fn project_row(row: &rusqlite::Row<'_>, first: usize) -> rusqlite::Result<ProjectRow> {
    Ok((
        row.get(first)?,
        row.get(first + 1)?,
        row.get(first + 2)?,
        row.get(first + 3)?,
        row.get(first + 4)?,
        row.get(first + 5)?,
    ))
}

/// Every live project, by ID.
fn read_all_projects(connection: &Connection) -> Result<Vec<Project>, StorageError> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT project_id, {PROJECT_COLUMNS} FROM projects ORDER BY project_id"
        ))
        .map_err(|error| sqlite_error("unable to list projects", error))?;
    let rows = statement
        .query_map([], |row| project_row(row, 0))
        .map_err(|error| sqlite_error("unable to list projects", error))?;
    rows.map(|row| {
        project_from_row(&row.map_err(|error| sqlite_error("unable to read a project", error))?)
    })
    .collect()
}

/// Saves where `project_id` sits, as of `moved_at`.
fn write_placement(
    connection: &Connection,
    project_id: ProjectId,
    parent: Option<ProjectId>,
    moved_at: DateTime<Utc>,
) -> Result<(), StorageError> {
    connection
        .execute(
            "UPDATE projects SET parent_project_id = ?1, moved_at_utc = ?2 WHERE project_id = ?3",
            params![
                parent.map(|parent| parent.to_string()),
                time_text(moved_at),
                project_id.to_string()
            ],
        )
        .map_err(|error| sqlite_error("unable to move a project", error))?;
    Ok(())
}

/// The instant to record for a new local move of `project`: now, but always after its last move,
/// so the move wins even if the clock went back.
fn next_move_instant(project: &Project) -> DateTime<Utc> {
    whole_milliseconds(Utc::now()).max(project.moved_at() + chrono::Duration::milliseconds(1))
}

/// Reads one live project inside `connection`.
fn read_project(
    connection: &Connection,
    project_id: ProjectId,
) -> Result<Option<Project>, StorageError> {
    let row = connection
        .query_row(
            &format!("SELECT project_id, {PROJECT_COLUMNS} FROM projects WHERE project_id = ?1"),
            params![project_id.to_string()],
            |row| project_row(row, 0),
        )
        .optional()
        .map_err(|error| sqlite_error("unable to load a project", error))?;
    row.as_ref().map(project_from_row).transpose()
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
    // Its chat (#97), kept on this device only.
    connection
        .execute(
            "DELETE FROM chat_messages WHERE chat_key = 'project:' || ?1",
            params![project_text],
        )
        .map_err(|error| sqlite_error("unable to delete a project's chat", error))?;
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

    fn create_project_in(
        &self,
        name: &ProjectName,
        parent: Option<ProjectId>,
    ) -> Result<Project, StorageError> {
        let Some(parent) = parent else {
            return self.create_project(name);
        };
        if read_project(&self.connection, parent)?.is_none() {
            return Err(StorageError::Rejected {
                reason: String::from("the parent project does not exist"),
            });
        }
        let project = self.create_project(name)?;
        write_placement(
            &self.connection,
            project.project_id(),
            Some(parent),
            project.created_at(),
        )?;
        let created_at = project.created_at();
        Ok(project.with_placement(Some(parent), created_at))
    }

    fn move_project(
        &self,
        project_id: ProjectId,
        parent: Option<ProjectId>,
    ) -> Result<Project, StorageError> {
        let transaction = self
            .connection
            .write_transaction()
            .map_err(|error| sqlite_error("unable to begin moving a project", error))?;
        let projects = read_all_projects(&transaction)?;
        let Some(current) = projects
            .iter()
            .find(|project| project.project_id() == project_id)
        else {
            return Err(StorageError::Rejected {
                reason: String::from("the project does not exist"),
            });
        };
        if let Some(parent) = parent {
            if !projects
                .iter()
                .any(|project| project.project_id() == parent)
            {
                return Err(StorageError::Rejected {
                    reason: String::from("the parent project does not exist"),
                });
            }
            if is_within(&shown_parents(&projects), parent, project_id) {
                return Err(StorageError::Rejected {
                    reason: String::from("a project cannot move inside itself"),
                });
            }
        }
        let moved_at = next_move_instant(current);
        let moved = current.clone().with_placement(parent, moved_at);
        write_placement(&transaction, project_id, parent, moved_at)?;
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit moving a project", error))?;
        tracing::info!(%project_id, top_level = parent.is_none(), "Project moved");
        Ok(moved)
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
        Ok(
            Project::new(project_id, name.clone(), current.created_at(), updated_at)
                .with_placement(current.parent(), current.moved_at()),
        )
    }

    fn delete_project(&self, project_id: ProjectId) -> Result<bool, StorageError> {
        let transaction = self
            .connection
            .write_transaction()
            .map_err(|error| sqlite_error("unable to begin a project deletion", error))?;
        // Its subprojects move up into the parent it shows in (#142), as local moves that sync.
        let projects = read_all_projects(&transaction)?;
        let grandparent = shown_parents(&projects).get(&project_id).copied().flatten();
        for child in projects
            .iter()
            .filter(|project| project.parent() == Some(project_id))
        {
            write_placement(
                &transaction,
                child.project_id(),
                grandparent,
                next_move_instant(child),
            )?;
        }
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
            .prepare(&format!(
                "SELECT (SELECT COUNT(*) FROM meetings m WHERE m.project_id = p.project_id),
                        (SELECT COUNT(*) FROM attachments a WHERE a.project_id = p.project_id),
                        (SELECT MAX(m.created_at_utc) FROM meetings m WHERE m.project_id = p.project_id),
                        p.project_id, {}
                 FROM projects p ORDER BY p.name COLLATE NOCASE, p.project_id",
                PROJECT_COLUMNS
                    .split(", ")
                    .map(|column| format!("p.{column}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
            .map_err(|error| sqlite_error("unable to list projects", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    project_row(row, 3)?,
                ))
            })
            .map_err(|error| sqlite_error("unable to list projects", error))?;
        let mut summaries = rows
            .map(|row| {
                let (meetings, resources, latest, project_row) =
                    row.map_err(|error| sqlite_error("unable to read a project", error))?;
                Ok(ProjectSummary {
                    project: project_from_row(&project_row)?,
                    meeting_count: usize::try_from(meetings).unwrap_or(0),
                    resource_count: usize::try_from(resources).unwrap_or(0),
                    latest_meeting_at: latest.as_deref().map(parse_sqlite_timestamp).transpose()?,
                    parent_id: None,
                })
            })
            .collect::<Result<Vec<ProjectSummary>, StorageError>>()?;
        let projects: Vec<Project> = summaries
            .iter()
            .map(|summary| summary.project.clone())
            .collect();
        let parents = shown_parents(&projects);
        for summary in &mut summaries {
            summary.parent_id = parents
                .get(&summary.project.project_id())
                .copied()
                .flatten();
        }
        Ok(summaries)
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
            .write_transaction()
            .map_err(|error| sqlite_error("unable to begin moving a meeting", error))?;
        let document = read_document(&transaction, self.embedding_cipher.as_ref(), meeting_id)?;
        let moved = document_codec.with_project(&document, project_id)?;
        if moved != document {
            write_document(
                &transaction,
                self.embedding_cipher.as_ref(),
                meeting_id,
                &moved,
            )?;
            apply_projection(
                &transaction,
                self.embedding_cipher.as_ref(),
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

    fn delete_project_resource(&self, attachment_id: uuid::Uuid) -> Result<bool, StorageError> {
        self.delete_resource_rows(attachment_id)
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
                project
                    .parent()
                    .map(|parent| parent.to_string())
                    .unwrap_or_default(),
                time_text(project.moved_at()),
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
            .write_transaction()
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
            let stored = read_project(&transaction, incoming.project_id())?;
            let Some(stored) = stored else {
                transaction
                    .execute(
                        "INSERT INTO projects (project_id, name, created_at_utc, updated_at_utc,
                             parent_project_id, moved_at_utc)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            incoming.project_id().to_string(),
                            incoming.name().as_str(),
                            time_text(incoming.created_at()),
                            time_text(incoming.updated_at()),
                            incoming.parent().map(|parent| parent.to_string()),
                            time_text(incoming.moved_at()),
                        ],
                    )
                    .map_err(|error| sqlite_error("unable to merge a project", error))?;
                outcome.changed = true;
                continue;
            };
            // The name and the placement each keep their newest version (#142).
            if incoming.supersedes(&stored) {
                transaction
                    .execute(
                        "UPDATE projects SET name = ?1, updated_at_utc = ?2 WHERE project_id = ?3",
                        params![
                            incoming.name().as_str(),
                            time_text(incoming.updated_at()),
                            incoming.project_id().to_string(),
                        ],
                    )
                    .map_err(|error| sqlite_error("unable to merge a project", error))?;
                outcome.changed = true;
            }
            if incoming.placement_supersedes(&stored) {
                write_placement(
                    &transaction,
                    incoming.project_id(),
                    incoming.parent(),
                    incoming.moved_at(),
                )?;
                outcome.changed = true;
            }
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
