//! Declares persistence for this node's P2P identity and its trusted (paired) peers.
//!
//! The libp2p adapter encodes its keypair as opaque bytes and asks this port to keep it, so peer
//! IDs stay stable across restarts (a prerequisite for remembering paired devices, Issue #43).
//! Implementations seal the bytes at rest; callers never see the stored form.

use zeroize::Zeroizing;

use crate::domain::{errors::StorageError, models::DeviceId, peer_trust::TrustedPeer};

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

/// Stores the allowlist of devices paired with this node; only they may sync.
pub trait TrustedPeerStore {
    /// Adds `peer`, or updates its name and pairing time when it is already trusted.
    ///
    /// Inputs: A peer whose pairing the owner confirmed. Outputs: Success or a storage error.
    /// Side effects: Upserts one row.
    fn trust_peer(&self, peer: &TrustedPeer) -> Result<(), StorageError>;

    /// Removes `peer_id` from the allowlist.
    ///
    /// Inputs: The peer ID to revoke. Outputs: Whether the peer was trusted, or a storage error.
    /// Side effects: Deletes at most one row. Callers also disconnect the peer.
    fn revoke_peer(&self, peer_id: &DeviceId) -> Result<bool, StorageError>;

    /// Lists trusted peers, oldest pairing first.
    ///
    /// Outputs: The peers, or a storage error (including a stored name that no longer validates).
    /// Side effects: None.
    fn trusted_peers(&self) -> Result<Vec<TrustedPeer>, StorageError>;

    /// Reports whether `peer_id` is trusted.
    ///
    /// Outputs: `true` for a trusted peer, or a storage error. Side effects: None.
    fn is_trusted(&self, peer_id: &DeviceId) -> Result<bool, StorageError>;
}
