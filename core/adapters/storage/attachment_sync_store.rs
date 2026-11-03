//! Reads and stores attachment files for sync with paired devices (Issue #57).
//!
//! A file from a paired device is written to a private partial file inside its meeting's folder,
//! then checked against the size and SHA-256 its metadata names, and only then moved to
//! `<attachment directory>/<meeting ID>/<attachment ID>-<safe name>` with its row written. The
//! name, type and meeting ID are peer input: the name is reduced to safe characters, the type must
//! match the extension among those that can be attached locally, and the meeting ID must be a plain
//! folder name. Serving reads only files of attachments linked to a meeting. Logs carry IDs only.

use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    attachment_file_store::{kept_file_name, supported_mime_type},
    private_files::{
        create_private_directory, file_error, is_plain_folder_name, remove_meeting_folder,
    },
    sqlite_adapter::SqliteNoteRepository,
};
use crate::{
    domain::{
        errors::StorageError,
        models::{AttachmentOwner, FileChunk, MeetingId, SharedAttachment},
    },
    ports::file_transfer::{AttachmentLibrary, FILE_TRANSFER_CHUNK_BYTES, MAX_FILE_TRANSFER_BYTES},
};

/// The P2P thread's view of attachments: its own database connection and the attachment folder.
pub struct AttachmentSyncStore {
    repository: SqliteNoteRepository,
    attachment_directory: PathBuf,
    /// Rows whose file could not be hashed, skipped until the next start so they don't hold up
    /// the others.
    unhashable: HashSet<String>,
}

impl AttachmentSyncStore {
    /// Creates the store.
    ///
    /// Inputs: A database connection and the folder attachments are kept in.
    /// Outputs: The store. Side effects: None; folders are created when a file arrives.
    pub fn new(repository: SqliteNoteRepository, attachment_directory: PathBuf) -> Self {
        Self {
            repository,
            attachment_directory,
            unhashable: HashSet::new(),
        }
    }

    /// The folder of a meeting or project, after checking a meeting ID cannot reach outside the
    /// attachment folder. A project's folder is `project-<uuid>`, always a plain name.
    ///
    /// Outputs: The path, or a rejection. Side effects: None.
    fn owner_directory(&self, owner: &AttachmentOwner) -> Result<PathBuf, StorageError> {
        match owner {
            AttachmentOwner::Meeting(meeting_id) => {
                if !is_plain_folder_name(meeting_id.as_str()) {
                    return Err(StorageError::Rejected {
                        reason: String::from("an attachment names an unusable meeting ID"),
                    });
                }
                Ok(self.attachment_directory.join(meeting_id.as_str()))
            }
            AttachmentOwner::Project(project_id) => Ok(self.attachment_directory.join(
                super::attachment_file_store::project_folder_name(*project_id),
            )),
        }
    }

    /// The partial file of an attachment. Outputs: The path, or a rejection. Side effects: None.
    fn partial_path(&self, attachment: &SharedAttachment) -> Result<PathBuf, StorageError> {
        Ok(self
            .owner_directory(&attachment.owner)?
            .join(format!(".{}.part", attachment.attachment_id)))
    }
}

/// Computes a file's SHA-256 and length without loading it.
///
/// Inputs: The open file, at its start. Outputs: The lowercase hexadecimal hash and the length.
/// Side effects: Reads the file.
fn hash_file(file: &mut File) -> std::io::Result<(String, u64)> {
    let mut hasher = Sha256::new();
    let size_bytes = std::io::copy(file, &mut hasher)?;
    Ok((format!("{:x}", hasher.finalize()), size_bytes))
}

impl AttachmentLibrary for AttachmentSyncStore {
    /// Removes the meeting's folder in the attachment store.
    ///
    /// Inputs: The deleted meeting.
    /// Outputs: Success (also when it had no folder), or a storage error.
    /// Side effects: Deletes the folder and its files, which never logs their names.
    fn remove_meeting_files(&mut self, meeting_id: &MeetingId) -> Result<(), StorageError> {
        remove_meeting_folder(&self.attachment_directory, meeting_id.as_str())
            .map_err(|error| file_error("unable to remove a deleted meeting's files", &error))
    }

    /// Removes the project's folder in the attachment store (Issue #85).
    ///
    /// Inputs: The deleted project.
    /// Outputs: Success (also when it had no folder), or a storage error.
    /// Side effects: Deletes the folder and its files, which never logs their names.
    fn remove_project_files(
        &mut self,
        project_id: crate::domain::projects::ProjectId,
    ) -> Result<(), StorageError> {
        remove_meeting_folder(
            &self.attachment_directory,
            &super::attachment_file_store::project_folder_name(project_id),
        )
        .map_err(|error| file_error("unable to remove a deleted project's files", &error))
    }

    fn hash_next_attachment(&mut self) -> Result<(), StorageError> {
        let Some((attachment_id, file_path)) = self
            .repository
            .attachments_awaiting_hash()?
            .into_iter()
            .find(|(attachment_id, _)| !self.unhashable.contains(attachment_id))
        else {
            return Ok(());
        };
        let hashed = File::open(&file_path).and_then(|mut file| hash_file(&mut file));
        match hashed {
            Ok((content_hash, size_bytes)) if size_bytes <= MAX_FILE_TRANSFER_BYTES => self
                .repository
                .record_attachment_hash(&attachment_id, &content_hash, size_bytes),
            Ok(_) => {
                self.unhashable.insert(attachment_id);
                Ok(())
            }
            Err(error) => {
                tracing::warn!(%attachment_id, error = %error, "an attachment file could not be read");
                self.unhashable.insert(attachment_id);
                Ok(())
            }
        }
    }

    fn shared_attachments(&self) -> Result<Vec<SharedAttachment>, StorageError> {
        self.repository.shared_attachment_rows()
    }

    fn has_attachment(&self, attachment_id: Uuid) -> Result<bool, StorageError> {
        self.repository.attachment_exists(attachment_id)
    }

    fn has_owner(&self, owner: &AttachmentOwner) -> Result<bool, StorageError> {
        self.repository.attachment_owner_exists(owner)
    }

    fn read_chunk(&self, hash: &str, offset: u64) -> Result<Option<FileChunk>, StorageError> {
        let Some(file_path) = self.repository.shared_attachment_path(hash)? else {
            return Ok(None);
        };
        let mut file = match File::open(&file_path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(file_error("unable to open a shared attachment", &error)),
        };
        let total_size = file
            .metadata()
            .map_err(|error| file_error("unable to read a shared attachment", &error))?
            .len();
        if offset > total_size {
            return Ok(None);
        }
        let mut data = Vec::with_capacity(FILE_TRANSFER_CHUNK_BYTES);
        file.seek(SeekFrom::Start(offset))
            .and_then(|_| {
                file.take(FILE_TRANSFER_CHUNK_BYTES as u64)
                    .read_to_end(&mut data)
            })
            .map_err(|error| file_error("unable to read a shared attachment", &error))?;
        Ok(Some(FileChunk::new(hash, total_size, offset, data)))
    }

    fn begin_incoming(&mut self, attachment: &SharedAttachment) -> Result<(), StorageError> {
        let allowed_type = supported_mime_type(Path::new(&attachment.file_name))?;
        if allowed_type != attachment.mime_type || attachment.size_bytes > MAX_FILE_TRANSFER_BYTES {
            return Err(StorageError::Rejected {
                reason: String::from("an attachment's type or size is not acceptable"),
            });
        }
        create_private_directory(&self.owner_directory(&attachment.owner)?)?;
        let partial_path = self.partial_path(attachment)?;
        let _ = std::fs::remove_file(&partial_path);
        let mut open_options = OpenOptions::new();
        open_options.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut open_options, 0o600);
        open_options
            .open(&partial_path)
            .map_err(|error| file_error("unable to create a partial attachment", &error))?;
        Ok(())
    }

    fn append_incoming(
        &mut self,
        attachment: &SharedAttachment,
        data: &[u8],
    ) -> Result<(), StorageError> {
        OpenOptions::new()
            .append(true)
            .open(self.partial_path(attachment)?)
            .and_then(|mut file| file.write_all(data))
            .map_err(|error| file_error("unable to write a partial attachment", &error))
    }

    fn finish_incoming(&mut self, attachment: &SharedAttachment) -> Result<(), StorageError> {
        let partial_path = self.partial_path(attachment)?;
        let verified = File::open(&partial_path)
            .and_then(|mut file| {
                file.sync_all()?;
                hash_file(&mut file)
            })
            .map_err(|error| file_error("unable to verify a received attachment", &error));
        match verified {
            Ok((content_hash, size_bytes))
                if content_hash == attachment.content_hash
                    && size_bytes == attachment.size_bytes => {}
            Ok(_) => {
                self.discard_incoming(attachment);
                return Err(StorageError::Rejected {
                    reason: String::from("a received attachment does not match its SHA-256"),
                });
            }
            Err(error) => {
                self.discard_incoming(attachment);
                return Err(error);
            }
        }
        let safe_name = kept_file_name(Path::new(&attachment.file_name));
        let final_path = self
            .owner_directory(&attachment.owner)?
            .join(format!("{}-{safe_name}", attachment.attachment_id));
        if let Err(error) = std::fs::rename(&partial_path, &final_path) {
            self.discard_incoming(attachment);
            return Err(file_error("unable to store a received attachment", &error));
        }
        let stored = self
            .repository
            .insert_received_attachment(attachment, &final_path.to_string_lossy());
        if stored.is_err() {
            let _ = std::fs::remove_file(&final_path);
        }
        stored
    }

    fn discard_incoming(&mut self, attachment: &SharedAttachment) {
        if let Ok(partial_path) = self.partial_path(attachment)
            && let Err(error) = std::fs::remove_file(partial_path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(attachment_id = %attachment.attachment_id, error = %error, "a partial attachment could not be removed");
        }
    }
}
