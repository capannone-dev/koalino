//! Declares encrypted P2P networking contracts without exposing libp2p types to use cases.

use crate::domain::{errors::NetworkError, models::DeviceId};

/// Describes a portable state change emitted by an encrypted peer network.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NetworkEvent {
    /// An authenticated peer became discoverable through a decentralized mechanism.
    PeerDiscovered(DeviceId),
    /// A previously discovered peer is no longer reachable through that mechanism.
    PeerExpired(DeviceId),
    /// An authenticated peer delivered an opaque application payload.
    MessageReceived {
        /// Identifies the authenticated sender of the payload.
        peer_device_id: DeviceId,
        /// Contains opaque application data; callers must apply their own payload validation.
        payload: Vec<u8>,
    },
}

/// Sends messages exclusively through an end-to-end encrypted peer transport.
pub trait P2pNetwork {
    /// Publishes an opaque payload to an already-authenticated peer.
    ///
    /// Inputs: `peer_device_id` identifies the destination and `payload` is application-encrypted data.
    /// Outputs: Success or a portable network error.
    /// Side effects: An implementation transmits data over libp2p encrypted transport.
    fn send_encrypted(
        &mut self,
        peer_device_id: &DeviceId,
        payload: &[u8],
    ) -> Result<(), NetworkError>;

    /// Returns one already-observed portable network event without waiting for socket activity.
    ///
    /// Inputs: None.
    /// Outputs: The next event, `None` when no event is pending, or a portable network error.
    /// Side effects: An implementation may remove the returned event from an internal queue.
    fn poll_event(&mut self) -> Result<Option<NetworkEvent>, NetworkError>;
}

/// Discovers peers using decentralized mechanisms such as mDNS and NAT traversal.
pub trait PeerDiscovery {
    /// Lists currently reachable authenticated peer device identifiers.
    ///
    /// Inputs: None.
    /// Outputs: Reachable peers or a portable network error.
    /// Side effects: An implementation may query or update decentralized peer-discovery state.
    fn reachable_peers(&self) -> Result<Vec<DeviceId>, NetworkError>;
}
