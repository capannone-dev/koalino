//! Opens the encrypted local stores shared by the CLI, daemon, and FFI composition roots.
//!
//! The order matters: the database key is unlocked and verified against the stored key-check
//! value before either adapter opens, so legacy plaintext embeddings are only ever migrated
//! under the correct key, and a wrong passphrase stops start-up without touching any row.

use std::{path::Path, sync::Arc};

use anyhow::{Context, Result};

use crate::{
    adapters::storage::{
        sqlite_adapter::SqliteNoteRepository,
        sqlite_encryption_metadata::SqliteEncryptionMetadataStore,
        sqlite_voice_vector_db::SqliteVoiceVectorDB,
    },
    application::crypto::key_manager::{KeyManager, KeyManagerConfig},
    domain::models::DeviceId,
    ports::crypto::EmbeddingCipher,
};

/// The note repository and voice store opened on one database with one verified key.
pub struct EncryptedStores {
    /// Meetings, transcripts, clusters, and review clips.
    pub note_repository: SqliteNoteRepository,
    /// Speaker identities and voice variants.
    pub voice_store: SqliteVoiceVectorDB,
    /// The verified cipher, for opening further connections (e.g. the recording worker's)
    /// without unlocking the key again.
    pub embedding_cipher: Arc<dyn EmbeddingCipher>,
}

/// Unlocks the database key and opens both stores on `database_path`.
///
/// Inputs: The SQLite path, the device identifier for repository logs, and the key
/// configuration (normally `KeyManagerConfig::from_environment()`).
/// Outputs: Both stores, or an error naming the failed step (never key material).
/// Side effects: May create the database file, the key file, and the encryption metadata on
/// first start; runs Argon2id when a passphrase is configured; re-encrypts legacy plaintext
/// embeddings. Call it off UI threads.
pub fn open_encrypted_stores(
    database_path: &Path,
    device_id: DeviceId,
    key_config: KeyManagerConfig,
) -> Result<EncryptedStores> {
    let metadata_store = SqliteEncryptionMetadataStore::open(database_path)
        .context("unable to open encryption metadata")?;
    let embedding_cipher: Arc<dyn EmbeddingCipher> = Arc::new(
        KeyManager::new(key_config)
            .unlock(&metadata_store)
            .context("unable to unlock the voice embedding encryption key")?,
    );
    let note_repository =
        SqliteNoteRepository::open(database_path, device_id, Arc::clone(&embedding_cipher))
            .context("unable to open the note repository")?;
    let voice_store = SqliteVoiceVectorDB::open(database_path, Arc::clone(&embedding_cipher))
        .context("unable to open the voice store")?;
    Ok(EncryptedStores {
        note_repository,
        voice_store,
        embedding_cipher,
    })
}

/// Builds a key-file configuration rooted at `key_directory`, for restart tests.
///
/// Inputs: The directory holding (or receiving) the random key file.
/// Outputs: A configuration that ignores the process environment.
/// Side effects: None.
#[cfg(test)]
pub(crate) fn key_file_config_for_tests(key_directory: &Path) -> KeyManagerConfig {
    let key_directory = key_directory.to_string_lossy().into_owned();
    KeyManagerConfig::from_lookup(|name| {
        (name == crate::application::crypto::key_manager::KEY_DIRECTORY_VARIABLE)
            .then(|| key_directory.clone())
    })
}
