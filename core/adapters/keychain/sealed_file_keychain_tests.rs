//! Tests the sealed secret file.

use super::sealed_file_keychain::SealedFileKeychain;
use crate::ports::crypto::KeychainStore;

fn path(name: &str) -> std::path::PathBuf {
    let path =
        std::env::temp_dir().join(format!("note-taker-sealed-{name}-{}", std::process::id()));
    drop(std::fs::remove_file(&path));
    path
}

/// Verifies a round trip, that the file never holds the secret in the clear, and deletion.
#[test]
fn a_secret_round_trips_and_is_not_stored_in_the_clear() {
    let file = path("round");
    let store = SealedFileKeychain::new(file.clone(), Some(vec![7; 32]));
    assert!(store.load_key().unwrap_or_else(|e| panic!("{e}")).is_none());
    store
        .store_key(b"sk-secret")
        .unwrap_or_else(|e| panic!("{e}"));
    let on_disk = std::fs::read(&file).unwrap_or_else(|e| panic!("{e}"));
    assert!(!on_disk.windows(9).any(|window| window == b"sk-secret"));
    let loaded = store.load_key().unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        loaded.as_deref().map(Vec::as_slice),
        Some(b"sk-secret".as_slice())
    );
    store.delete_key().unwrap_or_else(|e| panic!("{e}"));
    assert!(store.load_key().unwrap_or_else(|e| panic!("{e}")).is_none());
}

/// Verifies another key cannot open the file, and no key means unavailable.
#[test]
fn the_wrong_key_or_no_key_cannot_open_it() {
    let file = path("wrong");
    SealedFileKeychain::new(file.clone(), Some(vec![7; 32]))
        .store_key(b"sk-secret")
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        SealedFileKeychain::new(file.clone(), Some(vec![8; 32]))
            .load_key()
            .is_err()
    );
    assert!(SealedFileKeychain::new(file, None).store_key(b"x").is_err());
}
