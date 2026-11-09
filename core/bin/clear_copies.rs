//! Clear copies of sealed attachments for the app to show (#99).
//!
//! Attachment files are sealed at rest, but Flutter's image, text and PDF views read files from
//! disk. So the app asks for a clear copy of one attachment when it shows it. Copies live in one
//! private folder (0700, files 0600 on Unix) beside the attachment folder, named
//! `<attachment ID>-<name>` so the extension survives. A copy is reused while it is fresh; one
//! older than `COPY_LIFETIME` is removed when another copy is made, and the whole folder is
//! emptied when the app starts. Names and contents are never logged.

use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use uuid::Uuid;

use crate::{
    adapters::storage::{private_files::create_private_directory, sealed_files::SealedFiles},
    domain::errors::StorageError,
};

/// How long a clear copy is kept after it was made.
pub const COPY_LIFETIME: Duration = Duration::from_secs(30 * 60);

/// The folder of clear copies.
#[derive(Clone, Debug)]
pub struct ClearCopies {
    directory: PathBuf,
}

impl ClearCopies {
    /// Creates the folder's handle; the folder is made on the first copy.
    ///
    /// Side effects: None.
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    /// The folder beside an attachment folder: `<its parent>/clear-copies`.
    ///
    /// Side effects: None.
    pub fn beside(attachment_directory: &Path) -> Self {
        Self::new(
            attachment_directory
                .parent()
                .map_or_else(PathBuf::new, Path::to_path_buf)
                .join("clear-copies"),
        )
    }

    /// Gives a clear copy of a stored file, made now or reused while fresh.
    ///
    /// Inputs: The sealer, the attachment and its stored file. Outputs: The copy's path, or a
    /// rejection for a missing or unreadable file (nothing is left behind).
    /// Side effects: Creates the folder; removes copies older than `COPY_LIFETIME`; writes the
    /// copy.
    pub fn copy(
        &self,
        sealed_files: &SealedFiles,
        attachment_id: Uuid,
        stored_path: &Path,
    ) -> Result<PathBuf, StorageError> {
        create_private_directory(&self.directory)?;
        self.remove_old();
        let stored_name = stored_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let id_prefix = format!("{attachment_id}-");
        // Stored names start with the attachment ID already; others get it, so copies of
        // different attachments never share a name.
        let name = if stored_name.starts_with(&id_prefix) {
            stored_name
        } else {
            format!("{id_prefix}{stored_name}")
        };
        let copy = self.directory.join(name);
        if copy.is_file() {
            return Ok(copy);
        }
        sealed_files.copy_clear(stored_path, &copy)?;
        Ok(copy)
    }

    /// Empties the folder, as the app does when it starts.
    ///
    /// Side effects: Deletes the folder and its copies; failures (a copy still open elsewhere on
    /// Windows) are ignored and retried at the next start.
    pub fn wipe(&self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }

    /// Removes copies made more than `COPY_LIFETIME` ago.
    fn remove_old(&self) {
        let Ok(entries) = std::fs::read_dir(&self.directory) else {
            return;
        };
        let now = SystemTime::now();
        for entry in entries.flatten() {
            let old = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .is_ok_and(|modified| {
                    now.duration_since(modified)
                        .is_ok_and(|age| age > COPY_LIFETIME)
                });
            if old {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}
