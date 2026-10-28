//! Specifies the host-provided keychain: it serves the key the app handed over (the Android
//! Keystore unwraps it), reports a missing key as unavailable, and never accepts a new key.

use super::host_provided_keychain::HostProvidedKeychain;
use crate::{domain::errors::CryptoError, ports::crypto::KeychainStore};

/// A provided key is returned unchanged.
#[test]
fn a_provided_key_is_served() {
    let keychain = HostProvidedKeychain::new(Some(vec![7; 32]), "Android Keystore");

    let loaded = keychain
        .load_key()
        .unwrap_or_else(|error| panic!("a provided key must load: {error}"));

    assert_eq!(loaded.as_deref().map(Vec::as_slice), Some(&[7_u8; 32][..]));
    assert_eq!(keychain.location(), "Android Keystore");
}

/// Without a provided key the store is unavailable, so the app shows its keychain error.
#[test]
fn a_missing_key_is_unavailable() {
    let keychain = HostProvidedKeychain::new(None, "Android Keystore");

    let outcome = keychain.load_key();

    assert!(
        matches!(outcome, Err(CryptoError::KeychainUnavailable { .. })),
        "expected KeychainUnavailable"
    );
}

/// The host owns key creation and storage; Rust never writes through this adapter.
#[test]
fn storing_a_key_is_refused() {
    let keychain = HostProvidedKeychain::new(Some(vec![7; 32]), "Android Keystore");

    let outcome = keychain.store_key(&[9; 32]);

    assert!(matches!(outcome, Err(CryptoError::Keychain { .. })));
    let still_served = keychain.load_key().ok().flatten();
    assert_eq!(
        still_served.as_deref().map(Vec::as_slice),
        Some(&[7_u8; 32][..])
    );
}
