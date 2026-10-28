//! Applies portable encrypted-network events without depending on libp2p types.

use crate::{
    domain::{errors::NetworkError, models::DeviceId},
    ports::network::{NetworkEvent, P2pNetwork},
};

/// Describes a failure while applying an event received through the P2P port.
#[derive(Debug, thiserror::Error)]
pub enum NetworkEventHandlingError {
    /// The injected encrypted-network port could not provide its next event.
    #[error(transparent)]
    Network(#[from] NetworkError),
}

/// Maintains application-visible reachability from portable P2P discovery events.
pub struct NetworkEventHandler {
    reachable_peer_device_ids: Vec<DeviceId>,
}

impl NetworkEventHandler {
    /// Creates an event handler with no discovered peers.
    ///
    /// Inputs: None.
    /// Outputs: A handler with an empty in-memory reachability view.
    /// Side effects: None.
    pub fn new() -> Self {
        Self {
            reachable_peer_device_ids: Vec::new(),
        }
    }

    /// Pulls and applies at most one pending event from an injected P2P port.
    ///
    /// Inputs: `p2p_network` supplies only portable `NetworkEvent` values.
    /// Outputs: Success when there is no event or it is applied; a portable network error otherwise.
    /// Side effects: Updates in-memory reachability state and consumes one queued port event.
    pub fn process_next_event<P2pNetworkPort>(
        &mut self,
        p2p_network: &mut P2pNetworkPort,
    ) -> Result<(), NetworkEventHandlingError>
    where
        P2pNetworkPort: P2pNetwork,
    {
        if let Some(network_event) = p2p_network.poll_event()? {
            self.handle_event(network_event);
        }

        Ok(())
    }

    /// Returns discovered peers in the order in which they first became reachable.
    ///
    /// Inputs: None.
    /// Outputs: A borrowed ordered slice of portable device identifiers.
    /// Side effects: None.
    pub fn reachable_peers(&self) -> &[DeviceId] {
        &self.reachable_peer_device_ids
    }

    /// Applies one portable event to application-owned network state.
    ///
    /// Inputs: `network_event` was translated by a P2P adapter and contains no adapter types.
    /// Outputs: None.
    /// Side effects: Adds or removes an in-memory reachable peer; opaque payloads are deferred to
    /// future application protocols and are intentionally not logged.
    fn handle_event(&mut self, network_event: NetworkEvent) {
        match network_event {
            NetworkEvent::PeerDiscovered(discovered_device_id) => {
                if !self
                    .reachable_peer_device_ids
                    .contains(&discovered_device_id)
                {
                    self.reachable_peer_device_ids.push(discovered_device_id);
                }
            }
            NetworkEvent::PeerExpired(expired_device_id) => {
                self.reachable_peer_device_ids
                    .retain(|reachable_device_id| reachable_device_id != &expired_device_id);
            }
            NetworkEvent::MessageReceived { .. } | NetworkEvent::DeliveryFailed(_) => {}
        }
    }
}

impl Default for NetworkEventHandler {
    /// Creates an empty event handler for APIs that require a default value.
    ///
    /// Inputs: None.
    /// Outputs: An event handler with no discovered peers.
    /// Side effects: None.
    fn default() -> Self {
        Self::new()
    }
}
