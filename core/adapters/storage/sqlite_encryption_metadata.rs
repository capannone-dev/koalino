//! Persists the encryption metadata record of a database (Issue #31).
//!
//! `encryption_meta` holds exactly one row (`id = 1`): the key source (`key_file` or
//! `passphrase`), the Argon2id salt and parameters for passphrase keys, and the sealed key-check
//! value. None of it is secret; it lets `KeyManager` rebuild and verify the key on every start.
//! The record is written once and never overwritten, so concurrent first starts cannot replace
//! each other's key.

use std::{path::Path, time::Duration};

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};

use super::sqlite_adapter::sqlite_error;
use crate::{
    domain::errors::StorageError,
    ports::crypto::{
        EncryptionKeySource, EncryptionMetadata, EncryptionMetadataRepository,
        KeyDerivationParameters,
    },
};

/// How long a write waits for a lock held by another connection to the same file.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
/// Stored `key_source` value for random key files.
const KEY_FILE_SOURCE: &str = "key_file";
/// Stored `key_source` value for passphrase-derived keys.
const PASSPHRASE_SOURCE: &str = "passphrase";

/// Stores the encryption metadata record in SQLite.
pub struct SqliteEncryptionMetadataStore {
    connection: Connection,
}

impl SqliteEncryptionMetadataStore {
    /// Opens (or creates) the metadata table in the database at `database_path`.
    ///
    /// Inputs: A writable SQLite path, normally the shared application database.
    /// Outputs: The store, or a portable storage error.
    /// Side effects: Opens or creates the file and creates `encryption_meta` when absent.
    pub fn open(database_path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let sqlite_connection = Connection::open(database_path)
            .map_err(|error| sqlite_error("unable to open encryption metadata database", error))?;
        Self::from_connection(sqlite_connection)
    }

    /// Opens an isolated in-memory metadata store for tests.
    ///
    /// Inputs: None.
    /// Outputs: The store, or a portable storage error.
    /// Side effects: Allocates an in-memory SQLite database.
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let sqlite_connection = Connection::open_in_memory()
            .map_err(|error| sqlite_error("unable to open in-memory metadata store", error))?;
        Self::from_connection(sqlite_connection)
    }

    /// Configures the connection and creates the metadata table idempotently.
    ///
    /// Inputs: An open connection.
    /// Outputs: The store, or a portable storage error.
    /// Side effects: Sets a busy timeout and executes `CREATE TABLE IF NOT EXISTS`.
    fn from_connection(sqlite_connection: Connection) -> Result<Self, StorageError> {
        sqlite_connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(|error| sqlite_error("unable to configure metadata busy timeout", error))?;
        sqlite_connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS encryption_meta (
                    id INTEGER PRIMARY KEY NOT NULL CHECK (id = 1),
                    key_source TEXT NOT NULL,
                    salt BLOB,
                    argon2_memory_kib INTEGER,
                    argon2_iterations INTEGER,
                    argon2_parallelism INTEGER,
                    key_check BLOB NOT NULL,
                    created_at TEXT NOT NULL
                );",
            )
            .map_err(|error| sqlite_error("unable to initialize encryption metadata", error))?;
        Ok(Self {
            connection: sqlite_connection,
        })
    }
}

/// Raw `encryption_meta` columns before validation.
type MetadataRow = (
    String,
    Option<Vec<u8>>,
    Option<u32>,
    Option<u32>,
    Option<u32>,
    Vec<u8>,
);

impl EncryptionMetadataRepository for SqliteEncryptionMetadataStore {
    /// Loads the single metadata record.
    ///
    /// Inputs: None.
    /// Outputs: The record, `None` before first encryption, or a rejection for a corrupt row.
    /// Side effects: Reads `encryption_meta`.
    fn load_encryption_metadata(&self) -> Result<Option<EncryptionMetadata>, StorageError> {
        let stored_row: Option<MetadataRow> = self
            .connection
            .query_row(
                "SELECT key_source, salt, argon2_memory_kib, argon2_iterations, argon2_parallelism, key_check
                 FROM encryption_meta WHERE id = 1",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| sqlite_error("unable to load encryption metadata", error))?;
        let Some((key_source, salt, memory_kib, iterations, parallelism, key_check)) = stored_row
        else {
            return Ok(None);
        };
        let key_source = match (
            key_source.as_str(),
            salt,
            memory_kib,
            iterations,
            parallelism,
        ) {
            (KEY_FILE_SOURCE, _, _, _, _) => EncryptionKeySource::KeyFile,
            (
                PASSPHRASE_SOURCE,
                Some(salt),
                Some(memory_kib),
                Some(iterations),
                Some(parallelism),
            ) => EncryptionKeySource::Passphrase {
                salt,
                parameters: KeyDerivationParameters::new(memory_kib, iterations, parallelism),
            },
            _ => {
                return Err(StorageError::Rejected {
                    reason: String::from("stored encryption metadata is corrupt"),
                });
            }
        };
        Ok(Some(EncryptionMetadata::new(key_source, key_check)))
    }

    /// Inserts the record unless one already exists.
    ///
    /// Inputs: The metadata created on first unlock.
    /// Outputs: `true` when inserted, `false` when a record already existed, or a storage error.
    /// Side effects: Writes at most one `encryption_meta` row.
    fn save_encryption_metadata(
        &self,
        metadata: &EncryptionMetadata,
    ) -> Result<bool, StorageError> {
        let (key_source, salt, parameters) = match metadata.key_source() {
            EncryptionKeySource::KeyFile => (KEY_FILE_SOURCE, None, None),
            EncryptionKeySource::Passphrase { salt, parameters } => {
                (PASSPHRASE_SOURCE, Some(salt.as_slice()), Some(*parameters))
            }
        };
        let inserted_rows = self
            .connection
            .execute(
                "INSERT INTO encryption_meta
                    (id, key_source, salt, argon2_memory_kib, argon2_iterations, argon2_parallelism, key_check, created_at)
                 VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(id) DO NOTHING",
                params![
                    key_source,
                    salt,
                    parameters.map(|parameters| parameters.memory_kib()),
                    parameters.map(|parameters| parameters.iterations()),
                    parameters.map(|parameters| parameters.parallelism()),
                    metadata.key_check(),
                    Utc::now().to_rfc3339(),
                ],
            )
            .map_err(|error| sqlite_error("unable to save encryption metadata", error))?;
        Ok(inserted_rows == 1)
    }
}
