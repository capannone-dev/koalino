//! Defines projects (Issue #85): named folders that group meetings and hold external resources.
//!
//! A project's identity is a random UUID shared by every paired device. Its name can change on any
//! device; `Project::supersedes` is the rule that makes all devices settle on the same name (the
//! newest change wins, and at the same instant the greater name). Deletions are recorded as
//! `ProjectTombstone`s, which always win, so a deleted project cannot return from a device that
//! was offline. Names are user content: adapters must not log them.
//!
//! Projects nest (#142): each one has a parent, or none at the top level, set by its own
//! last-writer-wins rule (`moved_at`), so a move on one device and a rename on another both
//! survive. Moves are refused when they would put a project inside itself, but two devices can
//! still each make a move that together forms a loop; `shown_parents` then breaks every loop at
//! its newest move, the same way on every device. A project whose parent is unknown here (deleted,
//! or not synced yet) shows at the top level.

use std::{collections::HashMap, fmt};

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Identifies one project on every paired device.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct ProjectId(Uuid);

impl ProjectId {
    /// Creates a new random ID.
    ///
    /// Inputs: None. Outputs: A version-4 UUID ID. Side effects: Reads the system random source.
    pub fn new_random() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing UUID.
    ///
    /// Inputs: The UUID. Outputs: The ID. Side effects: None.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Reads an ID written by `to_string`.
    ///
    /// Inputs: Text from storage, a document, a peer or a UI. Outputs: The ID, or `None` for text
    /// that is not a UUID. Side effects: None.
    pub fn parse(text: &str) -> Option<Self> {
        Uuid::parse_str(text.trim()).ok().map(Self)
    }

    /// Returns the UUID.
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl fmt::Display for ProjectId {
    /// Writes the hyphenated lowercase UUID.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Describes why text is not a valid project name.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProjectNameError {
    /// The name is empty or only whitespace.
    #[error("project name must not be blank")]
    Blank,
    /// The trimmed name is too long.
    #[error("project name must be at most {max_characters} characters")]
    TooLong {
        /// The maximum number of Unicode scalar values.
        max_characters: usize,
    },
    /// The name contains a control character such as a newline.
    #[error("project name must not contain control characters")]
    ControlCharacter,
}

/// A validated project name.
///
/// Invariant: trimmed, non-empty, at most `MAX_CHARACTERS` Unicode scalar values, and free of
/// control characters, as meeting titles are.
#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct ProjectName(String);

impl ProjectName {
    /// Longest accepted name, counted in Unicode scalar values after trimming.
    pub const MAX_CHARACTERS: usize = 200;

    /// Validates a name typed by the owner or received from a paired device.
    ///
    /// Inputs: The raw text; surrounding whitespace is ignored.
    /// Outputs: The trimmed name, or `Blank`, `TooLong` or `ControlCharacter`.
    /// Side effects: None.
    pub fn parse(raw_name: &str) -> Result<Self, ProjectNameError> {
        let trimmed_name = raw_name.trim();
        if trimmed_name.is_empty() {
            return Err(ProjectNameError::Blank);
        }
        if trimmed_name.chars().count() > Self::MAX_CHARACTERS {
            return Err(ProjectNameError::TooLong {
                max_characters: Self::MAX_CHARACTERS,
            });
        }
        if trimmed_name.chars().any(char::is_control) {
            return Err(ProjectNameError::ControlCharacter);
        }
        Ok(Self(trimmed_name.to_owned()))
    }

    /// Borrows the name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One project as every device stores and shares it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Project {
    project_id: ProjectId,
    name: ProjectName,
    /// When the project was created; never changes.
    created_at: DateTime<Utc>,
    /// When the name last changed; decides which name wins between devices.
    updated_at: DateTime<Utc>,
    /// The project this one sits in, or `None` at the top level (#142).
    parent: Option<ProjectId>,
    /// When the parent last changed (the creation instant until the first move); decides which
    /// placement wins between devices.
    moved_at: DateTime<Utc>,
}

impl Project {
    /// Creates a project value.
    ///
    /// Inputs: The ID, the name, the creation instant and the instant of the latest rename (the
    /// creation instant for a project never renamed). Outputs: The project. Side effects: None.
    pub fn new(
        project_id: ProjectId,
        name: ProjectName,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            project_id,
            name,
            created_at,
            updated_at,
            parent: None,
            moved_at: created_at,
        }
    }

    /// Places the project under `parent` (or at the top level), as of `moved_at`.
    ///
    /// Inputs: The parent and the instant of the move. Outputs: The project so placed.
    /// Side effects: None.
    #[must_use]
    pub fn with_placement(mut self, parent: Option<ProjectId>, moved_at: DateTime<Utc>) -> Self {
        self.parent = parent;
        self.moved_at = moved_at;
        self
    }

    /// Returns the stored parent; `shown_parents` decides where the project actually shows.
    pub fn parent(&self) -> Option<ProjectId> {
        self.parent
    }

    /// Returns the instant of the latest move (the creation instant if never moved).
    pub fn moved_at(&self) -> DateTime<Utc> {
        self.moved_at
    }

    /// Tells whether this version's placement replaces `other`'s, for the same project.
    ///
    /// Inputs: Another version. Outputs: `true` when this one moved later, or at the same instant
    /// to a greater parent (the top level counts as least); `false` for an equal placement.
    /// Side effects: None.
    pub fn placement_supersedes(&self, other: &Self) -> bool {
        (self.moved_at, self.parent) > (other.moved_at, other.parent)
    }

    /// Returns the ID.
    pub fn project_id(&self) -> ProjectId {
        self.project_id
    }

    /// Borrows the name.
    pub fn name(&self) -> &ProjectName {
        &self.name
    }

    /// Returns the creation instant.
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// Returns the instant of the latest rename.
    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    /// Tells whether this version of a project replaces `other`, a version of the same project.
    ///
    /// Inputs: Another version. Outputs: `true` when this one changed later, or at the same
    /// instant with a greater name; `false` for an equal version. Every device applies the same
    /// rule, so they converge whatever order versions arrive in. Side effects: None.
    pub fn supersedes(&self, other: &Self) -> bool {
        (self.updated_at, &self.name) > (other.updated_at, &other.name)
    }
}

/// Records that a project was deleted, so no device brings it back.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectTombstone {
    /// The deleted project.
    pub project_id: ProjectId,
    /// When it was deleted.
    pub deleted_at: DateTime<Utc>,
}

/// Every project and deletion record a device holds, as exchanged with paired devices.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectLibrary {
    /// The live projects.
    pub projects: Vec<Project>,
    /// The deleted projects, kept forever.
    pub tombstones: Vec<ProjectTombstone>,
}

/// Where each project shows: under its parent, or at the top level (`None`) when the parent is
/// unknown, itself, or part of a loop broken here (#142).
///
/// Inputs: Every live project. Outputs: Each project's shown parent. Every loop (possible only
/// through moves made on two devices before they synced) is broken at the member that moved
/// last, ties going to the greater ID; loops never share members, so the result does not depend
/// on the order of `projects`. Side effects: None.
pub fn shown_parents(projects: &[Project]) -> HashMap<ProjectId, Option<ProjectId>> {
    let known: HashMap<ProjectId, &Project> = projects
        .iter()
        .map(|project| (project.project_id(), project))
        .collect();
    let mut parents: HashMap<ProjectId, Option<ProjectId>> = projects
        .iter()
        .map(|project| {
            let parent = project
                .parent()
                .filter(|parent| *parent != project.project_id() && known.contains_key(parent));
            (project.project_id(), parent)
        })
        .collect();
    let mut ids: Vec<ProjectId> = parents.keys().copied().collect();
    ids.sort();
    for start in ids {
        let mut path: Vec<ProjectId> = Vec::new();
        let mut current = Some(start);
        while let Some(project_id) = current {
            if let Some(position) = path.iter().position(|seen| *seen == project_id) {
                let breaker = path[position..].iter().copied().max_by_key(|member| {
                    (known.get(member).map(|project| project.moved_at()), *member)
                });
                if let Some(breaker) = breaker {
                    parents.insert(breaker, None);
                }
                break;
            }
            path.push(project_id);
            current = parents.get(&project_id).copied().flatten();
        }
    }
    parents
}

/// Tells whether `project_id` is `ancestor` or sits somewhere inside it.
///
/// Inputs: Shown parents (from `shown_parents`, so free of loops), the project and the possible
/// ancestor. Outputs: Whether moving `ancestor` into `project_id` would make a loop.
/// Side effects: None.
pub fn is_within(
    parents: &HashMap<ProjectId, Option<ProjectId>>,
    project_id: ProjectId,
    ancestor: ProjectId,
) -> bool {
    let mut current = Some(project_id);
    // Bounded by the number of projects, in case a caller passes parents that still loop.
    for _ in 0..=parents.len() {
        match current {
            Some(project_id) if project_id == ancestor => return true,
            Some(project_id) => current = parents.get(&project_id).copied().flatten(),
            None => return false,
        }
    }
    false
}
