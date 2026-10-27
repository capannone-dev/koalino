//! Resolves the database encryption key from a passphrase or a local random key file.
//!
//! Key sources, chosen once per database and recorded in its encryption metadata:
//!
//! - **Passphrase** (`NOTE_TAKER_ENCRYPTION_PASSPHRASE` set): the key is derived with Argon2id
//!   from the passphrase and a random 16-byte salt. The salt and parameters are stored in the
//!   database (they are not secret), so the database plus the passphrase is always sufficient.
//!   Nothing secret touches the disk, but a forgotten passphrase makes the embeddings
//!   unrecoverable, and the passphrase is visible to anything that can read the process
//!   environment.
//! - **Key file** (no passphrase): a random 256-bit key is generated once and stored in
//!   `encryption.key` (owner-only permissions) inside `NOTE_TAKER_ENCRYPTION_KEY_DIR`, or
//!   `~/.note_taker` by default. This needs no user interaction and protects a database file
//!   that leaks on its own (backups, sync folders, copies), but not against an attacker who can
//!   read the user's files, because the key sits on the same disk.
//!
//! Every unlock verifies the database's key-check value, so a wrong passphrase or a replaced key
//! file is reported before any embedding is read or migrated. The returned `VoiceEncryptor`
//! holds the key for the session, so derivation runs once per process.

use std::{
    fmt,
    fs::{self, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

use aes_gcm::aead::{OsRng, rand_core::RngCore};
use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::Zeroizing;

use super::voice_encryptor::{EncryptionKey, VoiceEncryptor};
use crate::{
    domain::errors::{CryptoError, StorageError},
    ports::crypto::{
        EncryptionKeySource, EncryptionMetadata, EncryptionMetadataRepository,
        KeyDerivationParameters,
    },
};

/// Environment variable holding the optional encryption passphrase.
pub const PASSPHRASE_VARIABLE: &str = "NOTE_TAKER_ENCRYPTION_PASSPHRASE";
/// Environment variable overriding the directory that holds the random key file.
pub const KEY_DIRECTORY_VARIABLE: &str = "NOTE_TAKER_ENCRYPTION_KEY_DIR";
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
    derivation_parameters: KeyDerivationParameters,
}

impl KeyManagerConfig {
    /// Reads the configuration through `lookup`, which returns an environment variable's value.
    ///
    /// Inputs: A variable lookup; a blank passphrase counts as unset. The key directory is
    /// `NOTE_TAKER_ENCRYPTION_KEY_DIR`, else `$HOME/.note_taker`, else `%USERPROFILE%\.note_taker`.
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
            .field("derivation_parameters", &self.derivation_parameters)
            .finish()
    }
}

/// Unlocks a database's encryption key according to a `KeyManagerConfig`.
pub struct KeyManager {
    config: KeyManagerConfig,
}

impl KeyManager {
    /// Creates a key manager.
    ///
    /// Inputs: The configuration.
    /// Outputs: The manager.
    /// Side effects: None.
    pub fn new(config: KeyManagerConfig) -> Self {
        Self { config }
    }

    /// Returns the encryptor for the database behind `metadata_repository`.
    ///
    /// Inputs: The database's encryption metadata store.
    /// Outputs: An encryptor holding the verified key, or `KeySourceMismatch`, `WrongKey`,
    /// `KeyFile`, `KeyDerivation`, or `Metadata` errors.
    /// Side effects: On a database's first unlock, generates the salt (passphrase) or reads or
    /// creates the key file (no passphrase), then stores the metadata record. Later unlocks only
    /// read. Never logs key material.
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
    /// Outputs: The encryptor, or a mismatch, key-file, derivation, or wrong-key error.
    /// Side effects: Reads the key file or runs Argon2id.
    fn unlock_existing(
        &self,
        metadata: &EncryptionMetadata,
    ) -> Result<VoiceEncryptor, CryptoError> {
        let key = match (metadata.key_source(), &self.config.passphrase) {
            (EncryptionKeySource::KeyFile, None) => load_key_file(&self.config.key_file_path()?)?,
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
    /// Side effects: Generates a salt and runs Argon2id, or reads or creates the key file.
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
                load_or_create_key_file(&self.config.key_file_path()?)?,
            )),
        }
    }
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
