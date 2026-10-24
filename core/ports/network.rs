//! Declares encrypted P2P networking contracts without exposing libp2p types to use cases.

use crate::domain::{errors::NetworkError, models::DeviceId};

/// Sends messages exclusively through an end-to-end encrypted peer transport.
pub trait P2pNetwork {
    /// Publishes an opaque payload to an already-authenticated peer.
    ///
    /// Inputs: `peer_device_id` identifies the destination and `payload` is application-encrypted data.
    /// Outputs: Success or a portable network error.
    /// Side effects: An implementation transmits data over libp2p encrypted transport.
    fn send_encrypted(&self, peer_device_id: &DeviceId, payload: &[u8])
    -> Result<(), NetworkError>;
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
