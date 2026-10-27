//! Encrypts voice embeddings at rest and manages the key that protects them (Issue #31).

/// Resolves the database encryption key from a passphrase or a local random key file.
pub mod key_manager;
#[cfg(test)]
mod key_manager_tests;
/// Seals and opens embeddings with AES-256-GCM.
pub mod voice_encryptor;
#[cfg(test)]
mod voice_encryptor_tests;
