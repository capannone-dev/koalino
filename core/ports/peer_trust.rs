//! Declares persistence for this node's P2P identity and its trusted (paired) peers, and the
//! transport contract of the pairing exchange.
//!
//! The libp2p adapter encodes its keypair as opaque bytes and asks this port to keep it, so peer
//! IDs stay stable across restarts (a prerequisite for remembering paired devices, Issue #43).
//! Implementations seal the bytes at rest; callers never see the stored form.

use zeroize::Zeroizing;

use std::time::Duration;

use crate::domain::{
    errors::{NetworkError, StorageError},
    models::DeviceId,
    peer_trust::{DeviceName, TrustedPeer},
};

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

/// Something that happened during pairing, reported by the transport.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingEvent {
    /// An unpaired device asked to pair. The owner must compare codes and answer.
    Requested {
        /// The requesting device's authenticated peer ID.
        peer_id: DeviceId,
        /// The name it announced, already validated.
        device_name: DeviceName,
    },
    /// The device this node asked to pair answered.
    Answered {
        /// The answering device's peer ID.
        peer_id: DeviceId,
        /// Its name when its owner accepted; `None` for a rejection.
        accepted_as: Option<DeviceName>,
    },
    /// A pairing request this node sent could not be delivered or timed out.
    Failed {
        /// The peer the request was sent to.
        peer_id: DeviceId,
    },
}

/// The network side of pairing: admission control and the pairing exchange.
pub trait PairingTransport {
    /// This node's peer ID, used to derive pairing codes.
    fn local_peer_id(&self) -> DeviceId;

    /// Admits inbound connections from unpaired devices for `duration`, for pairing only.
    fn open_pairing_window(&mut self, duration: Duration);

    /// Closes the pairing window and disconnects devices that are still unpaired.
    fn end_pairing(&mut self);

    /// Lists unpaired devices currently visible on the local network.
    fn nearby_unpaired_peers(&self) -> Vec<DeviceId>;

    /// Asks `peer_id` to pair, announcing `local_name`.
    ///
    /// Outputs: Success once the request is queued; the outcome arrives as a `PairingEvent`.
    /// Side effects: Allows a connection to the peer and sends the request.
    fn request_pairing(
        &mut self,
        peer_id: &DeviceId,
        local_name: &DeviceName,
    ) -> Result<(), NetworkError>;

    /// Answers the pending request from `peer_id`: `Some(name)` accepts, `None` rejects.
    ///
    /// Outputs: Success, or an error when no request from that peer is pending.
    /// Side effects: Sends the answer.
    fn answer_pairing(
        &mut self,
        peer_id: &DeviceId,
        accept_as: Option<&DeviceName>,
    ) -> Result<(), NetworkError>;

    /// Lets `peer_id` connect and sync.
    fn trust_peer(&mut self, peer_id: &DeviceId) -> Result<(), NetworkError>;

    /// Stops trusting `peer_id` and disconnects it.
    fn revoke_peer(&mut self, peer_id: &DeviceId) -> Result<(), NetworkError>;

    /// Removes the next pairing event, if any.
    fn poll_pairing_event(&mut self) -> Option<PairingEvent>;
}
