//! Keeps a small secret on platforms whose keychain the core cannot reach (Android).
//!
//! The secret is sealed with AES-256-GCM under the database key, which the Android Keystore
//! protects. So the file is useless without the device's Keystore, and the owner's secret never
//! sits on disk in the clear.

use std::{fs, path::PathBuf};

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use zeroize::Zeroizing;

use crate::{domain::errors::CryptoError, ports::crypto::KeychainStore};

/// Length of the nonce stored at the start of the file.
const NONCE_LENGTH: usize = 12;

/// A file sealed under a key the host app protects.
pub struct SealedFileKeychain {
    path: PathBuf,
    /// Wiped on drop; `None` when the host provided no key, which makes the store unavailable.
    sealing_key: Option<Zeroizing<Vec<u8>>>,
}

impl SealedFileKeychain {
    /// Creates the store.
    ///
    /// Inputs: The sealed file's path and the host-protected 32-byte key, if any.
    /// Outputs: The store. Side effects: None.
    pub fn new(path: PathBuf, sealing_key: Option<Vec<u8>>) -> Self {
        Self {
            path,
            sealing_key: sealing_key.map(Zeroizing::new),
        }
    }

    /// Builds the cipher, or reports the store as unavailable.
    fn cipher(&self) -> Result<Aes256Gcm, CryptoError> {
        self.sealing_key
            .as_ref()
            .and_then(|key| Aes256Gcm::new_from_slice(key).ok())
            .ok_or_else(|| CryptoError::KeychainUnavailable {
                reason: "no protected key is available".to_owned(),
            })
    }

    /// Builds a store failure without any secret material.
    fn failure(&self, reason: &str) -> CryptoError {
        CryptoError::Keychain {
            reason: format!("{}: {reason}", self.location()),
        }
    }
}

impl KeychainStore for SealedFileKeychain {
    /// Opens the sealed file. Outputs: The secret, or `None` when there is no file.
    /// Side effects: Reads the file.
    fn load_key(&self) -> Result<Option<Zeroizing<Vec<u8>>>, CryptoError> {
        let cipher = self.cipher()?;
        let sealed = match fs::read(&self.path) {
            Ok(sealed) => sealed,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(self.failure("the file could not be read")),
        };
        if sealed.len() < NONCE_LENGTH {
            return Err(self.failure("the file is damaged"));
        }
        let (nonce, ciphertext) = sealed.split_at(NONCE_LENGTH);
        cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map(|secret| Some(Zeroizing::new(secret)))
            .map_err(|_| self.failure("the file could not be opened"))
    }

    /// Seals the secret under a fresh nonce. Side effects: Replaces the file via a temporary one.
    fn store_key(&self, key_bytes: &[u8]) -> Result<(), CryptoError> {
        let cipher = self.cipher()?;
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, key_bytes)
            .map_err(|_| self.failure("the secret could not be sealed"))?;
        let mut sealed = nonce.to_vec();
        sealed.extend_from_slice(&ciphertext);
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|_| self.failure("no folder for the file"))?;
        }
        let temporary = self.path.with_extension("tmp");
        fs::write(&temporary, sealed).map_err(|_| self.failure("the file could not be written"))?;
        fs::rename(&temporary, &self.path).map_err(|_| self.failure("the file could not be saved"))
    }

    /// Names the store. Side effects: None.
    fn location(&self) -> &str {
        "Android Keystore-protected file"
    }
}

impl SealedFileKeychain {
    /// Deletes the file; a missing file counts as deleted.
    ///
    /// Outputs: Success or a store failure. Side effects: Removes the file.
    pub fn delete_key(&self) -> Result<(), CryptoError> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(self.failure("the file could not be removed")),
        }
    }
}
