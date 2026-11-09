//! Specifies the Teams bridge secret kept in the keychain (#102): read when present, created
//! once by the browser host's installer, and never replaced.

use std::sync::Mutex;

use zeroize::Zeroizing;

use super::bridge_secret::{ensure_bridge_secret, load_bridge_secret};
use crate::{domain::errors::CryptoError, ports::crypto::KeychainStore};

/// An in-memory keychain entry.
#[derive(Default)]
struct MemoryKeychain(Mutex<Option<Vec<u8>>>);

impl KeychainStore for MemoryKeychain {
    /// Returns the stored bytes.
    fn load_key(&self) -> Result<Option<Zeroizing<Vec<u8>>>, CryptoError> {
        Ok(self
            .0
            .lock()
            .ok()
            .and_then(|stored| stored.clone())
            .map(Zeroizing::new))
    }

    /// Replaces the stored bytes.
    fn store_key(&self, key_bytes: &[u8]) -> Result<(), CryptoError> {
        if let Ok(mut stored) = self.0.lock() {
            *stored = Some(key_bytes.to_vec());
        }
        Ok(())
    }

    /// Names the entry.
    fn location(&self) -> &str {
        "memory"
    }
}

#[test]
fn a_missing_secret_reads_as_none() {
    let keychain = MemoryKeychain::default();
    let secret = load_bridge_secret(&keychain)
        .unwrap_or_else(|error| panic!("reading should succeed: {error}"));
    assert!(secret.is_none());
}

#[test]
fn ensuring_creates_a_random_secret_once() {
    let keychain = MemoryKeychain::default();
    let created = ensure_bridge_secret(&keychain)
        .unwrap_or_else(|error| panic!("creating should succeed: {error}"));
    assert_eq!(created.len(), 64, "32 random bytes in hex");
    assert!(
        created
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    );

    let again = ensure_bridge_secret(&keychain)
        .unwrap_or_else(|error| panic!("reading should succeed: {error}"));
    assert_eq!(
        again.as_str(),
        created.as_str(),
        "an existing secret is kept"
    );
    let loaded = load_bridge_secret(&keychain)
        .unwrap_or_else(|error| panic!("reading should succeed: {error}"));
    assert_eq!(
        loaded.as_deref().map(String::as_str),
        Some(created.as_str())
    );
}

#[test]
fn a_secret_that_is_not_text_is_an_error() {
    let keychain = MemoryKeychain::default();
    let _ = keychain.store_key(&[0xFF, 0xFE]);
    assert!(load_bridge_secret(&keychain).is_err());
}
