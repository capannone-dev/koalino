//! Encrypts voice embeddings at rest and manages the key that protects them (Issues #31, #40),
//! and agrees the keys that protect review clips sent to paired devices (Issue #50).

/// Resolves the database encryption key from a passphrase, the platform keychain, or a key file.
pub mod key_manager;
#[cfg(test)]
mod key_manager_tests;
/// Agrees the per-pair key of two paired devices with X25519 and HKDF-SHA256 (Issue #50).
pub mod pair_key;
#[cfg(test)]
mod pair_key_tests;
/// Seals and opens embeddings with AES-256-GCM.
pub mod voice_encryptor;
#[cfg(test)]
mod voice_encryptor_tests;
