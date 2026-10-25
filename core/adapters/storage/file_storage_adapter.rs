//! Implements atomic SHA-256 content-addressed attachment storage outside SQLite.

use std::{
    fs,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use crate::{
    application::file_sync_manager::sha256_hex, domain::errors::StorageError,
    ports::file_transfer::FileStorage,
};

/// Stores each verified attachment exactly once at `<root>/<sha256>`.
pub struct ContentAddressedFileStorage {
    root_directory: PathBuf,
}

impl ContentAddressedFileStorage {
    /// Opens or creates a local content-addressed attachment directory.
    ///
    /// Inputs: `root_directory` must be a private writable directory such as `~/.note_taker/files`.
    /// Outputs: A storage adapter ready to atomically save verified files.
    /// Side effects: Creates the directory when absent.
    pub fn new(root_directory: impl AsRef<Path>) -> Result<Self, StorageError> {
        let root_directory = root_directory.as_ref().to_path_buf();
        fs::create_dir_all(&root_directory).map_err(io_error)?;
        Ok(Self { root_directory })
    }

    /// Returns the local CAS root for composition roots and operational diagnostics.
    ///
    /// Inputs: None.
    /// Outputs: The private root path without enumerating files.
    /// Side effects: None.
    pub fn root_directory(&self) -> &Path {
        &self.root_directory
    }

    fn object_path(&self, hash: &str) -> Result<PathBuf, StorageError> {
        if !valid_hash(hash) {
            return Err(StorageError::Rejected {
                reason: "content address must be lowercase SHA-256".to_owned(),
            });
        }
        Ok(self.root_directory.join(hash))
    }
}

impl FileStorage for ContentAddressedFileStorage {
    /// Verifies data against `hash`, writes a unique temporary file, and atomically renames it into CAS.
    fn save_file(&self, hash: &str, data: &[u8]) -> Result<PathBuf, StorageError> {
        let final_path = self.object_path(hash)?;
        if final_path.exists() {
            return Ok(final_path);
        }
        if sha256_hex(data) != hash {
            return Err(StorageError::Rejected {
                reason: "file content does not match its SHA-256 address".to_owned(),
            });
        }
        let temporary_path = self
            .root_directory
            .join(format!(".transfer-{}.tmp", Uuid::new_v4()));
        let write_result = fs::write(&temporary_path, data).map_err(io_error);
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary_path);
            return Err(error);
        }
        match fs::rename(&temporary_path, &final_path) {
            Ok(()) => Ok(final_path),
            Err(_error) if final_path.exists() => {
                let _ = fs::remove_file(&temporary_path);
                Ok(final_path)
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary_path);
                Err(io_error(error))
            }
        }
    }

    /// Loads only a validated SHA-256-addressed object.
    fn load_file(&self, hash: &str) -> Result<Vec<u8>, StorageError> {
        fs::read(self.object_path(hash)?).map_err(io_error)
    }
    /// Checks the addressed object path without reading its content.
    fn has_file(&self, hash: &str) -> bool {
        self.object_path(hash).is_ok_and(|path| path.is_file())
    }
    /// Lists valid object names and ignores temporary/foreign directory entries.
    fn list_available_hashes(&self) -> Result<Vec<String>, StorageError> {
        let directory_entries = fs::read_dir(&self.root_directory).map_err(io_error)?;
        let mut hashes = Vec::new();
        for directory_entry in directory_entries {
            let directory_entry = directory_entry.map_err(io_error)?;
            if directory_entry.file_type().map_err(io_error)?.is_file()
                && let Some(hash) = directory_entry
                    .file_name()
                    .to_str()
                    .filter(|hash| valid_hash(hash))
            {
                hashes.push(hash.to_owned());
            }
        }
        hashes.sort();
        Ok(hashes)
    }
}

fn valid_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}
fn io_error(error: std::io::Error) -> StorageError {
    StorageError::Rejected {
        reason: format!("content-addressed storage I/O failed: {error}"),
    }
}
