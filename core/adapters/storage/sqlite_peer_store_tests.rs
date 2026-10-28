//! Specifies the peer store's node-identity contract: one sealed identity per database, kept
//! across reopenings, agreed on by concurrent first starts, and never stored in plaintext.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use rusqlite::Connection;

use super::sqlite_peer_store::SqlitePeerStore;
use crate::{
    application::crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
    ports::{crypto::EmbeddingCipher, peer_trust::NodeIdentityStore},
};

/// A uniquely named directory under the system temp dir, removed on drop.
struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    /// Creates an empty scratch directory.
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("peer-store-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path)
            .unwrap_or_else(|error| panic!("scratch directory must be created: {error}"));
        Self(path)
    }

    /// Returns the database path inside the directory.
    fn database_path(&self) -> PathBuf {
        self.0.join("notes.sqlite3")
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Returns a cipher with a fixed key, so reopened stores can read what earlier ones sealed.
fn fixed_cipher() -> Arc<dyn EmbeddingCipher> {
    Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([3; 32])))
}

/// Opens the peer store at `database_path`, panicking with context on failure.
fn open_store(database_path: &Path) -> SqlitePeerStore {
    SqlitePeerStore::open(database_path, fixed_cipher())
        .unwrap_or_else(|error| panic!("peer store must open: {error}"))
}

/// The first candidate becomes the identity; later candidates are ignored.
#[test]
fn the_first_node_identity_is_kept_and_later_candidates_are_ignored() {
    let store = SqlitePeerStore::open_in_memory(fixed_cipher())
        .unwrap_or_else(|error| panic!("peer store must open: {error}"));

    let first = store
        .load_or_insert_node_identity(b"first-identity")
        .unwrap_or_else(|error| panic!("identity must persist: {error}"));
    let second = store
        .load_or_insert_node_identity(b"second-identity")
        .unwrap_or_else(|error| panic!("identity must load: {error}"));

    assert_eq!(first.as_slice(), b"first-identity");
    assert_eq!(second.as_slice(), b"first-identity");
}

/// The identity survives closing and reopening the database, and two stores opened on the same
/// file (a daemon and the app racing at first start) agree on it.
#[test]
fn the_node_identity_persists_and_is_shared_by_connections() {
    let scratch = ScratchDirectory::new();
    let daemon_store = open_store(&scratch.database_path());
    let app_store = open_store(&scratch.database_path());

    let daemon_identity = daemon_store
        .load_or_insert_node_identity(b"daemon-candidate")
        .unwrap_or_else(|error| panic!("identity must persist: {error}"));
    let app_identity = app_store
        .load_or_insert_node_identity(b"app-candidate")
        .unwrap_or_else(|error| panic!("identity must load: {error}"));
    drop((daemon_store, app_store));
    let reopened_identity = open_store(&scratch.database_path())
        .load_or_insert_node_identity(b"late-candidate")
        .unwrap_or_else(|error| panic!("identity must reload: {error}"));

    assert_eq!(daemon_identity.as_slice(), b"daemon-candidate");
    assert_eq!(app_identity.as_slice(), b"daemon-candidate");
    assert_eq!(reopened_identity.as_slice(), b"daemon-candidate");
}

/// The private key is sealed at rest: the raw bytes never appear in the database file's rows.
#[test]
fn the_node_identity_is_sealed_at_rest() {
    let scratch = ScratchDirectory::new();
    open_store(&scratch.database_path())
        .load_or_insert_node_identity(b"plaintext-node-secret")
        .unwrap_or_else(|error| panic!("identity must persist: {error}"));

    let stored: Vec<u8> = Connection::open(scratch.database_path())
        .and_then(|connection| {
            connection.query_row("SELECT sealed_identity FROM p2p_node_identity", [], |row| {
                row.get(0)
            })
        })
        .unwrap_or_else(|error| panic!("sealed row must exist: {error}"));

    assert!(
        !stored
            .windows(21)
            .any(|window| window == b"plaintext-node-secret")
    );
}

/// A store opened with a different key refuses the identity instead of replacing it.
#[test]
fn a_wrong_key_is_an_error_not_a_new_identity() {
    let scratch = ScratchDirectory::new();
    open_store(&scratch.database_path())
        .load_or_insert_node_identity(b"original")
        .unwrap_or_else(|error| panic!("identity must persist: {error}"));
    let wrong_key_store = SqlitePeerStore::open(
        scratch.database_path(),
        Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([9; 32]))),
    )
    .unwrap_or_else(|error| panic!("peer store must open: {error}"));

    assert!(
        wrong_key_store
            .load_or_insert_node_identity(b"replacement")
            .is_err()
    );
}
