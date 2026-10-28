//! Opens the encrypted local stores shared by the CLI, daemon, and FFI composition roots.
//!
//! The order matters: the database key is unlocked and verified against the stored key-check
//! value before either adapter opens, so legacy plaintext embeddings are only ever migrated
//! under the correct key, and a wrong passphrase stops start-up without touching any row.
//!
//! This is also where the platform keychain adapter (Issue #40) is injected into the key manager,
//! unless `NOTE_TAKER_KEYCHAIN_ENABLED` disables it or a passphrase is configured.

use std::{path::Path, sync::Arc};

use anyhow::{Context, Result};

use crate::{
    adapters::{
        keychain::platform_keychain::PlatformKeychain,
        storage::{
            sqlite_adapter::SqliteNoteRepository,
            sqlite_encryption_metadata::SqliteEncryptionMetadataStore,
            sqlite_voice_vector_db::SqliteVoiceVectorDB,
        },
    },
    application::{
        crypto::key_manager::{KeyManager, KeyManagerConfig},
        meeting_document::AutomergeMeetingDocumentCodec,
    },
    domain::models::DeviceId,
    ports::crypto::{EmbeddingCipher, KeychainStore},
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
/// Side effects: May create the database file, the keychain entry or key file, and the
/// encryption metadata on first start; may migrate the key file into the platform keychain and
/// delete it; runs Argon2id when a passphrase is configured; re-encrypts legacy plaintext
/// embeddings. Keychain access may block on a platform unlock prompt. Call it off UI threads.
pub fn open_encrypted_stores(
    database_path: &Path,
    device_id: DeviceId,
    key_config: KeyManagerConfig,
) -> Result<EncryptedStores> {
    open_encrypted_stores_with_keychain(database_path, device_id, key_config, || {
        Arc::new(PlatformKeychain::new())
    })
}

/// Unlocks the database key through a chosen keychain and opens both stores.
///
/// Inputs: As `open_encrypted_stores`, plus a factory for the keychain to use when the
/// configuration enables one (the FFI passes the host-provided Android Keystore key there).
/// Outputs and side effects: As `open_encrypted_stores`; the factory runs only when the keychain
/// is enabled and no passphrase is set.
pub fn open_encrypted_stores_with_keychain(
    database_path: &Path,
    device_id: DeviceId,
    key_config: KeyManagerConfig,
    keychain: impl FnOnce() -> Arc<dyn KeychainStore>,
) -> Result<EncryptedStores> {
    let metadata_store = SqliteEncryptionMetadataStore::open(database_path)
        .context("unable to open encryption metadata")?;
    let uses_keychain = key_config.keychain_enabled() && !key_config.uses_passphrase();
    let mut key_manager = KeyManager::new(key_config);
    if uses_keychain {
        key_manager = key_manager.with_keychain(keychain());
    }
    let embedding_cipher: Arc<dyn EmbeddingCipher> = Arc::new(
        key_manager
            .unlock(&metadata_store)
            .context("unable to unlock the voice embedding encryption key")?,
    );
    let note_repository =
        open_note_repository(database_path, device_id, Arc::clone(&embedding_cipher))?;
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
/// Outputs: A configuration that ignores the process environment and disables the platform
/// keychain, so tests never read or write the developer's real keychain.
/// Side effects: None.
#[cfg(test)]
pub(crate) fn key_file_config_for_tests(key_directory: &Path) -> KeyManagerConfig {
    let key_directory = key_directory.to_string_lossy().into_owned();
    KeyManagerConfig::from_lookup(|name| match name {
        crate::application::crypto::key_manager::KEY_DIRECTORY_VARIABLE => {
            Some(key_directory.clone())
        }
        crate::application::crypto::key_manager::KEYCHAIN_ENABLED_VARIABLE => {
            Some(String::from("false"))
        }
        _ => None,
    })
}

/// Opens the note repository with the meeting document as the source of truth (Issue #43).
///
/// Inputs: The database path, this device's ID, and the verified cipher.
/// Outputs: The repository, or an error naming the step that failed.
/// Side effects: Opens a SQLite connection; on the first open after upgrading, copies legacy
/// transcript and segment rows into their meeting documents.
pub fn open_note_repository(
    database_path: &Path,
    device_id: DeviceId,
    embedding_cipher: Arc<dyn EmbeddingCipher>,
) -> Result<SqliteNoteRepository> {
    SqliteNoteRepository::open(database_path, device_id, embedding_cipher)
        .and_then(|repository| {
            repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
        })
        .context("unable to open the note repository")
}
