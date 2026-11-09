//! Specifies how Teams screen-share captures become image files linked to their meeting.
//!
//! Each test uses a unique directory under the OS temp directory and an in-memory repository.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::{
    adapters::storage::{
        sealed_files::is_sealed, slide_attachment_store::SlideAttachmentStore,
        sqlite_adapter::SqliteNoteRepository,
    },
    application::{crypto::voice_encryptor::ephemeral_cipher, meeting_crdt::MeetingCrdtService},
    domain::models::{
        DeviceId, ExternalMeetingDocument, MeetingDocumentKind, MeetingDocumentMediaType,
        MeetingId, SlideCapture,
    },
    ports::{
        external_bridge::AttachmentWorkflow, screen_share::MeetingSlideSink,
        storage::AttachmentRepository,
    },
};

/// A minimal PNG signature followed by filler bytes.
const PNG_BYTES: &[u8] = b"\x89PNG\r\n\x1a\nslide";

/// Deletes the temporary slide directory when dropped.
struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    /// Reserves a unique directory path (created by the store).
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("note-taker-slides-{}", Uuid::new_v4())))
    }
}

impl Drop for TemporaryDirectory {
    /// Removes the directory tree, ignoring one that was never created.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Opens an in-memory repository holding the meeting `meeting_uuid`.
fn repository_with_meeting(meeting_uuid: Uuid) -> SqliteNoteRepository {
    let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .unwrap_or_else(|error| panic!("repository should open: {error}"));
    MeetingCrdtService::new(&repository)
        .create_meeting(&MeetingId::new(meeting_uuid.to_string()))
        .unwrap_or_else(|error| panic!("meeting should be created: {error}"));
    repository
}

/// A store over `slide_directory`, sealing with the repository's key.
fn slide_store(
    slide_directory: &TemporaryDirectory,
    repository: SqliteNoteRepository,
) -> SlideAttachmentStore<SqliteNoteRepository> {
    let sealed_files = repository.sealed_files();
    SlideAttachmentStore::new(
        slide_directory.0.clone(),
        repository,
        DeviceId::new("device-test"),
        sealed_files,
    )
}

/// Verifies a capture is written to disk and linked to its meeting as a PNG attachment.
#[test]
fn slide_capture_is_saved_and_linked_to_the_meeting() {
    let slide_directory = TemporaryDirectory::new();
    let meeting_uuid = Uuid::new_v4();
    let store = slide_store(&slide_directory, repository_with_meeting(meeting_uuid));
    store
        .handle_slide_capture(SlideCapture::new(
            meeting_uuid,
            PNG_BYTES.to_vec(),
            34_000,
            "screen_share".to_owned(),
        ))
        .unwrap_or_else(|error| panic!("slide should be stored: {error}"));

    let attachments = store
        .attachment_repository()
        .get_attachments_for_meeting(&MeetingId::new(meeting_uuid.to_string()))
        .unwrap_or_else(|error| panic!("attachments should load: {error}"));
    assert_eq!(attachments.len(), 1);
    let attachment = &attachments[0];
    assert_eq!(attachment.mime_type(), "image/png");
    let saved_path = Path::new(attachment.file_path());
    assert!(saved_path.starts_with(&slide_directory.0));
    assert!(saved_path.to_string_lossy().ends_with(".png"));
    let saved_bytes = store
        .attachment_repository()
        .sealed_files()
        .read_all(saved_path)
        .unwrap_or_else(|error| panic!("slide file should exist: {error}"));
    assert_eq!(saved_bytes.as_slice(), PNG_BYTES);
    assert!(
        is_sealed(saved_path).unwrap_or(false),
        "slides are sealed (#99)"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(saved_path)
            .unwrap_or_else(|error| panic!("metadata should load: {error}"))
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0, "slides must be private to the user");
    }
}

/// Verifies JPEG captures are accepted with the right type and unknown formats are rejected.
#[test]
fn image_formats_are_detected_from_their_signature() {
    let slide_directory = TemporaryDirectory::new();
    let meeting_uuid = Uuid::new_v4();
    let store = slide_store(&slide_directory, repository_with_meeting(meeting_uuid));
    let capture = |image_bytes: &[u8]| {
        SlideCapture::new(
            meeting_uuid,
            image_bytes.to_vec(),
            1,
            "screen_share".to_owned(),
        )
    };
    store
        .handle_slide_capture(capture(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00]))
        .unwrap_or_else(|error| panic!("JPEG should be stored: {error}"));
    assert!(store.handle_slide_capture(capture(b"GIF89a")).is_err());

    let attachments = store
        .attachment_repository()
        .get_attachments_for_meeting(&MeetingId::new(meeting_uuid.to_string()))
        .unwrap_or_else(|error| panic!("attachments should load: {error}"));
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].mime_type(), "image/jpeg");
}

/// Verifies a capture for an unknown meeting leaves no orphaned file behind.
#[test]
fn capture_for_an_unknown_meeting_is_rejected_without_leaving_a_file() {
    let slide_directory = TemporaryDirectory::new();
    let store = slide_store(&slide_directory, repository_with_meeting(Uuid::new_v4()));
    let unknown_meeting = Uuid::new_v4();
    let outcome = store.handle_slide_capture(SlideCapture::new(
        unknown_meeting,
        PNG_BYTES.to_vec(),
        1,
        "screen_share".to_owned(),
    ));
    assert!(outcome.is_err());
    let meeting_directory = slide_directory.0.join(unknown_meeting.to_string());
    let leftover_files = std::fs::read_dir(&meeting_directory)
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(leftover_files, 0);
}

/// Opens an in-memory repository holding the meeting `meeting_id`.
fn repository_with_named_meeting(meeting_id: &MeetingId) -> SqliteNoteRepository {
    let repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .unwrap_or_else(|error| panic!("repository should open: {error}"));
    MeetingCrdtService::new(&repository)
        .create_meeting(meeting_id)
        .unwrap_or_else(|error| panic!("meeting should be created: {error}"));
    repository
}

/// Verifies a local screen-share slide (Issue #49) is linked to a recorded `meeting-<uuid>`
/// meeting, under a folder named after it, with its meeting time in the file name.
#[test]
fn local_slide_is_attached_to_a_recorded_meeting() {
    let slide_directory = TemporaryDirectory::new();
    let meeting_id = MeetingId::new(format!("meeting-{}", Uuid::new_v4()));
    let store = slide_store(&slide_directory, repository_with_named_meeting(&meeting_id));

    store
        .attach_slide(&meeting_id, PNG_BYTES, 6_000)
        .unwrap_or_else(|error| panic!("slide should be stored: {error}"));

    let attachments = store
        .attachment_repository()
        .get_attachments_for_meeting(&meeting_id)
        .unwrap_or_else(|error| panic!("attachments should load: {error}"));
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].meeting_time_ms(), Some(6_000));
    let saved_path = Path::new(attachments[0].file_path());
    assert_eq!(
        saved_path.parent(),
        Some(slide_directory.0.join(meeting_id.as_str()).as_path())
    );
    let file_name = saved_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    assert!(file_name.starts_with("slide-0000006000-"), "{file_name}");
}

/// Verifies a meeting ID that is not a plain folder name cannot place a file elsewhere.
#[test]
fn meeting_ids_that_are_not_plain_folder_names_are_rejected() {
    let slide_directory = TemporaryDirectory::new();
    let store = slide_store(&slide_directory, repository_with_meeting(Uuid::new_v4()));

    for unsafe_id in ["../outside", "a/b", "", ".", "..", "with space"] {
        let outcome = store.attach_slide(&MeetingId::new(unsafe_id), PNG_BYTES, 1);
        assert!(outcome.is_err(), "{unsafe_id:?} should be rejected");
    }
    assert!(!slide_directory.0.exists(), "no folder may be created");
}

/// Verifies a Teams transcript is stored sealed as a WebVTT attachment of its meeting (#102).
#[test]
fn meeting_document_is_saved_and_linked_to_the_meeting() {
    let slide_directory = TemporaryDirectory::new();
    let meeting_uuid = Uuid::new_v4();
    let store = slide_store(&slide_directory, repository_with_meeting(meeting_uuid));
    let text = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\n<v Ada>Hello</v>\n";
    store
        .handle_meeting_document(ExternalMeetingDocument::new(
            meeting_uuid,
            MeetingDocumentKind::TeamsTranscript,
            MeetingDocumentMediaType::WebVtt,
            text.to_owned(),
            0,
        ))
        .unwrap_or_else(|error| panic!("document should be stored: {error}"));

    let attachments = store
        .attachment_repository()
        .get_attachments_for_meeting(&MeetingId::new(meeting_uuid.to_string()))
        .unwrap_or_else(|error| panic!("attachments should load: {error}"));
    assert_eq!(attachments.len(), 1);
    assert_eq!(attachments[0].mime_type(), "text/vtt");
    let saved_path = Path::new(attachments[0].file_path());
    let file_name = saved_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    assert!(file_name.starts_with("teams-transcript-"), "{file_name}");
    assert!(file_name.ends_with(".vtt"), "{file_name}");
    assert!(
        is_sealed(saved_path).unwrap_or(false),
        "documents are sealed"
    );
    let saved_bytes = store
        .attachment_repository()
        .sealed_files()
        .read_all(saved_path)
        .unwrap_or_else(|error| panic!("document should be readable: {error}"));
    assert_eq!(saved_bytes.as_slice(), text.as_bytes());
}

/// Verifies a document for an unknown meeting is rejected without leaving a file.
#[test]
fn document_for_an_unknown_meeting_is_rejected_without_leaving_a_file() {
    let slide_directory = TemporaryDirectory::new();
    let store = slide_store(&slide_directory, repository_with_meeting(Uuid::new_v4()));
    let unknown_meeting = Uuid::new_v4();
    let outcome = store.handle_meeting_document(ExternalMeetingDocument::new(
        unknown_meeting,
        MeetingDocumentKind::TeamsCaptions,
        MeetingDocumentMediaType::PlainText,
        "captions".to_owned(),
        0,
    ));
    assert!(outcome.is_err());
    let leftover = std::fs::read_dir(slide_directory.0.join(unknown_meeting.to_string()))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(leftover, 0);
}
