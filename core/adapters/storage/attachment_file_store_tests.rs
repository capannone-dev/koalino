//! Specifies how picked files and camera photos become private copies linked to their meeting
//! (Issue #54): the copy keeps the original name inside the meeting's folder, carries the meeting
//! time when given, leaves the source alone, and nothing is left behind when an import fails.
//! Each test uses unique directories under the OS temp directory and an in-memory repository.
//! Project resources (Issue #85) are copied the same way into the project's folder, and also
//! accept emails (`.eml`) and GIFs; an imported e-mail also adds its supported attachments to the
//! project.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::{
    adapters::{
        email::mail_parser_reader::MailParserEmailReader,
        storage::{
            attachment_file_store::{AttachmentFileStore, project_folder_name},
            sqlite_adapter::SqliteNoteRepository,
        },
    },
    application::{crypto::voice_encryptor::ephemeral_cipher, meeting_crdt::MeetingCrdtService},
    domain::{
        models::{DeviceId, MeetingId},
        projects::{ProjectId, ProjectName},
    },
    ports::{projects::ProjectRepository, storage::AttachmentRepository},
};

/// A temporary directory deleted when dropped.
struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    /// Creates a unique, empty directory.
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!("note-taker-{label}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path)
            .unwrap_or_else(|error| panic!("temporary directory should be created: {error}"));
        Self(path)
    }

    /// Writes `contents` to `name` inside the directory and returns its path.
    fn file(&self, name: &str, contents: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, contents)
            .unwrap_or_else(|error| panic!("fixture file should be written: {error}"));
        path
    }
}

impl Drop for TemporaryDirectory {
    /// Removes the directory tree.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The meeting every test attaches to.
fn meeting_id() -> MeetingId {
    MeetingId::new("meeting-0a6c2f1e")
}

/// Opens an in-memory repository holding `meeting_id()`.
fn repository_with_meeting() -> SqliteNoteRepository {
    let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .unwrap_or_else(|error| panic!("repository should open: {error}"));
    MeetingCrdtService::new(&repository)
        .create_meeting(&meeting_id())
        .unwrap_or_else(|error| panic!("meeting should be created: {error}"));
    repository
}

/// A store writing into a fresh attachment directory.
fn store(attachment_directory: &TemporaryDirectory) -> AttachmentFileStore {
    AttachmentFileStore::new(
        attachment_directory.0.join("attachments"),
        DeviceId::new("device-test"),
    )
}

/// Lists the files left in the meeting's attachment folder.
fn files_in_meeting_folder(attachment_directory: &TemporaryDirectory) -> Vec<PathBuf> {
    std::fs::read_dir(
        attachment_directory
            .0
            .join("attachments")
            .join(meeting_id().as_str()),
    )
    .map(|entries| {
        entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .collect()
    })
    .unwrap_or_default()
}

/// Verifies a photo is copied privately into the meeting's folder with its meeting time, and
/// the source file is left for the caller.
#[test]
fn a_photo_is_copied_privately_and_placed_at_its_meeting_time() {
    let sources = TemporaryDirectory::new("sources");
    let attachment_directory = TemporaryDirectory::new("attachments");
    let photo = sources.file("CAP0001.jpg", b"\xFF\xD8\xFFphoto");
    let repository = repository_with_meeting();

    let attachment = store(&attachment_directory)
        .import_file(&repository, &meeting_id(), &photo, Some(42_000))
        .unwrap_or_else(|error| panic!("photo should be imported: {error}"));

    assert_eq!(attachment.mime_type(), "image/jpeg");
    assert_eq!(attachment.meeting_time_ms(), Some(42_000));
    let copy = Path::new(attachment.file_path());
    assert_eq!(
        copy.parent(),
        Some(
            attachment_directory
                .0
                .join("attachments")
                .join(meeting_id().as_str())
                .as_path()
        )
    );
    assert!(copy.to_string_lossy().ends_with("-CAP0001.jpg"));
    assert_eq!(
        std::fs::read(copy).unwrap_or_default(),
        b"\xFF\xD8\xFFphoto".to_vec()
    );
    assert!(photo.exists(), "the source belongs to the caller");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(copy)
            .map(|metadata| metadata.permissions().mode())
            .unwrap_or(0o777);
        assert_eq!(mode & 0o077, 0, "attachments must be private to the user");
    }
    let stored = repository
        .get_attachments_for_meeting(&meeting_id())
        .unwrap_or_else(|error| panic!("attachments should load: {error}"));
    assert_eq!(stored, vec![attachment]);
}

/// Verifies documents of the supported kinds are accepted with their MIME types, and a file
/// added after the meeting has no meeting time (it goes at the end).
#[test]
fn documents_are_accepted_with_their_types() {
    let sources = TemporaryDirectory::new("sources");
    let attachment_directory = TemporaryDirectory::new("attachments");
    let repository = repository_with_meeting();
    let store = store(&attachment_directory);

    let mime_types: Vec<String> = [
        "Agenda.PDF",
        "notes.txt",
        "minutes.docx",
        "budget.xlsx",
        "deck.pptx",
        "board.png",
    ]
    .iter()
    .map(|name| {
        let attachment = store
            .import_file(&repository, &meeting_id(), &sources.file(name, b"x"), None)
            .unwrap_or_else(|error| panic!("{name} should be imported: {error}"));
        assert_eq!(attachment.meeting_time_ms(), None);
        attachment.mime_type().to_owned()
    })
    .collect();

    assert_eq!(
        mime_types,
        vec![
            "application/pdf",
            "text/plain",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            "image/png",
        ]
    );
}

/// Verifies unsupported files, missing files and folders are rejected without writing anything.
#[test]
fn unsupported_or_missing_files_are_rejected() {
    let sources = TemporaryDirectory::new("sources");
    let attachment_directory = TemporaryDirectory::new("attachments");
    let repository = repository_with_meeting();
    let store = store(&attachment_directory);

    for source in [
        sources.file("tool.exe", b"MZ"),
        sources.file("no_extension", b"x"),
        sources.0.join("missing.pdf"),
        sources.0.clone(),
    ] {
        assert!(
            store
                .import_file(&repository, &meeting_id(), &source, None)
                .is_err(),
            "{} should be rejected",
            source.display()
        );
    }
    assert!(files_in_meeting_folder(&attachment_directory).is_empty());
}

/// Verifies a meeting ID that is not a plain folder name cannot place a file elsewhere.
#[test]
fn unsafe_meeting_ids_are_rejected() {
    let sources = TemporaryDirectory::new("sources");
    let attachment_directory = TemporaryDirectory::new("attachments");
    let photo = sources.file("photo.jpg", b"\xFF\xD8\xFF");

    for unsafe_id in ["../outside", "a/b", "", ".."] {
        let outcome = store(&attachment_directory).import_file(
            &repository_with_meeting(),
            &MeetingId::new(unsafe_id),
            &photo,
            None,
        );
        assert!(outcome.is_err(), "{unsafe_id:?} should be rejected");
    }
    assert!(!attachment_directory.0.join("attachments").exists());
}

/// Verifies an import into an unknown meeting leaves no copy behind.
#[test]
fn an_unknown_meeting_leaves_no_copy() {
    let sources = TemporaryDirectory::new("sources");
    let attachment_directory = TemporaryDirectory::new("attachments");
    let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .unwrap_or_else(|error| panic!("repository should open: {error}"));

    let outcome = store(&attachment_directory).import_file(
        &repository,
        &meeting_id(),
        &sources.file("photo.jpg", b"\xFF\xD8\xFF"),
        None,
    );

    assert!(outcome.is_err());
    assert!(files_in_meeting_folder(&attachment_directory).is_empty());
}

/// Verifies odd characters in a file name are replaced, so the copy stays a plain name.
#[test]
fn file_names_are_reduced_to_safe_characters() {
    let sources = TemporaryDirectory::new("sources");
    let attachment_directory = TemporaryDirectory::new("attachments");
    let repository = repository_with_meeting();

    let attachment = store(&attachment_directory)
        .import_file(
            &repository,
            &meeting_id(),
            &sources.file("Q3 plan: v2 (final).pdf", b"%PDF"),
            None,
        )
        .unwrap_or_else(|error| panic!("document should be imported: {error}"));

    let file_name = Path::new(attachment.file_path())
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    assert!(
        file_name.ends_with("-Q3_plan__v2__final_.pdf"),
        "{file_name}"
    );
}

/// Opens an in-memory repository holding one project, and returns both.
fn repository_with_project() -> (SqliteNoteRepository, ProjectId) {
    let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .unwrap_or_else(|error| panic!("repository should open: {error}"));
    let project = repository
        .create_project(&ProjectName::parse("Hiring").unwrap_or_else(|error| panic!("{error}")))
        .unwrap_or_else(|error| panic!("project should be created: {error}"));
    (repository, project.project_id())
}

/// Verifies a resource is copied privately into its project's folder and listed as the project's.
#[test]
fn a_project_resource_is_copied_into_the_project_folder() {
    let sources = TemporaryDirectory::new("sources");
    let attachment_directory = TemporaryDirectory::new("attachments");
    let (repository, project_id) = repository_with_project();
    let source = sources.file("Interview notes.md", b"# Notes");

    let resource = store(&attachment_directory)
        .import_project_file(&repository, project_id, &source)
        .unwrap_or_else(|error| panic!("resource should be imported: {error}"));

    assert_eq!(resource.project_id(), Some(project_id));
    assert_eq!(resource.meeting_id(), None);
    assert_eq!(resource.mime_type(), "text/markdown");
    let copy = Path::new(resource.file_path());
    assert_eq!(
        copy.parent(),
        Some(
            attachment_directory
                .0
                .join("attachments")
                .join(project_folder_name(project_id))
                .as_path()
        )
    );
    assert_eq!(std::fs::read(copy).ok(), Some(b"# Notes".to_vec()));
    assert!(source.exists());
    assert_eq!(
        repository.project_resources(project_id).unwrap_or_default(),
        vec![resource]
    );
}

/// Verifies emails and GIFs are accepted as resources with their types.
#[test]
fn emails_and_gifs_are_accepted() {
    let sources = TemporaryDirectory::new("sources");
    let attachment_directory = TemporaryDirectory::new("attachments");
    let (repository, project_id) = repository_with_project();
    let store = store(&attachment_directory);

    let mime_types: Vec<String> = ["Offer.eml", "reaction.gif", "readme.markdown"]
        .iter()
        .map(|name| {
            store
                .import_project_file(&repository, project_id, &sources.file(name, b"x"))
                .unwrap_or_else(|error| panic!("{name} should be imported: {error}"))
                .mime_type()
                .to_owned()
        })
        .collect();

    assert_eq!(
        mime_types,
        vec!["message/rfc822", "image/gif", "text/markdown"]
    );
}

/// Verifies a resource for an unknown project leaves no copy behind.
#[test]
fn an_unknown_project_leaves_no_copy() {
    let sources = TemporaryDirectory::new("sources");
    let attachment_directory = TemporaryDirectory::new("attachments");
    let (repository, _) = repository_with_project();
    let unknown = ProjectId::new_random();

    let result = store(&attachment_directory).import_project_file(
        &repository,
        unknown,
        &sources.file("notes.md", b"x"),
    );

    assert!(result.is_err());
    let folder = attachment_directory
        .0
        .join("attachments")
        .join(project_folder_name(unknown));
    assert_eq!(
        std::fs::read_dir(folder)
            .map(|entries| entries.count())
            .unwrap_or(0),
        0
    );
}

/// An e-mail with a PDF, an executable and a nameless picture attached.
const EMAIL_WITH_FILES: &str = "From: Giacomo <giacomo@example.com>\r
Subject: Candidates\r
Content-Type: multipart/mixed; boundary=\"m\"\r
\r
--m\r
Content-Type: text/plain\r
\r
CVs attached.\r
--m\r
Content-Type: application/pdf\r
Content-Disposition: attachment; filename=\"../../cv anna.pdf\"\r
Content-Transfer-Encoding: base64\r
\r
JVBERi0xLjQK\r
--m\r
Content-Type: application/octet-stream\r
Content-Disposition: attachment; filename=\"setup.exe\"\r
\r
MZ\r
--m\r
Content-Type: image/png\r
Content-Disposition: attachment\r
Content-Transfer-Encoding: base64\r
\r
iVBORw0K\r
--m--\r
";

/// Verifies an e-mail is kept as a resource and its supported attachments become resources too.
#[test]
fn an_email_is_kept_and_its_supported_attachments_become_resources() {
    let sources = TemporaryDirectory::new("sources");
    let attachment_directory = TemporaryDirectory::new("attachments");
    let (repository, project_id) = repository_with_project();
    let source = sources.file("Candidates.eml", EMAIL_WITH_FILES.as_bytes());

    let imported = store(&attachment_directory)
        .import_project_email(
            &repository,
            &MailParserEmailReader::new(),
            project_id,
            &source,
        )
        .unwrap_or_else(|error| panic!("e-mail should be imported: {error}"));

    assert_eq!(imported.email.mime_type(), "message/rfc822");
    assert_eq!(
        std::fs::read(imported.email.file_path()).ok(),
        Some(EMAIL_WITH_FILES.as_bytes().to_vec())
    );
    let extracted: Vec<(String, &str)> = imported
        .extracted
        .iter()
        .map(|resource| {
            let name = Path::new(resource.file_path())
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let without_id = name
                .strip_prefix(&format!("{}-", resource.attachment_id()))
                .map(str::to_owned)
                .unwrap_or(name);
            (without_id, resource.mime_type())
        })
        .collect();
    assert_eq!(
        extracted,
        vec![("cv_anna.pdf".to_owned(), "application/pdf")]
    );
    assert_eq!(
        std::fs::read(imported.extracted[0].file_path()).ok(),
        Some(b"%PDF-1.4\n".to_vec())
    );
    assert_eq!(imported.skipped_attachments, 2);
    assert_eq!(
        repository
            .project_resources(project_id)
            .unwrap_or_default()
            .len(),
        2
    );
}

/// Verifies a file that is not an e-mail, or not named `.eml`, is refused and leaves nothing.
#[test]
fn a_file_that_is_not_an_email_is_refused() {
    let sources = TemporaryDirectory::new("sources");
    let attachment_directory = TemporaryDirectory::new("attachments");
    let (repository, project_id) = repository_with_project();
    let empty = sources.file("empty.eml", b"");
    let named_otherwise = sources.file("notes.md", EMAIL_WITH_FILES.as_bytes());
    let file_store = store(&attachment_directory);

    for source in [empty, named_otherwise] {
        assert!(
            file_store
                .import_project_email(
                    &repository,
                    &MailParserEmailReader::new(),
                    project_id,
                    &source
                )
                .is_err()
        );
    }
    assert!(
        repository
            .project_resources(project_id)
            .unwrap_or_default()
            .is_empty()
    );
    assert!(
        std::fs::read_dir(
            attachment_directory
                .0
                .join("attachments")
                .join(project_folder_name(project_id))
        )
        .map(|entries| entries.count())
        .unwrap_or(0)
            == 0
    );
}

/// Verifies a project takes any file, kept as `application/octet-stream` when its type is not one
/// the app shows, while a meeting still takes only pictures and documents (#94).
#[test]
fn a_project_takes_any_file_but_a_meeting_only_pictures_and_documents() {
    let sources = TemporaryDirectory::new("sources");
    let attachment_directory = TemporaryDirectory::new("attachments");
    let (repository, project_id) = repository_with_project();

    let model = store(&attachment_directory)
        .import_project_file(
            &repository,
            project_id,
            &sources.file("site.dwg", b"AC1032"),
        )
        .unwrap_or_else(|error| panic!("any file should be accepted: {error}"));
    let nameless = store(&attachment_directory)
        .import_project_file(&repository, project_id, &sources.file("Makefile", b"all:"))
        .unwrap_or_else(|error| panic!("a file without extension should be accepted: {error}"));

    assert_eq!(model.mime_type(), "application/octet-stream");
    assert_eq!(nameless.mime_type(), "application/octet-stream");
    let meeting_repository = repository_with_meeting();
    assert!(
        store(&attachment_directory)
            .import_file(
                &meeting_repository,
                &meeting_id(),
                &sources.file("setup.exe", b"MZ"),
                None
            )
            .is_err()
    );
}
