//! Specifies the peer store's node-identity contract: one sealed identity per database, kept
//! across reopenings, agreed on by concurrent first starts, and never stored in plaintext. The
//! pair-agreement secret and the per-pair keys of Issue #50 follow the same rules, and a pair key
//! is pinned once agreed.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use rusqlite::Connection;

use super::sqlite_peer_store::SqlitePeerStore;
use crate::{
    application::crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
    domain::{
        models::DeviceId,
        network_addresses::{NetworkAddresses, StoredNetworkAddresses},
        peer_trust::{DeviceName, PairKey, TrustedPeer},
    },
    ports::{
        crypto::EmbeddingCipher,
        peer_trust::{
            NetworkAddressStore, NodeIdentityStore, PairAgreementSecretStore, PairKeyPinning,
            TrustedPeerStore,
        },
    },
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

/// Builds a trusted-peer record with a valid name.
fn trusted_peer(peer_id: &str, name: &str, paired_at_ms: u64) -> TrustedPeer {
    TrustedPeer::new(
        DeviceId::new(peer_id),
        DeviceName::parse(name).unwrap_or_else(|error| panic!("valid name: {error}")),
        paired_at_ms,
    )
}

/// Trusted peers are listed in pairing order, and pairing again updates the name and time.
#[test]
fn trusting_lists_peers_and_re_pairing_updates_them() {
    let store = SqlitePeerStore::open_in_memory(fixed_cipher())
        .unwrap_or_else(|error| panic!("peer store must open: {error}"));

    for peer in [
        trusted_peer("peer-phone", "Phone", 2_000),
        trusted_peer("peer-laptop", "Laptop", 1_000),
        trusted_peer("peer-phone", "Work phone", 3_000),
    ] {
        store
            .trust_peer(&peer)
            .unwrap_or_else(|error| panic!("peer must be trusted: {error}"));
    }
    let peers = store
        .trusted_peers()
        .unwrap_or_else(|error| panic!("peers must list: {error}"));

    assert_eq!(
        peers,
        vec![
            trusted_peer("peer-laptop", "Laptop", 1_000),
            trusted_peer("peer-phone", "Work phone", 3_000),
        ]
    );
    assert!(
        store
            .is_trusted(&DeviceId::new("peer-phone"))
            .unwrap_or_else(|error| panic!("trust must be queryable: {error}"))
    );
    assert!(
        !store
            .is_trusted(&DeviceId::new("peer-stranger"))
            .unwrap_or_else(|error| panic!("trust must be queryable: {error}"))
    );
}

/// Revoking removes the peer, reports whether it existed, and persists across reopening.
#[test]
fn revoking_a_peer_removes_it_persistently() {
    let scratch = ScratchDirectory::new();
    let store = open_store(&scratch.database_path());
    store
        .trust_peer(&trusted_peer("peer-phone", "Phone", 1))
        .unwrap_or_else(|error| panic!("peer must be trusted: {error}"));

    let revoked = store
        .revoke_peer(&DeviceId::new("peer-phone"))
        .unwrap_or_else(|error| panic!("revoke must succeed: {error}"));
    let revoked_again = store
        .revoke_peer(&DeviceId::new("peer-phone"))
        .unwrap_or_else(|error| panic!("revoke must succeed: {error}"));
    drop(store);
    let reopened_peers = open_store(&scratch.database_path())
        .trusted_peers()
        .unwrap_or_else(|error| panic!("peers must list: {error}"));

    assert!(revoked);
    assert!(!revoked_again);
    assert!(reopened_peers.is_empty());
}

/// The first pair-agreement secret is kept across reopenings and sealed at rest.
#[test]
fn the_pair_agreement_secret_is_kept_and_sealed() {
    let scratch = ScratchDirectory::new();
    let first = open_store(&scratch.database_path())
        .load_or_insert_pair_agreement_secret(b"agreement-secret-one")
        .unwrap_or_else(|error| panic!("secret must persist: {error}"));
    let reopened = open_store(&scratch.database_path())
        .load_or_insert_pair_agreement_secret(b"agreement-secret-two")
        .unwrap_or_else(|error| panic!("secret must load: {error}"));

    let stored: Vec<u8> = Connection::open(scratch.database_path())
        .and_then(|connection| {
            connection.query_row(
                "SELECT sealed_secret FROM p2p_pair_agreement_secret",
                [],
                |row| row.get(0),
            )
        })
        .unwrap_or_else(|error| panic!("sealed row must exist: {error}"));

    assert_eq!(first.as_slice(), b"agreement-secret-one");
    assert_eq!(reopened.as_slice(), b"agreement-secret-one");
    assert!(
        !stored
            .windows(20)
            .any(|window| window == b"agreement-secret-one")
    );
}

/// The first key agreed with a paired device is stored sealed and pinned: agreeing it again is a
/// no-op, a different key is reported and ignored, and re-pairing keeps it.
#[test]
fn a_pair_key_is_pinned_sealed_and_kept_on_re_pairing() {
    let scratch = ScratchDirectory::new();
    let store = open_store(&scratch.database_path());
    let phone_id = DeviceId::new("peer-phone");
    store
        .trust_peer(&trusted_peer("peer-phone", "Phone", 1))
        .unwrap_or_else(|error| panic!("peer must be trusted: {error}"));
    assert!(
        store
            .pair_key(&phone_id)
            .unwrap_or_else(|error| panic!("pair key must be queryable: {error}"))
            .is_none()
    );

    let pin = |key_byte: u8| {
        store
            .pin_pair_key(&phone_id, &PairKey::from_bytes([key_byte; 32]))
            .unwrap_or_else(|error| panic!("pair key must pin: {error}"))
    };
    assert_eq!(pin(4), PairKeyPinning::Stored);
    assert_eq!(pin(4), PairKeyPinning::AlreadyPinned);
    assert_eq!(pin(5), PairKeyPinning::Mismatch);
    store
        .trust_peer(&trusted_peer("peer-phone", "Work phone", 2))
        .unwrap_or_else(|error| panic!("peer must be re-trusted: {error}"));
    drop(store);

    let pinned = open_store(&scratch.database_path())
        .pair_key(&phone_id)
        .unwrap_or_else(|error| panic!("pair key must load: {error}"))
        .unwrap_or_else(|| panic!("pair key must be stored"));
    let stored: Vec<u8> = Connection::open(scratch.database_path())
        .and_then(|connection| {
            connection.query_row("SELECT sealed_pair_key FROM p2p_trusted_peers", [], |row| {
                row.get(0)
            })
        })
        .unwrap_or_else(|error| panic!("sealed pair key must exist: {error}"));
    assert!(pinned.matches(&PairKey::from_bytes([4; 32])));
    assert!(!stored.windows(32).any(|window| window == [4; 32]));
}

/// Only paired devices get a pair key, and revoking a device deletes its key with it.
#[test]
fn pair_keys_belong_to_paired_devices_only() {
    let store = SqlitePeerStore::open_in_memory(fixed_cipher())
        .unwrap_or_else(|error| panic!("peer store must open: {error}"));
    let phone_id = DeviceId::new("peer-phone");

    let for_stranger = store
        .pin_pair_key(&phone_id, &PairKey::from_bytes([4; 32]))
        .unwrap_or_else(|error| panic!("pinning must answer: {error}"));
    store
        .trust_peer(&trusted_peer("peer-phone", "Phone", 1))
        .and_then(|()| store.pin_pair_key(&phone_id, &PairKey::from_bytes([4; 32])))
        .and_then(|_| store.revoke_peer(&phone_id))
        .and_then(|_| store.trust_peer(&trusted_peer("peer-phone", "Phone", 2)))
        .unwrap_or_else(|error| panic!("pair, revoke and pair again must succeed: {error}"));

    assert_eq!(for_stranger, PairKeyPinning::UnknownPeer);
    assert!(
        store
            .pair_key(&phone_id)
            .unwrap_or_else(|error| panic!("pair key must be queryable: {error}"))
            .is_none()
    );
}

/// A peer table written before Issue #50 gains the pair-key column when the store opens.
#[test]
fn an_older_peer_table_gains_the_pair_key_column() {
    let scratch = ScratchDirectory::new();
    Connection::open(scratch.database_path())
        .and_then(|connection| {
            connection.execute_batch(
                "CREATE TABLE p2p_trusted_peers (
                    peer_id TEXT PRIMARY KEY NOT NULL,
                    device_name TEXT NOT NULL,
                    paired_at_ms INTEGER NOT NULL
                );
                INSERT INTO p2p_trusted_peers VALUES ('peer-phone', 'Phone', 1);",
            )
        })
        .unwrap_or_else(|error| panic!("legacy table must be created: {error}"));

    let store = open_store(&scratch.database_path());
    let pinning = store
        .pin_pair_key(&DeviceId::new("peer-phone"), &PairKey::from_bytes([4; 32]))
        .unwrap_or_else(|error| panic!("pair key must pin: {error}"));

    assert_eq!(pinning, PairKeyPinning::Stored);
    assert_eq!(
        store
            .trusted_peers()
            .unwrap_or_else(|error| panic!("peers must list: {error}")),
        vec![trusted_peer("peer-phone", "Phone", 1)]
    );
}

/// Verifies network addresses start empty, and that a saved value survives reopening.
#[test]
fn network_addresses_survive_reopening() {
    let scratch = ScratchDirectory::new();
    let database_path = scratch.database_path();
    let saved = StoredNetworkAddresses {
        configured: Some(NetworkAddresses {
            relay: Some("/ip4/203.0.113.5/tcp/4001/p2p/12D3KooWRelay".to_owned()),
            peers: vec!["/ip4/203.0.113.6/tcp/4001/p2p/12D3KooWPeer".to_owned()],
        }),
        invited_peers: vec!["/ip4/203.0.113.7/tcp/4001/p2p/12D3KooWInvited".to_owned()],
    };
    let open = || open_store(&database_path);

    let first_read = open()
        .load_network_addresses()
        .unwrap_or_else(|error| panic!("addresses must load: {error}"));
    open()
        .save_network_addresses(&saved)
        .unwrap_or_else(|error| panic!("addresses must save: {error}"));
    let second_read = open()
        .load_network_addresses()
        .unwrap_or_else(|error| panic!("addresses must load: {error}"));

    assert_eq!(first_read, StoredNetworkAddresses::default());
    assert_eq!(second_read, saved);
}
