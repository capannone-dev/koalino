//! Specifies projects in SQLite (Issue #85): creating, renaming and deleting them, moving meetings
//! in and out through the meeting's CRDT document, project resources kept apart from meeting
//! attachments, and merging a paired device's project list so every device converges and a
//! deletion always wins.

use std::{path::PathBuf, sync::Arc};

use chrono::{DateTime, TimeZone, Utc};
use uuid::Uuid;

use super::sqlite_adapter::SqliteNoteRepository;
use crate::{
    application::{
        crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
        meeting_document::AutomergeMeetingDocumentCodec,
    },
    domain::{
        models::{Attachment, DeviceId, MeetingId},
        projects::{Project, ProjectId, ProjectLibrary, ProjectName, ProjectTombstone},
    },
    ports::{
        projects::{ProjectLibrarySync, ProjectRepository},
        storage::{AttachmentRepository, NoteRepository},
    },
};

/// A repository with the meeting-document codec, in a scratch directory removed on drop.
struct Device {
    directory: PathBuf,
    repository: SqliteNoteRepository,
}

impl Device {
    /// Opens an empty device database.
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("projects-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory)
            .unwrap_or_else(|error| panic!("scratch directory must be created: {error}"));
        let repository = SqliteNoteRepository::open(
            directory.join("notes.sqlite3"),
            DeviceId::new("device"),
            Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([5; 32]))),
        )
        .and_then(|repository| {
            repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
        })
        .unwrap_or_else(|error| panic!("repository must open: {error}"));
        Self {
            directory,
            repository,
        }
    }

    /// Creates a project named `name`.
    fn create(&self, name: &str) -> Project {
        self.repository
            .create_project(&name_of(name))
            .unwrap_or_else(|error| panic!("project must be created: {error}"))
    }

    /// Stores an empty meeting.
    fn meeting(&self, meeting_id: &str) -> MeetingId {
        let meeting_id = MeetingId::new(meeting_id);
        self.repository
            .create_meeting(&meeting_id, &automerge::AutoCommit::new().save())
            .unwrap_or_else(|error| panic!("meeting must be created: {error}"));
        meeting_id
    }

    /// Moves `meeting_id` into `project_id`.
    fn move_to(&self, meeting_id: &MeetingId, project_id: Option<ProjectId>) {
        self.repository
            .move_meeting_to_project(meeting_id, project_id)
            .unwrap_or_else(|error| panic!("meeting must move: {error}"));
    }

    /// Stores a resource row for `project_id`.
    fn resource(&self, project_id: ProjectId) -> Attachment {
        let attachment = Attachment::new(
            Uuid::new_v4(),
            None,
            "/data/attachments/notes.md",
            "text/markdown",
            Utc::now(),
            "device",
        )
        .with_project_id(project_id);
        self.repository
            .save_attachment(&attachment)
            .unwrap_or_else(|error| panic!("resource must be saved: {error}"));
        attachment
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// A valid name.
fn name_of(name: &str) -> ProjectName {
    ProjectName::parse(name).unwrap_or_else(|error| panic!("{error}"))
}

/// A fixed instant `seconds` after a reference point.
fn at(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_800_000_000 + seconds, 0)
        .single()
        .unwrap_or_else(|| panic!("the instant must exist"))
}

/// The names of the listed projects, in list order.
fn listed_names(device: &Device) -> Vec<String> {
    device
        .repository
        .list_projects()
        .unwrap_or_else(|error| panic!("projects must list: {error}"))
        .into_iter()
        .map(|summary| summary.project.name().as_str().to_owned())
        .collect()
}

#[test]
fn projects_are_created_renamed_and_listed_by_name() {
    let device = Device::new();
    let website = device.create("website");
    device.create("Budget 2027");

    let renamed = device
        .repository
        .rename_project(website.project_id(), &name_of("Website relaunch"))
        .unwrap_or_else(|error| panic!("project must be renamed: {error}"));

    assert_eq!(renamed.name().as_str(), "Website relaunch");
    assert!(renamed.updated_at() >= website.updated_at());
    assert_eq!(
        listed_names(&device),
        vec!["Budget 2027", "Website relaunch"]
    );
    assert_eq!(
        device
            .repository
            .load_project(website.project_id())
            .ok()
            .flatten(),
        Some(renamed)
    );
}

#[test]
fn renaming_an_unknown_project_is_refused() {
    let device = Device::new();

    assert!(
        device
            .repository
            .rename_project(ProjectId::new_random(), &name_of("Ghost"))
            .is_err()
    );
}

#[test]
fn a_meeting_moves_into_a_project_and_out_again() {
    let device = Device::new();
    let project = device.create("Hiring");
    let older = device.meeting("meeting-older");
    let newer = device.meeting("meeting-newer");
    let outside = device.meeting("meeting-outside");

    device.move_to(&older, Some(project.project_id()));
    device.move_to(&newer, Some(project.project_id()));

    let summary = &device.repository.list_projects().unwrap_or_default()[0];
    assert_eq!(summary.meeting_count, 2);
    let mut in_project = device
        .repository
        .project_meeting_ids(project.project_id())
        .unwrap_or_default();
    in_project.sort_by(|first, second| first.as_str().cmp(second.as_str()));
    assert_eq!(in_project, vec![newer.clone(), older.clone()]);
    assert!(!in_project.contains(&outside));

    device.move_to(&older, None);

    assert_eq!(
        device
            .repository
            .project_meeting_ids(project.project_id())
            .unwrap_or_default(),
        vec![newer.clone()]
    );
    assert_eq!(
        device.repository.meeting_projects().unwrap_or_default(),
        vec![(newer, project.project_id())]
    );
}

#[test]
fn the_meeting_document_carries_its_project() {
    let device = Device::new();
    let project = device.create("Hiring");
    let meeting = device.meeting("meeting-doc");

    device.move_to(&meeting, Some(project.project_id()));
    let document = device
        .repository
        .load_crdt_document(&meeting)
        .unwrap_or_else(|error| panic!("document must load: {error}"));

    let receiver = Device::new();
    receiver.create("Unrelated");
    receiver
        .repository
        .create_meeting(&meeting, &document)
        .unwrap_or_else(|error| panic!("peer document must be stored: {error}"));
    // The project itself has not arrived yet, so the meeting is in no project here.
    assert!(
        receiver
            .repository
            .meeting_projects()
            .unwrap_or_default()
            .is_empty()
    );

    receiver
        .repository
        .import_project_library(
            &device
                .repository
                .export_project_library()
                .unwrap_or_default(),
        )
        .unwrap_or_else(|error| panic!("library must merge: {error}"));

    assert_eq!(
        receiver.repository.meeting_projects().unwrap_or_default(),
        vec![(meeting, project.project_id())]
    );
}

#[test]
fn moving_into_an_unknown_project_or_an_unknown_meeting_is_refused() {
    let device = Device::new();
    let project = device.create("Hiring");
    let meeting = device.meeting("meeting-real");

    assert!(
        device
            .repository
            .move_meeting_to_project(&meeting, Some(ProjectId::new_random()))
            .is_err()
    );
    assert!(
        device
            .repository
            .move_meeting_to_project(&MeetingId::new("meeting-ghost"), Some(project.project_id()))
            .is_err()
    );
}

#[test]
fn resources_belong_to_their_project_and_never_wait_for_a_meeting() {
    let device = Device::new();
    let project = device.create("Hiring");
    let resource = device.resource(project.project_id());

    assert_eq!(
        device
            .repository
            .project_resources(project.project_id())
            .unwrap_or_default(),
        vec![resource]
    );
    assert!(
        device
            .repository
            .get_unlinked_attachments()
            .unwrap_or_default()
            .is_empty()
    );
    assert_eq!(
        device.repository.list_projects().unwrap_or_default()[0].resource_count,
        1
    );
}

#[test]
fn a_resource_of_an_unknown_project_is_refused() {
    let device = Device::new();
    let orphan = Attachment::new(
        Uuid::new_v4(),
        None,
        "/data/attachments/x.pdf",
        "application/pdf",
        Utc::now(),
        "device",
    )
    .with_project_id(ProjectId::new_random());

    assert!(device.repository.save_attachment(&orphan).is_err());
}

#[test]
fn deleting_a_project_keeps_its_meetings_and_removes_its_resources() {
    let device = Device::new();
    let project = device.create("Hiring");
    let meeting = device.meeting("meeting-kept");
    device.move_to(&meeting, Some(project.project_id()));
    device.resource(project.project_id());

    assert_eq!(
        device.repository.delete_project(project.project_id()).ok(),
        Some(true)
    );

    assert!(listed_names(&device).is_empty());
    assert!(
        device
            .repository
            .meeting_projects()
            .unwrap_or_default()
            .is_empty()
    );
    assert!(
        device
            .repository
            .project_resources(project.project_id())
            .unwrap_or_default()
            .is_empty()
    );
    assert!(
        device
            .repository
            .load_meeting_metadata(&meeting)
            .ok()
            .flatten()
            .is_some()
    );
    assert_eq!(
        device.repository.delete_project(project.project_id()).ok(),
        Some(false)
    );
}

#[test]
fn paired_devices_converge_on_the_newest_name() {
    let first = Device::new();
    let second = Device::new();
    let project_id = ProjectId::from_uuid(Uuid::from_u128(85));
    let version = |name: &str, seconds: i64| ProjectLibrary {
        projects: vec![Project::new(project_id, name_of(name), at(0), at(seconds))],
        tombstones: Vec::new(),
    };
    first
        .repository
        .import_project_library(&version("Old", 10))
        .unwrap_or_else(|error| panic!("{error}"));

    let renamed = first
        .repository
        .import_project_library(&version("New", 20))
        .unwrap_or_else(|error| panic!("{error}"));
    let stale = first
        .repository
        .import_project_library(&version("Stale", 15))
        .unwrap_or_else(|error| panic!("{error}"));
    second
        .repository
        .import_project_library(&version("Stale", 15))
        .unwrap_or_else(|error| panic!("{error}"));
    second
        .repository
        .import_project_library(&version("New", 20))
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(renamed.changed);
    assert!(!stale.changed);
    assert_eq!(listed_names(&first), vec!["New"]);
    assert_eq!(listed_names(&second), vec!["New"]);
    assert_eq!(
        first.repository.project_library_digest().ok(),
        second.repository.project_library_digest().ok()
    );
}

#[test]
fn a_deletion_wins_over_any_version_and_reports_the_project() {
    let device = Device::new();
    let project = device.create("Hiring");
    device.resource(project.project_id());
    let tombstone = ProjectTombstone {
        project_id: project.project_id(),
        deleted_at: at(0),
    };
    let newer_version = Project::new(
        project.project_id(),
        name_of("Revived"),
        project.created_at(),
        at(1_000_000_000),
    );

    let deleted = device
        .repository
        .import_project_library(&ProjectLibrary {
            projects: Vec::new(),
            tombstones: vec![tombstone],
        })
        .unwrap_or_else(|error| panic!("{error}"));
    let revived = device
        .repository
        .import_project_library(&ProjectLibrary {
            projects: vec![newer_version],
            tombstones: Vec::new(),
        })
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(deleted.deleted_project_ids, vec![project.project_id()]);
    assert!(!revived.changed);
    assert!(listed_names(&device).is_empty());
    assert!(
        device
            .repository
            .project_resources(project.project_id())
            .unwrap_or_default()
            .is_empty()
    );
    assert_eq!(
        device
            .repository
            .export_project_library()
            .unwrap_or_default()
            .tombstones
            .len(),
        1
    );
}
