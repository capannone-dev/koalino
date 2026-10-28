//! Declares persistence for this node's P2P identity.
//!
//! The libp2p adapter encodes its keypair as opaque bytes and asks this port to keep it, so peer
//! IDs stay stable across restarts (a prerequisite for remembering paired devices, Issue #43).
//! Implementations seal the bytes at rest; callers never see the stored form.

use zeroize::Zeroizing;

use crate::domain::errors::StorageError;

/// Stores the single P2P identity of the node that owns a database.
pub trait NodeIdentityStore {
    /// Returns the stored identity, first storing `candidate` when none exists yet.
    ///
    /// Inputs: `candidate` is a freshly generated, encoded private identity; it is only kept when
    /// the database has none, so concurrent first starts all end up with the same identity.
    /// Outputs: The stored identity in memory that is wiped on drop, or a storage error. A sealed
    /// identity that cannot be opened (wrong key, tampering) is an error, never replaced.
    /// Side effects: May insert one sealed row.
    fn load_or_insert_node_identity(
        &self,
        candidate: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, StorageError>;
}
