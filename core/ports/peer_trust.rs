//! Declares persistence for this node's P2P identity and its trusted (paired) peers, and the
//! transport contract of the pairing exchange.
//!
//! The libp2p adapter encodes its keypair as opaque bytes and asks this port to keep it, so peer
//! IDs stay stable across restarts (a prerequisite for remembering paired devices, Issue #43).
//! Implementations seal the bytes at rest; callers never see the stored form. The same holds for
//! the X25519 secret and the per-pair keys that protect review clips between paired devices
//! (Issue #50).

use zeroize::Zeroizing;

use std::time::Duration;

use crate::domain::{
    errors::{NetworkError, StorageError},
    models::DeviceId,
    network_addresses::StoredNetworkAddresses,
    peer_trust::{DeviceName, PairKey, TrustedPeer},
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

/// Stores this node's single X25519 pair-agreement secret (Issue #50).
pub trait PairAgreementSecretStore {
    /// Returns the stored secret, first storing `candidate` when none exists yet.
    ///
    /// Inputs: Freshly generated secret bytes, kept only when the database has none.
    /// Outputs: The stored secret in memory that is wiped on drop, or a storage error; a sealed
    /// secret that cannot be opened is an error, never replaced.
    /// Side effects: May insert one sealed row.
    fn load_or_insert_pair_agreement_secret(
        &self,
        candidate: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, StorageError>;
}

/// What happened when a pair key was offered for a paired device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairKeyPinning {
    /// The device had no key; this one is now stored.
    Stored,
    /// The device already has this exact key.
    AlreadyPinned,
    /// The device already has a different key, which is kept; the offered one is discarded.
    Mismatch,
    /// The device is not paired, so nothing was stored.
    UnknownPeer,
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

    /// Pins the key agreed with a paired device, the first time one is agreed (Issue #50).
    ///
    /// Inputs: The paired device and the key derived from its public key.
    /// Outputs: What happened (see `PairKeyPinning`), or a storage error.
    /// Side effects: Stores the key sealed in the device's row when it has none. Re-pairing keeps
    /// the key; revoking deletes it with the row.
    fn pin_pair_key(
        &self,
        peer_id: &DeviceId,
        key: &PairKey,
    ) -> Result<PairKeyPinning, StorageError>;

    /// Returns the key pinned for a paired device.
    ///
    /// Outputs: The key, `None` when the device is unpaired or has not sent its public key yet, or
    /// a storage error (including a sealed key that cannot be opened). Side effects: None.
    fn pair_key(&self, peer_id: &DeviceId) -> Result<Option<PairKey>, StorageError>;

    /// Remembers the ID a paired device writes on its meetings, so they keep its name while it is
    /// away (#88).
    ///
    /// Inputs: The paired device and the ID from its manifest. Outputs: Success (nothing happens
    /// for an unpaired device), or a storage error. Side effects: Updates the device's row; stores
    /// that keep nothing ignore it.
    fn remember_storage_device_id(
        &self,
        _peer_id: &DeviceId,
        _storage_device_id: &DeviceId,
    ) -> Result<(), StorageError> {
        Ok(())
    }
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

/// Keeps the relay, static peers, and invited peers between restarts (Issues #64 and #65).
pub trait NetworkAddressStore {
    /// Reads what is stored.
    ///
    /// Outputs: The addresses, empty if never saved, or a storage error.
    /// Side effects: Reads one row.
    fn load_network_addresses(&self) -> Result<StoredNetworkAddresses, StorageError>;

    /// Replaces what is stored.
    ///
    /// Outputs: Success or a storage error. Side effects: Writes one row.
    fn save_network_addresses(
        &self,
        addresses: &StoredNetworkAddresses,
    ) -> Result<(), StorageError>;
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

    /// The name a nearby unpaired device gave when asked (#121), or `None` while unknown: it
    /// answers only while its pairing window is open. The name is a claim, never trust.
    fn nearby_peer_name(&self, _peer_id: &DeviceId) -> Option<DeviceName> {
        None
    }

    /// Sets the name this device gives nearby devices that ask (#121).
    ///
    /// Side effects: Later answers carry it.
    fn set_local_name(&mut self, _local_name: &DeviceName) {}

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
