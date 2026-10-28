//! Resolves the database encryption key from a passphrase, the platform keychain, or a local
//! random key file.
//!
//! Key sources, chosen once per database and recorded in its encryption metadata:
//!
//! - **Passphrase** (`NOTE_TAKER_ENCRYPTION_PASSPHRASE` set): the key is derived with Argon2id
//!   from the passphrase and a random 16-byte salt. The salt and parameters are stored in the
//!   database (they are not secret), so the database plus the passphrase is always sufficient.
//!   Nothing secret touches the disk, but a forgotten passphrase makes the embeddings
//!   unrecoverable, and the passphrase is visible to anything that can read the process
//!   environment.
//! - **Random key** (no passphrase): a random 256-bit key is generated once. It lives in the
//!   platform keychain (`KeychainStore`, Issue #40) when one is injected and
//!   `NOTE_TAKER_KEYCHAIN_ENABLED` is not false. Only when the keychain is disabled that way does
//!   it live in `encryption.key` (owner-only permissions) inside
//!   `NOTE_TAKER_ENCRYPTION_KEY_DIR`, or `~/.note_taker` by default. The key file protects a
//!   database file that leaks on its own, but not against an attacker who can read the user's
//!   files; the keychain additionally keeps the key off the data disk and behind the OS login.
//!
//! Keychain resolution, when the keychain is active:
//!
//! - Entry present: it is used. A leftover key file with the same key (an interrupted migration)
//!   is deleted; a key file with a different key is kept and tried as well.
//! - Entry absent, key file present: the file's key is migrated. It is written to the keychain,
//!   read back, and the file is deleted only when the read-back matches; otherwise unlocking
//!   fails and the file is kept for a later attempt.
//! - Entry absent, no key file: a new database gets a fresh key in the keychain; an encrypted
//!   database fails with `KeyMissing`.
//! - Keychain unavailable or failing: unlocking fails with `KeychainUnavailable` or `Keychain`.
//!   There is no silent fallback to a key file (owner decision, see `DECISION_LOG.md`), so the
//!   caller reports the error or offers a retry.
//!
//! Every unlock verifies the database's key-check value, so a wrong passphrase or a replaced key
//! file is reported before any embedding is read or migrated. The returned `VoiceEncryptor`
//! holds the key for the session, so derivation runs once per process.

use std::{
    fmt,
    fs::{self, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use aes_gcm::aead::{OsRng, rand_core::RngCore};
use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::Zeroizing;

use super::voice_encryptor::{EncryptionKey, VoiceEncryptor};
use crate::{
    domain::errors::{CryptoError, StorageError},
    ports::crypto::{
        EncryptionKeySource, EncryptionMetadata, EncryptionMetadataRepository,
        KeyDerivationParameters, KeychainStore,
    },
};

/// Environment variable holding the optional encryption passphrase.
pub const PASSPHRASE_VARIABLE: &str = "NOTE_TAKER_ENCRYPTION_PASSPHRASE";
/// Environment variable overriding the directory that holds the random key file.
pub const KEY_DIRECTORY_VARIABLE: &str = "NOTE_TAKER_ENCRYPTION_KEY_DIR";
/// Environment variable that disables the platform keychain when set to false, 0, no, or off
/// (any case); unset or any other value leaves it enabled.
pub const KEYCHAIN_ENABLED_VARIABLE: &str = "NOTE_TAKER_KEYCHAIN_ENABLED";
/// File name of the random key inside the key directory.
pub const KEY_FILE_NAME: &str = "encryption.key";
/// Directory created under the home directory when no override is configured.
const DEFAULT_KEY_DIRECTORY_NAME: &str = ".note_taker";
/// Length of the random Argon2id salt.
const SALT_LENGTH: usize = 16;

/// Selects the key source and key location.
pub struct KeyManagerConfig {
    passphrase: Option<Zeroizing<String>>,
    key_directory: Option<PathBuf>,
    keychain_enabled: bool,
    derivation_parameters: KeyDerivationParameters,
}

impl KeyManagerConfig {
    /// Reads the configuration through `lookup`, which returns an environment variable's value.
    ///
    /// Inputs: A variable lookup; a blank passphrase counts as unset. The key directory is
    /// `NOTE_TAKER_ENCRYPTION_KEY_DIR`, else `$HOME/.note_taker`, else `%USERPROFILE%\.note_taker`.
    /// The keychain is enabled unless `NOTE_TAKER_KEYCHAIN_ENABLED` is false, 0, no, or off.
    /// Outputs: The configuration with default (OWASP) Argon2id parameters.
    /// Side effects: None beyond calling `lookup`.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        let non_blank = |value: &String| !value.trim().is_empty();
        let key_directory = lookup(KEY_DIRECTORY_VARIABLE)
            .filter(non_blank)
            .map(PathBuf::from)
            .or_else(|| {
                lookup("HOME")
                    .filter(non_blank)
                    .or_else(|| lookup("USERPROFILE").filter(non_blank))
                    .map(|home| PathBuf::from(home).join(DEFAULT_KEY_DIRECTORY_NAME))
            });
        Self {
            passphrase: lookup(PASSPHRASE_VARIABLE)
                .filter(non_blank)
                .map(Zeroizing::new),
            key_directory,
            keychain_enabled: !lookup(KEYCHAIN_ENABLED_VARIABLE)
                .is_some_and(|value| disables_keychain(&value)),
            derivation_parameters: KeyDerivationParameters::default(),
        }
    }

    /// Reads the configuration from the process environment.
    ///
    /// Inputs: None.
    /// Outputs: See `from_lookup`.
    /// Side effects: Reads environment variables.
    pub fn from_environment() -> Self {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Overrides the Argon2id parameters used when a passphrase database is first created.
    ///
    /// Inputs: The parameters; existing databases keep the parameters they recorded.
    /// Outputs: The updated configuration.
    /// Side effects: None.
    pub fn with_derivation_parameters(mut self, parameters: KeyDerivationParameters) -> Self {
        self.derivation_parameters = parameters;
        self
    }

    /// Reports whether a passphrase is configured.
    pub fn uses_passphrase(&self) -> bool {
        self.passphrase.is_some()
    }

    /// Reports whether the platform keychain may hold the random key. Passphrase mode never uses
    /// it, whatever this returns.
    pub fn keychain_enabled(&self) -> bool {
        self.keychain_enabled
    }

    /// Returns the directory that holds (or will hold) the random key file, if one is known.
    pub fn key_directory(&self) -> Option<&Path> {
        self.key_directory.as_deref()
    }

    /// Names the configured key source for error messages.
    fn configured_label(&self) -> &'static str {
        if self.uses_passphrase() {
            "passphrase"
        } else {
            EncryptionKeySource::KeyFile.label()
        }
    }

    /// Returns the key file path, or a `KeyFile` error when no directory is known.
    fn key_file_path(&self) -> Result<PathBuf, CryptoError> {
        self.key_directory
            .as_ref()
            .map(|directory| directory.join(KEY_FILE_NAME))
            .ok_or_else(|| CryptoError::KeyFile {
                reason: format!(
                    "no key directory is known; set {KEY_DIRECTORY_VARIABLE} or {PASSPHRASE_VARIABLE}"
                ),
            })
    }
}

impl fmt::Debug for KeyManagerConfig {
    /// Prints the configuration with the passphrase redacted.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KeyManagerConfig")
            .field(
                "passphrase",
                &self.passphrase.as_ref().map(|_| "<redacted>"),
            )
            .field("key_directory", &self.key_directory)
            .field("keychain_enabled", &self.keychain_enabled)
            .field("derivation_parameters", &self.derivation_parameters)
            .finish()
    }
}

/// Whether resolving the random key may create one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RandomKeyNeed {
    /// The database has no metadata yet, so a missing key is generated.
    CreateIfMissing,
    /// The database is already encrypted, so a missing key is an error.
    MustExist,
}

/// The random key to use, plus a conflicting key-file key to try if it does not match.
struct RandomKeys {
    /// The keychain key, or the key file's key when the keychain is not used.
    preferred: EncryptionKey,
    /// A key file whose key differs from the keychain entry; kept, never deleted.
    conflicting_key_file: Option<EncryptionKey>,
}

impl RandomKeys {
    /// Wraps a single key with no alternative.
    fn single(preferred: EncryptionKey) -> Self {
        Self {
            preferred,
            conflicting_key_file: None,
        }
    }
}

/// Unlocks a database's encryption key according to a `KeyManagerConfig`.
pub struct KeyManager {
    config: KeyManagerConfig,
    keychain: Option<Arc<dyn KeychainStore>>,
}

impl KeyManager {
    /// Creates a key manager.
    ///
    /// Inputs: The configuration.
    /// Outputs: The manager.
    /// Side effects: None.
    pub fn new(config: KeyManagerConfig) -> Self {
        Self {
            config,
            keychain: None,
        }
    }

    /// Adds the platform keychain as the preferred home of the random key.
    ///
    /// Inputs: The keychain store. It is ignored in passphrase mode and when the configuration
    /// disables the keychain.
    /// Outputs: The updated manager.
    /// Side effects: None until `unlock`.
    pub fn with_keychain(mut self, keychain: Arc<dyn KeychainStore>) -> Self {
        self.keychain = Some(keychain);
        self
    }

    /// Returns the encryptor for the database behind `metadata_repository`.
    ///
    /// Inputs: The database's encryption metadata store.
    /// Outputs: An encryptor holding the verified key, or `KeySourceMismatch`, `WrongKey`,
    /// `KeyFile`, `KeyDerivation`, or `Metadata` errors.
    /// Side effects: On a database's first unlock, generates the salt (passphrase) or reads or
    /// creates the random key (keychain or key file), then stores the metadata record. Without a
    /// passphrase, any unlock may migrate the key file into the keychain and delete it, or delete
    /// a leftover key file that matches the keychain. Logs keychain fallbacks; never logs key
    /// material.
    pub fn unlock<MetadataStore>(
        &self,
        metadata_repository: &MetadataStore,
    ) -> Result<VoiceEncryptor, CryptoError>
    where
        MetadataStore: EncryptionMetadataRepository + ?Sized,
    {
        if let Some(metadata) = metadata_repository
            .load_encryption_metadata()
            .map_err(metadata_error)?
        {
            return self.unlock_existing(&metadata);
        }
        let (key_source, key) = self.create_key()?;
        let encryptor = VoiceEncryptor::new(&key);
        let metadata = EncryptionMetadata::new(key_source, encryptor.seal_key_check()?);
        if metadata_repository
            .save_encryption_metadata(&metadata)
            .map_err(metadata_error)?
        {
            tracing::info!(
                key_source = metadata.key_source().label(),
                "Database encryption initialized"
            );
            return Ok(encryptor);
        }
        // Another process initialized this database first; adopt its record.
        let stored_metadata = metadata_repository
            .load_encryption_metadata()
            .map_err(metadata_error)?
            .ok_or_else(|| CryptoError::Metadata {
                reason: String::from("encryption metadata disappeared during initialization"),
            })?;
        self.unlock_existing(&stored_metadata)
    }

    /// Rebuilds the key recorded in `metadata` and verifies it.
    ///
    /// Inputs: The stored metadata.
    /// Outputs: The encryptor, or a mismatch, key-file, keychain, missing-key, derivation, or
    /// wrong-key error.
    /// Side effects: Reads the keychain or key file (see `random_keys`) or runs Argon2id.
    fn unlock_existing(
        &self,
        metadata: &EncryptionMetadata,
    ) -> Result<VoiceEncryptor, CryptoError> {
        let key = match (metadata.key_source(), &self.config.passphrase) {
            (EncryptionKeySource::KeyFile, None) => {
                let random_keys = self.random_keys(RandomKeyNeed::MustExist)?;
                return verify_random_keys(random_keys, metadata.key_check());
            }
            (EncryptionKeySource::Passphrase { salt, parameters }, Some(passphrase)) => {
                derive_key_from_passphrase(passphrase, salt, *parameters)?
            }
            (stored_source, _) => {
                return Err(CryptoError::KeySourceMismatch {
                    stored: stored_source.label(),
                    configured: self.config.configured_label(),
                });
            }
        };
        let encryptor = VoiceEncryptor::new(&key);
        encryptor.verify_key_check(metadata.key_check())?;
        Ok(encryptor)
    }

    /// Creates the key for a database that has no metadata yet.
    ///
    /// Inputs: None.
    /// Outputs: The key source to record and the key.
    /// Side effects: Generates a salt and runs Argon2id, or resolves the random key through the
    /// keychain or key file (see `random_keys`).
    fn create_key(&self) -> Result<(EncryptionKeySource, EncryptionKey), CryptoError> {
        match &self.config.passphrase {
            Some(passphrase) => {
                let mut salt = vec![0_u8; SALT_LENGTH];
                OsRng.fill_bytes(&mut salt);
                let parameters = self.config.derivation_parameters;
                let key = derive_key_from_passphrase(passphrase, &salt, parameters)?;
                Ok((EncryptionKeySource::Passphrase { salt, parameters }, key))
            }
            None => Ok((
                EncryptionKeySource::KeyFile,
                self.random_keys(RandomKeyNeed::CreateIfMissing)?.preferred,
            )),
        }
    }

    /// Returns the keychain when it may hold the random key: injected, enabled, no passphrase.
    fn active_keychain(&self) -> Option<&dyn KeychainStore> {
        if !self.config.keychain_enabled || self.config.uses_passphrase() {
            return None;
        }
        self.keychain.as_deref()
    }

    /// Resolves the random key through the keychain, migrating an old key file into it.
    ///
    /// Inputs: Whether a missing key may be generated.
    /// Outputs: The key(s) to try, or `KeychainUnavailable` / `Keychain` (unreachable, failing,
    /// or corrupt entry), `KeyMissing`, or `KeyFile`.
    /// Side effects: Reads and may write the keychain; may read or delete a key file, and creates
    /// one only when the keychain is disabled. Logs failures and migrations without key material.
    fn random_keys(&self, need: RandomKeyNeed) -> Result<RandomKeys, CryptoError> {
        let Some(keychain) = self.active_keychain() else {
            return self.key_file_key(need).map(RandomKeys::single);
        };
        match keychain.load_key() {
            Ok(Some(key_bytes)) => {
                let keychain_key = key_from_keychain_bytes(keychain, &key_bytes)?;
                Ok(self.reconcile_leftover_key_file(keychain_key))
            }
            Ok(None) => self
                .key_without_keychain_entry(keychain, need)
                .map(RandomKeys::single),
            Err(keychain_error) => {
                log_keychain_failure(keychain, &keychain_error, "the database cannot be unlocked");
                Err(keychain_error)
            }
        }
    }

    /// Resolves the key when the keychain is reachable but has no entry.
    ///
    /// Inputs: The keychain and whether a missing key may be generated.
    /// Outputs: The migrated or new key, or `KeyMissing` / `KeyFile` / keychain errors.
    /// Side effects: Migrates an existing key file (see `migrate_key_file`), or stores a new key
    /// in the keychain. Never writes a key file.
    fn key_without_keychain_entry(
        &self,
        keychain: &dyn KeychainStore,
        need: RandomKeyNeed,
    ) -> Result<EncryptionKey, CryptoError> {
        let key_file_path = self.config.key_file_path();
        if let Ok(key_file_path) = &key_file_path
            && key_file_path.exists()
        {
            return migrate_key_file(keychain, key_file_path);
        }
        if need == RandomKeyNeed::MustExist {
            return Err(self.key_missing(&format!("{} does not exist", keychain.location())));
        }
        match store_and_read_back(keychain, &EncryptionKey::generate()) {
            Ok(stored_key) => {
                tracing::info!(
                    keychain = keychain.location(),
                    "Generated encryption key in the platform keychain"
                );
                Ok(stored_key)
            }
            Err(keychain_error) => {
                log_keychain_failure(keychain, &keychain_error, "the new key was not stored");
                Err(keychain_error)
            }
        }
    }

    /// Checks for a key file next to a keychain entry and decides what to do with it.
    ///
    /// Inputs: The key read from the keychain.
    /// Outputs: The keychain key, plus the file's key when it differs.
    /// Side effects: Deletes a key file holding the same key (an interrupted migration); keeps a
    /// conflicting or unreadable one and logs a warning.
    fn reconcile_leftover_key_file(&self, keychain_key: EncryptionKey) -> RandomKeys {
        let Ok(key_file_path) = self.config.key_file_path() else {
            return RandomKeys::single(keychain_key);
        };
        if !key_file_path.exists() {
            return RandomKeys::single(keychain_key);
        }
        match load_key_file(&key_file_path) {
            Ok(file_key) if file_key.as_bytes() == keychain_key.as_bytes() => {
                delete_migrated_key_file(&key_file_path);
                RandomKeys::single(keychain_key)
            }
            Ok(file_key) => {
                tracing::warn!(
                    path = %key_file_path.display(),
                    "Encryption key file differs from the keychain entry; keeping both"
                );
                RandomKeys {
                    preferred: keychain_key,
                    conflicting_key_file: Some(file_key),
                }
            }
            Err(file_error) => {
                tracing::warn!(error = %file_error, "Ignoring an unreadable leftover encryption key file");
                RandomKeys::single(keychain_key)
            }
        }
    }

    /// Loads, or for a new database creates, the key file.
    ///
    /// Inputs: Whether a missing file may be created.
    /// Outputs: The key, or a `KeyFile` error.
    /// Side effects: See `load_or_create_key_file` and `load_key_file`.
    fn key_file_key(&self, need: RandomKeyNeed) -> Result<EncryptionKey, CryptoError> {
        let key_file_path = self.config.key_file_path()?;
        match need {
            RandomKeyNeed::CreateIfMissing => load_or_create_key_file(&key_file_path),
            RandomKeyNeed::MustExist => load_key_file(&key_file_path),
        }
    }

    /// Builds the `KeyMissing` error naming the keychain state and the key file.
    ///
    /// Inputs: What happened to the keychain entry.
    /// Outputs: The error; it names locations only.
    /// Side effects: None.
    fn key_missing(&self, keychain_state: &str) -> CryptoError {
        let key_file_state = match self.config.key_file_path() {
            Ok(key_file_path) => format!("{} does not exist", key_file_path.display()),
            Err(_) => format!("no key file directory is known ({KEY_DIRECTORY_VARIABLE} or HOME)"),
        };
        CryptoError::KeyMissing {
            reason: format!(
                "{keychain_state}, and {key_file_state}; restore the keychain entry or the key \
                 file to read this database's voice embeddings"
            ),
        }
    }
}

/// Tries the preferred random key, then a conflicting key file's key, against the key check.
///
/// Inputs: The candidate keys and the database's key-check value.
/// Outputs: The encryptor for the first key that verifies, or `WrongKey`.
/// Side effects: Logs a warning when only the conflicting key file matches.
fn verify_random_keys(
    random_keys: RandomKeys,
    key_check: &[u8],
) -> Result<VoiceEncryptor, CryptoError> {
    let preferred = VoiceEncryptor::new(&random_keys.preferred);
    match (
        preferred.verify_key_check(key_check),
        random_keys.conflicting_key_file,
    ) {
        (Ok(()), _) => Ok(preferred),
        (Err(CryptoError::WrongKey), Some(file_key)) => {
            let from_file = VoiceEncryptor::new(&file_key);
            from_file.verify_key_check(key_check)?;
            tracing::warn!(
                "This database uses the key file, not the keychain entry; keep the key file"
            );
            Ok(from_file)
        }
        (Err(error), _) => Err(error),
    }
}

/// Copies the key file's key into the keychain and deletes the file once the copy is verified.
///
/// Inputs: The keychain and the existing key file.
/// Outputs: The file's key once the keychain holds it, a `KeyFile` error for an unreadable or
/// invalid file, or a keychain error when the copy fails, so the file is not used instead.
/// Side effects: Writes and reads back the keychain entry; deletes the file only when the
/// read-back equals the file. Any failure keeps the file and logs an error.
fn migrate_key_file(
    keychain: &dyn KeychainStore,
    key_file_path: &Path,
) -> Result<EncryptionKey, CryptoError> {
    let file_key = load_key_file(key_file_path)?;
    match store_and_read_back(keychain, &file_key) {
        Ok(stored_key) if stored_key.as_bytes() == file_key.as_bytes() => {
            tracing::info!(
                keychain = keychain.location(),
                path = %key_file_path.display(),
                "Migrated the encryption key file into the platform keychain"
            );
            delete_migrated_key_file(key_file_path);
        }
        Ok(_) => {
            tracing::error!(
                keychain = keychain.location(),
                path = %key_file_path.display(),
                "Encryption key migration failed: the keychain returned a different key; keeping the key file"
            );
            return Err(CryptoError::Keychain {
                reason: format!(
                    "{} returned a different key than the one written; the key file was kept",
                    keychain.location()
                ),
            });
        }
        Err(keychain_error) => {
            tracing::error!(
                keychain = keychain.location(),
                path = %key_file_path.display(),
                error = %keychain_error,
                "Encryption key migration to the platform keychain failed; keeping the key file"
            );
            return Err(keychain_error);
        }
    }
    Ok(file_key)
}

/// Writes `key` to the keychain and returns what the keychain then holds.
///
/// Inputs: The keychain and the key to store.
/// Outputs: The key read back (callers compare it when it must equal `key`), or `Keychain` /
/// `KeychainUnavailable` when the write fails or the entry is empty or invalid afterwards.
/// Side effects: Writes and reads the keychain entry.
fn store_and_read_back(
    keychain: &dyn KeychainStore,
    key: &EncryptionKey,
) -> Result<EncryptionKey, CryptoError> {
    keychain.store_key(key.as_bytes())?;
    match keychain.load_key()? {
        Some(key_bytes) => key_from_keychain_bytes(keychain, &key_bytes),
        None => Err(CryptoError::Keychain {
            reason: format!(
                "{} was empty right after it was written",
                keychain.location()
            ),
        }),
    }
}

/// Validates a keychain secret as a 32-byte key.
///
/// Inputs: The keychain (for the message) and the secret.
/// Outputs: The key, or `Keychain` when the entry has another length.
/// Side effects: None.
fn key_from_keychain_bytes(
    keychain: &dyn KeychainStore,
    key_bytes: &[u8],
) -> Result<EncryptionKey, CryptoError> {
    let key_array: [u8; 32] = key_bytes.try_into().map_err(|_| CryptoError::Keychain {
        reason: format!(
            "{} does not hold a 32-byte encryption key; restore the original entry",
            keychain.location()
        ),
    })?;
    Ok(EncryptionKey::from_bytes(key_array))
}

/// Deletes a key file whose key is now in the keychain.
///
/// Inputs: The key file path.
/// Outputs: None; a failure is logged because the key is safe in both places.
/// Side effects: Removes the file. A file another process already removed counts as success.
fn delete_migrated_key_file(key_file_path: &Path) {
    match fs::remove_file(key_file_path) {
        Ok(()) => {
            tracing::info!(path = %key_file_path.display(), "Deleted the migrated encryption key file")
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => tracing::warn!(
            path = %key_file_path.display(),
            %error,
            "The key is in the keychain, but the old key file could not be deleted; delete it manually"
        ),
    }
}

/// Logs a keychain failure at a level that matches its cause.
///
/// Inputs: The keychain, the error, and what happens next.
/// Outputs: None.
/// Side effects: A warning for a keychain that does not exist on this machine, an error for an
/// access failure.
fn log_keychain_failure(keychain: &dyn KeychainStore, error: &CryptoError, consequence: &str) {
    if matches!(error, CryptoError::KeychainUnavailable { .. }) {
        tracing::warn!(keychain = keychain.location(), %error, "Platform keychain unavailable; {consequence}");
    } else {
        tracing::error!(keychain = keychain.location(), %error, "Platform keychain access failed; {consequence}");
    }
}

/// Reports whether a `NOTE_TAKER_KEYCHAIN_ENABLED` value turns the keychain off.
fn disables_keychain(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "no" | "off"
    )
}

/// Derives a 256-bit key from a passphrase with Argon2id (version 0x13).
///
/// Inputs: The passphrase, a salt of at least 8 bytes, and the cost parameters.
/// Outputs: The key, or `KeyDerivation` for invalid parameters or salt.
/// Side effects: Allocates `memory_kib` KiB and runs CPU-bound hashing; call it off UI threads.
pub fn derive_key_from_passphrase(
    passphrase: &str,
    salt: &[u8],
    parameters: KeyDerivationParameters,
) -> Result<EncryptionKey, CryptoError> {
    let derivation_error = |error: argon2::Error| CryptoError::KeyDerivation {
        reason: error.to_string(),
    };
    let argon2_parameters = Params::new(
        parameters.memory_kib(),
        parameters.iterations(),
        parameters.parallelism(),
        Some(32),
    )
    .map_err(derivation_error)?;
    let mut key_bytes = Zeroizing::new([0_u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2_parameters)
        .hash_password_into(passphrase.as_bytes(), salt, key_bytes.as_mut())
        .map_err(derivation_error)?;
    Ok(EncryptionKey::from_bytes(*key_bytes))
}

/// Loads the key file, creating it with a fresh random key when it does not exist.
///
/// Inputs: The key file path.
/// Outputs: The key, or a `KeyFile` error.
/// Side effects: May create the key directory (mode 0700 on Unix) and the key file (mode 0600,
/// never overwritten).
fn load_or_create_key_file(key_file_path: &Path) -> Result<EncryptionKey, CryptoError> {
    if key_file_path.exists() {
        return load_key_file(key_file_path);
    }
    if let Some(key_directory) = key_file_path.parent() {
        create_private_directory(key_directory)?;
    }
    let key = EncryptionKey::generate();
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(key_file_path) {
        Ok(mut key_file) => {
            key_file
                .write_all(key.as_bytes())
                .and_then(|()| key_file.sync_all())
                .map_err(|error| key_file_error(key_file_path, "write", &error))?;
            tracing::info!(path = %key_file_path.display(), "Generated encryption key file");
            Ok(key)
        }
        // Another process created the file between the existence check and the open.
        Err(error) if error.kind() == ErrorKind::AlreadyExists => load_key_file(key_file_path),
        Err(error) => Err(key_file_error(key_file_path, "create", &error)),
    }
}

/// Reads and validates an existing key file.
///
/// Inputs: The key file path.
/// Outputs: The key, or a `KeyFile` error when it is missing, unreadable, or not 32 bytes.
/// Side effects: Reads the file; warns (without key bytes) when it is readable by other users.
fn load_key_file(key_file_path: &Path) -> Result<EncryptionKey, CryptoError> {
    let key_bytes = Zeroizing::new(fs::read(key_file_path).map_err(|error| {
        if error.kind() == ErrorKind::NotFound {
            CryptoError::KeyFile {
                reason: format!(
                    "{} is missing; restore it to read this database's voice embeddings",
                    key_file_path.display()
                ),
            }
        } else {
            key_file_error(key_file_path, "read", &error)
        }
    })?);
    let key_array: [u8; 32] =
        key_bytes
            .as_slice()
            .try_into()
            .map_err(|_| CryptoError::KeyFile {
                reason: format!("{} must contain exactly 32 bytes", key_file_path.display()),
            })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(file_metadata) = fs::metadata(key_file_path)
            && file_metadata.permissions().mode() & 0o077 != 0
        {
            tracing::warn!(path = %key_file_path.display(), "Encryption key file is accessible to other users");
        }
    }
    Ok(EncryptionKey::from_bytes(key_array))
}

/// Creates the key directory (and missing parents) with owner-only permissions on Unix.
///
/// Inputs: The directory path.
/// Outputs: Success, or a `KeyFile` error.
/// Side effects: Creates directories; existing directories keep their permissions.
fn create_private_directory(key_directory: &Path) -> Result<(), CryptoError> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(key_directory)
        .map_err(|error| key_file_error(key_directory, "create directory", &error))
}

/// Builds a `KeyFile` error naming the path and operation.
fn key_file_error(path: &Path, operation: &str, error: &std::io::Error) -> CryptoError {
    CryptoError::KeyFile {
        reason: format!("unable to {operation} {}: {error}", path.display()),
    }
}

/// Wraps a metadata storage failure.
fn metadata_error(error: StorageError) -> CryptoError {
    CryptoError::Metadata {
        reason: error.to_string(),
    }
}
