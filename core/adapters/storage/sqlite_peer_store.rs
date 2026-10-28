//! Persists the P2P node identity (sealed with the database key) in SQLite.
//!
//! The identity lives in the singleton table `p2p_node_identity`. It is inserted with
//! `INSERT OR IGNORE` and then read back, so a daemon and an app starting together on one database
//! agree on a single identity.

use std::{path::Path, sync::Arc, time::Duration};

use rusqlite::{Connection, params};
use zeroize::Zeroizing;

use super::sqlite_adapter::sqlite_error;
use crate::{
    domain::errors::StorageError,
    ports::{crypto::EmbeddingCipher, peer_trust::NodeIdentityStore},
};

/// How long a write waits for a concurrent writer before failing.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// AES-GCM associated-data context binding the sealed identity to its record.
const NODE_IDENTITY_CONTEXT: &str = "p2p_node_identity";

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
    /// Side effects: Sets a busy timeout and executes `CREATE TABLE IF NOT EXISTS`.
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
                );",
            )
            .map_err(|error| sqlite_error("unable to initialize peer store schema", error))?;
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
