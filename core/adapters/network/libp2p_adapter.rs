//! Builds and drives a Noise-encrypted libp2p swarm while exposing only portable P2P port values.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    io,
};

use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, StreamExt};
use libp2p::{
    PeerId, SwarmBuilder, mdns, noise, request_response,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent, behaviour::toggle::Toggle},
    tcp, yamux,
};

/// Limits one framed application payload to a bounded amount of adapter memory.
const MAX_SYNC_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;

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
    pending_network_events: VecDeque<NetworkEvent>,
    /// Retains adapter-private attachment frames separately from CRDT network events.
    pending_file_transfer_payloads: VecDeque<Vec<u8>>,
    /// Unpaired peers advertising themselves on the LAN, offered as pairing candidates.
    nearby_unpaired_peer_ids: HashSet<DeviceId>,
    /// Pairing requests awaiting the owner's answer, by requesting peer.
    pending_pairing_answers: HashMap<PeerId, request_response::ResponseChannel<PairingResponse>>,
    /// Pairing events not yet taken by the application.
    pending_pairing_events: VecDeque<PairingEvent>,
}

impl Libp2pNetworkAdapter {
    /// Constructs a swarm using libp2p's Noise encrypted TCP transport, Yamux, and mDNS.
    ///
    /// Inputs: `node_identity`, normally from `NodeIdentity::load_or_create`.
    /// Outputs: A configured adapter or a portable security-policy error if mDNS cannot initialize.
    /// Side effects: Allocates libp2p state. It neither listens on a TCP address nor sends data.
    pub fn new(node_identity: NodeIdentity) -> Result<Self, NetworkError> {
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
            .with_behaviour(|_| {
                Ok(NoteTakerBehaviour {
                    peer_gate: PeerGate::default(),
                    mdns: Toggle::from(Some(mdns::tokio::Behaviour::new(
                        mdns::Config::default(),
                        local_peer_id,
                    )?)),
                    sync_payloads: Self::sync_payload_behaviour(),
                    file_transfers: file_transfer_behaviour::new_file_transfer_behaviour(),
                    pairing: new_pairing_behaviour(),
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
            pending_file_transfer_payloads: VecDeque::new(),
            nearby_unpaired_peer_ids: HashSet::new(),
            pending_pairing_answers: HashMap::new(),
            pending_pairing_events: VecDeque::new(),
        })
    }

    /// Allows `peer_device_id` to connect and sync, as after a confirmed pairing.
    ///
    /// Inputs: A peer ID in the adapter's portable form. Outputs: Success, or `SecurityPolicy`
    /// for an ID that is not a libp2p peer ID.
    /// Side effects: Updates the connection gate; a later mDNS discovery dials the peer.
    pub fn trust_peer(&mut self, peer_device_id: &DeviceId) -> Result<(), NetworkError> {
        let peer_id = Self::peer_id_from_device_id(peer_device_id)?;
        self.swarm.behaviour_mut().peer_gate.trust(peer_id);
        self.nearby_unpaired_peer_ids.remove(peer_device_id);
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
        if let SwarmEvent::Behaviour(behaviour_event) = swarm_event {
            match behaviour_event {
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
            }
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
    /// Side effects: Adds reachability and queues a portable discovery event on first observation.
    fn record_discovered_peer(&mut self, peer_id: PeerId) {
        let peer_device_id = Self::device_id_from_peer_id(peer_id);
        if !self.swarm.behaviour().peer_gate.is_trusted(&peer_id) {
            self.nearby_unpaired_peer_ids.insert(peer_device_id);
            return;
        }
        if !self.swarm.is_connected(&peer_id) && self.swarm.dial(peer_id).is_err() {
            tracing::debug!("could not dial a discovered paired peer");
        }
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
        self.nearby_unpaired_peer_ids.remove(&peer_device_id);
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
    fn new_for_memory_transport() -> Result<Self, NetworkError> {
        Self::new_for_memory_transport_with(NodeIdentity::ephemeral())
    }

    /// Like `new_for_memory_transport`, with a given identity.
    #[cfg(test)]
    fn new_for_memory_transport_with(node_identity: NodeIdentity) -> Result<Self, NetworkError> {
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
            },
            local_peer_id,
            Config::with_tokio_executor(),
        );
        Ok(Self {
            swarm,
            reachable_peer_device_ids: HashSet::new(),
            pending_network_events: VecDeque::new(),
            pending_file_transfer_payloads: VecDeque::new(),
            nearby_unpaired_peer_ids: HashSet::new(),
            pending_pairing_answers: HashMap::new(),
            pending_pairing_events: VecDeque::new(),
        })
    }

    /// Starts the test-only swarm on a libp2p memory address.
    ///
    /// Inputs: `memory_address` must be a valid unoccupied `/memory/<port>` multiaddress.
    /// Outputs: Success when the swarm begins listening, or a portable configuration error.
    /// Side effects: Registers an in-process memory listener and no operating-system socket.
    #[cfg(test)]
    fn listen_on_memory(&mut self, memory_address: libp2p::Multiaddr) -> Result<(), NetworkError> {
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
    fn dial_memory(&mut self, memory_address: libp2p::Multiaddr) -> Result<(), NetworkError> {
        self.swarm
            .dial(memory_address)
            .map_err(|error| NetworkError::SecurityPolicy {
                reason: format!("cannot dial in-memory transport: {error}"),
            })
    }

    /// Returns this adapter's translated local peer identity for adapter-local integration tests.
    ///
    /// Inputs: None.
    /// Outputs: The portable representation of the adapter's generated libp2p peer identity.
    /// Side effects: None.
    #[cfg(test)]
    fn local_device_id(&self) -> DeviceId {
        Self::device_id_from_peer_id(*self.swarm.local_peer_id())
    }

    /// Reports whether this adapter currently has an authenticated connection to a test peer.
    ///
    /// Inputs: `peer_device_id` uses the adapter's peer-identity translation.
    /// Outputs: `true` only while the corresponding libp2p peer is connected.
    /// Side effects: None.
    #[cfg(test)]
    fn is_connected_to(&self, peer_device_id: &DeviceId) -> bool {
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
                        if let Some(network_event) = receiving_adapter.poll_event().unwrap_or_else(|error| panic!("adapter B event queue should be readable: {error}")) {
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
                        if let Some(network_event) = receiving_adapter.poll_event().unwrap_or_else(|error| panic!("adapter B event queue should be readable: {error}")) {
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
        assert!(
            receiver
                .poll_event()
                .unwrap_or_else(|error| panic!("event queue should be readable: {error}"))
                .is_none()
        );
    }
}
