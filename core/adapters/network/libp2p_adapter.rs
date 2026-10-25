//! Builds and drives a Noise-encrypted libp2p swarm while exposing only portable P2P port values.

use std::collections::{HashSet, VecDeque};

use futures::StreamExt;
use libp2p::{
    PeerId, SwarmBuilder, mdns, noise,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    tcp, yamux,
};

use crate::{
    domain::{errors::NetworkError, models::DeviceId},
    ports::network::{NetworkEvent, P2pNetwork, PeerDiscovery},
};

/// Aggregates the decentralized discovery behaviours carried by the libp2p swarm.
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "NoteTakerBehaviourEvent")]
struct NoteTakerBehaviour {
    /// Discovers peers on the local network without a centralized registry.
    mdns: mdns::tokio::Behaviour,
}

/// Normalizes individual behaviour events for the swarm event loop.
#[derive(Debug)]
enum NoteTakerBehaviourEvent {
    /// Carries a local-network mDNS discovery or expiry update.
    Mdns(mdns::Event),
}

impl From<mdns::Event> for NoteTakerBehaviourEvent {
    /// Wraps an mDNS event for the generated libp2p behaviour dispatcher.
    ///
    /// Inputs: `mdns_event` is emitted by libp2p's local discovery behaviour.
    /// Outputs: The equivalent aggregate behaviour event.
    /// Side effects: None.
    fn from(mdns_event: mdns::Event) -> Self {
        Self::Mdns(mdns_event)
    }
}

/// Owns a local-only libp2p swarm and translates its events at the hexagonal boundary.
pub struct Libp2pNetworkAdapter {
    swarm: Swarm<NoteTakerBehaviour>,
    reachable_peer_device_ids: HashSet<DeviceId>,
    pending_network_events: VecDeque<NetworkEvent>,
}

impl Libp2pNetworkAdapter {
    /// Constructs a swarm using libp2p's Noise encrypted TCP transport, Yamux, and mDNS.
    ///
    /// Inputs: None; libp2p generates an ephemeral Ed25519 identity for this process.
    /// Outputs: A configured adapter or a portable security-policy error if mDNS cannot initialize.
    /// Side effects: Allocates libp2p state. It neither listens on a TCP address nor sends data.
    pub fn new() -> Result<Self, NetworkError> {
        let local_identity_keypair = libp2p::identity::Keypair::generate_ed25519();
        let local_peer_id = local_identity_keypair.public().to_peer_id();
        let swarm = SwarmBuilder::with_existing_identity(local_identity_keypair)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )
            .map_err(|error| NetworkError::SecurityPolicy {
                reason: format!("cannot configure Noise-encrypted TCP transport: {error}"),
            })?
            .with_behaviour(|_| {
                Ok(NoteTakerBehaviour {
                    mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?,
                })
            })
            .map_err(|error| NetworkError::SecurityPolicy {
                reason: format!("cannot initialize mDNS discovery: {error}"),
            })?
            .build();

        Ok(Self {
            swarm,
            reachable_peer_device_ids: HashSet::new(),
            pending_network_events: VecDeque::new(),
        })
    }

    /// Awaits one libp2p swarm event and queues its portable representation when relevant.
    ///
    /// Inputs: None; callers must execute this future on the application's Tokio runtime.
    /// Outputs: Success after one swarm event is consumed, or a portable disconnection error.
    /// Side effects: Drives the swarm, updates adapter reachability, and queues translated events.
    pub async fn drive_next_swarm_event(&mut self) -> Result<(), NetworkError> {
        let swarm_event = self.swarm.next().await.ok_or(NetworkError::Disconnected)?;
        self.translate_swarm_event(swarm_event);
        Ok(())
    }

    /// Translates libp2p behaviour events into port-level events without exposing peer IDs above it.
    ///
    /// Inputs: `swarm_event` is emitted by the owned libp2p swarm.
    /// Outputs: None.
    /// Side effects: Updates local discovery state and may enqueue a portable discovery event.
    fn translate_swarm_event(&mut self, swarm_event: SwarmEvent<NoteTakerBehaviourEvent>) {
        if let SwarmEvent::Behaviour(NoteTakerBehaviourEvent::Mdns(mdns_event)) = swarm_event {
            self.translate_mdns_event(mdns_event);
        }
    }

    /// Converts a local mDNS update into portable device reachability changes.
    ///
    /// Inputs: `mdns_event` lists libp2p peers and their local addresses.
    /// Outputs: None.
    /// Side effects: Updates adapter reachability and appends translated events to its queue.
    fn translate_mdns_event(&mut self, mdns_event: mdns::Event) {
        match mdns_event {
            mdns::Event::Discovered(discovered_peers) => {
                for (peer_id, _discovered_address) in discovered_peers {
                    self.record_discovered_peer(peer_id);
                }
            }
            mdns::Event::Expired(expired_peers) => {
                for (peer_id, _expired_address) in expired_peers {
                    self.record_expired_peer(peer_id);
                }
            }
        }
    }

    /// Records a peer discovery exactly once, even if mDNS reports several addresses.
    ///
    /// Inputs: `peer_id` is the libp2p identity authenticated by mDNS discovery.
    /// Outputs: None.
    /// Side effects: Adds reachability and queues a portable discovery event on first observation.
    fn record_discovered_peer(&mut self, peer_id: PeerId) {
        let peer_device_id = Self::device_id_from_peer_id(peer_id);
        if self
            .reachable_peer_device_ids
            .insert(peer_device_id.clone())
        {
            self.pending_network_events
                .push_back(NetworkEvent::PeerDiscovered(peer_device_id));
        }
    }

    /// Records a peer expiry when mDNS no longer advertises the peer.
    ///
    /// Inputs: `peer_id` is the libp2p identity whose discovery entry expired.
    /// Outputs: None.
    /// Side effects: Removes reachability and queues a portable expiry event when it was known.
    fn record_expired_peer(&mut self, peer_id: PeerId) {
        let peer_device_id = Self::device_id_from_peer_id(peer_id);
        if self.reachable_peer_device_ids.remove(&peer_device_id) {
            self.pending_network_events
                .push_back(NetworkEvent::PeerExpired(peer_device_id));
        }
    }

    /// Converts a libp2p peer identity into the opaque device identifier used by the port.
    ///
    /// Inputs: `peer_id` is an authenticated libp2p public-key-derived identity.
    /// Outputs: A portable device identifier with no libp2p type leakage.
    /// Side effects: None.
    fn device_id_from_peer_id(peer_id: PeerId) -> DeviceId {
        DeviceId::new(peer_id.to_string())
    }
}

impl P2pNetwork for Libp2pNetworkAdapter {
    /// Rejects payload transmission until a dedicated encrypted application protocol is configured.
    ///
    /// Inputs: `peer_device_id` identifies the target and `payload` is opaque application data.
    /// Outputs: A security-policy error because Issue #6 configures discovery and transport only.
    /// Side effects: None; payload bytes are neither sent nor logged.
    fn send_encrypted(
        &mut self,
        _peer_device_id: &DeviceId,
        _payload: &[u8],
    ) -> Result<(), NetworkError> {
        Err(NetworkError::SecurityPolicy {
            reason: "an encrypted application payload protocol is not configured".to_owned(),
        })
    }

    /// Removes and returns one translated event already observed by the swarm driver.
    ///
    /// Inputs: None.
    /// Outputs: A queued portable event, `None` when the driver has no queued event, or an error.
    /// Side effects: Removes one event from the adapter's in-memory queue.
    fn poll_event(&mut self) -> Result<Option<NetworkEvent>, NetworkError> {
        Ok(self.pending_network_events.pop_front())
    }
}

impl PeerDiscovery for Libp2pNetworkAdapter {
    /// Lists peers currently reachable through local mDNS discovery.
    ///
    /// Inputs: None.
    /// Outputs: Portable device identifiers currently held in the adapter's discovery state.
    /// Side effects: None.
    fn reachable_peers(&self) -> Result<Vec<DeviceId>, NetworkError> {
        let mut reachable_peer_device_ids: Vec<_> =
            self.reachable_peer_device_ids.iter().cloned().collect();
        reachable_peer_device_ids.sort_by(|left_device_id, right_device_id| {
            left_device_id.as_str().cmp(right_device_id.as_str())
        });
        Ok(reachable_peer_device_ids)
    }
}
