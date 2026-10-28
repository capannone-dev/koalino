//! Serves a database key that the host app manages, for platforms the `keyring` crate does not
//! cover. On Android the Flutter app's Kotlin `EncryptionKeyVault` unwraps the key with a
//! non-exportable Android Keystore key and hands it over through
//! `note_taker_provide_encryption_key`; this adapter then answers the key manager like a
//! keychain entry. Creating and storing keys is the host's job, so writes are refused.

use zeroize::Zeroizing;

use crate::{domain::errors::CryptoError, ports::crypto::KeychainStore};

/// A read-only keychain over a key the host provided, or none.
pub struct HostProvidedKeychain {
    /// The key bytes, wiped on drop; `None` when the host has not provided one.
    key: Option<Zeroizing<Vec<u8>>>,
    /// Names the host store for messages, e.g. `Android Keystore`.
    location: String,
}

impl HostProvidedKeychain {
    /// Wraps the host's key.
    ///
    /// Inputs: The key bytes, if the host provided them (the key manager checks the length), and
    /// the store's name for messages.
    /// Outputs: The adapter.
    /// Side effects: None; the bytes are wiped when the adapter is dropped.
    pub fn new(key: Option<Vec<u8>>, location: &str) -> Self {
        Self {
            key: key.map(Zeroizing::new),
            location: location.to_owned(),
        }
    }
}

impl KeychainStore for HostProvidedKeychain {
    /// Returns a copy of the provided key, or `KeychainUnavailable` when none was provided.
    fn load_key(&self) -> Result<Option<Zeroizing<Vec<u8>>>, CryptoError> {
        match &self.key {
            Some(key) => Ok(Some(Zeroizing::new(key.to_vec()))),
            None => Err(CryptoError::KeychainUnavailable {
                reason: format!(
                    "the app has not provided the key from the {}",
                    self.location
                ),
            }),
        }
    }

    /// Refuses: the host creates and stores the key.
    fn store_key(&self, _key_bytes: &[u8]) -> Result<(), CryptoError> {
        Err(CryptoError::Keychain {
            reason: format!(
                "the {} key is managed by the app, not by the core",
                self.location
            ),
        })
    }

    /// Names the host store.
    fn location(&self) -> &str {
        &self.location
    }
}
