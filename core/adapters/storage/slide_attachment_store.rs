//! Stores Teams screen-share captures as image files and links them to their meeting.
//!
//! Implements the `AttachmentWorkflow` port: each `SlideCapture` is written to
//! `<slide_directory>/<meeting UUID>/slide-<timestamp ms>-<attachment UUID>.<png|jpg>` (directory
//! 0700, file 0600 on Unix) and recorded through the `AttachmentRepository` as an attachment of
//! the meeting, so it appears next to manually added attachments. Only PNG and JPEG images,
//! recognized by their signature, are accepted. If the attachment row cannot be saved (for
//! example, the meeting does not exist), the file is deleted again. Image bytes are never logged.

use std::{
    fs::{DirBuilder, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use chrono::Utc;
use uuid::Uuid;

use crate::{
    domain::{
        errors::StorageError,
        models::{Attachment, DeviceId, MeetingId, SlideCapture},
    },
    ports::{external_bridge::AttachmentWorkflow, storage::AttachmentRepository},
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
        let (mime_type, extension) = detect_image_format(slide_capture.image_bytes())?;
        let attachment_id = Uuid::new_v4();
        let meeting_directory = self
            .slide_directory
            .join(slide_capture.meeting_id().to_string());
        create_private_directory(&meeting_directory)?;
        let slide_path = meeting_directory.join(format!(
            "slide-{:010}-{attachment_id}.{extension}",
            slide_capture.timestamp_ms()
        ));
        write_private_file(&slide_path, slide_capture.image_bytes())?;

        let meeting_id = MeetingId::new(slide_capture.meeting_id().to_string());
        let attachment = Attachment::new(
            attachment_id,
            Some(meeting_id.clone()),
            slide_path.to_string_lossy().into_owned(),
            mime_type,
            Utc::now(),
            self.device_id.as_str(),
        );
        if let Err(error) = self.attachment_repository.save_attachment(&attachment) {
            if let Err(removal_error) = std::fs::remove_file(&slide_path) {
                tracing::warn!(%attachment_id, error = %removal_error, "Unlinked slide file could not be removed");
            }
            return Err(error);
        }
        tracing::info!(
            meeting_id = %meeting_id.as_str(),
            %attachment_id,
            timestamp_ms = slide_capture.timestamp_ms(),
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

/// Creates `directory` (and parents) readable only by the user on Unix.
fn create_private_directory(directory: &Path) -> Result<(), StorageError> {
    let mut directory_builder = DirBuilder::new();
    directory_builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut directory_builder, 0o700);
    directory_builder
        .create(directory)
        .map_err(|error| file_error("unable to create the slide directory", &error))
}

/// Writes a new file readable only by the user on Unix; never overwrites.
fn write_private_file(path: &Path, contents: &[u8]) -> Result<(), StorageError> {
    let mut open_options = OpenOptions::new();
    open_options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut open_options, 0o600);
    let mut file: File = open_options
        .open(path)
        .map_err(|error| file_error("unable to create the slide file", &error))?;
    file.write_all(contents)
        .and_then(|()| file.sync_all())
        .map_err(|error| file_error("unable to write the slide file", &error))
}

/// Builds a storage rejection for a file-system failure.
fn file_error(operation: &str, error: &std::io::Error) -> StorageError {
    StorageError::Rejected {
        reason: format!("{operation}: {error}"),
    }
}
