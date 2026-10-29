//! Persists the P2P node identity (sealed with the database key) and the trusted-peer allowlist.
//!
//! The identity lives in the singleton table `p2p_node_identity`. It is inserted with
//! `INSERT OR IGNORE` and then read back, so a daemon and an app starting together on one database
//! agree on a single identity.
//!
//! The X25519 pair-agreement secret (Issue #50) lives the same way in `p2p_pair_agreement_secret`.
//! Each trusted peer's agreed key is sealed in `p2p_trusted_peers.sealed_pair_key`, bound to the
//! peer ID; older tables gain that column when the store opens. Keys are never logged.

use std::{path::Path, sync::Arc, time::Duration};

use rusqlite::{Connection, OptionalExtension, params};
use zeroize::Zeroizing;

use super::sqlite_adapter::sqlite_error;
use crate::{
    domain::{
        errors::StorageError,
        models::DeviceId,
        peer_trust::{DeviceName, PairKey, TrustedPeer},
    },
    ports::{
        crypto::EmbeddingCipher,
        peer_trust::{
            NodeIdentityStore, PairAgreementSecretStore, PairKeyPinning, TrustedPeerStore,
        },
    },
};

/// How long a write waits for a concurrent writer before failing.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// AES-GCM associated-data context binding the sealed identity to its record.
const NODE_IDENTITY_CONTEXT: &str = "p2p_node_identity";

/// AES-GCM associated-data context binding the sealed pair-agreement secret to its record.
const PAIR_AGREEMENT_SECRET_CONTEXT: &str = "p2p_pair_agreement_secret";

/// The AES-GCM associated-data context binding a sealed pair key to its peer's row.
fn pair_key_context(peer_id: &DeviceId) -> String {
    format!("p2p_trusted_peers:pair_key:{}", peer_id.as_str())
}

/// SQLite-backed peer store.
pub struct SqlitePeerStore {
    connection: Connection,
    cipher: Arc<dyn EmbeddingCipher>,
}

impl SqlitePeerStore {
    /// Opens (creating when needed) the peer tables in the database at `database_path`.
    ///
    /// Inputs: The database path, shared with the note repository, and the database's cipher.
    /// Outputs: A ready store, or a storage error.
    /// Side effects: Opens a SQLite connection and creates the peer tables idempotently.
    pub fn open(
        database_path: impl AsRef<Path>,
        cipher: Arc<dyn EmbeddingCipher>,
    ) -> Result<Self, StorageError> {
        let connection = Connection::open(database_path)
            .map_err(|error| sqlite_error("unable to open peer store database", error))?;
        Self::from_connection(connection, cipher)
    }

    /// Opens an isolated in-memory peer store for tests.
    ///
    /// Inputs: The cipher for sealed values. Outputs: A ready store, or a storage error.
    /// Side effects: Allocates an in-memory SQLite database.
    pub fn open_in_memory(cipher: Arc<dyn EmbeddingCipher>) -> Result<Self, StorageError> {
        let connection = Connection::open_in_memory()
            .map_err(|error| sqlite_error("unable to open in-memory peer store", error))?;
        Self::from_connection(connection, cipher)
    }

    /// Configures the connection and creates the peer schema.
    ///
    /// Inputs: An open connection and the cipher. Outputs: The store, or a storage error.
    /// Side effects: Sets a busy timeout, executes `CREATE TABLE IF NOT EXISTS`, and adds the
    /// pair-key column to a peer table created before Issue #50.
    fn from_connection(
        connection: Connection,
        cipher: Arc<dyn EmbeddingCipher>,
    ) -> Result<Self, StorageError> {
        connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(|error| sqlite_error("unable to configure peer store busy timeout", error))?;
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS p2p_node_identity (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    sealed_identity BLOB NOT NULL
                );
                CREATE TABLE IF NOT EXISTS p2p_pair_agreement_secret (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    sealed_secret BLOB NOT NULL
                );
                CREATE TABLE IF NOT EXISTS p2p_trusted_peers (
                    peer_id TEXT PRIMARY KEY NOT NULL,
                    device_name TEXT NOT NULL,
                    paired_at_ms INTEGER NOT NULL,
                    sealed_pair_key BLOB
                );",
            )
            .map_err(|error| sqlite_error("unable to initialize peer store schema", error))?;
        let has_pair_key_column: bool = connection
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM pragma_table_info('p2p_trusted_peers')
                 WHERE name = 'sealed_pair_key')",
                [],
                |row| row.get(0),
            )
            .map_err(|error| sqlite_error("unable to inspect the trusted-peer columns", error))?;
        if !has_pair_key_column {
            connection
                .execute_batch("ALTER TABLE p2p_trusted_peers ADD COLUMN sealed_pair_key BLOB")
                .map_err(|error| sqlite_error("unable to add the pair-key column", error))?;
        }
        Ok(Self { connection, cipher })
    }
}

impl NodeIdentityStore for SqlitePeerStore {
    /// Seals and inserts `candidate` unless an identity exists, then opens the stored one.
    ///
    /// Side effects: Writes at most one row; logs only that an identity was loaded.
    fn load_or_insert_node_identity(
        &self,
        candidate: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, StorageError> {
        let sealed_candidate = self
            .cipher
            .seal_secret(NODE_IDENTITY_CONTEXT, candidate)
            .map_err(|error| StorageError::Rejected {
                reason: format!("unable to seal the node identity: {error}"),
            })?;
        self.connection
            .execute(
                "INSERT OR IGNORE INTO p2p_node_identity (singleton, sealed_identity)
                 VALUES (1, ?1)",
                params![sealed_candidate],
            )
            .map_err(|error| sqlite_error("unable to store the node identity", error))?;
        let stored_identity: Vec<u8> = self
            .connection
            .query_row(
                "SELECT sealed_identity FROM p2p_node_identity WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|error| sqlite_error("unable to load the node identity", error))?;
        let opened_identity = self
            .cipher
            .open_secret(NODE_IDENTITY_CONTEXT, &stored_identity)
            .map_err(|error| StorageError::Rejected {
                reason: format!("the stored node identity cannot be opened: {error}"),
            })?;
        tracing::debug!("P2P node identity loaded");
        Ok(opened_identity)
    }
}

impl PairAgreementSecretStore for SqlitePeerStore {
    /// Seals and inserts `candidate` unless a secret exists, then opens the stored one.
    ///
    /// Side effects: Writes at most one row; logs nothing about the secret.
    fn load_or_insert_pair_agreement_secret(
        &self,
        candidate: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, StorageError> {
        let sealed_candidate = self
            .cipher
            .seal_secret(PAIR_AGREEMENT_SECRET_CONTEXT, candidate)
            .map_err(|error| StorageError::Rejected {
                reason: format!("unable to seal the pair-agreement secret: {error}"),
            })?;
        self.connection
            .execute(
                "INSERT OR IGNORE INTO p2p_pair_agreement_secret (singleton, sealed_secret)
                 VALUES (1, ?1)",
                params![sealed_candidate],
            )
            .map_err(|error| sqlite_error("unable to store the pair-agreement secret", error))?;
        let stored_secret: Vec<u8> = self
            .connection
            .query_row(
                "SELECT sealed_secret FROM p2p_pair_agreement_secret WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|error| sqlite_error("unable to load the pair-agreement secret", error))?;
        self.cipher
            .open_secret(PAIR_AGREEMENT_SECRET_CONTEXT, &stored_secret)
            .map_err(|error| StorageError::Rejected {
                reason: format!("the stored pair-agreement secret cannot be opened: {error}"),
            })
    }
}

impl TrustedPeerStore for SqlitePeerStore {
    /// Upserts the peer row. Side effects: Writes one row; logs the peer ID only.
    fn trust_peer(&self, peer: &TrustedPeer) -> Result<(), StorageError> {
        let paired_at_ms =
            i64::try_from(peer.paired_at_ms()).map_err(|_| StorageError::Rejected {
                reason: String::from("pairing time exceeds SQLite integer range"),
            })?;
        self.connection
            .execute(
                "INSERT INTO p2p_trusted_peers (peer_id, device_name, paired_at_ms)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(peer_id) DO UPDATE
                 SET device_name = excluded.device_name, paired_at_ms = excluded.paired_at_ms",
                params![
                    peer.peer_id().as_str(),
                    peer.device_name().as_str(),
                    paired_at_ms
                ],
            )
            .map_err(|error| sqlite_error("unable to store the trusted peer", error))?;
        tracing::info!(peer_id = %peer.peer_id().as_str(), "P2P peer trusted");
        Ok(())
    }

    /// Deletes the peer row. Side effects: Deletes at most one row; logs the peer ID only.
    fn revoke_peer(&self, peer_id: &DeviceId) -> Result<bool, StorageError> {
        let removed_rows = self
            .connection
            .execute(
                "DELETE FROM p2p_trusted_peers WHERE peer_id = ?1",
                params![peer_id.as_str()],
            )
            .map_err(|error| sqlite_error("unable to revoke the trusted peer", error))?;
        if removed_rows > 0 {
            tracing::info!(peer_id = %peer_id.as_str(), "P2P peer revoked");
        }
        Ok(removed_rows > 0)
    }

    /// Reads every peer row, oldest pairing first (ties by peer ID).
    fn trusted_peers(&self) -> Result<Vec<TrustedPeer>, StorageError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT peer_id, device_name, paired_at_ms FROM p2p_trusted_peers
                 ORDER BY paired_at_ms, peer_id",
            )
            .map_err(|error| sqlite_error("unable to prepare the trusted-peer listing", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(|error| sqlite_error("unable to list trusted peers", error))?;
        let mut peers = Vec::new();
        for row in rows {
            let (peer_id, device_name, paired_at_ms) =
                row.map_err(|error| sqlite_error("unable to read a trusted peer", error))?;
            let device_name =
                DeviceName::parse(&device_name).map_err(|error| StorageError::Rejected {
                    reason: format!("stored trusted-peer name is invalid: {error}"),
                })?;
            let paired_at_ms = u64::try_from(paired_at_ms).map_err(|_| StorageError::Rejected {
                reason: String::from("stored pairing time is negative"),
            })?;
            peers.push(TrustedPeer::new(
                DeviceId::new(peer_id),
                device_name,
                paired_at_ms,
            ));
        }
        Ok(peers)
    }

    /// Looks up one peer row.
    fn is_trusted(&self, peer_id: &DeviceId) -> Result<bool, StorageError> {
        self.connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM p2p_trusted_peers WHERE peer_id = ?1)",
                params![peer_id.as_str()],
                |row| row.get(0),
            )
            .map_err(|error| sqlite_error("unable to query a trusted peer", error))
    }

    /// Stores the key only into a paired row that has none, so the first key stays pinned.
    ///
    /// Side effects: Writes at most one column; logs the peer ID only.
    fn pin_pair_key(
        &self,
        peer_id: &DeviceId,
        key: &PairKey,
    ) -> Result<PairKeyPinning, StorageError> {
        let sealed_key = self
            .cipher
            .seal_secret(&pair_key_context(peer_id), key.as_bytes())
            .map_err(|error| StorageError::Rejected {
                reason: format!("unable to seal the pair key: {error}"),
            })?;
        let stored_rows = self
            .connection
            .execute(
                "UPDATE p2p_trusted_peers SET sealed_pair_key = ?1
                 WHERE peer_id = ?2 AND sealed_pair_key IS NULL",
                params![sealed_key, peer_id.as_str()],
            )
            .map_err(|error| sqlite_error("unable to store the pair key", error))?;
        if stored_rows > 0 {
            tracing::info!(peer_id = %peer_id.as_str(), "pair key agreed with a paired device");
            return Ok(PairKeyPinning::Stored);
        }
        Ok(match self.pair_key(peer_id)? {
            None => PairKeyPinning::UnknownPeer,
            Some(pinned_key) if pinned_key.matches(key) => PairKeyPinning::AlreadyPinned,
            Some(_) => PairKeyPinning::Mismatch,
        })
    }

    /// Reads and opens the peer's sealed key.
    fn pair_key(&self, peer_id: &DeviceId) -> Result<Option<PairKey>, StorageError> {
        let sealed_key: Option<Vec<u8>> = self
            .connection
            .query_row(
                "SELECT sealed_pair_key FROM p2p_trusted_peers WHERE peer_id = ?1",
                params![peer_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| sqlite_error("unable to load the pair key", error))?
            .flatten();
        let Some(sealed_key) = sealed_key else {
            return Ok(None);
        };
        let key_bytes = self
            .cipher
            .open_secret(&pair_key_context(peer_id), &sealed_key)
            .map_err(|error| StorageError::Rejected {
                reason: format!("the stored pair key cannot be opened: {error}"),
            })?;
        let key_bytes: [u8; 32] =
            key_bytes
                .as_slice()
                .try_into()
                .map_err(|_| StorageError::Rejected {
                    reason: String::from("the stored pair key has the wrong length"),
                })?;
        Ok(Some(PairKey::from_bytes(key_bytes)))
    }
}
