//! Specifies the platform keychain adapter: absent entries, round trips, error classification,
//! secret-free error messages, and the documented entry names.
//!
//! Tests use `keyring_core`'s in-memory mock credential, so `make rust-test` never touches the
//! developer's real keychain. The one real-store test is `#[ignore]`d; run it explicitly with
//! `cargo test --lib real_platform_keychain -- --ignored`.

use keyring_core::{Entry, Error as KeyringError, api::CredentialStoreApi, mock};

use super::platform_keychain::{
    KEYCHAIN_ACCOUNT, KEYCHAIN_SERVICE, PlatformKeychain, platform_location,
};
use crate::{domain::errors::CryptoError, ports::crypto::KeychainStore};

/// Location string used by the mock-backed adapters.
const MOCK_LOCATION: &str = "mock item note-taker/encryption-key";

/// Builds an entry in a fresh, private mock store (never the process-wide default store).
fn mock_entry() -> Entry {
    mock::Store::new()
        .and_then(|store| store.build(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT, None))
        .unwrap_or_else(|error| panic!("mock entry must build: {error}"))
}

/// Makes the next operation on `entry` fail with `error`.
fn fail_next_call(entry: &Entry, error: KeyringError) {
    let Some(mock_credential) = entry.as_any().downcast_ref::<mock::Cred>() else {
        panic!("entry must be a mock credential");
    };
    mock_credential.set_error(error);
}

/// Wraps a platform error message the way keyring stores do.
fn platform_error(message: &str) -> Box<dyn std::error::Error + Send + Sync> {
    Box::from(message)
}

/// An entry that was never written reads as `None`, not as an error.
#[test]
fn an_absent_entry_reads_as_none() {
    let keychain = PlatformKeychain::from_entry(Ok(mock_entry()), MOCK_LOCATION);

    assert_eq!(
        keychain
            .load_key()
            .map(|key| key.map(|bytes| bytes.to_vec())),
        Ok(None)
    );
}

/// A stored key reads back byte for byte, and deleting it makes it absent again.
#[test]
fn a_stored_key_reads_back_and_can_be_deleted() {
    let keychain = PlatformKeychain::from_entry(Ok(mock_entry()), MOCK_LOCATION);
    let key_bytes: Vec<u8> = (0..32).collect();

    let stored = keychain.store_key(&key_bytes);
    let loaded = keychain
        .load_key()
        .map(|key| key.map(|bytes| bytes.to_vec()));
    let deleted = keychain.delete_key();
    let after_delete = keychain
        .load_key()
        .map(|key| key.map(|bytes| bytes.to_vec()));

    assert_eq!(stored, Ok(()));
    assert_eq!(loaded, Ok(Some(key_bytes)));
    assert_eq!(deleted, Ok(()));
    assert_eq!(after_delete, Ok(None));
}

/// No credential store on this machine (e.g. headless Linux) → `KeychainUnavailable`.
#[test]
fn a_missing_platform_store_is_unavailable() {
    let keychain = PlatformKeychain::from_entry(Err(KeyringError::NoDefaultStore), MOCK_LOCATION);

    assert!(matches!(
        keychain.load_key(),
        Err(CryptoError::KeychainUnavailable { .. })
    ));
    assert!(matches!(
        keychain.store_key(&[0; 32]),
        Err(CryptoError::KeychainUnavailable { .. })
    ));
}

/// A broken backend is "unavailable"; a locked or denied store is an access error. Both name the
/// entry.
#[test]
fn operation_errors_are_classified_and_name_the_entry() {
    let entry = mock_entry();
    fail_next_call(
        &entry,
        KeyringError::PlatformFailure(platform_error("ServiceUnknown")),
    );
    let keychain = PlatformKeychain::from_entry(Ok(entry), MOCK_LOCATION);
    let platform_failure = keychain.load_key();

    let entry = mock_entry();
    fail_next_call(
        &entry,
        KeyringError::NoStorageAccess(platform_error("keyring is locked")),
    );
    let keychain = PlatformKeychain::from_entry(Ok(entry), MOCK_LOCATION);
    let access_denied = keychain.store_key(&[0; 32]);

    let Err(unavailable @ CryptoError::KeychainUnavailable { .. }) = platform_failure else {
        panic!("platform failure must be KeychainUnavailable: {platform_failure:?}");
    };
    let Err(denied @ CryptoError::Keychain { .. }) = access_denied else {
        panic!("access failure must be Keychain: {access_denied:?}");
    };
    assert!(
        unavailable.to_string().contains(MOCK_LOCATION),
        "{unavailable}"
    );
    assert!(
        unavailable.to_string().contains("ServiceUnknown"),
        "{unavailable}"
    );
    assert!(denied.to_string().contains("keyring is locked"), "{denied}");
}

/// Errors that carry the stored bytes never copy them into the message.
#[test]
fn error_messages_never_contain_secret_bytes() {
    let secret = vec![0xAB; 32];
    let entry = mock_entry();
    fail_next_call(
        &entry,
        KeyringError::BadDataFormat(secret.clone(), platform_error("bad blob")),
    );
    let keychain = PlatformKeychain::from_entry(Ok(entry), MOCK_LOCATION);

    let outcome = keychain.load_key();

    let Err(error) = outcome else {
        panic!("a malformed blob must be an error");
    };
    let message = format!("{error} {error:?}");
    assert!(
        !message.contains("171"),
        "decimal secret bytes leaked: {message}"
    );
    assert!(
        !message.to_lowercase().contains("ab, "),
        "hex secret bytes leaked: {message}"
    );
}

/// The production entry uses the documented service and account names. Checked through the
/// location string so the test does not initialize the real credential store.
#[test]
fn the_platform_entry_uses_the_documented_names() {
    let location = platform_location(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT);

    assert_eq!(KEYCHAIN_SERVICE, "note-taker");
    assert_eq!(KEYCHAIN_ACCOUNT, "encryption-key");
    #[cfg(target_os = "windows")]
    assert!(location.contains("note-taker:encryption-key"), "{location}");
    #[cfg(not(target_os = "windows"))]
    assert!(
        location.contains("service=note-taker") && location.contains("=encryption-key"),
        "{location}"
    );
}

/// Round-trips a throwaway entry through the real OS keychain, then deletes it.
#[test]
#[ignore = "touches the real OS keychain; run explicitly with --ignored"]
fn real_platform_keychain_round_trip() {
    let service = format!("note-taker-test-{}", uuid::Uuid::new_v4());
    let keychain = PlatformKeychain::for_service(&service);
    // Not valid UTF-8 and covering 0x00 and 0x80..=0xFF, like a real random key.
    let key_bytes: [u8; 32] = std::array::from_fn(|index| (index as u8).wrapping_mul(37) ^ 0xC3);

    let stored = keychain.store_key(&key_bytes);
    let loaded = keychain
        .load_key()
        .map(|key| key.map(|bytes| bytes.to_vec()));
    let deleted = keychain.delete_key();

    assert_eq!(stored, Ok(()), "{}", keychain.location());
    assert_eq!(loaded, Ok(Some(key_bytes.to_vec())));
    assert_eq!(deleted, Ok(()));
}

/// Migrates a real key file into a throwaway real OS keychain entry through `KeyManager`, then
/// deletes the entry.
#[test]
#[ignore = "touches the real OS keychain; run explicitly with --ignored"]
fn real_platform_keychain_migration() {
    use std::sync::Arc;

    use crate::{
        adapters::storage::sqlite_encryption_metadata::SqliteEncryptionMetadataStore,
        application::crypto::key_manager::{
            KEY_DIRECTORY_VARIABLE, KEY_FILE_NAME, KeyManager, KeyManagerConfig,
        },
    };

    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_test_writer()
        .try_init();
    let service = format!("note-taker-test-{}", uuid::Uuid::new_v4());
    let keychain = Arc::new(PlatformKeychain::for_service(&service));
    let key_directory = std::env::temp_dir().join(format!("note-taker-migration-{service}"));
    let key_file = key_directory.join(KEY_FILE_NAME);
    let directory_value = key_directory.to_string_lossy().into_owned();
    let config = || {
        KeyManagerConfig::from_lookup(|name| {
            (name == KEY_DIRECTORY_VARIABLE).then(|| directory_value.clone())
        })
    };
    let _ = std::fs::create_dir_all(&key_directory);
    let metadata = SqliteEncryptionMetadataStore::open(key_directory.join("metadata.sqlite3"))
        .unwrap_or_else(|error| panic!("metadata store must open: {error}"));

    let file_only = KeyManager::new(config()).unlock(&metadata);
    let file_key = std::fs::read(&key_file).unwrap_or_default();
    let keychain_store: Arc<dyn KeychainStore> = keychain.clone();
    let migrated = KeyManager::new(config())
        .with_keychain(keychain_store)
        .unlock(&metadata);
    let stored = keychain
        .load_key()
        .map(|key| key.map(|bytes| bytes.to_vec()));
    let file_left = key_file.exists();
    let deleted = keychain.delete_key();
    let _ = std::fs::remove_dir_all(&key_directory);

    assert!(file_only.is_ok(), "{:?}", file_only.err());
    assert!(migrated.is_ok(), "{:?}", migrated.err());
    assert_eq!(stored, Ok(Some(file_key)));
    assert!(!file_left, "the migrated key file must be deleted");
    assert_eq!(deleted, Ok(()));
}
