//! Builds and drives a Noise-encrypted libp2p swarm while exposing only portable P2P port values.
//!
//! Besides direct TCP connections, a device can reach its peers through a relay on the owner's own
//! daemon (Circuit Relay v2) and upgrade to a direct connection by hole punching (DCUtR, with
//! Identify supplying the observed addresses). Noise runs end to end over a relayed connection, so
//! the relay forwards only ciphertext, and the destination's connection gate still decides who
//! gets in. Only peers paired with the relay may use it.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    io,
};

use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, StreamExt};
use libp2p::{
    Multiaddr, PeerId, SwarmBuilder,
    core::transport::ListenerId,
    dcutr, identify, mdns,
    multiaddr::Protocol,
    noise, relay, request_response,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent, behaviour::toggle::Toggle},
    tcp, yamux,
};

/// Limits one framed application payload to a bounded amount of adapter memory.
const MAX_SYNC_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;

/// Identifies this application to peers in the Identify exchange.
const IDENTIFY_PROTOCOL_VERSION: &str = "/note-taker/identify/1";
/// How long one relayed connection lasts before the devices dial again. libp2p's default, two
/// minutes, suits public relays; this one serves only the owner's devices.
const MAX_RELAYED_CIRCUIT_DURATION: std::time::Duration = std::time::Duration::from_secs(60 * 60);
/// How many bytes one relayed connection carries before the devices dial again; libp2p's default
/// of 128 KiB would not fit one sync message (`MAX_SYNC_PAYLOAD_BYTES`).
const MAX_RELAYED_CIRCUIT_BYTES: u64 = 1024 * 1024 * 1024;

/// Names the versioned libp2p substream protocol that carries opaque sync payloads.
#[derive(Clone)]
struct SyncPayloadProtocol;

impl AsRef<str> for SyncPayloadProtocol {
    /// Returns the protocol name negotiated by connected Note Taker peers.
    ///
    /// Inputs: None.
    /// Outputs: The stable, versioned protocol identifier.
    /// Side effects: None.
    fn as_ref(&self) -> &str {
        "/note-taker/sync-message/1"
    }
}

/// Encodes bounded opaque application bytes on a request-response substream.
#[derive(Clone, Default)]
struct SyncPayloadCodec;

impl SyncPayloadCodec {
    /// Reads one big-endian length-prefixed opaque payload from a libp2p substream.
    ///
    /// Inputs: `stream` is an authenticated negotiated substream positioned at a payload frame.
    /// Outputs: The payload bytes or an I/O error for malformed or over-sized frames.
    /// Side effects: Consumes bytes from the supplied stream without logging their contents.
    async fn read_payload<Stream>(&mut self, stream: &mut Stream) -> io::Result<Vec<u8>>
    where
        Stream: AsyncRead + Unpin + Send,
    {
        let mut encoded_payload_length = [0_u8; std::mem::size_of::<u32>()];
        stream.read_exact(&mut encoded_payload_length).await?;
        let payload_length = u32::from_be_bytes(encoded_payload_length) as usize;
        if payload_length > MAX_SYNC_PAYLOAD_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "sync payload exceeds the protocol size limit",
            ));
        }
        let mut payload = vec![0_u8; payload_length];
        stream.read_exact(&mut payload).await?;
        Ok(payload)
    }

    /// Writes one big-endian length-prefixed opaque payload to a libp2p substream.
    ///
    /// Inputs: `stream` is an authenticated negotiated substream and `payload` is opaque application data.
    /// Outputs: Success after the frame is flushed, or an I/O error if transmission cannot continue.
    /// Side effects: Writes application bytes to the encrypted libp2p substream without logging them.
    async fn write_payload<Stream>(
        &mut self,
        stream: &mut Stream,
        payload: Vec<u8>,
    ) -> io::Result<()>
    where
        Stream: AsyncWrite + Unpin + Send,
    {
        if payload.len() > MAX_SYNC_PAYLOAD_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "sync payload exceeds the protocol size limit",
            ));
        }
        let payload_length = u32::try_from(payload.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "sync payload exceeds the protocol size limit",
            )
        })?;
        stream.write_all(&payload_length.to_be_bytes()).await?;
        stream.write_all(&payload).await?;
        stream.close().await
    }
}

impl request_response::Codec for SyncPayloadCodec {
    type Protocol = SyncPayloadProtocol;
    type Request = Vec<u8>;
    type Response = Vec<u8>;

    /// Reads one sync payload sent as a request by an authenticated remote peer.
    async fn read_request<Stream>(
        &mut self,
        _protocol: &Self::Protocol,
        stream: &mut Stream,
    ) -> io::Result<Self::Request>
    where
        Stream: AsyncRead + Unpin + Send,
    {
        self.read_payload(stream).await
    }

    /// Reads an acknowledgement response for a previously sent sync payload.
    async fn read_response<Stream>(
        &mut self,
        _protocol: &Self::Protocol,
        stream: &mut Stream,
    ) -> io::Result<Self::Response>
    where
        Stream: AsyncRead + Unpin + Send,
    {
        self.read_payload(stream).await
    }

    /// Frames and writes an outbound sync payload request.
    async fn write_request<Stream>(
        &mut self,
        _protocol: &Self::Protocol,
        stream: &mut Stream,
        request: Self::Request,
    ) -> io::Result<()>
    where
        Stream: AsyncWrite + Unpin + Send,
    {
        self.write_payload(stream, request).await
    }

    /// Frames and writes the empty acknowledgement response for an inbound sync payload.
    async fn write_response<Stream>(
        &mut self,
        _protocol: &Self::Protocol,
        stream: &mut Stream,
        response: Self::Response,
    ) -> io::Result<()>
    where
        Stream: AsyncWrite + Unpin + Send,
    {
        self.write_payload(stream, response).await
    }
}

use crate::{
    adapters::network::{
        file_transfer_behaviour::{self, FileTransferRequestPayload, FileTransferResponsePayload},
        pairing_behaviour::{PairingCodec, PairingRequest, PairingResponse, new_pairing_behaviour},
        peer_gate::PeerGate,
    },
    domain::{errors::NetworkError, models::DeviceId, peer_trust::DeviceName},
    ports::{
        network::{NetworkEvent, P2pNetwork, PeerDiscovery},
        peer_trust::{NodeIdentityStore, PairingEvent, PairingTransport},
    },
};
use zeroize::Zeroizing;

/// Aggregates the decentralized discovery behaviours carried by the libp2p swarm.
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "NoteTakerBehaviourEvent")]
struct NoteTakerBehaviour {
    /// Refuses connections from unpaired peers; listed first so it is consulted first.
    peer_gate: PeerGate,
    /// Discovers peers on the local network without a centralized registry.
    mdns: Toggle<mdns::tokio::Behaviour>,
    /// Carries opaque versioned application payloads on encrypted authenticated substreams.
    sync_payloads: request_response::Behaviour<SyncPayloadCodec>,
    /// Carries attachment-only frames and never CRDT sync bytes.
    file_transfers: request_response::Behaviour<file_transfer_behaviour::FileTransferCodec>,
    /// Carries the pairing exchange, the only protocol an unpaired peer may use.
    pairing: request_response::Behaviour<PairingCodec>,
    /// Forwards encrypted circuits between paired devices, on the node chosen as relay.
    relay_server: Toggle<relay::Behaviour>,
    /// Holds a reservation on the owner's relay and opens circuits through it.
    relay_client: Toggle<relay::client::Behaviour>,
    /// Upgrades a relayed connection to a direct one by hole punching where the NAT allows.
    hole_punching: Toggle<dcutr::Behaviour>,
    /// Learns this node's observed addresses, which hole punching tries.
    identify: Toggle<identify::Behaviour>,
}

/// Normalizes individual behaviour events for the swarm event loop.
#[derive(Debug)]
enum NoteTakerBehaviourEvent {
    /// Carries a local-network mDNS discovery or expiry update.
    Mdns(mdns::Event),
    /// Carries an inbound or outbound event from the private sync-payload protocol.
    SyncPayload(request_response::Event<Vec<u8>, Vec<u8>>),
    /// Carries a message on the independently negotiated attachment protocol.
    FileTransfer(request_response::Event<FileTransferRequestPayload, FileTransferResponsePayload>),
    /// Carries a pairing request, answer, or failure.
    Pairing(request_response::Event<PairingRequest, PairingResponse>),
    /// Carries what the relay service did for a client.
    RelayServer(Box<relay::Event>),
    /// Carries the state of this node's reservation and circuits.
    RelayClient(relay::client::Event),
    /// Carries a hole-punching result.
    HolePunching(dcutr::Event),
    /// Marks an Identify exchange; Identify reports observed addresses to the swarm itself.
    Identify,
}

impl From<relay::Event> for NoteTakerBehaviourEvent {
    /// Wraps a relay service event.
    fn from(relay_event: relay::Event) -> Self {
        Self::RelayServer(Box::new(relay_event))
    }
}

impl From<relay::client::Event> for NoteTakerBehaviourEvent {
    /// Wraps a relay client event.
    fn from(relay_event: relay::client::Event) -> Self {
        Self::RelayClient(relay_event)
    }
}

impl From<dcutr::Event> for NoteTakerBehaviourEvent {
    /// Wraps a hole-punching event.
    fn from(hole_punching_event: dcutr::Event) -> Self {
        Self::HolePunching(hole_punching_event)
    }
}

impl From<identify::Event> for NoteTakerBehaviourEvent {
    /// Wraps an Identify event.
    fn from(_identify_event: identify::Event) -> Self {
        Self::Identify
    }
}

impl From<request_response::Event<PairingRequest, PairingResponse>> for NoteTakerBehaviourEvent {
    /// Wraps a pairing-protocol event.
    fn from(pairing_event: request_response::Event<PairingRequest, PairingResponse>) -> Self {
        Self::Pairing(pairing_event)
    }
}

impl From<std::convert::Infallible> for NoteTakerBehaviourEvent {
    /// The peer gate emits no events.
    fn from(never: std::convert::Infallible) -> Self {
        match never {}
    }
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

impl From<request_response::Event<Vec<u8>, Vec<u8>>> for NoteTakerBehaviourEvent {
    /// Wraps a private payload-protocol event for the generated libp2p behaviour dispatcher.
    ///
    /// Inputs: `sync_payload_event` is emitted by the libp2p request-response behaviour.
    /// Outputs: The equivalent aggregate behaviour event.
    /// Side effects: None.
    fn from(sync_payload_event: request_response::Event<Vec<u8>, Vec<u8>>) -> Self {
        Self::SyncPayload(sync_payload_event)
    }
}

impl From<request_response::Event<FileTransferRequestPayload, FileTransferResponsePayload>>
    for NoteTakerBehaviourEvent
{
    /// Wraps an attachment-only protocol event without conflating it with CRDT payloads.
    fn from(
        file_transfer_event: request_response::Event<
            FileTransferRequestPayload,
            FileTransferResponsePayload,
        >,
    ) -> Self {
        Self::FileTransfer(file_transfer_event)
    }
}

/// This node's long-lived libp2p identity (an Ed25519 keypair).
///
/// Paired devices are remembered by peer ID, so the identity must survive restarts: it is kept,
/// sealed with the database key, through `NodeIdentityStore`. `Debug` never prints key material
/// or the peer ID.
pub struct NodeIdentity(libp2p::identity::Keypair);

impl NodeIdentity {
    /// Loads this node's identity from `store`, creating and storing one on first use.
    ///
    /// Inputs: The store for the node's database.
    /// Outputs: The identity, or `SecurityPolicy` when the store fails or holds bytes that are not
    /// an encoded keypair (never replaced, because a new identity would break every pairing).
    /// Side effects: Generates a keypair from operating-system randomness; may write one row.
    pub fn load_or_create(store: &impl NodeIdentityStore) -> Result<Self, NetworkError> {
        let candidate_keypair = libp2p::identity::Keypair::generate_ed25519();
        let encoded_candidate =
            Zeroizing::new(candidate_keypair.to_protobuf_encoding().map_err(|error| {
                NetworkError::SecurityPolicy {
                    reason: format!("cannot encode the node identity: {error}"),
                }
            })?);
        let stored_identity = store
            .load_or_insert_node_identity(&encoded_candidate)
            .map_err(|error| NetworkError::SecurityPolicy {
                reason: format!("cannot load the node identity: {error}"),
            })?;
        libp2p::identity::Keypair::from_protobuf_encoding(&stored_identity)
            .map(Self)
            .map_err(|_| NetworkError::SecurityPolicy {
                reason: "the stored node identity is not a valid keypair".to_owned(),
            })
    }

    /// Generates an identity that is never stored, for tests and throwaway nodes.
    ///
    /// Side effects: Reads operating-system randomness.
    pub fn ephemeral() -> Self {
        Self(libp2p::identity::Keypair::generate_ed25519())
    }

    /// Returns this node's peer ID in the portable form used by the network port.
    pub fn peer_device_id(&self) -> DeviceId {
        Libp2pNetworkAdapter::device_id_from_peer_id(self.0.public().to_peer_id())
    }
}

impl std::fmt::Debug for NodeIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("NodeIdentity([redacted])")
    }
}

/// Owns a local-only libp2p swarm and translates its events at the hexagonal boundary.
pub struct Libp2pNetworkAdapter {
    swarm: Swarm<NoteTakerBehaviour>,
    reachable_peer_device_ids: HashSet<DeviceId>,
    /// Peers configured by address (`NOTE_TAKER_P2P_PEERS`), for networks where mDNS cannot
    /// find them; `redial_paired_peers` treats them as rediscovered.
    static_peer_ids: HashSet<PeerId>,
    pending_network_events: VecDeque<NetworkEvent>,
    /// Retains adapter-private attachment frames separately from CRDT network events.
    pending_file_transfer_payloads: VecDeque<Vec<u8>>,
    /// Unpaired peers advertising themselves on the LAN, offered as pairing candidates.
    nearby_unpaired_peer_ids: HashSet<DeviceId>,
    /// Pairing requests awaiting the owner's answer, by requesting peer.
    pending_pairing_answers: HashMap<PeerId, request_response::ResponseChannel<PairingResponse>>,
    /// Pairing events not yet taken by the application.
    pending_pairing_events: VecDeque<PairingEvent>,
    /// The relay this device reaches its peers through, when one is configured.
    relay: Option<RelayRoute>,
    /// Circuits this node's relay service has accepted.
    relayed_circuit_count: u64,
    /// Strangers admitted for pairing that this node's relay service disconnected.
    refused_relay_client_count: u64,
}

/// The relay a device uses and the state of its reservation there.
struct RelayRoute {
    /// The relay's address, ending in `/p2p/<relay id>`.
    address: Multiaddr,
    peer_id: PeerId,
    /// The listener holding the reservation, while one is being held or requested.
    listener: Option<ListenerId>,
    /// The relay accepted the reservation, so peers can reach this device through it.
    reservation_active: bool,
}

/// How long a connection with nothing keeping it open survives; libp2p's own default.
#[cfg(test)]
const DEFAULT_IDLE_CONNECTION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

impl Libp2pNetworkAdapter {
    /// Constructs a swarm using libp2p's Noise encrypted TCP transport, Yamux, and mDNS.
    ///
    /// Inputs: `node_identity`, normally from `NodeIdentity::load_or_create`.
    /// Outputs: A configured adapter or a portable security-policy error if mDNS cannot initialize.
    /// Side effects: Allocates libp2p state. It neither listens on a TCP address nor sends data.
    pub fn new(node_identity: NodeIdentity) -> Result<Self, NetworkError> {
        Self::build(node_identity, true, false)
    }

    /// Like `new`, and also offers a relay service to the devices paired with this one.
    ///
    /// Inputs: `node_identity`, as for `new`. Call `advertise_relay_address` too: without an
    /// address to hand out, the service cannot grant reservations.
    /// Outputs: A configured adapter or a portable security-policy error.
    /// Side effects: Allocates libp2p state.
    pub fn new_relay_server(node_identity: NodeIdentity) -> Result<Self, NetworkError> {
        Self::build(node_identity, true, true)
    }

    /// Like `new`, without mDNS, so that tests choose every route between their nodes.
    #[cfg(test)]
    fn without_mdns_for_tests(
        node_identity: NodeIdentity,
        relay_server: bool,
    ) -> Result<Self, NetworkError> {
        Self::build(node_identity, false, relay_server)
    }

    /// Builds the swarm.
    ///
    /// Inputs: The identity; whether to discover peers with mDNS and to offer a relay service.
    /// Outputs: The adapter, or a portable security-policy error.
    /// Side effects: Allocates libp2p state.
    fn build(
        node_identity: NodeIdentity,
        use_mdns: bool,
        relay_server: bool,
    ) -> Result<Self, NetworkError> {
        let local_identity_keypair = node_identity.0;
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
            .with_relay_client(noise::Config::new, yamux::Config::default)
            .map_err(|error| NetworkError::SecurityPolicy {
                reason: format!("cannot configure the relay transport: {error}"),
            })?
            .with_behaviour(|keypair, relay_client| {
                let mdns = if use_mdns {
                    Some(mdns::tokio::Behaviour::new(
                        mdns::Config::default(),
                        local_peer_id,
                    )?)
                } else {
                    None
                };
                Ok(NoteTakerBehaviour {
                    peer_gate: PeerGate::default(),
                    mdns: Toggle::from(mdns),
                    sync_payloads: Self::sync_payload_behaviour(),
                    file_transfers: file_transfer_behaviour::new_file_transfer_behaviour(),
                    pairing: new_pairing_behaviour(),
                    relay_server: Toggle::from(relay_server.then(|| {
                        relay::Behaviour::new(
                            local_peer_id,
                            relay::Config {
                                max_circuit_duration: MAX_RELAYED_CIRCUIT_DURATION,
                                max_circuit_bytes: MAX_RELAYED_CIRCUIT_BYTES,
                                ..relay::Config::default()
                            },
                        )
                    })),
                    relay_client: Toggle::from(Some(relay_client)),
                    hole_punching: Toggle::from(Some(dcutr::Behaviour::new(local_peer_id))),
                    identify: Toggle::from(Some(identify::Behaviour::new(identify::Config::new(
                        IDENTIFY_PROTOCOL_VERSION.to_owned(),
                        keypair.public(),
                    )))),
                })
            })
            .map_err(|error| NetworkError::SecurityPolicy {
                reason: format!("cannot initialize mDNS discovery: {error}"),
            })?
            .build();

        Ok(Self::around_swarm(swarm))
    }

    /// Wraps a built swarm with empty adapter state.
    fn around_swarm(swarm: Swarm<NoteTakerBehaviour>) -> Self {
        Self {
            swarm,
            reachable_peer_device_ids: HashSet::new(),
            static_peer_ids: HashSet::new(),
            pending_network_events: VecDeque::new(),
            pending_file_transfer_payloads: VecDeque::new(),
            nearby_unpaired_peer_ids: HashSet::new(),
            pending_pairing_answers: HashMap::new(),
            pending_pairing_events: VecDeque::new(),
            relay: None,
            relayed_circuit_count: 0,
            refused_relay_client_count: 0,
        }
    }

    /// Declares the public address this node's relay service hands to its clients.
    ///
    /// Inputs: A multiaddress reachable from the clients' networks, without `/p2p/`, such as
    /// `/ip4/203.0.113.5/tcp/4001`.
    /// Outputs: Success, or `SecurityPolicy` for an address that does not parse.
    /// Side effects: Records it as this node's external address.
    pub fn advertise_relay_address(&mut self, public_address: &str) -> Result<(), NetworkError> {
        let parsed_address: Multiaddr =
            public_address
                .parse()
                .map_err(|error| NetworkError::SecurityPolicy {
                    reason: format!("invalid P2P relay address: {error}"),
                })?;
        self.swarm.add_external_address(parsed_address);
        Ok(())
    }

    /// Reaches this device's peers through the relay at `relay_address`, and lets them reach it.
    ///
    /// Inputs: A multiaddress ending in `/p2p/<relay id>`. The relay must be paired with this
    /// device; until then it is only listed as nearby, as `add_static_peer` does.
    /// Outputs: Success, or `SecurityPolicy` for an address that does not parse, lacks the peer
    /// ID, or names this node.
    /// Side effects: Records the relay. `redial_paired_peers` keeps the reservation and dials
    /// paired peers through it; direct routes keep working, so losing the relay costs only this
    /// route.
    pub fn use_relay(&mut self, relay_address: &str) -> Result<(), NetworkError> {
        let relay_device_id = self.add_static_peer(relay_address)?;
        let peer_id = Self::peer_id_from_device_id(&relay_device_id)?;
        let address = relay_address
            .parse()
            .map_err(|error| NetworkError::SecurityPolicy {
                reason: format!("invalid P2P relay address: {error}"),
            })?;
        self.relay = Some(RelayRoute {
            address,
            peer_id,
            listener: None,
            reservation_active: false,
        });
        self.maintain_relay_route();
        Ok(())
    }

    /// Reports whether the relay holds a reservation for this device.
    pub fn has_relay_reservation(&self) -> bool {
        self.relay
            .as_ref()
            .is_some_and(|relay| relay.reservation_active)
    }

    /// Counts the circuits this node's relay service has accepted.
    pub fn relayed_circuits(&self) -> u64 {
        self.relayed_circuit_count
    }

    /// Counts the strangers this node's relay service disconnected.
    pub fn refused_relay_clients(&self) -> u64 {
        self.refused_relay_client_count
    }

    /// Requests a reservation once connected to the relay, and teaches the swarm the relayed
    /// address of each paired peer that is not connected.
    ///
    /// Side effects: May start a listener on the relay circuit and record peer addresses. A
    /// failed request closes the listener, so the next call tries again.
    fn maintain_relay_route(&mut self) {
        let Some(relay) = self.relay.as_ref() else {
            return;
        };
        let (relay_peer_id, relay_address) = (relay.peer_id, relay.address.clone());
        let circuit_address = relay_address.with(Protocol::P2pCircuit);
        if relay.listener.is_none()
            && self.swarm.is_connected(&relay_peer_id)
            && self.swarm.behaviour().peer_gate.is_trusted(&relay_peer_id)
        {
            match self.swarm.listen_on(circuit_address.clone()) {
                Ok(listener) => {
                    if let Some(relay) = self.relay.as_mut() {
                        relay.listener = Some(listener);
                    }
                }
                Err(_) => tracing::debug!("could not request a relay reservation"),
            }
        }
        let unconnected_peers: Vec<PeerId> = self
            .swarm
            .behaviour()
            .peer_gate
            .trusted_peers()
            .filter(|peer_id| **peer_id != relay_peer_id && !self.swarm.is_connected(peer_id))
            .copied()
            .collect();
        for peer_id in unconnected_peers {
            self.swarm.add_peer_address(
                peer_id,
                circuit_address.clone().with(Protocol::P2p(peer_id)),
            );
        }
    }

    /// Forgets the reservation when its listener is gone or the relay's last connection closed.
    ///
    /// Inputs: The closed listener, if that is what happened.
    /// Side effects: Removes the listener, so `maintain_relay_route` requests a new reservation.
    fn forget_relay_reservation(&mut self, closed_listener: Option<ListenerId>) {
        let Some(relay) = self.relay.as_mut() else {
            return;
        };
        if closed_listener.is_some() && closed_listener != relay.listener {
            return;
        }
        relay.reservation_active = false;
        if let Some(listener) = relay.listener.take() {
            self.swarm.remove_listener(listener);
        }
    }

    /// Applies what the relay service did for a client: only paired devices may use it, though a
    /// pairing window also admits strangers to this node.
    ///
    /// Side effects: Disconnects a stranger, and counts refusals and circuits.
    fn translate_relay_server_event(&mut self, relay_event: relay::Event) {
        let client = match relay_event {
            relay::Event::ReservationReqAccepted { src_peer_id, .. } => src_peer_id,
            relay::Event::CircuitReqAccepted { src_peer_id, .. } => {
                self.relayed_circuit_count = self.relayed_circuit_count.saturating_add(1);
                src_peer_id
            }
            _ => return,
        };
        if !self.swarm.behaviour().peer_gate.is_trusted(&client) {
            self.refused_relay_client_count = self.refused_relay_client_count.saturating_add(1);
            tracing::warn!("disconnected an unpaired device that tried to use the relay");
            self.swarm
                .behaviour_mut()
                .peer_gate
                .disconnect_if_untrusted(client);
        }
    }

    /// Starts listening for encrypted TCP connections on `listen_address`.
    ///
    /// Inputs: A multiaddress such as `/ip4/0.0.0.0/tcp/0` (port 0 picks a free port).
    /// Outputs: Success once the listener is registered, or `SecurityPolicy` for an address that
    /// does not parse or cannot be bound.
    /// Side effects: Binds a TCP socket when the swarm next runs. Every connection on it is
    /// authenticated with Noise and admitted only by the connection gate.
    pub fn listen_on(&mut self, listen_address: &str) -> Result<(), NetworkError> {
        let parsed_address: libp2p::Multiaddr =
            listen_address
                .parse()
                .map_err(|error| NetworkError::SecurityPolicy {
                    reason: format!("invalid P2P listen address: {error}"),
                })?;
        self.swarm
            .listen_on(parsed_address)
            .map(|_| ())
            .map_err(|error| NetworkError::SecurityPolicy {
                reason: format!("cannot listen for P2P connections: {error}"),
            })
    }

    /// Lists the addresses this node currently listens on, as multiaddress text.
    ///
    /// Side effects: None.
    pub fn listen_addresses(&self) -> Vec<String> {
        self.swarm
            .listeners()
            .map(|listen_address| listen_address.to_string())
            .collect()
    }

    /// Adds a peer reachable at a known address, for networks where mDNS cannot find it (another
    /// subnet, NAT, a VPN, or a tunnel).
    ///
    /// Inputs: A multiaddress ending in `/p2p/<peer id>`, such as
    /// `/ip4/192.168.1.20/tcp/4001/p2p/12D3Koo…`.
    /// Outputs: The peer's ID, or `SecurityPolicy` for an address that does not parse, lacks the
    /// peer ID, or names this node.
    /// Side effects: Records the address for dialing. An unpaired peer is listed as nearby, so the
    /// owner can pair with it; a paired one is dialed. The peer is still authenticated by Noise
    /// and admitted only by the connection gate, so a wrong address cannot impersonate it.
    pub fn add_static_peer(&mut self, peer_address: &str) -> Result<DeviceId, NetworkError> {
        let parsed_address: libp2p::Multiaddr =
            peer_address
                .parse()
                .map_err(|error| NetworkError::SecurityPolicy {
                    reason: format!("invalid P2P peer address: {error}"),
                })?;
        let Some(libp2p::multiaddr::Protocol::P2p(peer_id)) = parsed_address.iter().last() else {
            return Err(NetworkError::SecurityPolicy {
                reason: "a P2P peer address must end with /p2p/<peer id>".to_owned(),
            });
        };
        if peer_id == *self.swarm.local_peer_id() {
            return Err(NetworkError::SecurityPolicy {
                reason: "a P2P peer address must not name this device".to_owned(),
            });
        }
        self.swarm.add_peer_address(peer_id, parsed_address);
        self.static_peer_ids.insert(peer_id);
        self.record_discovered_peer(peer_id);
        Ok(Self::device_id_from_peer_id(peer_id))
    }

    /// Dials every paired peer that is not connected, and lists unpaired configured peers as
    /// nearby, so a device that comes back online reconnects.
    ///
    /// Paired connections stay open while both devices are online, so this only matters after
    /// one went away. mDNS reports a peer once and then keeps its address cached for minutes,
    /// so waiting for a new announcement would leave a returning device unsynced that long.
    ///
    /// Side effects: May schedule dials to the addresses mDNS or the configuration supplied; a
    /// peer with no known address fails to dial quietly. The P2P task calls it periodically.
    pub fn redial_paired_peers(&mut self) {
        self.maintain_relay_route();
        let static_peer_ids: Vec<PeerId> = self.static_peer_ids.iter().copied().collect();
        for peer_id in static_peer_ids {
            self.record_discovered_peer(peer_id);
        }
        let paired_peer_ids: Vec<PeerId> = self
            .swarm
            .behaviour()
            .peer_gate
            .trusted_peers()
            .copied()
            .collect();
        for peer_id in paired_peer_ids {
            self.record_discovered_peer(peer_id);
        }
    }

    /// Allows `peer_device_id` to connect and sync, as after a confirmed pairing.
    ///
    /// Inputs: A peer ID in the adapter's portable form. Outputs: Success, or `SecurityPolicy`
    /// for an ID that is not a libp2p peer ID.
    /// Side effects: Updates the connection gate. A peer already connected (for pairing) is
    /// reported as `PeerDiscovered` at once; otherwise a later mDNS discovery dials it.
    pub fn trust_peer(&mut self, peer_device_id: &DeviceId) -> Result<(), NetworkError> {
        let peer_id = Self::peer_id_from_device_id(peer_device_id)?;
        self.swarm.behaviour_mut().peer_gate.trust(peer_id);
        self.nearby_unpaired_peer_ids.remove(peer_device_id);
        self.record_connected_peer(peer_id);
        Ok(())
    }

    /// Stops trusting `peer_device_id` and closes its connections.
    ///
    /// Inputs: A peer ID in portable form. Outputs: Success, or `SecurityPolicy` for a bad ID.
    /// Side effects: Updates the gate, queues a disconnect, and removes it from reachable peers.
    pub fn revoke_peer(&mut self, peer_device_id: &DeviceId) -> Result<(), NetworkError> {
        let peer_id = Self::peer_id_from_device_id(peer_device_id)?;
        self.swarm.behaviour_mut().peer_gate.revoke(&peer_id);
        if self.reachable_peer_device_ids.remove(peer_device_id) {
            self.pending_network_events
                .push_back(NetworkEvent::PeerExpired(peer_device_id.clone()));
        }
        Ok(())
    }

    /// Admits inbound connections from unpaired peers for `duration`, for pairing only.
    ///
    /// Side effects: Updates the gate. Payloads from such peers are still dropped.
    pub fn open_pairing_window(&mut self, duration: std::time::Duration) {
        self.swarm
            .behaviour_mut()
            .peer_gate
            .open_pairing_window(duration);
    }

    /// Allows this node to connect to the unpaired `peer_device_id` to pair with it.
    ///
    /// Inputs: A peer ID in portable form. Outputs: Success, or `SecurityPolicy` for a bad ID.
    /// Side effects: Updates the gate.
    pub fn allow_pairing_dial(&mut self, peer_device_id: &DeviceId) -> Result<(), NetworkError> {
        let peer_id = Self::peer_id_from_device_id(peer_device_id)?;
        self.swarm
            .behaviour_mut()
            .peer_gate
            .allow_pairing_dial(peer_id);
        Ok(())
    }

    /// Ends pairing: closes the window and disconnects every peer that is still unpaired.
    ///
    /// Side effects: Updates the gate and queues disconnects.
    pub fn end_pairing(&mut self) {
        self.swarm.behaviour_mut().peer_gate.end_pairing();
    }

    /// Lists unpaired peers currently advertising on the LAN, sorted by peer ID.
    ///
    /// Outputs: Candidate peer IDs for pairing. Side effects: None.
    pub fn nearby_unpaired_peers(&self) -> Vec<DeviceId> {
        let mut nearby_peers: Vec<DeviceId> =
            self.nearby_unpaired_peer_ids.iter().cloned().collect();
        nearby_peers.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        nearby_peers
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
        match swarm_event {
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                self.record_connected_peer(peer_id);
                if self
                    .relay
                    .as_ref()
                    .is_some_and(|relay| relay.peer_id == peer_id)
                {
                    self.maintain_relay_route();
                }
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                num_established: 0,
                ..
            } => {
                self.record_disconnected_peer(peer_id);
                if self
                    .relay
                    .as_ref()
                    .is_some_and(|relay| relay.peer_id == peer_id)
                {
                    self.forget_relay_reservation(None);
                }
            }
            SwarmEvent::ListenerClosed { listener_id, .. } => {
                self.forget_relay_reservation(Some(listener_id));
            }
            SwarmEvent::Behaviour(behaviour_event) => match behaviour_event {
                NoteTakerBehaviourEvent::Mdns(mdns_event) => self.translate_mdns_event(mdns_event),
                NoteTakerBehaviourEvent::SyncPayload(sync_payload_event) => {
                    self.translate_sync_payload_event(sync_payload_event);
                }
                NoteTakerBehaviourEvent::FileTransfer(file_transfer_event) => {
                    self.translate_file_transfer_event(file_transfer_event);
                }
                NoteTakerBehaviourEvent::Pairing(pairing_event) => {
                    self.translate_pairing_event(pairing_event);
                }
                NoteTakerBehaviourEvent::RelayServer(relay_event) => {
                    self.translate_relay_server_event(*relay_event);
                }
                NoteTakerBehaviourEvent::RelayClient(
                    relay::client::Event::ReservationReqAccepted { .. },
                ) => {
                    if let Some(relay) = self.relay.as_mut() {
                        if !relay.reservation_active {
                            tracing::info!("the relay accepted this device's reservation");
                        }
                        relay.reservation_active = true;
                    }
                }
                NoteTakerBehaviourEvent::HolePunching(hole_punching_event) => {
                    if hole_punching_event.result.is_ok() {
                        tracing::info!("a relayed connection became direct");
                    }
                }
                NoteTakerBehaviourEvent::RelayClient(_) | NoteTakerBehaviourEvent::Identify => {}
            },
            _ => {}
        }
    }

    /// Creates the private request-response behaviour used for application sync bytes.
    ///
    /// Inputs: None.
    /// Outputs: A behaviour that negotiates the current versioned sync-payload protocol.
    /// Side effects: Allocates in-memory libp2p behaviour state only.
    fn sync_payload_behaviour() -> request_response::Behaviour<SyncPayloadCodec> {
        request_response::Behaviour::new(
            [(SyncPayloadProtocol, request_response::ProtocolSupport::Full)],
            request_response::Config::default(),
        )
    }

    /// Converts an inbound private protocol request to an adapter-neutral application event.
    ///
    /// Inputs: `sync_payload_event` originated from the adapter's request-response behaviour.
    /// Outputs: None.
    /// Side effects: Queues one `MessageReceived` event and acknowledges inbound requests.
    fn translate_sync_payload_event(
        &mut self,
        sync_payload_event: request_response::Event<Vec<u8>, Vec<u8>>,
    ) {
        if let request_response::Event::OutboundFailure { peer, .. } = &sync_payload_event {
            if self.swarm.behaviour().peer_gate.is_trusted(peer) {
                tracing::debug!("a sync payload was not acknowledged by a paired peer");
                self.pending_network_events
                    .push_back(NetworkEvent::DeliveryFailed(Self::device_id_from_peer_id(
                        *peer,
                    )));
            }
            return;
        }
        if let request_response::Event::Message {
            peer,
            message:
                request_response::Message::Request {
                    request, channel, ..
                },
            ..
        } = sync_payload_event
        {
            if !self.swarm.behaviour().peer_gate.is_trusted(&peer) {
                tracing::warn!("dropped a sync payload from an unpaired peer");
                return;
            }
            self.pending_network_events
                .push_back(NetworkEvent::MessageReceived {
                    peer_device_id: Self::device_id_from_peer_id(peer),
                    payload: request,
                });
            let _response_result = self
                .swarm
                .behaviour_mut()
                .sync_payloads
                .send_response(channel, Vec::new());
        }
    }

    /// Handles an attachment-only frame without injecting it into the CRDT payload event queue.
    ///
    /// Inputs: `file_transfer_event` is emitted only by `/note-taker/file-transfer/1`.
    /// Outputs: None.
    /// Side effects: queues its opaque adapter-private bytes and acknowledges the substream.
    fn translate_file_transfer_event(
        &mut self,
        file_transfer_event: request_response::Event<
            FileTransferRequestPayload,
            FileTransferResponsePayload,
        >,
    ) {
        if let request_response::Event::Message {
            peer,
            message:
                request_response::Message::Request {
                    request, channel, ..
                },
            ..
        } = file_transfer_event
        {
            if !self.swarm.behaviour().peer_gate.is_trusted(&peer) {
                tracing::warn!("dropped a file-transfer frame from an unpaired peer");
                return;
            }
            self.pending_file_transfer_payloads.push_back(request.0);
            let _ = self
                .swarm
                .behaviour_mut()
                .file_transfers
                .send_response(channel, FileTransferResponsePayload(Vec::new()));
        }
    }

    /// Turns pairing-protocol traffic into `PairingEvent`s.
    ///
    /// Inputs: An event from the pairing behaviour.
    /// Outputs: None.
    /// Side effects: A request with a valid name is held until the owner answers. A request with an
    /// invalid name is rejected at once. Answers and failures are queued as events.
    fn translate_pairing_event(
        &mut self,
        pairing_event: request_response::Event<PairingRequest, PairingResponse>,
    ) {
        match pairing_event {
            request_response::Event::Message {
                peer,
                message:
                    request_response::Message::Request {
                        request, channel, ..
                    },
                ..
            } => match DeviceName::parse(&request.device_name) {
                Ok(device_name) => {
                    self.pending_pairing_answers.insert(peer, channel);
                    self.pending_pairing_events
                        .push_back(PairingEvent::Requested {
                            peer_id: Self::device_id_from_peer_id(peer),
                            device_name,
                        });
                }
                Err(_) => {
                    tracing::warn!("rejected a pairing request with an invalid device name");
                    let _ = self
                        .swarm
                        .behaviour_mut()
                        .pairing
                        .send_response(channel, PairingResponse::new(None));
                }
            },
            request_response::Event::Message {
                peer,
                message: request_response::Message::Response { response, .. },
                ..
            } => {
                let accepted_as = response
                    .accepted_as
                    .as_deref()
                    .and_then(|name| DeviceName::parse(name).ok());
                self.pending_pairing_events
                    .push_back(PairingEvent::Answered {
                        peer_id: Self::device_id_from_peer_id(peer),
                        accepted_as,
                    });
            }
            request_response::Event::OutboundFailure { peer, .. } => {
                self.pending_pairing_events.push_back(PairingEvent::Failed {
                    peer_id: Self::device_id_from_peer_id(peer),
                });
            }
            _ => {}
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
    /// Side effects: Lists an unpaired peer as nearby, or dials a paired one that is not
    /// connected. A paired peer is reported to the application once its connection is up (see
    /// `record_connected_peer`), so its first sync message never races the dial.
    fn record_discovered_peer(&mut self, peer_id: PeerId) {
        if !self.swarm.behaviour().peer_gate.is_trusted(&peer_id) {
            self.nearby_unpaired_peer_ids
                .insert(Self::device_id_from_peer_id(peer_id));
            return;
        }
        if !self.swarm.is_connected(&peer_id) && self.swarm.dial(peer_id).is_err() {
            tracing::debug!("could not dial a discovered paired peer");
        }
    }

    /// Reports a paired peer as available when it has an open connection.
    ///
    /// Inputs: `peer_id` just connected or just became trusted.
    /// Outputs: None.
    /// Side effects: Adds it to the reachable peers and queues `PeerDiscovered` the first time.
    fn record_connected_peer(&mut self, peer_id: PeerId) {
        if !self.swarm.behaviour().peer_gate.is_trusted(&peer_id)
            || !self.swarm.is_connected(&peer_id)
        {
            return;
        }
        let peer_device_id = Self::device_id_from_peer_id(peer_id);
        if self
            .reachable_peer_device_ids
            .insert(peer_device_id.clone())
        {
            self.pending_network_events
                .push_back(NetworkEvent::PeerDiscovered(peer_device_id));
        }
    }

    /// Reports a peer as gone when its last connection closed.
    ///
    /// Side effects: Removes it from the reachable peers and queues `PeerExpired` when it was
    /// reachable.
    fn record_disconnected_peer(&mut self, peer_id: PeerId) {
        let peer_device_id = Self::device_id_from_peer_id(peer_id);
        if self.reachable_peer_device_ids.remove(&peer_device_id) {
            self.pending_network_events
                .push_back(NetworkEvent::PeerExpired(peer_device_id));
        }
    }

    /// Records a peer expiry when mDNS no longer advertises the peer.
    ///
    /// Inputs: `peer_id` is the libp2p identity whose discovery entry expired.
    /// Outputs: None.
    /// Side effects: Removes it from the nearby unpaired peers. Reachability of paired peers
    /// follows their connections instead, so an open connection outlives a stale announcement.
    fn record_expired_peer(&mut self, peer_id: PeerId) {
        self.nearby_unpaired_peer_ids
            .remove(&Self::device_id_from_peer_id(peer_id));
    }

    /// Converts a libp2p peer identity into the opaque device identifier used by the port.
    ///
    /// Inputs: `peer_id` is an authenticated libp2p public-key-derived identity.
    /// Outputs: A portable device identifier with no libp2p type leakage.
    /// Side effects: None.
    fn device_id_from_peer_id(peer_id: PeerId) -> DeviceId {
        DeviceId::new(peer_id.to_string())
    }

    /// Decodes the opaque application device identifier back into the authenticated libp2p peer ID.
    ///
    /// Inputs: `peer_device_id` must originate from this adapter's peer identity translation.
    /// Outputs: The matching libp2p peer ID or a portable policy error for an invalid identifier.
    /// Side effects: None.
    fn peer_id_from_device_id(peer_device_id: &DeviceId) -> Result<PeerId, NetworkError> {
        peer_device_id
            .as_str()
            .parse()
            .map_err(|_| NetworkError::SecurityPolicy {
                reason: "device identifier is not a libp2p peer identity".to_owned(),
            })
    }

    /// Constructs an adapter whose authenticated transport is confined to libp2p memory addresses.
    ///
    /// Inputs: None; this test-only constructor generates an ephemeral identity.
    /// Outputs: An adapter that can listen and dial only through `MemoryTransport`.
    /// Side effects: Allocates an in-memory encrypted transport and no operating-system sockets.
    #[cfg(test)]
    pub(crate) fn new_for_memory_transport() -> Result<Self, NetworkError> {
        Self::new_for_memory_transport_with(NodeIdentity::ephemeral())
    }

    /// Like `new_for_memory_transport`, with a given identity.
    #[cfg(test)]
    fn new_for_memory_transport_with(node_identity: NodeIdentity) -> Result<Self, NetworkError> {
        Self::new_for_memory_transport_with_idle_timeout(
            node_identity,
            DEFAULT_IDLE_CONNECTION_TIMEOUT,
        )
    }

    /// Like `new_for_memory_transport_with`, closing connections that have nothing to keep them
    /// open after `idle_connection_timeout`, so tests need not wait the production timeout.
    #[cfg(test)]
    fn new_for_memory_transport_with_idle_timeout(
        node_identity: NodeIdentity,
        idle_connection_timeout: std::time::Duration,
    ) -> Result<Self, NetworkError> {
        use libp2p::{
            Transport,
            core::{transport::MemoryTransport, upgrade},
            swarm::Config,
        };

        let local_identity_keypair = node_identity.0;
        let local_peer_id = local_identity_keypair.public().to_peer_id();
        let noise_configuration = noise::Config::new(&local_identity_keypair).map_err(|error| {
            NetworkError::SecurityPolicy {
                reason: format!("cannot configure in-memory Noise transport: {error}"),
            }
        })?;
        let encrypted_memory_transport = MemoryTransport::default()
            .upgrade(upgrade::Version::V1Lazy)
            .authenticate(noise_configuration)
            .multiplex(yamux::Config::default())
            .boxed();
        let swarm = Swarm::new(
            encrypted_memory_transport,
            NoteTakerBehaviour {
                peer_gate: PeerGate::default(),
                mdns: Toggle::from(None),
                sync_payloads: Self::sync_payload_behaviour(),
                file_transfers: file_transfer_behaviour::new_file_transfer_behaviour(),
                pairing: new_pairing_behaviour(),
                relay_server: Toggle::from(None),
                relay_client: Toggle::from(None),
                hole_punching: Toggle::from(None),
                identify: Toggle::from(None),
            },
            local_peer_id,
            Config::with_tokio_executor().with_idle_connection_timeout(idle_connection_timeout),
        );
        Ok(Self::around_swarm(swarm))
    }

    /// Starts the test-only swarm on a libp2p memory address.
    ///
    /// Inputs: `memory_address` must be a valid unoccupied `/memory/<port>` multiaddress.
    /// Outputs: Success when the swarm begins listening, or a portable configuration error.
    /// Side effects: Registers an in-process memory listener and no operating-system socket.
    #[cfg(test)]
    pub(crate) fn listen_on_memory(
        &mut self,
        memory_address: libp2p::Multiaddr,
    ) -> Result<(), NetworkError> {
        self.swarm
            .listen_on(memory_address)
            .map(|_| ())
            .map_err(|error| NetworkError::SecurityPolicy {
                reason: format!("cannot listen on in-memory transport: {error}"),
            })
    }

    /// Dials a test-only libp2p memory listener.
    ///
    /// Inputs: `memory_address` identifies a listener created by `listen_on_memory`.
    /// Outputs: Success after the dial is scheduled, or a portable configuration error.
    /// Side effects: Schedules an in-process encrypted libp2p connection attempt.
    #[cfg(test)]
    pub(crate) fn dial_memory(
        &mut self,
        memory_address: libp2p::Multiaddr,
    ) -> Result<(), NetworkError> {
        self.swarm
            .dial(memory_address)
            .map_err(|error| NetworkError::SecurityPolicy {
                reason: format!("cannot dial in-memory transport: {error}"),
            })
    }

    /// Gives the swarm an address for `peer_device_id` without configuring it as a static peer,
    /// standing in for the address cache mDNS keeps on a real network.
    ///
    /// Side effects: Records the address for later dials only.
    #[cfg(test)]
    pub(crate) fn learn_peer_address(
        &mut self,
        peer_device_id: &DeviceId,
        address: libp2p::Multiaddr,
    ) -> Result<(), NetworkError> {
        let peer_id = Self::peer_id_from_device_id(peer_device_id)?;
        self.swarm.add_peer_address(peer_id, address);
        Ok(())
    }

    /// Returns this adapter's translated local peer identity for adapter-local integration tests.
    ///
    /// Inputs: None.
    /// Outputs: The portable representation of the adapter's generated libp2p peer identity.
    /// Side effects: None.
    #[cfg(test)]
    pub(crate) fn local_device_id(&self) -> DeviceId {
        Self::device_id_from_peer_id(*self.swarm.local_peer_id())
    }

    /// Reports whether this adapter currently has an authenticated connection to a test peer.
    ///
    /// Inputs: `peer_device_id` uses the adapter's peer-identity translation.
    /// Outputs: `true` only while the corresponding libp2p peer is connected.
    /// Side effects: None.
    #[cfg(test)]
    pub(crate) fn is_connected_to(&self, peer_device_id: &DeviceId) -> bool {
        Self::peer_id_from_device_id(peer_device_id)
            .is_ok_and(|peer_id| self.swarm.is_connected(&peer_id))
    }

    /// Queues an adapter-private attachment frame on the dedicated protocol for integration testing.
    ///
    /// Inputs: `peer_device_id` is connected and `payload` is a bounded already-framed attachment message.
    /// Outputs: Success after queuing, or a portable disconnected/policy error.
    /// Side effects: Opens an encrypted file-transfer substream, never the CRDT sync behaviour.
    #[cfg(test)]
    fn send_file_transfer_frame(
        &mut self,
        peer_device_id: &DeviceId,
        payload: Vec<u8>,
    ) -> Result<(), NetworkError> {
        let peer_id = Self::peer_id_from_device_id(peer_device_id)?;
        if !self.swarm.is_connected(&peer_id) {
            return Err(NetworkError::Disconnected);
        }
        self.swarm
            .behaviour_mut()
            .file_transfers
            .send_request(&peer_id, FileTransferRequestPayload(payload));
        Ok(())
    }

    /// Removes one adapter-private attachment frame observed by the test swarm driver.
    ///
    /// Inputs: None.
    /// Outputs: The next attachment frame, if available.
    /// Side effects: Removes it from the attachment-only queue.
    #[cfg(test)]
    fn poll_file_transfer_frame(&mut self) -> Option<Vec<u8>> {
        self.pending_file_transfer_payloads.pop_front()
    }
}

impl P2pNetwork for Libp2pNetworkAdapter {
    /// Queues an opaque application payload on the encrypted libp2p sync-payload protocol.
    ///
    /// Inputs: `peer_device_id` identifies the target and `payload` is opaque application data.
    /// Outputs: Success when the authenticated peer is connected, or a portable network error.
    /// Side effects: Queues payload bytes for encrypted transmission by the owned libp2p swarm.
    fn send_encrypted(
        &mut self,
        peer_device_id: &DeviceId,
        payload: &[u8],
    ) -> Result<(), NetworkError> {
        let peer_id = Self::peer_id_from_device_id(peer_device_id)?;
        if !self.swarm.behaviour().peer_gate.is_trusted(&peer_id) {
            return Err(NetworkError::SecurityPolicy {
                reason: "sync payloads are only sent to paired peers".to_owned(),
            });
        }
        if !self.swarm.is_connected(&peer_id) {
            return Err(NetworkError::Disconnected);
        }
        self.swarm
            .behaviour_mut()
            .sync_payloads
            .send_request(&peer_id, payload.to_vec());
        Ok(())
    }

    /// Queues an opaque application payload for every connected encrypted peer.
    ///
    /// Inputs: `payload` contains opaque application bytes.
    /// Outputs: Success after the payload is queued for every current peer.
    /// Side effects: Queues payload bytes for encrypted transmission by the owned libp2p swarm.
    fn broadcast_encrypted(&mut self, payload: &[u8]) -> Result<(), NetworkError> {
        let gate = &self.swarm.behaviour().peer_gate;
        let connected_peer_ids: Vec<_> = self
            .swarm
            .connected_peers()
            .filter(|peer_id| gate.is_trusted(peer_id))
            .copied()
            .collect();
        for connected_peer_id in connected_peer_ids {
            self.swarm
                .behaviour_mut()
                .sync_payloads
                .send_request(&connected_peer_id, payload.to_vec());
        }
        Ok(())
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

impl PairingTransport for Libp2pNetworkAdapter {
    fn local_peer_id(&self) -> DeviceId {
        Self::device_id_from_peer_id(*self.swarm.local_peer_id())
    }

    fn open_pairing_window(&mut self, duration: std::time::Duration) {
        Libp2pNetworkAdapter::open_pairing_window(self, duration);
    }

    fn end_pairing(&mut self) {
        self.pending_pairing_answers.clear();
        Libp2pNetworkAdapter::end_pairing(self);
    }

    fn nearby_unpaired_peers(&self) -> Vec<DeviceId> {
        Libp2pNetworkAdapter::nearby_unpaired_peers(self)
    }

    /// Allows the dial and sends the request; libp2p dials using addresses mDNS provided.
    fn request_pairing(
        &mut self,
        peer_id: &DeviceId,
        local_name: &DeviceName,
    ) -> Result<(), NetworkError> {
        let libp2p_peer_id = Self::peer_id_from_device_id(peer_id)?;
        self.allow_pairing_dial(peer_id)?;
        self.swarm
            .behaviour_mut()
            .pairing
            .send_request(&libp2p_peer_id, PairingRequest::new(local_name.as_str()));
        Ok(())
    }

    fn answer_pairing(
        &mut self,
        peer_id: &DeviceId,
        accept_as: Option<&DeviceName>,
    ) -> Result<(), NetworkError> {
        let libp2p_peer_id = Self::peer_id_from_device_id(peer_id)?;
        let channel = self
            .pending_pairing_answers
            .remove(&libp2p_peer_id)
            .ok_or_else(|| NetworkError::SecurityPolicy {
                reason: "no pairing request from this peer is pending".to_owned(),
            })?;
        self.swarm
            .behaviour_mut()
            .pairing
            .send_response(
                channel,
                PairingResponse::new(accept_as.map(DeviceName::as_str)),
            )
            .map_err(|_| NetworkError::Disconnected)
    }

    fn trust_peer(&mut self, peer_id: &DeviceId) -> Result<(), NetworkError> {
        Libp2pNetworkAdapter::trust_peer(self, peer_id)
    }

    fn revoke_peer(&mut self, peer_id: &DeviceId) -> Result<(), NetworkError> {
        Libp2pNetworkAdapter::revoke_peer(self, peer_id)
    }

    fn poll_pairing_event(&mut self) -> Option<PairingEvent> {
        self.pending_pairing_events.pop_front()
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

#[cfg(test)]
#[path = "libp2p_adapter_relay_tests.rs"]
mod relay_tests;

#[cfg(test)]
mod tests {
    //! Verifies the libp2p payload boundary using only in-memory encrypted transports.

    use std::time::Duration;

    use libp2p::Multiaddr;

    use super::{Libp2pNetworkAdapter, NodeIdentity};
    use crate::{
        adapters::storage::sqlite_peer_store::SqlitePeerStore,
        application::crypto::voice_encryptor::ephemeral_cipher,
        domain::{errors::StorageError, peer_trust::DeviceName},
        ports::{
            network::{NetworkEvent, P2pNetwork},
            peer_trust::{NodeIdentityStore, PairingEvent, PairingTransport},
        },
    };

    /// Makes two adapters trust each other, as a completed pairing would.
    fn trust_each_other(first: &mut Libp2pNetworkAdapter, second: &mut Libp2pNetworkAdapter) {
        let first_id = first.local_device_id();
        let second_id = second.local_device_id();
        first
            .trust_peer(&second_id)
            .unwrap_or_else(|error| panic!("peer must be trusted: {error}"));
        second
            .trust_peer(&first_id)
            .unwrap_or_else(|error| panic!("peer must be trusted: {error}"));
    }

    /// Drives both swarms until `condition` holds or `deadline` passes, returning whether it held.
    async fn drive_until(
        first: &mut Libp2pNetworkAdapter,
        second: &mut Libp2pNetworkAdapter,
        deadline: Duration,
        mut condition: impl FnMut(&mut Libp2pNetworkAdapter, &mut Libp2pNetworkAdapter) -> bool,
    ) -> bool {
        tokio::time::timeout(deadline, async {
            while !condition(first, second) {
                tokio::select! {
                    drive_result = first.drive_next_swarm_event() => {
                        drive_result.unwrap_or_else(|error| panic!("swarm must stay available: {error}"));
                    }
                    drive_result = second.drive_next_swarm_event() => {
                        drive_result.unwrap_or_else(|error| panic!("swarm must stay available: {error}"));
                    }
                }
            }
        })
        .await
        .is_ok()
    }

    /// Creates two memory adapters with `second` listening on `memory_address`.
    fn listening_pair(
        memory_address: &str,
    ) -> (Libp2pNetworkAdapter, Libp2pNetworkAdapter, Multiaddr) {
        let first = Libp2pNetworkAdapter::new_for_memory_transport()
            .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
        let mut second = Libp2pNetworkAdapter::new_for_memory_transport()
            .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
        let address: Multiaddr = memory_address
            .parse()
            .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
        second
            .listen_on_memory(address.clone())
            .unwrap_or_else(|error| panic!("adapter must listen: {error}"));
        (first, second, address)
    }

    /// Without pairing, a dial to an unknown node is refused: no connection, no payload.
    #[tokio::test]
    async fn untrusted_peers_cannot_connect() {
        let (mut dialer, mut listener, address) = listening_pair("/memory/9311");
        let listener_id = listener.local_device_id();
        let dialer_id = dialer.local_device_id();
        dialer
            .allow_pairing_dial(&listener_id)
            .unwrap_or_else(|error| panic!("pairing dial must be allowed: {error}"));
        dialer
            .dial_memory(address)
            .unwrap_or_else(|error| panic!("dial must be scheduled: {error}"));

        // The dialer's side finishes the handshake before the listener refuses, so only the
        // listener's view proves the refusal.
        let admitted = drive_until(
            &mut dialer,
            &mut listener,
            Duration::from_secs(1),
            |_, listener| listener.is_connected_to(&dialer_id),
        )
        .await;
        let dialer_dropped = drive_until(
            &mut dialer,
            &mut listener,
            Duration::from_secs(5),
            |dialer, _| !dialer.is_connected_to(&listener_id),
        )
        .await;

        assert!(
            !admitted,
            "a node without an open pairing window must refuse unknown peers"
        );
        assert!(
            dialer_dropped,
            "the refused connection must close on the dialer's side too"
        );
        assert!(dialer.send_encrypted(&listener_id, b"sync").is_err());
    }

    /// An open pairing window admits an unknown peer, but only for pairing: its sync payloads
    /// are dropped, and this node refuses to send sync payloads to it.
    #[tokio::test]
    async fn a_pairing_window_connection_cannot_exchange_sync_payloads() {
        let (mut dialer, mut listener, address) = listening_pair("/memory/9312");
        let listener_id = listener.local_device_id();
        let dialer_id = dialer.local_device_id();
        // The dialer already trusts the listener; the listener has not paired yet.
        dialer
            .trust_peer(&listener_id)
            .unwrap_or_else(|error| panic!("peer must be trusted: {error}"));
        listener.open_pairing_window(Duration::from_secs(60));
        dialer
            .dial_memory(address)
            .unwrap_or_else(|error| panic!("dial must be scheduled: {error}"));
        assert!(
            drive_until(
                &mut dialer,
                &mut listener,
                Duration::from_secs(5),
                |dialer, _| { dialer.is_connected_to(&listener_id) }
            )
            .await,
            "a pairing window must admit the unknown dialer"
        );

        dialer
            .send_encrypted(&listener_id, b"sync-before-pairing")
            .unwrap_or_else(|error| panic!("trusted send must be queued: {error}"));
        let received = drive_until(
            &mut dialer,
            &mut listener,
            Duration::from_secs(1),
            |_, listener| {
                matches!(
                    listener.poll_event(),
                    Ok(Some(NetworkEvent::MessageReceived { .. }))
                )
            },
        )
        .await;

        assert!(
            !received,
            "sync payloads from an unpaired peer must be dropped"
        );
        assert!(listener.send_encrypted(&dialer_id, b"sync").is_err());
    }

    /// Revoking a connected trusted peer disconnects it.
    #[tokio::test]
    async fn revoking_a_peer_disconnects_it() {
        let (mut dialer, mut listener, address) = listening_pair("/memory/9313");
        trust_each_other(&mut dialer, &mut listener);
        let listener_id = listener.local_device_id();
        let dialer_id = dialer.local_device_id();
        dialer
            .dial_memory(address)
            .unwrap_or_else(|error| panic!("dial must be scheduled: {error}"));
        assert!(
            drive_until(
                &mut dialer,
                &mut listener,
                Duration::from_secs(5),
                |dialer, _| { dialer.is_connected_to(&listener_id) }
            )
            .await
        );

        listener
            .revoke_peer(&dialer_id)
            .unwrap_or_else(|error| panic!("peer must be revoked: {error}"));
        let disconnected = drive_until(
            &mut dialer,
            &mut listener,
            Duration::from_secs(5),
            |dialer, _| !dialer.is_connected_to(&listener_id),
        )
        .await;

        assert!(disconnected, "revocation must close the connection");
    }

    /// Returns a validated device name.
    fn device_name(name: &str) -> DeviceName {
        DeviceName::parse(name).unwrap_or_else(|error| panic!("valid name: {error}"))
    }

    /// Connects `initiator` to `responder` (which has a pairing window open) and sends a
    /// pairing request, returning the request event the responder observed.
    async fn deliver_pairing_request(
        initiator: &mut Libp2pNetworkAdapter,
        responder: &mut Libp2pNetworkAdapter,
        address: Multiaddr,
    ) -> PairingEvent {
        let responder_id = responder.local_device_id();
        responder.open_pairing_window(Duration::from_secs(60));
        // In production mDNS supplies the address and libp2p dials on demand; memory transports
        // have no discovery, so connect first.
        initiator
            .allow_pairing_dial(&responder_id)
            .unwrap_or_else(|error| panic!("pairing dial must be allowed: {error}"));
        initiator
            .dial_memory(address)
            .unwrap_or_else(|error| panic!("dial must be scheduled: {error}"));
        assert!(
            drive_until(
                initiator,
                responder,
                Duration::from_secs(5),
                |initiator, _| { initiator.is_connected_to(&responder_id) }
            )
            .await
        );
        PairingTransport::request_pairing(initiator, &responder_id, &device_name("Laptop"))
            .unwrap_or_else(|error| panic!("request must be queued: {error}"));
        let mut observed = None;
        assert!(
            drive_until(
                initiator,
                responder,
                Duration::from_secs(5),
                |_, responder| {
                    observed = responder.poll_pairing_event();
                    observed.is_some()
                }
            )
            .await,
            "the responder must receive the pairing request"
        );
        observed.unwrap_or_else(|| panic!("an event was observed"))
    }

    /// Request, owner acceptance, and answer travel between two unpaired nodes.
    #[tokio::test]
    async fn an_accepted_pairing_request_reaches_both_sides() {
        let (mut initiator, mut responder, address) = listening_pair("/memory/9321");
        let initiator_id = initiator.local_device_id();
        let responder_id = responder.local_device_id();

        let request = deliver_pairing_request(&mut initiator, &mut responder, address).await;
        assert_eq!(
            request,
            PairingEvent::Requested {
                peer_id: initiator_id.clone(),
                device_name: device_name("Laptop"),
            }
        );
        responder
            .answer_pairing(&initiator_id, Some(&device_name("Phone")))
            .unwrap_or_else(|error| panic!("answer must be sent: {error}"));
        let mut answer = None;
        assert!(
            drive_until(
                &mut initiator,
                &mut responder,
                Duration::from_secs(5),
                |initiator, _| {
                    answer = initiator.poll_pairing_event();
                    answer.is_some()
                }
            )
            .await
        );

        assert_eq!(
            answer,
            Some(PairingEvent::Answered {
                peer_id: responder_id,
                accepted_as: Some(device_name("Phone")),
            })
        );
    }

    /// A rejection arrives as an answer without a name, and a second answer is refused.
    #[tokio::test]
    async fn a_rejected_pairing_request_is_reported_and_cannot_be_answered_twice() {
        let (mut initiator, mut responder, address) = listening_pair("/memory/9322");
        let initiator_id = initiator.local_device_id();
        let responder_id = responder.local_device_id();
        deliver_pairing_request(&mut initiator, &mut responder, address).await;

        responder
            .answer_pairing(&initiator_id, None)
            .unwrap_or_else(|error| panic!("rejection must be sent: {error}"));
        let mut answer = None;
        assert!(
            drive_until(
                &mut initiator,
                &mut responder,
                Duration::from_secs(5),
                |initiator, _| {
                    answer = initiator.poll_pairing_event();
                    answer.is_some()
                }
            )
            .await
        );

        assert_eq!(
            answer,
            Some(PairingEvent::Answered {
                peer_id: responder_id,
                accepted_as: None,
            })
        );
        assert!(responder.answer_pairing(&initiator_id, None).is_err());
    }

    /// Full pairing through `PairingManager` on two real swarms: after both owners confirm, each
    /// side has persisted the other, and sync payloads flow in both directions.
    #[tokio::test]
    async fn two_managers_pair_end_to_end_and_then_sync() {
        use crate::application::pairing_manager::{PairingManager, PairingUpdate};

        type Manager = PairingManager<Libp2pNetworkAdapter, SqlitePeerStore>;

        /// Drives both managers' swarms until `condition` holds (5 s limit).
        async fn drive(
            laptop: &mut Manager,
            phone: &mut Manager,
            mut condition: impl FnMut(&mut Manager, &mut Manager) -> bool,
        ) {
            tokio::time::timeout(Duration::from_secs(5), async {
                while !condition(laptop, phone) {
                    tokio::select! {
                        result = laptop.transport_mut().drive_next_swarm_event() => {
                            result.unwrap_or_else(|error| panic!("laptop swarm: {error}"));
                        }
                        result = phone.transport_mut().drive_next_swarm_event() => {
                            result.unwrap_or_else(|error| panic!("phone swarm: {error}"));
                        }
                    }
                }
            })
            .await
            .unwrap_or_else(|_| panic!("condition not reached before timeout"));
        }

        let (laptop_adapter, phone_adapter, phone_address) = listening_pair("/memory/9331");
        let store = || {
            SqlitePeerStore::open_in_memory(ephemeral_cipher())
                .unwrap_or_else(|error| panic!("peer store must open: {error}"))
        };
        let mut laptop: Manager = PairingManager::new(
            laptop_adapter,
            store(),
            device_name("Laptop"),
            Box::new(|| 1),
        );
        let mut phone: Manager =
            PairingManager::new(phone_adapter, store(), device_name("Phone"), Box::new(|| 2));
        let laptop_id = laptop.transport().local_device_id();
        let phone_id = phone.transport().local_device_id();

        // The owner makes the phone pairable; the laptop connects (mDNS would supply the address).
        phone.make_pairable(Duration::from_secs(60));
        laptop
            .transport_mut()
            .allow_pairing_dial(&phone_id)
            .unwrap_or_else(|error| panic!("dial must be allowed: {error}"));
        laptop
            .transport_mut()
            .dial_memory(phone_address)
            .unwrap_or_else(|error| panic!("dial must be scheduled: {error}"));
        drive(&mut laptop, &mut phone, |laptop, _| {
            laptop.transport().is_connected_to(&phone_id)
        })
        .await;
        let laptop_code = laptop
            .start_pairing(&phone_id)
            .unwrap_or_else(|error| panic!("pairing must start: {error}"));

        let mut phone_updates = Vec::new();
        drive(&mut laptop, &mut phone, |_, phone| {
            phone_updates.extend(
                phone
                    .process_events()
                    .unwrap_or_else(|error| panic!("phone events: {error}")),
            );
            !phone_updates.is_empty()
        })
        .await;
        let Some(PairingUpdate::IncomingRequest {
            code: phone_code, ..
        }) = phone_updates.first()
        else {
            panic!("the phone must show the request: {phone_updates:?}");
        };
        assert_eq!(
            phone_code, &laptop_code,
            "both screens must show the same code"
        );

        // The owner sees matching codes and confirms on both devices.
        laptop
            .confirm_code(&phone_id)
            .unwrap_or_else(|error| panic!("laptop confirm: {error}"));
        phone
            .accept_incoming(&laptop_id)
            .unwrap_or_else(|error| panic!("phone accept: {error}"));
        let mut laptop_updates = Vec::new();
        drive(&mut laptop, &mut phone, |laptop, _| {
            laptop_updates.extend(
                laptop
                    .process_events()
                    .unwrap_or_else(|error| panic!("laptop events: {error}")),
            );
            !laptop_updates.is_empty()
        })
        .await;

        assert_eq!(
            laptop_updates,
            vec![PairingUpdate::Paired {
                peer_id: phone_id.clone(),
                device_name: device_name("Phone"),
            }]
        );
        let laptop_trusted = laptop
            .trusted_devices()
            .unwrap_or_else(|error| panic!("laptop allowlist: {error}"));
        let phone_trusted = phone
            .trusted_devices()
            .unwrap_or_else(|error| panic!("phone allowlist: {error}"));
        assert_eq!(laptop_trusted.len(), 1);
        assert_eq!(laptop_trusted[0].peer_id(), &phone_id);
        assert_eq!(phone_trusted.len(), 1);
        assert_eq!(phone_trusted[0].device_name(), &device_name("Laptop"));

        // Paired: a sync payload from the laptop now reaches the phone.
        laptop
            .transport_mut()
            .send_encrypted(&phone_id, b"first-sync")
            .unwrap_or_else(|error| panic!("paired send must be queued: {error}"));
        let mut delivered = false;
        drive(&mut laptop, &mut phone, |_, phone| {
            delivered |= matches!(
                phone.transport_mut().poll_event(),
                Ok(Some(NetworkEvent::MessageReceived { .. }))
            );
            delivered
        })
        .await;
        assert!(delivered);
    }

    /// Loading the identity twice from one store yields the same peer ID, and an adapter built
    /// from it uses that ID, so paired devices recognise this node after a restart.
    #[test]
    fn a_stored_node_identity_keeps_the_peer_id_stable() {
        let store = SqlitePeerStore::open_in_memory(ephemeral_cipher())
            .unwrap_or_else(|error| panic!("peer store must open: {error}"));

        let first_identity = NodeIdentity::load_or_create(&store)
            .unwrap_or_else(|error| panic!("identity must be created: {error}"));
        let second_identity = NodeIdentity::load_or_create(&store)
            .unwrap_or_else(|error| panic!("identity must be loaded: {error}"));
        let first_peer = first_identity.peer_device_id();
        let adapter = Libp2pNetworkAdapter::new_for_memory_transport_with(first_identity)
            .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));

        assert_eq!(first_peer, second_identity.peer_device_id());
        assert_eq!(adapter.local_device_id(), first_peer);
        assert!(!format!("{second_identity:?}").contains(first_peer.as_str()));
    }

    /// A stored value that is not a libp2p keypair is reported, never silently replaced.
    #[test]
    fn an_undecodable_stored_identity_is_an_error() {
        /// Always returns bytes that are not a protobuf-encoded keypair.
        struct CorruptIdentityStore;
        impl NodeIdentityStore for CorruptIdentityStore {
            fn load_or_insert_node_identity(
                &self,
                _candidate: &[u8],
            ) -> Result<zeroize::Zeroizing<Vec<u8>>, StorageError> {
                Ok(zeroize::Zeroizing::new(b"not a keypair".to_vec()))
            }
        }

        assert!(NodeIdentity::load_or_create(&CorruptIdentityStore).is_err());
    }

    /// Proves that one swarm's opaque sync payload reaches a second swarm as a portable event.
    ///
    /// Inputs: Two adapter instances connected exclusively through libp2p's memory transport.
    /// Outputs: Success after swarm B observes the exact payload from swarm A.
    /// Side effects: Drives two in-memory Noise-encrypted swarms without binding TCP or UDP sockets.
    #[tokio::test]
    async fn memory_transport_delivers_sync_payload_as_message_received_event() {
        let mut sending_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
            .unwrap_or_else(|error| panic!("memory adapter A should initialize: {error}"));
        let mut receiving_adapter = Libp2pNetworkAdapter::new_for_memory_transport()
            .unwrap_or_else(|error| panic!("memory adapter B should initialize: {error}"));
        trust_each_other(&mut sending_adapter, &mut receiving_adapter);
        let listener_address: Multiaddr = "/memory/8128"
            .parse()
            .unwrap_or_else(|error| panic!("memory multiaddress should parse: {error}"));
        let expected_payload = b"versioned-sync-message".to_vec();

        receiving_adapter
            .listen_on_memory(listener_address.clone())
            .unwrap_or_else(|error| panic!("adapter B should listen in memory: {error}"));
        sending_adapter
            .dial_memory(listener_address)
            .unwrap_or_else(|error| panic!("adapter A should dial the memory listener: {error}"));

        let receiving_device_id = receiving_adapter.local_device_id();
        tokio::time::timeout(Duration::from_secs(5), async {
            while !sending_adapter.is_connected_to(&receiving_device_id) {
                tokio::select! {
                    drive_result = sending_adapter.drive_next_swarm_event() => {
                        drive_result.unwrap_or_else(|error| panic!("adapter A should establish its memory connection: {error}"));
                    }
                    drive_result = receiving_adapter.drive_next_swarm_event() => {
                        drive_result.unwrap_or_else(|error| panic!("adapter B should accept its memory connection: {error}"));
                    }
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("the in-memory swarms should connect before timeout"));
        sending_adapter
            .send_encrypted(&receiving_device_id, &expected_payload)
            .unwrap_or_else(|error| {
                panic!("adapter A should queue its encrypted payload: {error}")
            });

        let received_event = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                tokio::select! {
                    drive_result = sending_adapter.drive_next_swarm_event() => {
                        drive_result.unwrap_or_else(|error| panic!("adapter A swarm should remain available: {error}"));
                    }
                    drive_result = receiving_adapter.drive_next_swarm_event() => {
                        drive_result.unwrap_or_else(|error| panic!("adapter B swarm should remain available: {error}"));
                        // The connection itself is reported first; the payload follows it.
                        if let Some(network_event) = receiving_adapter.poll_event().unwrap_or_else(|error| panic!("adapter B event queue should be readable: {error}"))
                            && !matches!(network_event, NetworkEvent::PeerDiscovered(_))
                        {
                            break network_event;
                        }
                    }
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("adapter B should receive the payload before timeout"));

        assert_eq!(
            received_event,
            NetworkEvent::MessageReceived {
                peer_device_id: sending_adapter.local_device_id(),
                payload: expected_payload,
            }
        );

        let expected_broadcast_payload = b"broadcast-sync-message".to_vec();
        sending_adapter
            .broadcast_encrypted(&expected_broadcast_payload)
            .unwrap_or_else(|error| {
                panic!("adapter A should queue its encrypted broadcast: {error}")
            });
        let received_broadcast_event = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                tokio::select! {
                    drive_result = sending_adapter.drive_next_swarm_event() => {
                        drive_result.unwrap_or_else(|error| panic!("adapter A swarm should remain available: {error}"));
                    }
                    drive_result = receiving_adapter.drive_next_swarm_event() => {
                        drive_result.unwrap_or_else(|error| panic!("adapter B swarm should remain available: {error}"));
                        // The connection itself is reported first; the payload follows it.
                        if let Some(network_event) = receiving_adapter.poll_event().unwrap_or_else(|error| panic!("adapter B event queue should be readable: {error}"))
                            && !matches!(network_event, NetworkEvent::PeerDiscovered(_))
                        {
                            break network_event;
                        }
                    }
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("adapter B should receive the broadcast before timeout"));

        assert_eq!(
            received_broadcast_event,
            NetworkEvent::MessageReceived {
                peer_device_id: sending_adapter.local_device_id(),
                payload: expected_broadcast_payload,
            }
        );
    }

    /// Proves binary attachment bytes use the independent protocol behaviour over Noise memory transport.
    ///
    /// Inputs: Two adapters connected only through a libp2p memory address.
    /// Outputs: Success after the receiver observes the exact attachment frame outside CRDT events.
    /// Side effects: Drives in-memory encrypted swarms and never binds TCP or UDP sockets.
    #[tokio::test]
    async fn memory_transport_delivers_file_transfer_frame_on_separate_behaviour() {
        let mut sender = Libp2pNetworkAdapter::new_for_memory_transport()
            .unwrap_or_else(|error| panic!("sender should initialize: {error}"));
        let mut receiver = Libp2pNetworkAdapter::new_for_memory_transport()
            .unwrap_or_else(|error| panic!("receiver should initialize: {error}"));
        trust_each_other(&mut sender, &mut receiver);
        let address: Multiaddr = "/memory/8129"
            .parse()
            .unwrap_or_else(|error| panic!("memory address should parse: {error}"));
        receiver
            .listen_on_memory(address.clone())
            .unwrap_or_else(|error| panic!("receiver should listen: {error}"));
        sender
            .dial_memory(address)
            .unwrap_or_else(|error| panic!("sender should dial: {error}"));
        let receiver_device_id = receiver.local_device_id();
        tokio::time::timeout(Duration::from_secs(5), async {
            while !sender.is_connected_to(&receiver_device_id) {
                tokio::select! {
                    result = sender.drive_next_swarm_event() => result.unwrap_or_else(|error| panic!("sender drive failure: {error}")),
                    result = receiver.drive_next_swarm_event() => result.unwrap_or_else(|error| panic!("receiver drive failure: {error}")),
                }
            }
        }).await.unwrap_or_else(|_| panic!("memory swarms should connect"));
        let file_chunk_frame = vec![7_u8; 64 * 1024];
        sender
            .send_file_transfer_frame(&receiver_device_id, file_chunk_frame.clone())
            .unwrap_or_else(|error| panic!("file frame should queue: {error}"));
        let received = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                tokio::select! {
                    result = sender.drive_next_swarm_event() => result.unwrap_or_else(|error| panic!("sender drive failure: {error}")),
                    result = receiver.drive_next_swarm_event() => {
                        result.unwrap_or_else(|error| panic!("receiver drive failure: {error}"));
                        if let Some(frame) = receiver.poll_file_transfer_frame() { break frame; }
                    }
                }
            }
        }).await.unwrap_or_else(|_| panic!("file frame should arrive"));
        assert_eq!(received, file_chunk_frame);
        // Only the connection is reported on the sync queue; the file frame never appears there.
        assert_eq!(
            drain_events(&mut receiver),
            vec![NetworkEvent::PeerDiscovered(sender.local_device_id())]
        );
    }

    /// A TCP listen address that is not a multiaddress is refused without listening.
    #[tokio::test]
    async fn an_invalid_listen_address_is_refused() {
        let mut adapter = Libp2pNetworkAdapter::new(NodeIdentity::ephemeral())
            .unwrap_or_else(|error| panic!("adapter must initialize: {error}"));

        assert!(adapter.listen_on("0.0.0.0:4001").is_err());
        assert!(adapter.listen_addresses().is_empty());
    }

    /// Listening on a loopback TCP address reports the bound address once the swarm has run.
    #[tokio::test]
    async fn a_tcp_listener_reports_its_bound_address() {
        let mut adapter = Libp2pNetworkAdapter::new(NodeIdentity::ephemeral())
            .unwrap_or_else(|error| panic!("adapter must initialize: {error}"));

        adapter
            .listen_on("/ip4/127.0.0.1/tcp/0")
            .unwrap_or_else(|error| panic!("listening must start: {error}"));
        tokio::time::timeout(Duration::from_secs(5), async {
            while adapter.listen_addresses().is_empty() {
                adapter
                    .drive_next_swarm_event()
                    .await
                    .unwrap_or_else(|error| panic!("swarm must run: {error}"));
            }
        })
        .await
        .unwrap_or_else(|_| panic!("no listen address before timeout"));

        let addresses = adapter.listen_addresses();
        assert!(
            addresses[0].starts_with("/ip4/127.0.0.1/tcp/"),
            "unexpected address {addresses:?}"
        );
    }

    /// Collects every queued port event of `adapter`.
    fn drain_events(adapter: &mut Libp2pNetworkAdapter) -> Vec<NetworkEvent> {
        let mut events = Vec::new();
        while let Some(event) = adapter
            .poll_event()
            .unwrap_or_else(|error| panic!("event queue should be readable: {error}"))
        {
            events.push(event);
        }
        events
    }

    /// A paired peer is reported only once its connection is up, so the first sync message can be
    /// sent at once; it is reported gone when its last connection closes.
    #[tokio::test]
    async fn paired_peers_are_reported_when_connected_and_when_disconnected() {
        let (mut dialer, mut listener, address) = listening_pair("/memory/9361");
        trust_each_other(&mut dialer, &mut listener);
        let dialer_id = dialer.local_device_id();
        let listener_id = listener.local_device_id();
        dialer
            .dial_memory(address)
            .unwrap_or_else(|error| panic!("dial must be scheduled: {error}"));

        let mut dialer_events = Vec::new();
        let mut listener_events = Vec::new();
        let both_reported = drive_until(
            &mut dialer,
            &mut listener,
            Duration::from_secs(5),
            |dialer, listener| {
                dialer_events.extend(drain_events(dialer));
                listener_events.extend(drain_events(listener));
                !dialer_events.is_empty() && !listener_events.is_empty()
            },
        )
        .await;
        assert!(both_reported, "both sides must report the connection");
        assert_eq!(
            dialer_events,
            vec![NetworkEvent::PeerDiscovered(listener_id.clone())]
        );
        assert_eq!(
            listener_events,
            vec![NetworkEvent::PeerDiscovered(dialer_id.clone())]
        );
        assert!(
            dialer.send_encrypted(&listener_id, b"first").is_ok(),
            "a reported peer must accept a send immediately"
        );

        dialer
            .revoke_peer(&listener_id)
            .unwrap_or_else(|error| panic!("revocation must succeed: {error}"));
        let mut listener_after = Vec::new();
        let expired = drive_until(
            &mut dialer,
            &mut listener,
            Duration::from_secs(5),
            |_, listener| {
                listener_after.extend(drain_events(listener));
                listener_after.contains(&NetworkEvent::PeerExpired(dialer_id.clone()))
            },
        )
        .await;
        assert!(
            expired,
            "the listener must report the closed connection: {listener_after:?}"
        );
    }

    /// Trusting a peer that is already connected for pairing reports it at once, so sync starts
    /// without waiting for another discovery.
    #[tokio::test]
    async fn trusting_a_connected_peer_reports_it() {
        let (mut dialer, mut listener, address) = listening_pair("/memory/9362");
        let listener_id = listener.local_device_id();
        listener.open_pairing_window(Duration::from_secs(60));
        dialer
            .allow_pairing_dial(&listener_id)
            .and_then(|()| dialer.dial_memory(address))
            .unwrap_or_else(|error| panic!("dial must be scheduled: {error}"));
        let connected = drive_until(
            &mut dialer,
            &mut listener,
            Duration::from_secs(5),
            |dialer, _| dialer.is_connected_to(&listener_id),
        )
        .await;
        assert!(connected);
        assert!(
            drain_events(&mut dialer).is_empty(),
            "an unpaired peer is not reported"
        );

        dialer
            .trust_peer(&listener_id)
            .unwrap_or_else(|error| panic!("peer must be trusted: {error}"));

        assert_eq!(
            drain_events(&mut dialer),
            vec![NetworkEvent::PeerDiscovered(listener_id)]
        );
    }

    /// A sync payload the receiver drops (it does not trust the sender yet) is reported to the
    /// sender as `DeliveryFailed`, so it can send again once trust is mutual.
    #[tokio::test]
    async fn a_dropped_sync_payload_is_reported_to_the_sender() {
        let (mut sender, mut receiver, address) = listening_pair("/memory/9363");
        let receiver_id = receiver.local_device_id();
        receiver.open_pairing_window(Duration::from_secs(60));
        sender
            .trust_peer(&receiver_id)
            .and_then(|()| sender.dial_memory(address))
            .unwrap_or_else(|error| panic!("dial must be scheduled: {error}"));
        let connected = drive_until(
            &mut sender,
            &mut receiver,
            Duration::from_secs(5),
            |sender, _| sender.is_connected_to(&receiver_id),
        )
        .await;
        assert!(connected);
        sender
            .send_encrypted(&receiver_id, b"too-early")
            .unwrap_or_else(|error| panic!("send must be queued: {error}"));

        let mut sender_events = Vec::new();
        let reported = drive_until(
            &mut sender,
            &mut receiver,
            Duration::from_secs(5),
            |sender, _| {
                sender_events.extend(drain_events(sender));
                sender_events.contains(&NetworkEvent::DeliveryFailed(receiver_id.clone()))
            },
        )
        .await;

        assert!(
            reported,
            "the sender must learn of the drop: {sender_events:?}"
        );
        assert!(drain_events(&mut receiver).is_empty());
    }

    /// A configured peer address (for networks without mDNS) lists an unpaired peer as nearby,
    /// rejects an address without a peer ID, and is dialed again once the peer is paired.
    #[tokio::test]
    async fn configured_peer_addresses_are_offered_for_pairing_and_redialed_once_paired() {
        let mut dialer = Libp2pNetworkAdapter::new_for_memory_transport()
            .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
        let mut listener = Libp2pNetworkAdapter::new_for_memory_transport()
            .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"));
        let listener_id = listener.local_device_id();
        listener
            .listen_on_memory(
                "/memory/9381"
                    .parse()
                    .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}")),
            )
            .unwrap_or_else(|error| panic!("listener must listen: {error}"));

        assert!(dialer.add_static_peer("/memory/9381").is_err());
        let added = dialer
            .add_static_peer(&format!("/memory/9381/p2p/{}", listener_id.as_str()))
            .unwrap_or_else(|error| panic!("a full peer address must be accepted: {error}"));
        assert_eq!(added, listener_id);
        assert_eq!(dialer.nearby_unpaired_peers(), vec![listener_id.clone()]);

        dialer
            .trust_peer(&listener_id)
            .unwrap_or_else(|error| panic!("peer must be trusted: {error}"));
        listener
            .trust_peer(&dialer.local_device_id())
            .unwrap_or_else(|error| panic!("peer must be trusted: {error}"));
        dialer.redial_paired_peers();
        let connected = drive_until(
            &mut dialer,
            &mut listener,
            Duration::from_secs(5),
            |dialer, _| dialer.is_connected_to(&listener_id),
        )
        .await;

        assert!(connected, "a paired configured peer must be dialed");
        assert_eq!(
            drain_events(&mut dialer),
            vec![NetworkEvent::PeerDiscovered(listener_id)]
        );
    }

    /// Creates a memory adapter that closes idle connections after `SHORT_IDLE_TIMEOUT`.
    fn short_idle_adapter() -> Libp2pNetworkAdapter {
        Libp2pNetworkAdapter::new_for_memory_transport_with_idle_timeout(
            NodeIdentity::ephemeral(),
            SHORT_IDLE_TIMEOUT,
        )
        .unwrap_or_else(|error| panic!("memory adapter must initialize: {error}"))
    }

    /// An idle timeout short enough to observe in a test.
    const SHORT_IDLE_TIMEOUT: Duration = Duration::from_millis(200);

    /// Drives both swarms for `duration`, so idle timers can fire.
    async fn drive_for(
        first: &mut Libp2pNetworkAdapter,
        second: &mut Libp2pNetworkAdapter,
        duration: Duration,
    ) {
        drive_until(first, second, duration, |_, _| false).await;
    }

    /// A connection between paired devices stays open while nothing is being sent, so a change
    /// made later reaches the other device at once; a pairing-only connection still times out.
    #[tokio::test]
    async fn a_paired_connection_stays_open_while_idle() {
        let mut dialer = short_idle_adapter();
        let mut listener = short_idle_adapter();
        let address: Multiaddr = "/memory/9391"
            .parse()
            .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
        listener
            .listen_on_memory(address.clone())
            .unwrap_or_else(|error| panic!("adapter must listen: {error}"));
        let listener_id = listener.local_device_id();
        trust_each_other(&mut dialer, &mut listener);
        dialer
            .dial_memory(address)
            .unwrap_or_else(|error| panic!("dial must be scheduled: {error}"));
        let connected = drive_until(
            &mut dialer,
            &mut listener,
            Duration::from_secs(5),
            |dialer, _| dialer.is_connected_to(&listener_id),
        )
        .await;
        assert!(connected, "paired devices must connect");

        drive_for(&mut dialer, &mut listener, SHORT_IDLE_TIMEOUT * 5).await;

        assert!(
            dialer.is_connected_to(&listener_id),
            "an idle connection between paired devices must stay open"
        );
    }

    /// A pairing connection that becomes trusted stays open, while one that stays unpaired is
    /// closed by the idle timeout.
    #[tokio::test]
    async fn a_pairing_connection_stays_open_only_once_trusted() {
        for (memory_address, trusted) in [("/memory/9392", true), ("/memory/9393", false)] {
            let mut dialer = short_idle_adapter();
            let mut listener = short_idle_adapter();
            let address: Multiaddr = memory_address
                .parse()
                .unwrap_or_else(|error| panic!("memory multiaddress must parse: {error}"));
            listener
                .listen_on_memory(address.clone())
                .unwrap_or_else(|error| panic!("adapter must listen: {error}"));
            let listener_id = listener.local_device_id();
            listener.open_pairing_window(Duration::from_secs(60));
            dialer
                .allow_pairing_dial(&listener_id)
                .and_then(|()| dialer.dial_memory(address))
                .unwrap_or_else(|error| panic!("dial must be scheduled: {error}"));
            let connected = drive_until(
                &mut dialer,
                &mut listener,
                Duration::from_secs(5),
                |dialer, _| dialer.is_connected_to(&listener_id),
            )
            .await;
            assert!(connected, "the pairing dial must connect");
            if trusted {
                trust_each_other(&mut dialer, &mut listener);
            }

            drive_for(&mut dialer, &mut listener, SHORT_IDLE_TIMEOUT * 5).await;

            assert_eq!(
                dialer.is_connected_to(&listener_id),
                trusted,
                "trusted: {trusted}"
            );
        }
    }

    /// A paired device whose connection dropped is dialed again from the address discovery
    /// gave, even though it is not a configured peer and discovery reports nothing new.
    #[tokio::test]
    async fn a_paired_device_that_dropped_is_dialed_again() {
        let (mut dialer, mut listener, address) = listening_pair("/memory/9394");
        let dialer_id = dialer.local_device_id();
        let listener_id = listener.local_device_id();
        trust_each_other(&mut dialer, &mut listener);
        dialer
            .learn_peer_address(&listener_id, address)
            .unwrap_or_else(|error| panic!("address must be recorded: {error}"));
        dialer.redial_paired_peers();
        let connected = drive_until(
            &mut dialer,
            &mut listener,
            Duration::from_secs(5),
            |dialer, _| dialer.is_connected_to(&listener_id),
        )
        .await;
        assert!(
            connected,
            "a paired device with a known address must be dialed"
        );

        listener
            .revoke_peer(&dialer_id)
            .unwrap_or_else(|error| panic!("revocation must succeed: {error}"));
        let dropped = drive_until(
            &mut dialer,
            &mut listener,
            Duration::from_secs(5),
            |dialer, _| !dialer.is_connected_to(&listener_id),
        )
        .await;
        assert!(dropped, "the connection must drop");
        listener
            .trust_peer(&dialer_id)
            .unwrap_or_else(|error| panic!("peer must be trusted again: {error}"));
        dialer.redial_paired_peers();
        let reconnected = drive_until(
            &mut dialer,
            &mut listener,
            Duration::from_secs(5),
            |dialer, _| dialer.is_connected_to(&listener_id),
        )
        .await;

        assert!(reconnected, "the dropped device must be dialed again");
    }
}
