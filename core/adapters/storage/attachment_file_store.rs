//! Copies picked files and camera photos privately into a meeting's attachment folder (Issue #54).
//!
//! Picked files can live anywhere: Android hands out cache copies that the system may clear, and
//! camera photos land in a temporary folder. So each import copies the file to
//! `<attachment_directory>/<meeting ID>/<attachment UUID>-<original name>` (folder 0700, file
//! 0600 on Unix) and links the copy to the meeting through the `AttachmentRepository`, with the
//! meeting time when the attachment belongs at a point of the transcript. The source stays where
//! it was, and its owner deletes it. Only pictures and common document types are accepted, up to
//! `MAX_ATTACHMENT_BYTES`. If the row cannot be saved, the copy is removed again. File contents
//! and names are never logged.

use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use chrono::Utc;
use uuid::Uuid;

use crate::{
    adapters::storage::private_files::{
        create_private_directory, file_error, is_plain_folder_name, write_private_file,
    },
    domain::{
        errors::StorageError,
        models::{Attachment, DeviceId, MeetingId},
    },
    ports::storage::AttachmentRepository,
};

/// Largest file accepted (100 MiB): photos and documents, never recordings.
const MAX_ATTACHMENT_BYTES: u64 = 100 * 1024 * 1024;
/// Longest kept part of the original file name, in characters.
const MAX_KEPT_NAME_CHARACTERS: usize = 80;

/// Copies attachments under one directory, recording this device as their origin.
#[derive(Clone, Debug)]
pub struct AttachmentFileStore {
    attachment_directory: PathBuf,
    device_id: DeviceId,
}

impl AttachmentFileStore {
    /// Creates a store; folders are created on the first import.
    ///
    /// Inputs: The attachment root (normally `<data directory>/attachments`) and this device's
    /// ID. Outputs: The store. Side effects: None.
    pub fn new(attachment_directory: PathBuf, device_id: DeviceId) -> Self {
        Self {
            attachment_directory,
            device_id,
        }
    }

    /// Copies one file into the meeting's folder and links it to the meeting.
    ///
    /// Inputs: The repository holding the meeting, the meeting (whose ID must be a plain folder
    /// name such as `meeting-<uuid>`), the source file, and the meeting time it belongs at
    /// (`None` places it at the end of the meeting).
    /// Outputs: The stored attachment, or a rejection for an unsafe meeting ID, an unsupported
    /// type, a missing, oversized or unreadable source, or a row that cannot be saved (for
    /// example, an unknown meeting).
    /// Side effects: Creates folders and one file; writes one attachment row; removes the copy
    /// again when the row cannot be saved. Logs IDs only.
    pub fn import_file<RepositoryPort>(
        &self,
        attachment_repository: &RepositoryPort,
        meeting_id: &MeetingId,
        source_path: &Path,
        meeting_time_ms: Option<u64>,
    ) -> Result<Attachment, StorageError>
    where
        RepositoryPort: AttachmentRepository + ?Sized,
    {
        if !is_plain_folder_name(meeting_id.as_str()) {
            return Err(StorageError::Rejected {
                reason: String::from("meeting ID is not a valid attachment folder name"),
            });
        }
        let mime_type = supported_mime_type(source_path)?;
        let mut source_file = open_regular_file(source_path)?;
        let attachment_id = Uuid::new_v4();
        let meeting_directory = self.attachment_directory.join(meeting_id.as_str());
        create_private_directory(&meeting_directory)?;
        let copy_path =
            meeting_directory.join(format!("{attachment_id}-{}", kept_file_name(source_path)));
        // Reading one byte past the limit tells an oversized file from one exactly at it.
        let copied_bytes = write_private_file(
            &copy_path,
            &mut (&mut source_file).take(MAX_ATTACHMENT_BYTES + 1),
        )?;
        if copied_bytes > MAX_ATTACHMENT_BYTES {
            remove_copy(&copy_path, attachment_id);
            return Err(too_large());
        }

        let attachment = Attachment::new(
            attachment_id,
            Some(meeting_id.clone()),
            copy_path.to_string_lossy().into_owned(),
            mime_type,
            Utc::now(),
            self.device_id.as_str(),
        );
        let attachment = match meeting_time_ms {
            Some(meeting_time_ms) => attachment.with_meeting_time_ms(meeting_time_ms),
            None => attachment,
        };
        if let Err(error) = attachment_repository.save_attachment(&attachment) {
            remove_copy(&copy_path, attachment_id);
            return Err(error);
        }
        tracing::info!(
            meeting_id = %meeting_id.as_str(),
            %attachment_id,
            mime_type,
            has_meeting_time = meeting_time_ms.is_some(),
            "File attached to meeting"
        );
        Ok(attachment)
    }
}

/// Names the folder that holds a project's resource files (Issue #85): `project-<uuid>`.
///
/// Inputs: The project. Outputs: A plain folder name. Side effects: None.
pub fn project_folder_name(project_id: crate::domain::projects::ProjectId) -> String {
    format!("project-{project_id}")
}

/// Maps a supported file extension (any case) to its MIME type.
///
/// Inputs: The source path. Outputs: The MIME type, or a rejection for any other extension.
/// Side effects: None.
pub(super) fn supported_mime_type(source_path: &Path) -> Result<&'static str, StorageError> {
    let extension = source_path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("jpg" | "jpeg") => Ok("image/jpeg"),
        Some("png") => Ok("image/png"),
        Some("webp") => Ok("image/webp"),
        Some("heic") => Ok("image/heic"),
        Some("pdf") => Ok("application/pdf"),
        Some("txt") => Ok("text/plain"),
        Some("md") => Ok("text/markdown"),
        Some("docx") => {
            Ok("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        }
        Some("xlsx") => Ok("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        Some("pptx") => {
            Ok("application/vnd.openxmlformats-officedocument.presentationml.presentation")
        }
        Some("odt") => Ok("application/vnd.oasis.opendocument.text"),
        _ => Err(StorageError::Rejected {
            reason: String::from("only pictures and documents can be attached"),
        }),
    }
}

/// Opens a regular file no larger than `MAX_ATTACHMENT_BYTES`.
///
/// Inputs: The source path. Outputs: The open file, or a rejection for a missing path, a
/// folder, or an oversized file. Side effects: Opens the file for reading.
fn open_regular_file(source_path: &Path) -> Result<File, StorageError> {
    let source_file = File::open(source_path)
        .map_err(|error| file_error("unable to open the file to attach", &error))?;
    let metadata = source_file
        .metadata()
        .map_err(|error| file_error("unable to read the file to attach", &error))?;
    if !metadata.is_file() {
        return Err(StorageError::Rejected {
            reason: String::from("only files can be attached"),
        });
    }
    if metadata.len() > MAX_ATTACHMENT_BYTES {
        return Err(too_large());
    }
    Ok(source_file)
}

/// Reduces the source's file name to ASCII letters, digits, `.`, `-` and `_` (others become
/// `_`), keeping at most `MAX_KEPT_NAME_CHARACTERS` characters from the end, so the extension
/// survives.
///
/// Inputs: The source path. Outputs: A plain file name. Side effects: None.
pub(super) fn kept_file_name(source_path: &Path) -> String {
    let safe_name: Vec<char> = source_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    let kept_from = safe_name.len().saturating_sub(MAX_KEPT_NAME_CHARACTERS);
    safe_name[kept_from..].iter().collect()
}

/// Deletes a copy that will not be linked.
///
/// Side effects: Removes the file; a failure is logged with the attachment ID only.
fn remove_copy(copy_path: &Path, attachment_id: Uuid) {
    if let Err(error) = std::fs::remove_file(copy_path) {
        tracing::warn!(%attachment_id, error = %error, "Unlinked attachment copy could not be removed");
    }
}

/// The rejection for a file above `MAX_ATTACHMENT_BYTES`.
fn too_large() -> StorageError {
    StorageError::Rejected {
        reason: format!(
            "files larger than {} MiB cannot be attached",
            MAX_ATTACHMENT_BYTES / 1024 / 1024
        ),
    }
}
