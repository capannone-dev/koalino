//! Declares how projects (Issue #85) are stored and exchanged with paired devices.
//!
//! A project's meetings are not listed here: each meeting's own CRDT document names its project
//! (`MeetingDocumentCodec::with_project`), and the repository indexes that. A project's resources
//! are attachments owned by the project instead of a meeting, so they reuse the attachment store
//! and its sync. Names and resource files are user content: implementations must not log them.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::{
    errors::StorageError,
    models::{Attachment, MeetingId},
    projects::{Project, ProjectId, ProjectLibrary, ProjectName},
};

/// A project with what it holds, for lists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSummary {
    /// The project.
    pub project: Project,
    /// How many meetings it holds.
    pub meeting_count: usize,
    /// How many resources it holds.
    pub resource_count: usize,
    /// When its newest meeting started, if it holds any.
    pub latest_meeting_at: Option<DateTime<Utc>>,
    /// The project it shows inside, or `None` at the top level (#142): its stored parent unless
    /// that is unknown here or part of a loop (`projects::shown_parents`).
    pub parent_id: Option<ProjectId>,
}

/// What merging a paired device's project library changed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectLibraryImport {
    /// Whether anything was written.
    pub changed: bool,
    /// Projects deleted by this merge, whose resource files the caller must now remove.
    pub deleted_project_ids: Vec<ProjectId>,
}

/// Stores projects and indexes their meetings and resources.
pub trait ProjectRepository {
    /// Creates a project.
    ///
    /// Inputs: A validated name. Outputs: The new project, with a random ID and the current time
    /// as both its creation and last change. Side effects: Writes one row.
    fn create_project(&self, name: &ProjectName) -> Result<Project, StorageError>;

    /// Creates a project inside another one, or at the top level (#142).
    ///
    /// Inputs: A validated name and the parent. Outputs: The new project, or `Rejected` for an
    /// unknown parent. Side effects: Writes one row.
    fn create_project_in(
        &self,
        name: &ProjectName,
        parent: Option<ProjectId>,
    ) -> Result<Project, StorageError>;

    /// Moves a project inside another one, or to the top level (#142).
    ///
    /// Inputs: The project and its new parent. Outputs: The moved project, last moved now, or
    /// `Rejected` for an unknown project or parent, or a parent that is the project itself or
    /// inside it. Side effects: Updates one row; paired devices receive the move.
    fn move_project(
        &self,
        project_id: ProjectId,
        parent: Option<ProjectId>,
    ) -> Result<Project, StorageError>;

    /// Renames a project.
    ///
    /// Inputs: The project and its new name. Outputs: The renamed project, last changed now, or
    /// `Rejected` for an unknown or deleted project. Side effects: Updates one row.
    fn rename_project(
        &self,
        project_id: ProjectId,
        name: &ProjectName,
    ) -> Result<Project, StorageError>;

    /// Deletes a project for good.
    ///
    /// Inputs: The project. Outputs: Whether it existed. Its meetings stay and leave the project;
    /// its resource rows are deleted, and the caller removes their files. Its subprojects move up
    /// into its own parent (#142).
    /// Side effects: Deletes rows and records a deletion that paired devices also apply and that
    /// keeps the project from returning.
    fn delete_project(&self, project_id: ProjectId) -> Result<bool, StorageError>;

    /// Lists the live projects, by name.
    ///
    /// Outputs: Each project with its meeting and resource counts. Side effects: Reads rows.
    fn list_projects(&self) -> Result<Vec<ProjectSummary>, StorageError>;

    /// Loads one live project.
    ///
    /// Outputs: The project, or `None` when it is unknown or deleted. Side effects: Reads a row.
    fn load_project(&self, project_id: ProjectId) -> Result<Option<Project>, StorageError>;

    /// Lists a project's meetings, newest first, as the meeting list orders them.
    ///
    /// Outputs: Meeting IDs; empty for an unknown project. Side effects: Reads rows.
    fn project_meeting_ids(&self, project_id: ProjectId) -> Result<Vec<MeetingId>, StorageError>;

    /// Tells which live project each meeting belongs to.
    ///
    /// Outputs: (meeting, project) pairs for meetings in a live project; a meeting naming an
    /// unknown or deleted project is left out. Side effects: Reads rows.
    fn meeting_projects(&self) -> Result<Vec<(MeetingId, ProjectId)>, StorageError>;

    /// Moves a meeting into a project, or out of any project for `None`.
    ///
    /// Inputs: The meeting and the project, which must be live.
    /// Outputs: Success, or `Rejected` for an unknown meeting or project.
    /// Side effects: Changes the meeting's CRDT document, which paired devices then sync, and its
    /// index row.
    fn move_meeting_to_project(
        &self,
        meeting_id: &MeetingId,
        project_id: Option<ProjectId>,
    ) -> Result<(), StorageError>;

    /// Deletes one project resource's row and records the deletion, so paired devices delete it
    /// too and attachment sync never brings it back.
    ///
    /// Inputs: The resource's attachment ID. A meeting attachment is never touched.
    /// Outputs: Whether a resource was deleted, or a storage error.
    /// Side effects: Deletes one row and writes a deletion record. The file is the caller's.
    fn delete_project_resource(&self, attachment_id: Uuid) -> Result<bool, StorageError>;

    /// Lists a project's resources, oldest first.
    ///
    /// Outputs: The attachments it owns. Side effects: Reads rows.
    fn project_resources(&self, project_id: ProjectId) -> Result<Vec<Attachment>, StorageError>;
}

/// Exchanges the project list with paired devices.
pub trait ProjectLibrarySync {
    /// Fingerprints the projects and deletion records, so unchanged lists are not sent again.
    ///
    /// Outputs: A SHA-256 digest. Side effects: Reads rows.
    fn project_library_digest(&self) -> Result<[u8; 32], StorageError>;

    /// Lists every project and deletion record.
    ///
    /// Outputs: The library. Side effects: Reads rows.
    fn export_project_library(&self) -> Result<ProjectLibrary, StorageError>;

    /// Merges a paired device's library.
    ///
    /// Inputs: The library, already validated. Projects are added or renamed when their version
    /// supersedes this device's; deletion records always win and are kept forever.
    /// Outputs: What changed. Side effects: Writes rows; deletes the resource rows of projects it
    /// deletes, and the caller removes their files.
    fn import_project_library(
        &self,
        library: &ProjectLibrary,
    ) -> Result<ProjectLibraryImport, StorageError>;
}
