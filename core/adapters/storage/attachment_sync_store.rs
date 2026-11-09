//! Reads and stores attachment files for sync with paired devices (Issue #57).
//!
//! A file from a paired device is written to a private partial file inside its meeting's folder,
//! then checked against the size and SHA-256 its metadata names, and only then moved to
//! `<attachment directory>/<meeting ID>/<attachment ID>-<safe name>` with its row written. The
//! name, type and meeting ID are peer input: the name is reduced to safe characters, the type must
//! match the extension among those that can be attached locally, and the meeting ID must be a plain
//! folder name. Serving reads only files of attachments linked to a meeting. Logs carry IDs only.
//!
//! Files are sealed at rest with each device's own key (#99), so the hash, the size and the chunks
//! exchanged are those of the clear contents, sent over the encrypted P2P transport. A received
//! file is sealed part by part as it arrives, so its clear contents never touch the disk.

use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    attachment_file_store::{kept_file_name, project_mime_type, supported_mime_type},
    private_files::{
        create_private_directory, file_error, is_plain_folder_name, remove_meeting_folder,
    },
    sealed_files::{SealedAppender, SealedFiles},
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
    /// Files being received, sealed as their parts arrive (#99), by attachment ID.
    incoming: HashMap<Uuid, SealedAppender>,
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
            incoming: HashMap::new(),
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

/// Computes the SHA-256 and length of a file's clear contents without loading it whole.
///
/// Inputs: The sealer and the file (sealed, or clear from before #99). Outputs: The lowercase
/// hexadecimal hash and the clear length, or a rejection for an unreadable file.
/// Side effects: Reads (and decrypts) the file.
fn hash_file(sealed_files: &SealedFiles, path: &Path) -> Result<(String, u64), StorageError> {
    let mut reader = sealed_files.open(path)?;
    let mut hasher = Sha256::new();
    let size_bytes = std::io::copy(&mut reader, &mut hasher)
        .map_err(|error| file_error("unable to read an attachment", &error))?;
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

    /// Removes the stored file of one project resource deleted on a paired device (Issue #85).
    ///
    /// Inputs: The resource and its project. Outputs: Success (also when no file is stored), or a
    /// storage error. Side effects: Deletes every `<attachment ID>-*` file, and a leftover partial
    /// file, in the project's folder; never logs names.
    fn remove_resource_file(
        &mut self,
        attachment_id: Uuid,
        project_id: crate::domain::projects::ProjectId,
    ) -> Result<(), StorageError> {
        let project_folder =
            self.attachment_directory
                .join(super::attachment_file_store::project_folder_name(
                    project_id,
                ));
        let entries = match std::fs::read_dir(&project_folder) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(file_error("unable to list a project's files", &error)),
        };
        let stored_prefix = format!("{attachment_id}-");
        let partial_name = format!(".{attachment_id}.part");
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            if file_name.starts_with(&stored_prefix) || file_name == partial_name {
                std::fs::remove_file(entry.path()).map_err(|error| {
                    file_error("unable to remove a deleted resource's file", &error)
                })?;
            }
        }
        Ok(())
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
        let hashed = hash_file(&self.repository.sealed_files(), Path::new(&file_path));
        match hashed {
            Ok((content_hash, size_bytes)) if size_bytes <= MAX_FILE_TRANSFER_BYTES => self
                .repository
                .record_attachment_hash(&attachment_id, &content_hash, size_bytes),
            Ok(_) => {
                self.unhashable.insert(attachment_id);
                Ok(())
            }
            Err(_) => {
                tracing::warn!(%attachment_id, "an attachment file could not be read");
                self.unhashable.insert(attachment_id);
                Ok(())
            }
        }
    }

    fn shared_attachments(&self) -> Result<Vec<SharedAttachment>, StorageError> {
        self.repository.shared_attachment_rows()
    }

    fn has_attachment(&self, attachment_id: Uuid) -> Result<bool, StorageError> {
        self.repository.attachment_known(attachment_id)
    }

    fn has_owner(&self, owner: &AttachmentOwner) -> Result<bool, StorageError> {
        self.repository.attachment_owner_exists(owner)
    }

    fn read_chunk(&self, hash: &str, offset: u64) -> Result<Option<FileChunk>, StorageError> {
        let Some(file_path) = self.repository.shared_attachment_path(hash)? else {
            return Ok(None);
        };
        let file_path = Path::new(&file_path);
        if !file_path.exists() {
            return Ok(None);
        }
        // The chunk is sent over the encrypted P2P transport as clear contents: the paired
        // device seals it with its own key.
        let mut reader = self.repository.sealed_files().open(file_path)?;
        let total_size = reader
            .clear_len()
            .map_err(|error| file_error("unable to read a shared attachment", &error))?;
        if offset > total_size {
            return Ok(None);
        }
        let mut data = Vec::with_capacity(FILE_TRANSFER_CHUNK_BYTES);
        reader
            .seek_to(offset)
            .and_then(|()| {
                reader
                    .take(FILE_TRANSFER_CHUNK_BYTES as u64)
                    .read_to_end(&mut data)
            })
            .map_err(|error| file_error("unable to read a shared attachment", &error))?;
        Ok(Some(FileChunk::new(hash, total_size, offset, data)))
    }

    fn begin_incoming(&mut self, attachment: &SharedAttachment) -> Result<(), StorageError> {
        // A project takes any file (#94); a meeting only the types the app shows.
        let allowed_type = match attachment.owner {
            AttachmentOwner::Project(_) => project_mime_type(Path::new(&attachment.file_name)),
            AttachmentOwner::Meeting(_) => supported_mime_type(Path::new(&attachment.file_name))?,
        };
        if allowed_type != attachment.mime_type || attachment.size_bytes > MAX_FILE_TRANSFER_BYTES {
            return Err(StorageError::Rejected {
                reason: String::from("an attachment's type or size is not acceptable"),
            });
        }
        create_private_directory(&self.owner_directory(&attachment.owner)?)?;
        let partial_path = self.partial_path(attachment)?;
        if let Some(appender) = self.incoming.remove(&attachment.attachment_id) {
            appender.discard();
        }
        let _ = std::fs::remove_file(&partial_path);
        let appender = self
            .repository
            .sealed_files()
            .create_appender(&partial_path)?;
        self.incoming.insert(attachment.attachment_id, appender);
        Ok(())
    }

    fn append_incoming(
        &mut self,
        attachment: &SharedAttachment,
        data: &[u8],
    ) -> Result<(), StorageError> {
        match self.incoming.get_mut(&attachment.attachment_id) {
            Some(appender) => appender.append(data),
            None => Err(StorageError::Rejected {
                reason: String::from("a partial attachment was not started"),
            }),
        }
    }

    fn finish_incoming(&mut self, attachment: &SharedAttachment) -> Result<(), StorageError> {
        let partial_path = self.partial_path(attachment)?;
        let verified = match self.incoming.remove(&attachment.attachment_id) {
            Some(appender) => appender
                .finish()
                .and_then(|_| hash_file(&self.repository.sealed_files(), &partial_path)),
            None => Err(StorageError::Rejected {
                reason: String::from("a partial attachment was not started"),
            }),
        };
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
        if let Some(appender) = self.incoming.remove(&attachment.attachment_id) {
            appender.discard();
        }
        if let Ok(partial_path) = self.partial_path(attachment)
            && let Err(error) = std::fs::remove_file(partial_path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(attachment_id = %attachment.attachment_id, error = %error, "a partial attachment could not be removed");
        }
    }
}
