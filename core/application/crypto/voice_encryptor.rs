//! Seals and opens voice embeddings with AES-256-GCM.
//!
//! Sealed layout: `"NTE1"` format header (4 bytes) ‖ random nonce (12) ‖ ciphertext ‖ GCM tag
//! (16). The plaintext is the embedding as little-endian `f32` bytes. The header and a caller
//! context (the owning table and row, such as `voice_variants:<uuid>`) are authenticated as
//! associated data, so a sealed value copied onto another row, or with a forged header, fails to
//! open. Every call draws a fresh nonce from the operating system; with 96-bit random nonces
//! one key stays safe far beyond the size of a personal voice library.
//!
//! Plaintext buffers and keys are wiped from memory when dropped, and nothing here logs.

use std::sync::Arc;

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng, Payload, rand_core::RngCore},
};
use zeroize::Zeroizing;

use crate::{domain::errors::CryptoError, ports::crypto::EmbeddingCipher};

/// Identifies format version 1 (AES-256-GCM, 96-bit nonce) at the start of every sealed value.
pub const SEALED_EMBEDDING_HEADER: &[u8; 4] = b"NTE1";
/// Length of the random AES-GCM nonce.
const NONCE_LENGTH: usize = 12;
/// Length of the AES-GCM authentication tag.
const TAG_LENGTH: usize = 16;
/// Context binding the key-check value to the encryption metadata record.
const KEY_CHECK_CONTEXT: &[u8] = b"encryption_meta:key_check";
/// Known plaintext whose successful decryption proves the configured key is the database key.
const KEY_CHECK_PLAINTEXT: &[u8] = b"note-taker key check v1";

/// Holds a 256-bit key that is wiped on drop and never printed.
pub struct EncryptionKey(Zeroizing<[u8; 32]>);

impl EncryptionKey {
    /// Wraps existing key bytes.
    ///
    /// Inputs: 32 secret key bytes.
    /// Outputs: The key; the caller should not keep other copies of `key_bytes`.
    /// Side effects: None.
    pub fn from_bytes(key_bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(key_bytes))
    }

    /// Generates a key from the operating system's secure random number generator.
    ///
    /// Inputs: None.
    /// Outputs: A fresh random key.
    /// Side effects: Reads operating-system randomness.
    pub fn generate() -> Self {
        let mut key_bytes = Zeroizing::new([0_u8; 32]);
        OsRng.fill_bytes(key_bytes.as_mut());
        Self(key_bytes)
    }

    /// Borrows the raw key bytes.
    ///
    /// Inputs: None.
    /// Outputs: The 32 key bytes; never log or persist them outside the key file.
    /// Side effects: None.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for EncryptionKey {
    /// Prints a placeholder instead of key bytes.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("EncryptionKey(<redacted>)")
    }
}

/// Encrypts an embedding with no row context.
///
/// Inputs: `embedding` (any length, including empty) and a 256-bit `key`.
/// Outputs: The sealed value, or `EncryptionFailed`.
/// Side effects: Draws a random nonce.
pub fn encrypt_embedding(embedding: &[f32], key: &[u8; 32]) -> Result<Vec<u8>, CryptoError> {
    encrypt_embedding_with_context(embedding, key, b"")
}

/// Decrypts a value sealed by `encrypt_embedding`.
///
/// Inputs: The sealed bytes and the same `key`.
/// Outputs: The embedding, `MalformedCiphertext`, or `AuthenticationFailed`.
/// Side effects: None.
pub fn decrypt_embedding(encrypted: &[u8], key: &[u8; 32]) -> Result<Vec<f32>, CryptoError> {
    decrypt_embedding_with_context(encrypted, key, b"")
}

/// Encrypts an embedding bound to `context`.
///
/// Inputs: `embedding`, a 256-bit `key`, and the row `context` required again for decryption.
/// Outputs: The sealed value, or `EncryptionFailed`.
/// Side effects: Draws a random nonce.
pub fn encrypt_embedding_with_context(
    embedding: &[f32],
    key: &[u8; 32],
    context: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    seal_embedding_with(&Aes256Gcm::new(key.into()), embedding, context)
}

/// Decrypts a value sealed by `encrypt_embedding_with_context`.
///
/// Inputs: The sealed bytes, the same `key`, and the same `context`.
/// Outputs: The embedding, `MalformedCiphertext`, or `AuthenticationFailed`.
/// Side effects: None.
pub fn decrypt_embedding_with_context(
    encrypted: &[u8],
    key: &[u8; 32],
    context: &[u8],
) -> Result<Vec<f32>, CryptoError> {
    open_embedding_with(&Aes256Gcm::new(key.into()), encrypted, context)
}

/// Implements `EmbeddingCipher` with one database key held for the session.
pub struct VoiceEncryptor {
    cipher: Aes256Gcm,
}

impl VoiceEncryptor {
    /// Prepares the AES-256-GCM key schedule once, so each operation skips key setup.
    ///
    /// Inputs: The database key.
    /// Outputs: The encryptor; the key schedule is wiped when it is dropped.
    /// Side effects: None.
    pub fn new(key: &EncryptionKey) -> Self {
        Self {
            cipher: Aes256Gcm::new(key.as_bytes().into()),
        }
    }

    /// Creates an encryptor with a random key that is never stored.
    ///
    /// Inputs: None.
    /// Outputs: An encryptor for in-memory databases; its data is unreadable once dropped.
    /// Side effects: Reads operating-system randomness.
    pub fn ephemeral() -> Self {
        Self::new(&EncryptionKey::generate())
    }

    /// Seals the known key-check plaintext for the encryption metadata record.
    ///
    /// Inputs: None.
    /// Outputs: The sealed key-check value, or `EncryptionFailed`.
    /// Side effects: Draws a random nonce.
    pub fn seal_key_check(&self) -> Result<Vec<u8>, CryptoError> {
        seal_bytes(&self.cipher, KEY_CHECK_PLAINTEXT, KEY_CHECK_CONTEXT)
    }

    /// Confirms that `key_check` was sealed with this encryptor's key.
    ///
    /// Inputs: The stored key-check value.
    /// Outputs: `Ok(())`, or `WrongKey` for any other key or a damaged value.
    /// Side effects: None.
    pub fn verify_key_check(&self, key_check: &[u8]) -> Result<(), CryptoError> {
        match open_bytes(&self.cipher, key_check, KEY_CHECK_CONTEXT) {
            Ok(plaintext) if plaintext.as_slice() == KEY_CHECK_PLAINTEXT => Ok(()),
            _ => Err(CryptoError::WrongKey),
        }
    }
}

impl EmbeddingCipher for VoiceEncryptor {
    /// Seals `embedding` for the row named by `context`.
    fn seal_embedding(&self, context: &str, embedding: &[f32]) -> Result<Vec<u8>, CryptoError> {
        seal_embedding_with(&self.cipher, embedding, context.as_bytes())
    }

    /// Opens a value sealed for the row named by `context`.
    fn open_embedding(&self, context: &str, sealed: &[u8]) -> Result<Vec<f32>, CryptoError> {
        open_embedding_with(&self.cipher, sealed, context.as_bytes())
    }

    /// Seals `secret` for the record named by `context`.
    fn seal_secret(&self, context: &str, secret: &[u8]) -> Result<Vec<u8>, CryptoError> {
        seal_bytes(&self.cipher, secret, context.as_bytes())
    }

    /// Opens a secret sealed for the record named by `context`.
    fn open_secret(&self, context: &str, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
        open_bytes(&self.cipher, sealed, context.as_bytes())
    }
}

/// Returns a shareable cipher with a random, never-stored key for in-memory stores.
///
/// Inputs: None.
/// Outputs: A cipher handle suitable for `open_in_memory` storage adapters.
/// Side effects: Reads operating-system randomness.
pub fn ephemeral_cipher() -> Arc<dyn EmbeddingCipher> {
    Arc::new(VoiceEncryptor::ephemeral())
}

/// Serializes and seals one embedding.
///
/// Inputs: A prepared cipher, the embedding, and the row context.
/// Outputs: The sealed value, or `EncryptionFailed`.
/// Side effects: Draws a random nonce; the serialized plaintext is wiped afterwards.
fn seal_embedding_with(
    cipher: &Aes256Gcm,
    embedding: &[f32],
    context: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let plaintext = Zeroizing::new(
        embedding
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<u8>>(),
    );
    seal_bytes(cipher, &plaintext, context)
}

/// Opens and deserializes one embedding.
///
/// Inputs: A prepared cipher, the sealed value, and the row context.
/// Outputs: The embedding, `MalformedCiphertext`, or `AuthenticationFailed`.
/// Side effects: The intermediate plaintext bytes are wiped afterwards.
fn open_embedding_with(
    cipher: &Aes256Gcm,
    sealed: &[u8],
    context: &[u8],
) -> Result<Vec<f32>, CryptoError> {
    let plaintext = open_bytes(cipher, sealed, context)?;
    let (value_bytes, trailing_bytes) = plaintext.as_chunks::<4>();
    if !trailing_bytes.is_empty() {
        return Err(CryptoError::MalformedCiphertext);
    }
    Ok(value_bytes
        .iter()
        .map(|little_endian| f32::from_le_bytes(*little_endian))
        .collect())
}

/// Encrypts `plaintext` into the sealed layout.
///
/// Inputs: A prepared cipher, plaintext bytes, and the row context.
/// Outputs: Header ‖ nonce ‖ ciphertext ‖ tag, or `EncryptionFailed`.
/// Side effects: Draws a random nonce.
fn seal_bytes(
    cipher: &Aes256Gcm,
    plaintext: &[u8],
    context: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad: &associated_data(context),
            },
        )
        .map_err(|_| CryptoError::EncryptionFailed)?;
    let mut sealed =
        Vec::with_capacity(SEALED_EMBEDDING_HEADER.len() + NONCE_LENGTH + ciphertext.len());
    sealed.extend_from_slice(SEALED_EMBEDDING_HEADER);
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(&ciphertext);
    Ok(sealed)
}

/// Verifies and decrypts the sealed layout.
///
/// Inputs: A prepared cipher, the sealed value, and the row context.
/// Outputs: Plaintext bytes (wiped on drop), `MalformedCiphertext`, or `AuthenticationFailed`.
/// Side effects: None.
fn open_bytes(
    cipher: &Aes256Gcm,
    sealed: &[u8],
    context: &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    let header_length = SEALED_EMBEDDING_HEADER.len();
    if sealed.len() < header_length + NONCE_LENGTH + TAG_LENGTH
        || &sealed[..header_length] != SEALED_EMBEDDING_HEADER
    {
        return Err(CryptoError::MalformedCiphertext);
    }
    let (nonce_bytes, ciphertext) = sealed[header_length..].split_at(NONCE_LENGTH);
    cipher
        .decrypt(
            Nonce::from_slice(nonce_bytes),
            Payload {
                msg: ciphertext,
                aad: &associated_data(context),
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::AuthenticationFailed)
}

/// Builds the authenticated associated data: the format header followed by the row context.
///
/// Inputs: The row context.
/// Outputs: Header ‖ context.
/// Side effects: None.
fn associated_data(context: &[u8]) -> Vec<u8> {
    let mut associated = Vec::with_capacity(SEALED_EMBEDDING_HEADER.len() + context.len());
    associated.extend_from_slice(SEALED_EMBEDDING_HEADER);
    associated.extend_from_slice(context);
    associated
}
