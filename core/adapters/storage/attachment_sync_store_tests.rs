//! Specifies the sync side of attachment storage: linked files get hashed and listed without
//! local paths, only linked files can be read, and a file from a paired device is stored privately
//! under a safe name, only after its SHA-256 check.

use std::{path::PathBuf, sync::Arc};

use chrono::Utc;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{attachment_sync_store::AttachmentSyncStore, sqlite_adapter::SqliteNoteRepository};
use crate::{
    application::crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
    domain::{
        models::{Attachment, AttachmentOwner, DeviceId, MeetingId, SharedAttachment},
        projects::{ProjectId, ProjectName},
    },
    ports::{
        file_transfer::{AttachmentLibrary, FILE_TRANSFER_CHUNK_BYTES},
        projects::ProjectRepository,
        storage::{AttachmentRepository, NoteRepository},
    },
};

/// A store over a fresh database with the meeting `meeting-1`.
struct Fixture {
    directory: PathBuf,
    store: AttachmentSyncStore,
    repository: SqliteNoteRepository,
}

impl Fixture {
    /// Opens the fixture.
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("attachment-sync-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory)
            .unwrap_or_else(|error| panic!("scratch directory must be created: {error}"));
        let open = || {
            SqliteNoteRepository::open(
                directory.join("notes.sqlite3"),
                DeviceId::new("device"),
                Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([3; 32]))),
            )
            .unwrap_or_else(|error| panic!("repository must open: {error}"))
        };
        let repository = open();
        repository
            .create_meeting(&MeetingId::new("meeting-1"), &[1])
            .unwrap_or_else(|error| panic!("meeting must be created: {error}"));
        let store = AttachmentSyncStore::new(open(), directory.join("attachments"));
        Self {
            directory,
            store,
            repository,
        }
    }

    /// Writes `contents` to a file and adds a row, linked to `meeting-1` or unlinked.
    fn add_local(&self, number: u128, contents: &[u8], linked: bool) -> Uuid {
        let attachment_id = Uuid::from_u128(number);
        let path = self.directory.join(format!("{attachment_id}-photo.png"));
        std::fs::write(&path, contents)
            .unwrap_or_else(|error| panic!("file must be written: {error}"));
        let attachment = Attachment::new(
            attachment_id,
            linked.then(|| MeetingId::new("meeting-1")),
            path.to_string_lossy(),
            "image/png",
            Utc::now(),
            "device",
        )
        .with_meeting_time_ms(9_000);
        self.repository
            .save_attachment(&attachment)
            .unwrap_or_else(|error| panic!("row must save: {error}"));
        attachment_id
    }

    /// Lists shared attachments.
    fn shared(&self) -> Vec<SharedAttachment> {
        self.store
            .shared_attachments()
            .unwrap_or_else(|error| panic!("list must load: {error}"))
    }
}

/// The hexadecimal SHA-256 of `contents`.
fn sha256(contents: &[u8]) -> String {
    format!("{:x}", Sha256::digest(contents))
}

/// Metadata of an incoming file for `meeting-1`.
fn incoming(contents: &[u8]) -> SharedAttachment {
    SharedAttachment {
        attachment_id: Uuid::from_u128(0xAB),
        owner: AttachmentOwner::Meeting(MeetingId::new("meeting-1")),
        file_name: "whiteboard.png".to_owned(),
        mime_type: "image/png".to_owned(),
        content_hash: sha256(contents),
        size_bytes: contents.len() as u64,
        original_timestamp: Utc::now(),
        original_device_id: "phone".to_owned(),
        meeting_time_ms: Some(42_000),
    }
}

/// Runs the whole receive path and returns what `finish_incoming` said.
fn receive(
    fixture: &mut Fixture,
    attachment: &SharedAttachment,
    contents: &[u8],
) -> Result<(), crate::domain::errors::StorageError> {
    fixture.store.begin_incoming(attachment)?;
    fixture.store.append_incoming(attachment, contents)?;
    fixture.store.finish_incoming(attachment)
}

/// Files in a meeting's attachment folder.
fn stored_files(fixture: &Fixture) -> Vec<String> {
    std::fs::read_dir(fixture.directory.join("attachments/meeting-1"))
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn linked_attachments_are_hashed_one_per_call_and_listed_without_paths() {
    let mut fixture = Fixture::new();
    let first = fixture.add_local(1, b"first", true);
    fixture.add_local(2, b"second", true);
    fixture.add_local(3, b"unlinked", false);

    fixture
        .store
        .hash_next_attachment()
        .unwrap_or_else(|error| panic!("hash must run: {error}"));
    let after_one = fixture.shared();
    fixture
        .store
        .hash_next_attachment()
        .and_then(|()| fixture.store.hash_next_attachment())
        .unwrap_or_else(|error| panic!("hash must run: {error}"));
    let after_all = fixture.shared();

    assert_eq!(after_one.len(), 1);
    assert_eq!(after_one[0].attachment_id, first);
    assert_eq!(after_one[0].content_hash, sha256(b"first"));
    assert_eq!(after_one[0].size_bytes, 5);
    assert_eq!(after_one[0].file_name, "photo.png");
    assert_eq!(after_one[0].meeting_time_ms, Some(9_000));
    assert_eq!(after_all.len(), 2);
}

#[test]
fn a_missing_file_does_not_hold_up_the_others() {
    let mut fixture = Fixture::new();
    let missing = fixture.add_local(1, b"gone", true);
    std::fs::remove_file(fixture.directory.join(format!("{missing}-photo.png")))
        .unwrap_or_else(|error| panic!("file must be removed: {error}"));
    fixture.add_local(2, b"here", true);

    for _ in 0..2 {
        fixture
            .store
            .hash_next_attachment()
            .unwrap_or_else(|error| panic!("hash must run: {error}"));
    }

    assert_eq!(fixture.shared().len(), 1);
    assert_eq!(fixture.shared()[0].content_hash, sha256(b"here"));
}

#[test]
fn chunks_are_read_by_offset_only_for_linked_files() {
    let mut fixture = Fixture::new();
    let contents = vec![5_u8; FILE_TRANSFER_CHUNK_BYTES + 10];
    fixture.add_local(1, &contents, true);
    fixture.add_local(2, b"private until linked", false);
    for _ in 0..2 {
        fixture
            .store
            .hash_next_attachment()
            .unwrap_or_else(|error| panic!("hash must run: {error}"));
    }
    let hash = sha256(&contents);
    let read = |offset| {
        fixture
            .store
            .read_chunk(&hash, offset)
            .unwrap_or_else(|error| panic!("read must work: {error}"))
    };

    let first = read(0).unwrap_or_else(|| panic!("the first chunk must exist"));
    let last = read(FILE_TRANSFER_CHUNK_BYTES as u64)
        .unwrap_or_else(|| panic!("the last chunk must exist"));

    assert_eq!(first.data().len(), FILE_TRANSFER_CHUNK_BYTES);
    assert_eq!(first.total_size(), contents.len() as u64);
    assert_eq!(last.data().len(), 10);
    assert_eq!(read(contents.len() as u64 + 1), None);
    assert_eq!(
        fixture
            .store
            .read_chunk(&sha256(b"private until linked"), 0)
            .ok()
            .flatten(),
        None
    );
}

#[test]
fn a_verified_file_is_stored_privately_with_its_metadata() {
    let mut fixture = Fixture::new();
    let attachment = incoming(b"picture");

    receive(&mut fixture, &attachment, b"picture")
        .unwrap_or_else(|error| panic!("the file must be stored: {error}"));

    let stored = fixture
        .repository
        .get_attachments_for_meeting(&MeetingId::new("meeting-1"))
        .unwrap_or_else(|error| panic!("rows must load: {error}"));
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].attachment_id(), attachment.attachment_id);
    assert_eq!(stored[0].meeting_time_ms(), Some(42_000));
    assert_eq!(stored[0].original_device_id(), "phone");
    assert_eq!(
        std::fs::read(stored[0].file_path()).ok(),
        Some(b"picture".to_vec())
    );
    assert_eq!(
        stored_files(&fixture),
        vec![format!("{}-whiteboard.png", attachment.attachment_id)]
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(stored[0].file_path())
            .map(|metadata| metadata.permissions().mode() & 0o777)
            .unwrap_or_default();
        assert_eq!(mode, 0o600);
    }
}

#[test]
fn a_project_resource_is_stored_in_its_project_folder_and_shared_back() {
    let mut fixture = Fixture::new();
    let project = fixture
        .repository
        .create_project(&ProjectName::parse("Hiring").unwrap_or_else(|error| panic!("{error}")))
        .unwrap_or_else(|error| panic!("project must be created: {error}"));
    let mut resource = incoming(b"agenda");
    resource.owner = AttachmentOwner::Project(project.project_id());
    resource.file_name = "agenda.md".to_owned();
    resource.mime_type = "text/markdown".to_owned();
    resource.meeting_time_ms = None;

    receive(&mut fixture, &resource, b"agenda")
        .unwrap_or_else(|error| panic!("the resource must be stored: {error}"));

    let stored = fixture
        .repository
        .project_resources(project.project_id())
        .unwrap_or_else(|error| panic!("resources must load: {error}"));
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].meeting_id(), None);
    assert!(
        stored[0]
            .file_path()
            .contains(&format!("project-{}", project.project_id().as_uuid()))
    );
    assert_eq!(fixture.shared(), vec![resource]);
}

#[test]
fn a_resource_of_a_project_not_here_is_refused() {
    let mut fixture = Fixture::new();
    let mut resource = incoming(b"agenda");
    resource.owner = AttachmentOwner::Project(ProjectId::new_random());

    assert_eq!(fixture.store.has_owner(&resource.owner).ok(), Some(false));
    assert!(receive(&mut fixture, &resource, b"agenda").is_err());
    assert_eq!(
        fixture.store.has_attachment(resource.attachment_id).ok(),
        Some(false)
    );
}

#[test]
fn a_deleted_resource_loses_its_file_and_is_never_fetched_again() {
    let mut fixture = Fixture::new();
    let project = fixture
        .repository
        .create_project(&ProjectName::parse("Hiring").unwrap_or_else(|error| panic!("{error}")))
        .unwrap_or_else(|error| panic!("project must be created: {error}"));
    let mut resource = incoming(b"agenda");
    resource.owner = AttachmentOwner::Project(project.project_id());
    receive(&mut fixture, &resource, b"agenda")
        .unwrap_or_else(|error| panic!("the resource must be stored: {error}"));
    let mut neighbour = incoming(b"budget");
    neighbour.attachment_id = Uuid::from_u128(0xCD);
    neighbour.owner = resource.owner.clone();
    receive(&mut fixture, &neighbour, b"budget")
        .unwrap_or_else(|error| panic!("the neighbour must be stored: {error}"));
    let project_folder = fixture.directory.join(format!(
        "attachments/project-{}",
        project.project_id().as_uuid()
    ));

    fixture
        .repository
        .delete_project_resource(resource.attachment_id)
        .unwrap_or_else(|error| panic!("the resource must be deleted: {error}"));
    fixture
        .store
        .remove_resource_file(resource.attachment_id, project.project_id())
        .unwrap_or_else(|error| panic!("the file must be removed: {error}"));

    let left: Vec<String> = std::fs::read_dir(&project_folder)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(
        left,
        vec![format!("{}-whiteboard.png", neighbour.attachment_id)]
    );
    assert_eq!(
        fixture.store.has_attachment(resource.attachment_id).ok(),
        Some(true),
        "a deleted resource counts as held, so it is not fetched again"
    );
    assert!(receive(&mut fixture, &resource, b"agenda").is_err());
}

#[test]
fn a_file_that_fails_its_check_leaves_nothing_behind() {
    let mut fixture = Fixture::new();
    let attachment = incoming(b"picture");

    assert!(receive(&mut fixture, &attachment, b"forgery").is_err());

    assert!(stored_files(&fixture).is_empty());
    assert_eq!(
        fixture.store.has_attachment(attachment.attachment_id).ok(),
        Some(false)
    );
}

#[test]
fn a_name_from_a_peer_cannot_leave_the_meeting_folder() {
    let mut fixture = Fixture::new();
    let mut attachment = incoming(b"picture");
    attachment.file_name = "../../../evil name.png".to_owned();

    receive(&mut fixture, &attachment, b"picture")
        .unwrap_or_else(|error| panic!("the file must be stored: {error}"));

    assert_eq!(
        stored_files(&fixture),
        vec![format!("{}-evil_name.png", attachment.attachment_id)]
    );
}

#[test]
fn unusable_metadata_is_refused_before_anything_is_written() {
    let mut fixture = Fixture::new();
    let mut escaping_meeting = incoming(b"x");
    escaping_meeting.owner = AttachmentOwner::Meeting(MeetingId::new("../other"));
    let mut executable = incoming(b"x");
    executable.file_name = "tool.exe".to_owned();
    let mut wrong_type = incoming(b"x");
    wrong_type.mime_type = "text/html".to_owned();

    for attachment in [escaping_meeting, executable, wrong_type] {
        assert!(fixture.store.begin_incoming(&attachment).is_err());
    }

    assert!(
        !fixture
            .directory
            .join("attachments")
            .join("meeting-1")
            .exists()
    );
    assert!(!fixture.directory.join("other").exists());
}

/// A project resource of a type the app does not show arrives as `application/octet-stream`
/// (#94); a meeting attachment of that type, or one claiming another type, is still refused.
#[test]
fn a_project_resource_of_any_type_arrives_but_a_meeting_one_does_not() {
    let mut fixture = Fixture::new();
    let project = fixture
        .repository
        .create_project(&ProjectName::parse("Site").unwrap_or_else(|error| panic!("{error}")))
        .unwrap_or_else(|error| panic!("project must be created: {error}"));
    let mut resource = incoming(b"AC1032");
    resource.owner = AttachmentOwner::Project(project.project_id());
    resource.file_name = "site.dwg".to_owned();
    resource.mime_type = "application/octet-stream".to_owned();
    resource.meeting_time_ms = None;

    receive(&mut fixture, &resource, b"AC1032")
        .unwrap_or_else(|error| panic!("the resource must be stored: {error}"));

    let mut disguised = resource.clone();
    disguised.attachment_id = Uuid::from_u128(0xAC);
    disguised.mime_type = "text/html".to_owned();
    assert!(fixture.store.begin_incoming(&disguised).is_err());
    let mut meeting_file = incoming(b"MZ");
    meeting_file.attachment_id = Uuid::from_u128(0xAD);
    meeting_file.file_name = "setup.exe".to_owned();
    meeting_file.mime_type = "application/octet-stream".to_owned();
    assert!(fixture.store.begin_incoming(&meeting_file).is_err());
}
