//! Stores screen-share captures as image files and links them to their meeting.
//!
//! Implements the `AttachmentWorkflow` port (Teams bridge captures) and the `MeetingSlideSink`
//! port (local screen shares, Issue #49): each slide is written to
//! `<slide_directory>/<meeting ID>/slide-<timestamp ms>-<attachment UUID>.<png|jpg>` (directory
//! 0700, file 0600 on Unix) and recorded through the `AttachmentRepository` as an attachment of
//! the meeting, so it appears next to manually added attachments. Meeting IDs that are not plain
//! folder names are rejected, so no file lands outside the slide directory. Only PNG and JPEG images,
//! recognized by their signature, are accepted. If the attachment row cannot be saved (for
//! example, the meeting does not exist), the file is deleted again. Image bytes are never logged.

use std::path::PathBuf;

use chrono::Utc;
use uuid::Uuid;

use crate::{
    adapters::storage::private_files::{
        create_private_directory, is_plain_folder_name, write_private_file,
    },
    domain::{
        errors::StorageError,
        models::{Attachment, DeviceId, MeetingId, SlideCapture},
    },
    ports::{
        external_bridge::AttachmentWorkflow, screen_share::MeetingSlideSink,
        storage::AttachmentRepository,
    },
};

/// PNG file signature.
const PNG_SIGNATURE: &[u8] = b"\x89PNG";
/// JPEG start-of-image marker.
const JPEG_SIGNATURE: &[u8] = &[0xFF, 0xD8, 0xFF];

/// Writes slides under one directory and records them through an attachment repository.
pub struct SlideAttachmentStore<RepositoryPort> {
    slide_directory: PathBuf,
    attachment_repository: RepositoryPort,
    device_id: DeviceId,
}

impl<RepositoryPort> SlideAttachmentStore<RepositoryPort>
where
    RepositoryPort: AttachmentRepository,
{
    /// Creates a store; directories are created on the first capture.
    ///
    /// Inputs: The slide root (normally `~/.note_taker/slides`), the repository that records
    /// attachments, and this device's ID (stored as the attachment's origin).
    /// Outputs: The store.
    /// Side effects: None.
    pub fn new(
        slide_directory: PathBuf,
        attachment_repository: RepositoryPort,
        device_id: DeviceId,
    ) -> Self {
        Self {
            slide_directory,
            attachment_repository,
            device_id,
        }
    }

    /// Borrows the attachment repository (for listing stored slides).
    pub fn attachment_repository(&self) -> &RepositoryPort {
        &self.attachment_repository
    }
}

impl<RepositoryPort> AttachmentWorkflow for SlideAttachmentStore<RepositoryPort>
where
    RepositoryPort: AttachmentRepository,
{
    /// Saves the capture as a private image file and links it to its meeting.
    ///
    /// Inputs: A capture whose meeting must already exist in the repository.
    /// Outputs: Success, or a rejection for an unsupported image format, a file-system failure,
    /// or an attachment row that cannot be saved.
    /// Side effects: Creates directories and one file; writes one attachment row; removes the
    /// file again when the row cannot be saved; logs IDs only.
    fn handle_slide_capture(&self, slide_capture: SlideCapture) -> Result<(), StorageError> {
        self.store_slide(
            &MeetingId::new(slide_capture.meeting_id().to_string()),
            slide_capture.image_bytes(),
            slide_capture.timestamp_ms(),
        )
    }
}

impl<RepositoryPort> MeetingSlideSink for SlideAttachmentStore<RepositoryPort>
where
    RepositoryPort: AttachmentRepository,
{
    /// Saves a local screen-share slide (Issue #49) as a private image file of its meeting.
    ///
    /// Inputs: A recorded meeting (it must exist; its ID must be a plain folder name such as
    /// `meeting-<uuid>`), a PNG or JPEG image, and the meeting time it was shown at.
    /// Outputs: As `handle_slide_capture`; an unsafe meeting ID is rejected before any write.
    /// Side effects: As `handle_slide_capture`.
    fn attach_slide(
        &self,
        meeting_id: &MeetingId,
        encoded_image: &[u8],
        meeting_time_ms: u64,
    ) -> Result<(), StorageError> {
        self.store_slide(meeting_id, encoded_image, meeting_time_ms)
    }
}

impl<RepositoryPort> SlideAttachmentStore<RepositoryPort>
where
    RepositoryPort: AttachmentRepository,
{
    /// Writes one slide under `<slide_directory>/<meeting ID>/` and links it to the meeting.
    ///
    /// Inputs: The meeting, the image bytes, and the meeting time it was shown at (used in the
    /// file name and as the attachment's meeting time, so it sits at that point of the
    /// transcript; Issue #54).
    /// Outputs: Success, or a rejection for an unsafe meeting ID, an unsupported image format, a
    /// file-system failure, or an attachment row that cannot be saved.
    /// Side effects: Creates directories and one file; writes one attachment row; removes the
    /// file again when the row cannot be saved; logs IDs only.
    fn store_slide(
        &self,
        meeting_id: &MeetingId,
        image_bytes: &[u8],
        timestamp_ms: u64,
    ) -> Result<(), StorageError> {
        if !is_plain_folder_name(meeting_id.as_str()) {
            return Err(StorageError::Rejected {
                reason: String::from("meeting ID is not a valid slide folder name"),
            });
        }
        let (mime_type, extension) = detect_image_format(image_bytes)?;
        let attachment_id = Uuid::new_v4();
        let meeting_directory = self.slide_directory.join(meeting_id.as_str());
        create_private_directory(&meeting_directory)?;
        let slide_path = meeting_directory.join(format!(
            "slide-{timestamp_ms:010}-{attachment_id}.{extension}"
        ));
        write_private_file(&slide_path, &mut &image_bytes[..])?;

        let attachment = Attachment::new(
            attachment_id,
            Some(meeting_id.clone()),
            slide_path.to_string_lossy().into_owned(),
            mime_type,
            Utc::now(),
            self.device_id.as_str(),
        )
        .with_meeting_time_ms(timestamp_ms);
        if let Err(error) = self.attachment_repository.save_attachment(&attachment) {
            if let Err(removal_error) = std::fs::remove_file(&slide_path) {
                tracing::warn!(%attachment_id, error = %removal_error, "Unlinked slide file could not be removed");
            }
            return Err(error);
        }
        tracing::info!(
            meeting_id = %meeting_id.as_str(),
            %attachment_id,
            timestamp_ms,
            "Slide capture attached to meeting"
        );
        Ok(())
    }
}

/// Recognizes PNG and JPEG images by their signature.
///
/// Outputs: The MIME type and file extension, or a rejection for any other format.
fn detect_image_format(image_bytes: &[u8]) -> Result<(&'static str, &'static str), StorageError> {
    if image_bytes.starts_with(PNG_SIGNATURE) {
        Ok(("image/png", "png"))
    } else if image_bytes.starts_with(JPEG_SIGNATURE) {
        Ok(("image/jpeg", "jpg"))
    } else {
        Err(StorageError::Rejected {
            reason: String::from("slide capture is not a PNG or JPEG image"),
        })
    }
}
