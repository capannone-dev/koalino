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
//!
//! Project resources (Issue #85) go to `<attachment_directory>/project-<uuid>/` instead. An
//! imported e-mail is kept as it is, and each of its attachments of a supported type becomes a
//! resource of the same project; the others are counted and left inside the e-mail.

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
        email::MAX_EMAIL_BYTES,
        errors::StorageError,
        models::{Attachment, DeviceId, MeetingId},
        projects::ProjectId,
    },
    ports::{email::EmailReader, storage::AttachmentRepository},
};

/// What importing an e-mail into a project stored.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectEmailImport {
    /// The `.eml` file, kept as a resource.
    pub email: Attachment,
    /// Its attachments that became resources, in the e-mail's order.
    pub extracted: Vec<Attachment>,
    /// Attachments left out because their type is not accepted or they could not be stored.
    pub skipped_attachments: usize,
}

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
        let attachment = self.copy_and_link(
            attachment_repository,
            meeting_id.as_str(),
            source_path,
            |attachment| {
                let attachment = Attachment::new(
                    attachment.attachment_id(),
                    Some(meeting_id.clone()),
                    attachment.file_path(),
                    attachment.mime_type(),
                    attachment.original_timestamp(),
                    attachment.original_device_id(),
                );
                match meeting_time_ms {
                    Some(meeting_time_ms) => attachment.with_meeting_time_ms(meeting_time_ms),
                    None => attachment,
                }
            },
        )?;
        tracing::info!(
            meeting_id = %meeting_id.as_str(),
            attachment_id = %attachment.attachment_id(),
            mime_type = attachment.mime_type(),
            has_meeting_time = meeting_time_ms.is_some(),
            "File attached to meeting"
        );
        Ok(attachment)
    }

    /// Copies one file into a project's folder and links it to the project as a resource
    /// (Issue #85).
    ///
    /// Inputs: The repository holding the project, the project, and the source file.
    /// Outputs: The stored resource, or a rejection for an unsupported type, a missing,
    /// oversized or unreadable source, or a row that cannot be saved (for example, an unknown
    /// project).
    /// Side effects: Creates `<attachment_directory>/project-<uuid>/` and one file; writes one
    /// attachment row; removes the copy again when the row cannot be saved. Logs IDs only.
    pub fn import_project_file<RepositoryPort>(
        &self,
        attachment_repository: &RepositoryPort,
        project_id: ProjectId,
        source_path: &Path,
    ) -> Result<Attachment, StorageError>
    where
        RepositoryPort: AttachmentRepository + ?Sized,
    {
        let resource = self.copy_and_link(
            attachment_repository,
            &project_folder_name(project_id),
            source_path,
            |attachment| attachment.with_project_id(project_id),
        )?;
        tracing::info!(
            %project_id,
            attachment_id = %resource.attachment_id(),
            mime_type = resource.mime_type(),
            "File added to project"
        );
        Ok(resource)
    }

    /// Keeps an `.eml` e-mail as a project resource and adds each of its attachments of a
    /// supported type as a resource of the same project (Issue #85).
    ///
    /// Inputs: The repository holding the project, the e-mail reader, the project, and the
    /// source `.eml` file.
    /// Outputs: What was stored, or a rejection for a file not named `.eml`, larger than
    /// `MAX_EMAIL_BYTES`, unreadable, not an e-mail, or whose row cannot be saved. Once the e-mail
    /// is stored, an attachment that cannot be stored is skipped, never an error.
    /// Side effects: Creates the project's folder and files; writes rows. Logs IDs and counts
    /// only.
    pub fn import_project_email<RepositoryPort>(
        &self,
        attachment_repository: &RepositoryPort,
        email_reader: &dyn EmailReader,
        project_id: ProjectId,
        source_path: &Path,
    ) -> Result<ProjectEmailImport, StorageError>
    where
        RepositoryPort: AttachmentRepository + ?Sized,
    {
        if supported_mime_type(source_path)? != "message/rfc822" {
            return Err(StorageError::Rejected {
                reason: String::from("only .eml files can be imported as e-mails"),
            });
        }
        let mut raw_email = Vec::new();
        let read_limit = u64::try_from(MAX_EMAIL_BYTES).unwrap_or(u64::MAX);
        (&mut open_regular_file(source_path)?)
            .take(read_limit + 1)
            .read_to_end(&mut raw_email)
            .map_err(|error| file_error("unable to read the e-mail", &error))?;
        self.import_project_email_contents(
            attachment_repository,
            email_reader,
            project_id,
            source_path,
            &raw_email,
        )
    }

    /// Adds received contents, such as a web upload, to a project: an `.eml` name goes through
    /// `import_project_email_contents`, any other accepted type is stored as one resource.
    ///
    /// Inputs: The repository, the e-mail reader, the project, the name the file is known by
    /// (its extension gives the type; folders in it are ignored), and its contents.
    /// Outputs: What was stored (`extracted` empty and no skips for a plain file), or a
    /// rejection as for `import_project_file` and `import_project_email`.
    /// Side effects: Creates the project's folder and files; writes rows. Logs IDs only.
    pub fn import_project_contents<RepositoryPort>(
        &self,
        attachment_repository: &RepositoryPort,
        email_reader: &dyn EmailReader,
        project_id: ProjectId,
        named_as: &Path,
        contents: &[u8],
    ) -> Result<ProjectEmailImport, StorageError>
    where
        RepositoryPort: AttachmentRepository + ?Sized,
    {
        let named_as = named_as.file_name().map(PathBuf::from).unwrap_or_default();
        if supported_mime_type(&named_as)? == "message/rfc822" {
            return self.import_project_email_contents(
                attachment_repository,
                email_reader,
                project_id,
                &named_as,
                contents,
            );
        }
        let resource = self.store_and_link(
            attachment_repository,
            &project_folder_name(project_id),
            &named_as,
            &mut &contents[..],
            |attachment| attachment.with_project_id(project_id),
        )?;
        tracing::info!(
            %project_id,
            attachment_id = %resource.attachment_id(),
            mime_type = resource.mime_type(),
            "File added to project"
        );
        Ok(ProjectEmailImport {
            email: resource,
            extracted: Vec::new(),
            skipped_attachments: 0,
        })
    }

    /// Writes a note typed in the app into a project as a Markdown resource (Issue #85).
    ///
    /// Inputs: The repository, the project, the title (it names the file and heads the note) and
    /// the text. Outputs: The stored resource, or a rejection for a blank or multi-line title or
    /// a row that cannot be saved. Side effects: Creates one `.md` file and one row. Never logs
    /// the title or text.
    pub fn import_project_note<RepositoryPort>(
        &self,
        attachment_repository: &RepositoryPort,
        project_id: ProjectId,
        title: &str,
        text: &str,
    ) -> Result<Attachment, StorageError>
    where
        RepositoryPort: AttachmentRepository + ?Sized,
    {
        let title = title.trim();
        if title.is_empty() || title.chars().any(char::is_control) {
            return Err(StorageError::Rejected {
                reason: String::from("a note needs a one-line title"),
            });
        }
        let markdown = format!("# {title}\n\n{}\n", text.trim_end());
        let resource = self.store_and_link(
            attachment_repository,
            &project_folder_name(project_id),
            &PathBuf::from(format!("{title}.md")),
            &mut markdown.as_bytes(),
            |attachment| attachment.with_project_id(project_id),
        )?;
        tracing::info!(
            %project_id,
            attachment_id = %resource.attachment_id(),
            "Note added to project"
        );
        Ok(resource)
    }

    /// Keeps e-mail contents as a project resource named after `source_path` and adds its
    /// attachments of an accepted type, as `import_project_email` describes.
    ///
    /// Outputs: What was stored, or a rejection for contents that are too large, not an e-mail,
    /// or a row that cannot be saved. Side effects: Creates files; writes rows.
    fn import_project_email_contents<RepositoryPort>(
        &self,
        attachment_repository: &RepositoryPort,
        email_reader: &dyn EmailReader,
        project_id: ProjectId,
        source_path: &Path,
        raw_email: &[u8],
    ) -> Result<ProjectEmailImport, StorageError>
    where
        RepositoryPort: AttachmentRepository + ?Sized,
    {
        let email = email_reader
            .read_email(raw_email)
            .map_err(|error| StorageError::Rejected {
                reason: error.to_string(),
            })?;
        let folder_name = project_folder_name(project_id);
        let owned_by_project = |attachment: Attachment| attachment.with_project_id(project_id);
        let stored_email = self.store_and_link(
            attachment_repository,
            &folder_name,
            source_path,
            &mut &raw_email[..],
            owned_by_project,
        )?;
        let mut extracted = Vec::new();
        let mut skipped_attachments = 0;
        for attachment in &email.attachments {
            // Only the last part of the sender's name is kept, so it cannot name a folder.
            let attachment_name = Path::new(&attachment.file_name)
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_default();
            let stored = supported_mime_type(&attachment_name).and_then(|_| {
                self.store_and_link(
                    attachment_repository,
                    &folder_name,
                    &attachment_name,
                    &mut attachment.contents.as_slice(),
                    owned_by_project,
                )
            });
            match stored {
                Ok(resource) => extracted.push(resource),
                Err(_) => skipped_attachments += 1,
            }
        }
        tracing::info!(
            %project_id,
            attachment_id = %stored_email.attachment_id(),
            extracted_count = extracted.len(),
            skipped_count = skipped_attachments,
            "E-mail added to project"
        );
        Ok(ProjectEmailImport {
            email: stored_email,
            extracted,
            skipped_attachments,
        })
    }

    /// Copies `source_path` into `<attachment_directory>/<folder_name>/` and saves the row that
    /// `owned_by` builds from an unowned attachment describing the copy.
    ///
    /// Outputs: The saved attachment, or a rejection. Side effects: Creates folders and one
    /// file, writes one row, and removes the copy again when the row cannot be saved.
    fn copy_and_link<RepositoryPort>(
        &self,
        attachment_repository: &RepositoryPort,
        folder_name: &str,
        source_path: &Path,
        owned_by: impl FnOnce(Attachment) -> Attachment,
    ) -> Result<Attachment, StorageError>
    where
        RepositoryPort: AttachmentRepository + ?Sized,
    {
        supported_mime_type(source_path)?;
        let mut source_file = open_regular_file(source_path)?;
        self.store_and_link(
            attachment_repository,
            folder_name,
            source_path,
            &mut source_file,
            owned_by,
        )
    }

    /// Writes `contents` into `<attachment_directory>/<folder_name>/<new ID>-<safe name>` and
    /// saves the row that `owned_by` builds from an unowned attachment describing the file.
    ///
    /// Inputs: The repository, the owner's folder, the name the file is known by (its extension
    /// gives the type), its contents, and the owner.
    /// Outputs: The saved attachment, or a rejection for an unsupported type, contents above
    /// `MAX_ATTACHMENT_BYTES`, or a row that cannot be saved.
    /// Side effects: Creates folders and one file, writes one row, and removes the file again
    /// on failure.
    fn store_and_link<RepositoryPort>(
        &self,
        attachment_repository: &RepositoryPort,
        folder_name: &str,
        named_as: &Path,
        contents: &mut impl Read,
        owned_by: impl FnOnce(Attachment) -> Attachment,
    ) -> Result<Attachment, StorageError>
    where
        RepositoryPort: AttachmentRepository + ?Sized,
    {
        let mime_type = supported_mime_type(named_as)?;
        let attachment_id = Uuid::new_v4();
        let owner_directory = self.attachment_directory.join(folder_name);
        create_private_directory(&owner_directory)?;
        let copy_path =
            owner_directory.join(format!("{attachment_id}-{}", kept_file_name(named_as)));
        // Reading one byte past the limit tells an oversized file from one exactly at it.
        let copied_bytes =
            write_private_file(&copy_path, &mut contents.take(MAX_ATTACHMENT_BYTES + 1))?;
        if copied_bytes > MAX_ATTACHMENT_BYTES {
            remove_copy(&copy_path, attachment_id);
            return Err(too_large());
        }
        let attachment = owned_by(Attachment::new(
            attachment_id,
            None,
            copy_path.to_string_lossy().into_owned(),
            mime_type,
            Utc::now(),
            self.device_id.as_str(),
        ));
        if let Err(error) = attachment_repository.save_attachment(&attachment) {
            remove_copy(&copy_path, attachment_id);
            return Err(error);
        }
        Ok(attachment)
    }
}

/// Names the folder that holds a project's resource files (Issue #85): `project-<uuid>`.
///
/// Inputs: The project. Outputs: A plain folder name. Side effects: None.
pub fn project_folder_name(project_id: ProjectId) -> String {
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
        Some("gif") => Ok("image/gif"),
        Some("webp") => Ok("image/webp"),
        Some("heic") => Ok("image/heic"),
        Some("pdf") => Ok("application/pdf"),
        Some("txt") => Ok("text/plain"),
        Some("md" | "markdown") => Ok("text/markdown"),
        Some("eml") => Ok("message/rfc822"),
        Some("docx") => {
            Ok("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        }
        Some("xlsx") => Ok("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        Some("pptx") => {
            Ok("application/vnd.openxmlformats-officedocument.presentationml.presentation")
        }
        Some("odt") => Ok("application/vnd.oasis.opendocument.text"),
        _ => Err(StorageError::Rejected {
            reason: String::from("only pictures, documents and emails can be attached"),
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
