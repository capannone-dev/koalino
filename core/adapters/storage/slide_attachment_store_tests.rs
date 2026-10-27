//! Specifies how Teams screen-share captures become image files linked to their meeting.
//!
//! Each test uses a unique directory under the OS temp directory and an in-memory repository.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::{
    adapters::storage::{
        slide_attachment_store::SlideAttachmentStore, sqlite_adapter::SqliteNoteRepository,
    },
    application::{crypto::voice_encryptor::ephemeral_cipher, meeting_crdt::MeetingCrdtService},
    domain::models::{DeviceId, MeetingId, SlideCapture},
    ports::{external_bridge::AttachmentWorkflow, storage::AttachmentRepository},
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

/// Verifies a capture is written to disk and linked to its meeting as a PNG attachment.
#[test]
fn slide_capture_is_saved_and_linked_to_the_meeting() {
    let slide_directory = TemporaryDirectory::new();
    let meeting_uuid = Uuid::new_v4();
    let store = SlideAttachmentStore::new(
        slide_directory.0.clone(),
        repository_with_meeting(meeting_uuid),
        DeviceId::new("device-test"),
    );
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
    let saved_bytes = std::fs::read(saved_path)
        .unwrap_or_else(|error| panic!("slide file should exist: {error}"));
    assert_eq!(saved_bytes, PNG_BYTES);
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
    let store = SlideAttachmentStore::new(
        slide_directory.0.clone(),
        repository_with_meeting(meeting_uuid),
        DeviceId::new("device-test"),
    );
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
    let store = SlideAttachmentStore::new(
        slide_directory.0.clone(),
        repository_with_meeting(Uuid::new_v4()),
        DeviceId::new("device-test"),
    );
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
