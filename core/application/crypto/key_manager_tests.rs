//! Specifies key resolution: random key files, Argon2id passphrase keys, stored salts, and
//! wrong-key detection through the database key-check value. The keychain section (Issue #40)
//! specifies keychain storage, migration of the old key file, fallback to the file, and the
//! missing-key error.
//!
//! Tests use tiny Argon2id parameters so the unoptimized test build stays fast, unique temporary
//! key directories that are removed afterwards, and an in-memory `MockKeychain` instead of the
//! operating-system credential store.

use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
};

use zeroize::Zeroizing;

use super::{
    key_manager::{
        KEY_DIRECTORY_VARIABLE, KEY_FILE_NAME, KEYCHAIN_ENABLED_VARIABLE, KeyManager,
        KeyManagerConfig, PASSPHRASE_VARIABLE, derive_key_from_passphrase,
    },
    voice_encryptor::VoiceEncryptor,
};
use crate::{
    domain::errors::{CryptoError, StorageError},
    ports::crypto::{
        EmbeddingCipher, EncryptionKeySource, EncryptionMetadata, EncryptionMetadataRepository,
        KeyDerivationParameters, KeychainStore,
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
            stored: "random key",
            configured: "passphrase"
        })
    ));
    assert!(matches!(
        key_file_on_passphrase,
        Err(CryptoError::KeySourceMismatch {
            stored: "passphrase",
            configured: "random key"
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

// ---------------------------------------------------------------------------------------------
// Platform keychain (Issue #40)
// ---------------------------------------------------------------------------------------------

/// Location string the mock reports, checked in error messages.
const MOCK_KEYCHAIN_LOCATION: &str = "mock keychain entry note-taker/encryption-key";

/// An in-memory keychain entry with scripted failures and call counters.
#[derive(Default)]
struct MockKeychain {
    /// The secret held by the entry, if any.
    stored: Mutex<Option<Vec<u8>>>,
    /// Returned by every `load_key` call instead of reading `stored`.
    load_failure: Option<CryptoError>,
    /// Returned by every `store_key` call instead of writing `stored`.
    store_failure: Option<CryptoError>,
    /// Simulates a store that accepts a write but keeps different bytes.
    corrupts_writes: bool,
    load_calls: AtomicUsize,
    store_calls: AtomicUsize,
}

impl MockKeychain {
    /// An entry that already holds `secret`.
    fn holding(secret: &[u8]) -> Self {
        Self {
            stored: Mutex::new(Some(secret.to_vec())),
            ..Self::default()
        }
    }

    /// Returns a copy of the stored secret.
    fn stored_bytes(&self) -> Option<Vec<u8>> {
        self.stored
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Reports whether the key manager touched the keychain at all.
    fn was_used(&self) -> bool {
        self.load_calls.load(Ordering::SeqCst) + self.store_calls.load(Ordering::SeqCst) > 0
    }
}

impl KeychainStore for MockKeychain {
    /// Returns the scripted failure or the stored secret.
    fn load_key(&self) -> Result<Option<Zeroizing<Vec<u8>>>, CryptoError> {
        self.load_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(failure) = &self.load_failure {
            return Err(failure.clone());
        }
        Ok(self.stored_bytes().map(Zeroizing::new))
    }

    /// Returns the scripted failure or replaces the stored secret.
    fn store_key(&self, key_bytes: &[u8]) -> Result<(), CryptoError> {
        self.store_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(failure) = &self.store_failure {
            return Err(failure.clone());
        }
        let mut written = key_bytes.to_vec();
        if self.corrupts_writes
            && let Some(first_byte) = written.first_mut()
        {
            *first_byte ^= 0xff;
        }
        *self.stored.lock().unwrap_or_else(PoisonError::into_inner) = Some(written);
        Ok(())
    }

    /// Names the mock entry.
    fn location(&self) -> &str {
        MOCK_KEYCHAIN_LOCATION
    }
}

/// A keychain-enabled key manager over `key_directory` without a passphrase.
fn keychain_manager(
    key_directory: &TemporaryKeyDirectory,
    keychain: &Arc<MockKeychain>,
) -> KeyManager {
    let keychain_store: Arc<dyn KeychainStore> = keychain.clone();
    KeyManager::new(config(key_directory, None)).with_keychain(keychain_store)
}

/// Unlocks through the keychain, failing the test on error.
fn keychain_unlock(
    key_directory: &TemporaryKeyDirectory,
    keychain: &Arc<MockKeychain>,
    metadata: &InMemoryMetadata,
) -> VoiceEncryptor {
    keychain_manager(key_directory, keychain)
        .unlock(metadata)
        .unwrap_or_else(|error| panic!("key manager must unlock through the keychain: {error}"))
}

/// Reads the key file, failing the test when it is missing.
fn key_file_bytes(key_directory: &TemporaryKeyDirectory) -> Vec<u8> {
    std::fs::read(key_directory.key_file())
        .unwrap_or_else(|error| panic!("key file must exist: {error}"))
}

/// Writes `key_bytes` as the key file.
fn write_key_file(key_directory: &TemporaryKeyDirectory, key_bytes: &[u8]) {
    std::fs::create_dir_all(&key_directory.path)
        .and_then(|()| std::fs::write(key_directory.key_file(), key_bytes))
        .unwrap_or_else(|error| panic!("key file must be written: {error}"));
}

/// A keychain failure caused by a locked or denied store.
fn access_denied() -> CryptoError {
    CryptoError::Keychain {
        reason: String::from("access denied"),
    }
}

/// A keychain failure caused by a missing platform store (e.g. headless Linux).
fn no_secret_service() -> CryptoError {
    CryptoError::KeychainUnavailable {
        reason: String::from("no Secret Service on the session bus"),
    }
}

/// New database, empty keychain, no key file → the key is generated into the keychain only.
#[test]
fn a_new_key_is_stored_in_the_keychain_and_no_key_file_is_written() {
    let key_directory = TemporaryKeyDirectory::new();
    let keychain = Arc::new(MockKeychain::default());
    let metadata = InMemoryMetadata::default();

    let first_launch = keychain_unlock(&key_directory, &keychain, &metadata);
    let second_launch = keychain_unlock(&key_directory, &keychain, &metadata);

    let stored = keychain.stored_bytes().unwrap_or_default();
    assert_eq!(stored.len(), 32);
    assert_ne!(stored, vec![0; 32]);
    assert!(
        !key_directory.key_file().exists(),
        "no key file may be written"
    );
    assert_eq!(
        metadata
            .stored
            .borrow()
            .as_ref()
            .map(EncryptionMetadata::key_source),
        Some(&EncryptionKeySource::KeyFile)
    );
    assert_same_key(&first_launch, &second_launch);
}

/// Old key file, empty keychain → the file's key is copied into the keychain and still opens the
/// database that was encrypted with it.
#[test]
fn an_old_key_file_is_migrated_into_the_keychain() {
    let key_directory = TemporaryKeyDirectory::new();
    let metadata = InMemoryMetadata::default();
    let before_migration = unlock(config(&key_directory, None), &metadata);
    let file_key = key_file_bytes(&key_directory);
    let keychain = Arc::new(MockKeychain::default());

    let migrated = keychain_unlock(&key_directory, &keychain, &metadata);
    let after_migration = keychain_unlock(&key_directory, &keychain, &metadata);

    assert_eq!(keychain.stored_bytes(), Some(file_key));
    assert_same_key(&before_migration, &migrated);
    assert_same_key(&before_migration, &after_migration);
}

/// A successful migration deletes the old key file.
#[test]
fn a_migrated_key_file_is_deleted() {
    let key_directory = TemporaryKeyDirectory::new();
    let metadata = InMemoryMetadata::default();
    unlock(config(&key_directory, None), &metadata);
    let keychain = Arc::new(MockKeychain::default());

    keychain_unlock(&key_directory, &keychain, &metadata);

    assert!(
        !key_directory.key_file().exists(),
        "the key file must be removed once the keychain holds the key"
    );
}

/// The keychain refuses the write → the old file is preserved and still unlocks the database.
#[test]
fn a_failed_migration_preserves_the_key_file() {
    let key_directory = TemporaryKeyDirectory::new();
    let metadata = InMemoryMetadata::default();
    let before_migration = unlock(config(&key_directory, None), &metadata);
    let file_key = key_file_bytes(&key_directory);
    let keychain = Arc::new(MockKeychain {
        store_failure: Some(access_denied()),
        ..MockKeychain::default()
    });

    let outcome = keychain_unlock(&key_directory, &keychain, &metadata);

    assert_eq!(key_file_bytes(&key_directory), file_key);
    assert_same_key(&before_migration, &outcome);
}

/// The keychain accepts the write but returns different bytes → the file is preserved.
#[test]
fn a_migration_whose_read_back_differs_preserves_the_key_file() {
    let key_directory = TemporaryKeyDirectory::new();
    let metadata = InMemoryMetadata::default();
    let before_migration = unlock(config(&key_directory, None), &metadata);
    let keychain = Arc::new(MockKeychain {
        corrupts_writes: true,
        ..MockKeychain::default()
    });

    let outcome = keychain_unlock(&key_directory, &keychain, &metadata);

    assert!(
        key_directory.key_file().exists(),
        "unverified migration must keep the file"
    );
    assert_same_key(&before_migration, &outcome);
}

/// Keychain unavailable (headless Linux) on a new database → file-based storage is used.
#[test]
fn an_unavailable_keychain_falls_back_to_a_key_file() {
    let key_directory = TemporaryKeyDirectory::new();
    let metadata = InMemoryMetadata::default();
    let keychain = Arc::new(MockKeychain {
        load_failure: Some(no_secret_service()),
        ..MockKeychain::default()
    });

    let first_launch = keychain_unlock(&key_directory, &keychain, &metadata);
    let second_launch = keychain_unlock(&key_directory, &keychain, &metadata);

    assert_eq!(key_file_bytes(&key_directory).len(), 32);
    assert_eq!(keychain.store_calls.load(Ordering::SeqCst), 0);
    assert_same_key(&first_launch, &second_launch);
}

/// Keychain access denied on an existing key-file database → the file still unlocks it.
#[test]
fn a_failing_keychain_falls_back_to_the_existing_key_file() {
    let key_directory = TemporaryKeyDirectory::new();
    let metadata = InMemoryMetadata::default();
    let before = unlock(config(&key_directory, None), &metadata);
    let keychain = Arc::new(MockKeychain {
        load_failure: Some(access_denied()),
        ..MockKeychain::default()
    });

    let outcome = keychain_unlock(&key_directory, &keychain, &metadata);

    assert!(key_directory.key_file().exists());
    assert_same_key(&before, &outcome);
}

/// The keychain cannot store a newly generated key → it is written to the key file instead.
#[test]
fn a_new_key_the_keychain_cannot_store_falls_back_to_a_key_file() {
    let key_directory = TemporaryKeyDirectory::new();
    let metadata = InMemoryMetadata::default();
    let keychain = Arc::new(MockKeychain {
        store_failure: Some(access_denied()),
        ..MockKeychain::default()
    });

    let first_launch = keychain_unlock(&key_directory, &keychain, &metadata);
    let second_launch = keychain_unlock(&key_directory, &keychain, &metadata);

    assert_eq!(key_file_bytes(&key_directory).len(), 32);
    assert_same_key(&first_launch, &second_launch);
}

/// `NOTE_TAKER_KEYCHAIN_ENABLED=false` → the keychain is never touched.
#[test]
fn a_disabled_keychain_is_never_used() {
    let key_directory = TemporaryKeyDirectory::new();
    let variables = HashMap::from([
        (
            KEY_DIRECTORY_VARIABLE,
            key_directory.path.to_string_lossy().into_owned(),
        ),
        (KEYCHAIN_ENABLED_VARIABLE, String::from("false")),
    ]);
    let keychain = Arc::new(MockKeychain::default());
    let keychain_store: Arc<dyn KeychainStore> = keychain.clone();

    let outcome = KeyManager::new(KeyManagerConfig::from_lookup(|name| {
        variables.get(name).cloned()
    }))
    .with_keychain(keychain_store)
    .unlock(&InMemoryMetadata::default());

    assert!(outcome.is_ok());
    assert!(
        !keychain.was_used(),
        "a disabled keychain must not be read or written"
    );
    assert_eq!(key_file_bytes(&key_directory).len(), 32);
}

/// Passphrase mode derives the key and never reads or writes the keychain.
#[test]
fn passphrase_mode_never_uses_the_keychain() {
    let key_directory = TemporaryKeyDirectory::new();
    let metadata = InMemoryMetadata::default();
    let keychain = Arc::new(MockKeychain::default());
    let passphrase_manager = || {
        let keychain_store: Arc<dyn KeychainStore> = keychain.clone();
        KeyManager::new(config(&key_directory, Some("correct horse"))).with_keychain(keychain_store)
    };

    let first_launch = passphrase_manager().unlock(&metadata);
    let second_launch = passphrase_manager().unlock(&metadata);

    let (Ok(first_launch), Ok(second_launch)) = (first_launch, second_launch) else {
        panic!("passphrase unlocks must succeed");
    };
    assert_same_key(&first_launch, &second_launch);
    assert!(
        !keychain.was_used(),
        "the passphrase must not touch the keychain"
    );
    assert_eq!(keychain.stored_bytes(), None);
    assert!(!key_directory.key_file().exists());
}

/// An encrypted database whose key is in neither the keychain nor the file → a clear
/// `KeyMissing` error naming both places, and no replacement key is generated.
#[test]
fn a_key_missing_from_keychain_and_file_is_a_clear_error() {
    let key_directory = TemporaryKeyDirectory::new();
    let metadata = InMemoryMetadata::default();
    keychain_unlock(
        &key_directory,
        &Arc::new(MockKeychain::default()),
        &metadata,
    );
    let empty_keychain = Arc::new(MockKeychain::default());

    let outcome = keychain_manager(&key_directory, &empty_keychain).unlock(&metadata);

    let Err(error @ CryptoError::KeyMissing { .. }) = outcome else {
        panic!(
            "expected KeyMissing, got {:?}",
            outcome.map(|_| "an encryptor")
        );
    };
    let message = error.to_string();
    assert!(message.contains(MOCK_KEYCHAIN_LOCATION), "{message}");
    assert!(
        message.contains(&key_directory.key_file().display().to_string()),
        "{message}"
    );
    assert_eq!(empty_keychain.stored_bytes(), None, "no replacement key");
    assert!(
        !key_directory.key_file().exists(),
        "no replacement key file"
    );
}

/// Unavailable keychain and no key file for an encrypted database → `KeyMissing`, not a new key.
#[test]
fn a_key_missing_while_the_keychain_is_unavailable_is_a_clear_error() {
    let key_directory = TemporaryKeyDirectory::new();
    let metadata = InMemoryMetadata::default();
    keychain_unlock(
        &key_directory,
        &Arc::new(MockKeychain::default()),
        &metadata,
    );
    let unavailable = Arc::new(MockKeychain {
        load_failure: Some(no_secret_service()),
        ..MockKeychain::default()
    });

    let outcome = keychain_manager(&key_directory, &unavailable).unlock(&metadata);

    let Err(error @ CryptoError::KeyMissing { .. }) = outcome else {
        panic!(
            "expected KeyMissing, got {:?}",
            outcome.map(|_| "an encryptor")
        );
    };
    assert!(error.to_string().contains("no Secret Service"), "{error}");
    assert!(
        !key_directory.key_file().exists(),
        "no replacement key file"
    );
}

/// Keychain and file hold the same key (an interrupted migration) → the file is removed.
#[test]
fn a_leftover_key_file_matching_the_keychain_is_removed() {
    let key_directory = TemporaryKeyDirectory::new();
    let metadata = InMemoryMetadata::default();
    let keychain = Arc::new(MockKeychain::default());
    let before = keychain_unlock(&key_directory, &keychain, &metadata);
    write_key_file(&key_directory, &keychain.stored_bytes().unwrap_or_default());

    let outcome = keychain_unlock(&key_directory, &keychain, &metadata);

    assert!(!key_directory.key_file().exists());
    assert_same_key(&before, &outcome);
}

/// Keychain and file hold different keys → whichever matches the database unlocks it, and the
/// file is kept because it may be the only copy of that key.
#[test]
fn a_conflicting_key_file_is_kept_and_still_unlocks_its_database() {
    let key_directory = TemporaryKeyDirectory::new();
    let metadata = InMemoryMetadata::default();
    let file_encrypted = unlock(config(&key_directory, None), &metadata);
    let file_key = key_file_bytes(&key_directory);
    let keychain = Arc::new(MockKeychain::holding(&[7; 32]));

    let outcome = keychain_unlock(&key_directory, &keychain, &metadata);

    assert_same_key(&file_encrypted, &outcome);
    assert_eq!(key_file_bytes(&key_directory), file_key);
    assert_eq!(keychain.stored_bytes(), Some(vec![7; 32]));
}

/// A keychain entry that is not a 32-byte key is an error; it is neither replaced nor bypassed.
#[test]
fn a_corrupt_keychain_entry_is_rejected() {
    let key_directory = TemporaryKeyDirectory::new();
    let keychain = Arc::new(MockKeychain::holding(&[1, 2, 3]));

    let outcome = keychain_manager(&key_directory, &keychain).unlock(&InMemoryMetadata::default());

    assert!(matches!(outcome, Err(CryptoError::Keychain { .. })));
    assert_eq!(keychain.stored_bytes(), Some(vec![1, 2, 3]));
    assert!(!key_directory.key_file().exists());
}

/// `NOTE_TAKER_KEYCHAIN_ENABLED` defaults to true; false, 0, no, and off (any case) disable it.
#[test]
fn keychain_configuration_is_read_from_the_environment_lookup() {
    let keychain_enabled = |value: Option<&str>| {
        KeyManagerConfig::from_lookup(|name| {
            (name == KEYCHAIN_ENABLED_VARIABLE)
                .then_some(value)
                .flatten()
                .map(String::from)
        })
        .keychain_enabled()
    };

    assert!(keychain_enabled(None));
    assert!(keychain_enabled(Some("true")));
    assert!(keychain_enabled(Some("1")));
    assert!(keychain_enabled(Some("  ")));
    for disabled in ["false", "FALSE", "0", "no", "Off", " false "] {
        assert!(
            !keychain_enabled(Some(disabled)),
            "{disabled:?} must disable"
        );
    }
}
