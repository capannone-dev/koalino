//! Specifies key resolution: random key files, Argon2id passphrase keys, stored salts, and
//! wrong-key detection through the database key-check value.
//!
//! Tests use tiny Argon2id parameters so the unoptimized test build stays fast, and unique
//! temporary key directories that are removed afterwards.

use std::{cell::RefCell, collections::HashMap, path::PathBuf};

use super::{
    key_manager::{
        KEY_DIRECTORY_VARIABLE, KEY_FILE_NAME, KeyManager, KeyManagerConfig, PASSPHRASE_VARIABLE,
        derive_key_from_passphrase,
    },
    voice_encryptor::VoiceEncryptor,
};
use crate::{
    domain::errors::{CryptoError, StorageError},
    ports::crypto::{
        EmbeddingCipher, EncryptionKeySource, EncryptionMetadata, EncryptionMetadataRepository,
        KeyDerivationParameters,
    },
};

/// Holds one database's metadata record in memory.
#[derive(Default)]
struct InMemoryMetadata {
    stored: RefCell<Option<EncryptionMetadata>>,
}

impl EncryptionMetadataRepository for InMemoryMetadata {
    /// Returns the stored record.
    fn load_encryption_metadata(&self) -> Result<Option<EncryptionMetadata>, StorageError> {
        Ok(self.stored.borrow().clone())
    }

    /// Stores the record unless one exists.
    fn save_encryption_metadata(
        &self,
        metadata: &EncryptionMetadata,
    ) -> Result<bool, StorageError> {
        let mut stored = self.stored.borrow_mut();
        if stored.is_some() {
            return Ok(false);
        }
        *stored = Some(metadata.clone());
        Ok(true)
    }
}

/// A unique key directory removed when dropped.
struct TemporaryKeyDirectory {
    path: PathBuf,
}

impl TemporaryKeyDirectory {
    /// Reserves a path; the key manager creates the directory itself.
    fn new() -> Self {
        Self {
            path: std::env::temp_dir()
                .join(format!("note-taker-keys-{}", uuid::Uuid::new_v4()))
                .join(".note_taker"),
        }
    }

    /// Returns the key file path inside the directory.
    fn key_file(&self) -> PathBuf {
        self.path.join(KEY_FILE_NAME)
    }
}

impl Drop for TemporaryKeyDirectory {
    fn drop(&mut self) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
    }
}

/// Fast parameters for tests; production uses `KeyDerivationParameters::default()`.
fn fast_parameters() -> KeyDerivationParameters {
    KeyDerivationParameters::new(64, 1, 1)
}

/// Builds a configuration for `key_directory` with an optional passphrase.
fn config(key_directory: &TemporaryKeyDirectory, passphrase: Option<&str>) -> KeyManagerConfig {
    let mut variables = HashMap::from([(
        KEY_DIRECTORY_VARIABLE,
        key_directory.path.to_string_lossy().into_owned(),
    )]);
    if let Some(passphrase) = passphrase {
        variables.insert(PASSPHRASE_VARIABLE, String::from(passphrase));
    }
    KeyManagerConfig::from_lookup(|name| variables.get(name).cloned())
        .with_derivation_parameters(fast_parameters())
}

/// Unlocks, failing the test on error.
fn unlock(config: KeyManagerConfig, metadata: &InMemoryMetadata) -> VoiceEncryptor {
    KeyManager::new(config)
        .unlock(metadata)
        .unwrap_or_else(|error| panic!("key manager must unlock: {error}"))
}

/// Asserts that `reader` opens a value sealed by `writer`.
fn assert_same_key(writer: &VoiceEncryptor, reader: &VoiceEncryptor) {
    let sealed = writer
        .seal_embedding("key-test", &[0.5, 0.25])
        .unwrap_or_else(|error| panic!("embedding must seal: {error}"));
    assert_eq!(
        reader.open_embedding("key-test", &sealed),
        Ok(vec![0.5, 0.25])
    );
}

/// No passphrase provided → a random key is generated, stored owner-only, and reused.
#[test]
fn without_a_passphrase_a_random_key_file_is_generated_and_reused() {
    let key_directory = TemporaryKeyDirectory::new();
    let metadata = InMemoryMetadata::default();

    let first_launch = unlock(config(&key_directory, None), &metadata);
    let key_bytes = std::fs::read(key_directory.key_file())
        .unwrap_or_else(|error| panic!("key file must exist: {error}"));
    let second_launch = unlock(config(&key_directory, None), &metadata);

    assert_eq!(key_bytes.len(), 32);
    assert_ne!(key_bytes, vec![0; 32]);
    assert_eq!(
        metadata
            .stored
            .borrow()
            .as_ref()
            .map(EncryptionMetadata::key_source),
        Some(&EncryptionKeySource::KeyFile)
    );
    assert_same_key(&first_launch, &second_launch);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let file_mode = std::fs::metadata(key_directory.key_file())
            .map(|file| file.permissions().mode() & 0o777)
            .unwrap_or_default();
        let directory_mode = std::fs::metadata(&key_directory.path)
            .map(|directory| directory.permissions().mode() & 0o777)
            .unwrap_or_default();
        assert_eq!(file_mode, 0o600, "key file must be owner read/write only");
        assert_eq!(directory_mode, 0o700, "key directory must be owner-only");
    }
}

/// An existing key file is reused for a newly created database instead of being replaced.
#[test]
fn an_existing_key_file_is_reused_for_a_new_database() {
    let key_directory = TemporaryKeyDirectory::new();
    let first_database = unlock(config(&key_directory, None), &InMemoryMetadata::default());

    let second_database = unlock(config(&key_directory, None), &InMemoryMetadata::default());

    assert_same_key(&first_database, &second_database);
}

/// Passphrase provided → the key is derived from it with a stored salt; no key file is written.
#[test]
fn with_a_passphrase_the_key_is_derived_and_the_salt_is_stored_once() {
    let key_directory = TemporaryKeyDirectory::new();
    let metadata = InMemoryMetadata::default();

    let first_launch = unlock(config(&key_directory, Some("correct horse")), &metadata);
    let first_source = metadata
        .stored
        .borrow()
        .as_ref()
        .map(|stored| stored.key_source().clone());
    let second_launch = unlock(config(&key_directory, Some("correct horse")), &metadata);
    let second_source = metadata
        .stored
        .borrow()
        .as_ref()
        .map(|stored| stored.key_source().clone());

    assert!(!key_directory.key_file().exists());
    let Some(EncryptionKeySource::Passphrase { salt, parameters }) = first_source.clone() else {
        panic!("metadata must record a passphrase key: {first_source:?}");
    };
    assert_eq!(salt.len(), 16);
    assert_eq!(parameters, fast_parameters());
    assert_eq!(
        first_source, second_source,
        "the salt is generated only once"
    );
    assert_same_key(&first_launch, &second_launch);
}

/// Derivation is deterministic for one passphrase and salt, and differs for any other input.
#[test]
fn passphrase_derivation_depends_on_passphrase_and_salt() {
    let derive = |passphrase: &str, salt: &[u8]| {
        derive_key_from_passphrase(passphrase, salt, fast_parameters())
            .map(|key| *key.as_bytes())
            .unwrap_or_else(|error| panic!("key must derive: {error}"))
    };

    assert_eq!(derive("secret", &[1; 16]), derive("secret", &[1; 16]));
    assert_ne!(derive("secret", &[1; 16]), derive("Secret", &[1; 16]));
    assert_ne!(derive("secret", &[1; 16]), derive("secret", &[2; 16]));
    assert!(matches!(
        derive_key_from_passphrase("secret", &[1; 16], KeyDerivationParameters::new(0, 0, 0)),
        Err(CryptoError::KeyDerivation { .. })
    ));
}

/// A wrong passphrase is detected by the key-check value before any embedding is read.
#[test]
fn a_wrong_passphrase_is_rejected() {
    let key_directory = TemporaryKeyDirectory::new();
    let metadata = InMemoryMetadata::default();
    unlock(config(&key_directory, Some("correct horse")), &metadata);

    let outcome = KeyManager::new(config(&key_directory, Some("battery staple"))).unlock(&metadata);

    assert!(matches!(outcome, Err(CryptoError::WrongKey)));
}

/// Switching between key sources is refused rather than silently re-keying.
#[test]
fn switching_key_sources_is_refused() {
    let key_directory = TemporaryKeyDirectory::new();
    let key_file_database = InMemoryMetadata::default();
    let passphrase_database = InMemoryMetadata::default();
    unlock(config(&key_directory, None), &key_file_database);
    unlock(config(&key_directory, Some("secret")), &passphrase_database);

    let passphrase_on_key_file =
        KeyManager::new(config(&key_directory, Some("secret"))).unlock(&key_file_database);
    let key_file_on_passphrase =
        KeyManager::new(config(&key_directory, None)).unlock(&passphrase_database);

    assert!(matches!(
        passphrase_on_key_file,
        Err(CryptoError::KeySourceMismatch {
            stored: "key file",
            configured: "passphrase"
        })
    ));
    assert!(matches!(
        key_file_on_passphrase,
        Err(CryptoError::KeySourceMismatch {
            stored: "passphrase",
            configured: "key file"
        })
    ));
}

/// A key file deleted after the database was encrypted is reported, not silently regenerated.
#[test]
fn a_missing_key_file_for_an_encrypted_database_is_an_error() {
    let key_directory = TemporaryKeyDirectory::new();
    let metadata = InMemoryMetadata::default();
    unlock(config(&key_directory, None), &metadata);
    let _ = std::fs::remove_file(key_directory.key_file());

    let outcome = KeyManager::new(config(&key_directory, None)).unlock(&metadata);

    assert!(matches!(outcome, Err(CryptoError::KeyFile { .. })));
    assert!(
        !key_directory.key_file().exists(),
        "no replacement key may be created"
    );
}

/// A key file of the wrong length is rejected.
#[test]
fn a_corrupt_key_file_is_rejected() {
    let key_directory = TemporaryKeyDirectory::new();
    let _ = std::fs::create_dir_all(&key_directory.path);
    let _ = std::fs::write(key_directory.key_file(), [1, 2, 3]);

    let outcome =
        KeyManager::new(config(&key_directory, None)).unlock(&InMemoryMetadata::default());

    assert!(matches!(outcome, Err(CryptoError::KeyFile { .. })));
}

/// Configuration reads the passphrase and key directory, ignores a blank passphrase, and falls
/// back to `$HOME/.note_taker`.
#[test]
fn configuration_is_read_from_the_environment_lookup() {
    let from = |pairs: &[(&str, &str)]| {
        let variables: HashMap<String, String> = pairs
            .iter()
            .map(|(name, value)| (String::from(*name), String::from(*value)))
            .collect();
        KeyManagerConfig::from_lookup(|name| variables.get(name).cloned())
    };

    let with_passphrase = from(&[(PASSPHRASE_VARIABLE, "secret"), ("HOME", "/home/someone")]);
    let blank_passphrase = from(&[(PASSPHRASE_VARIABLE, "  "), ("HOME", "/home/someone")]);
    let overridden = from(&[(KEY_DIRECTORY_VARIABLE, "/keys"), ("HOME", "/home/someone")]);
    let nowhere = from(&[]);

    assert!(with_passphrase.uses_passphrase());
    assert!(!blank_passphrase.uses_passphrase());
    assert_eq!(
        with_passphrase.key_directory(),
        Some(PathBuf::from("/home/someone/.note_taker").as_path())
    );
    assert_eq!(
        overridden.key_directory(),
        Some(PathBuf::from("/keys").as_path())
    );
    assert_eq!(nowhere.key_directory(), None);
    assert!(!format!("{with_passphrase:?}").contains("secret"));
}

/// Without a passphrase or any key directory, unlocking fails with guidance.
#[test]
fn unlocking_without_a_key_directory_fails() {
    let outcome = KeyManager::new(KeyManagerConfig::from_lookup(|_| None))
        .unlock(&InMemoryMetadata::default());

    assert!(matches!(outcome, Err(CryptoError::KeyFile { .. })));
}
