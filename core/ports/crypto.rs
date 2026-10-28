//! Declares the at-rest encryption contracts used by storage adapters for voice embeddings.
//!
//! Storage adapters seal every embedding through `EmbeddingCipher` before writing it and open it
//! after reading, so they never handle keys or cipher primitives. `EncryptionMetadataRepository`
//! persists the non-secret facts needed to unlock a database again: which key source it uses, the
//! Argon2id salt and parameters for passphrase keys, and a key-check value that detects a wrong
//! key before any embedding is touched. `KeychainStore` (Issue #40) holds the random database key
//! in the operating system's credential store, so the application layer never links a platform
//! keychain library.

use zeroize::Zeroizing;

use crate::domain::errors::{CryptoError, StorageError};

/// Encrypts and authenticates derived voice embeddings, and other at-rest secrets, for storage.
pub trait EmbeddingCipher: Send + Sync {
    /// Encrypts `embedding` and binds it to `context`.
    ///
    /// Inputs: `context` names the owning row (for example `voice_variants:<uuid>`) and must be
    /// supplied again to open the value; `embedding` is a biometric vector that is never logged.
    /// Outputs: A self-describing sealed value, or `EncryptionFailed`.
    /// Side effects: Draws a fresh random nonce from the operating system.
    fn seal_embedding(&self, context: &str, embedding: &[f32]) -> Result<Vec<u8>, CryptoError>;

    /// Verifies and decrypts a value produced by `seal_embedding`.
    ///
    /// Inputs: The same `context` used when sealing and the stored bytes.
    /// Outputs: The original embedding, or `MalformedCiphertext` / `AuthenticationFailed`.
    /// Side effects: None.
    fn open_embedding(&self, context: &str, sealed: &[u8]) -> Result<Vec<f32>, CryptoError>;

    /// Encrypts an arbitrary secret (such as the P2P node key) and binds it to `context`.
    ///
    /// Inputs: `context` names the owning record (for example `p2p_node_identity`); `secret` is
    /// key material that is never logged.
    /// Outputs: A self-describing sealed value, or `EncryptionFailed`.
    /// Side effects: Draws a fresh random nonce from the operating system.
    fn seal_secret(&self, context: &str, secret: &[u8]) -> Result<Vec<u8>, CryptoError>;

    /// Verifies and decrypts a value produced by `seal_secret`.
    ///
    /// Inputs: The same `context` used when sealing and the stored bytes.
    /// Outputs: The secret in memory that is wiped on drop, or `MalformedCiphertext` /
    /// `AuthenticationFailed` (wrong key, wrong context, or tampering).
    /// Side effects: None.
    fn open_secret(&self, context: &str, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, CryptoError>;
}

/// Tunes Argon2id passphrase key derivation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyDerivationParameters {
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
}

impl KeyDerivationParameters {
    /// Creates parameters; validity is checked when a key is derived.
    ///
    /// Inputs: Memory cost in KiB, the number of passes, and the degree of parallelism.
    /// Outputs: The parameter set.
    /// Side effects: None.
    pub fn new(memory_kib: u32, iterations: u32, parallelism: u32) -> Self {
        Self {
            memory_kib,
            iterations,
            parallelism,
        }
    }

    /// Returns the memory cost in KiB.
    pub fn memory_kib(&self) -> u32 {
        self.memory_kib
    }

    /// Returns the number of passes.
    pub fn iterations(&self) -> u32 {
        self.iterations
    }

    /// Returns the degree of parallelism.
    pub fn parallelism(&self) -> u32 {
        self.parallelism
    }
}

impl Default for KeyDerivationParameters {
    /// Returns the OWASP minimum for Argon2id: 19 MiB of memory, 2 passes, 1 lane.
    fn default() -> Self {
        Self::new(19 * 1024, 2, 1)
    }
}

/// Records where a database's encryption key comes from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EncryptionKeySource {
    /// A random 32-byte key held in the platform keychain or, as a fallback, in a local key file.
    /// The name and stored value (`key_file`) predate the keychain and are kept for existing
    /// databases.
    KeyFile,
    /// A key derived with Argon2id from `NOTE_TAKER_ENCRYPTION_PASSPHRASE`.
    Passphrase {
        /// Random, non-secret salt generated when the database was first encrypted.
        salt: Vec<u8>,
        /// Parameters used for derivation, kept so later default changes cannot lock data out.
        parameters: KeyDerivationParameters,
    },
}

impl EncryptionKeySource {
    /// Names the source for error messages and documentation.
    ///
    /// Inputs: None.
    /// Outputs: `"random key"` or `"passphrase"`.
    /// Side effects: None.
    pub fn label(&self) -> &'static str {
        match self {
            Self::KeyFile => "random key",
            Self::Passphrase { .. } => "passphrase",
        }
    }
}

/// Holds the non-secret metadata needed to unlock an encrypted database.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptionMetadata {
    key_source: EncryptionKeySource,
    key_check: Vec<u8>,
}

impl EncryptionMetadata {
    /// Creates metadata from a key source and a sealed key-check value.
    ///
    /// Inputs: The key source and the key-check value sealed with the database key.
    /// Outputs: The metadata record.
    /// Side effects: None.
    pub fn new(key_source: EncryptionKeySource, key_check: Vec<u8>) -> Self {
        Self {
            key_source,
            key_check,
        }
    }

    /// Borrows the key source.
    pub fn key_source(&self) -> &EncryptionKeySource {
        &self.key_source
    }

    /// Borrows the sealed key-check value.
    pub fn key_check(&self) -> &[u8] {
        &self.key_check
    }
}

/// Persists the single encryption metadata record of a database.
pub trait EncryptionMetadataRepository {
    /// Loads the database's encryption metadata.
    ///
    /// Inputs: None.
    /// Outputs: The metadata, `None` before the database was first encrypted, or a storage error.
    /// Side effects: Reads adapter-owned metadata.
    fn load_encryption_metadata(&self) -> Result<Option<EncryptionMetadata>, StorageError>;

    /// Stores the metadata unless a record already exists.
    ///
    /// Inputs: The metadata created on first unlock.
    /// Outputs: `true` when stored, `false` when another process stored a record first, or a
    /// storage error.
    /// Side effects: Writes at most one metadata record; never overwrites an existing one.
    fn save_encryption_metadata(&self, metadata: &EncryptionMetadata)
    -> Result<bool, StorageError>;
}

/// Holds the random database key in one operating-system credential store entry.
///
/// Implementations are the macOS Keychain, the Windows Credential Manager, and the Secret Service
/// on Linux (`adapters::keychain`), or an in-memory double in tests. Errors distinguish a store
/// that does not exist on this machine (`KeychainUnavailable`) from one that refused or failed an
/// operation (`Keychain`); the key manager falls back to the key file in both cases.
pub trait KeychainStore: Send + Sync {
    /// Reads the stored key bytes.
    ///
    /// Inputs: None.
    /// Outputs: The bytes (wiped on drop), `None` when the entry does not exist, or
    /// `KeychainUnavailable` / `Keychain`. The length is validated by the caller.
    /// Side effects: Queries the credential store; the platform may prompt the user to unlock it.
    fn load_key(&self) -> Result<Option<Zeroizing<Vec<u8>>>, CryptoError>;

    /// Creates or replaces the entry with `key_bytes`.
    ///
    /// Inputs: The raw key; implementations must never log it.
    /// Outputs: Success, or `KeychainUnavailable` / `Keychain`.
    /// Side effects: Writes the credential store.
    fn store_key(&self, key_bytes: &[u8]) -> Result<(), CryptoError>;

    /// Names the store and entry for log and error messages (no secret material).
    ///
    /// Inputs: None.
    /// Outputs: For example `Secret Service item service=note-taker account=encryption-key`.
    /// Side effects: None.
    fn location(&self) -> &str;
}
