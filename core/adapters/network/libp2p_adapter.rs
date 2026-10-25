//! Builds and drives a Noise-encrypted libp2p swarm while exposing only portable P2P port values.

use std::{
    collections::{HashSet, VecDeque},
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

#[async_trait::async_trait]
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
    domain::{errors::NetworkError, models::DeviceId},
    ports::network::{NetworkEvent, P2pNetwork, PeerDiscovery},
};

/// Aggregates the decentralized discovery behaviours carried by the libp2p swarm.
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "NoteTakerBehaviourEvent")]
struct NoteTakerBehaviour {
    /// Discovers peers on the local network without a centralized registry.
    mdns: Toggle<mdns::tokio::Behaviour>,
    /// Carries opaque versioned application payloads on encrypted authenticated substreams.
    sync_payloads: request_response::Behaviour<SyncPayloadCodec>,
}

/// Normalizes individual behaviour events for the swarm event loop.
#[derive(Debug)]
enum NoteTakerBehaviourEvent {
    /// Carries a local-network mDNS discovery or expiry update.
    Mdns(mdns::Event),
    /// Carries an inbound or outbound event from the private sync-payload protocol.
    SyncPayload(request_response::Event<Vec<u8>, Vec<u8>>),
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
                    mdns: Toggle::from(Some(mdns::tokio::Behaviour::new(
                        mdns::Config::default(),
                        local_peer_id,
                    )?)),
                    sync_payloads: Self::sync_payload_behaviour(),
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
        if let SwarmEvent::Behaviour(behaviour_event) = swarm_event {
            match behaviour_event {
                NoteTakerBehaviourEvent::Mdns(mdns_event) => self.translate_mdns_event(mdns_event),
                NoteTakerBehaviourEvent::SyncPayload(sync_payload_event) => {
                    self.translate_sync_payload_event(sync_payload_event);
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
        use libp2p::{
            Transport,
            core::{transport::MemoryTransport, upgrade},
            swarm::Config,
        };

        let local_identity_keypair = libp2p::identity::Keypair::generate_ed25519();
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
                mdns: Toggle::from(None),
                sync_payloads: Self::sync_payload_behaviour(),
            },
            local_peer_id,
            Config::with_tokio_executor(),
        );
        Ok(Self {
            swarm,
            reachable_peer_device_ids: HashSet::new(),
            pending_network_events: VecDeque::new(),
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
        let connected_peer_ids: Vec<_> = self.swarm.connected_peers().copied().collect();
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

    use super::Libp2pNetworkAdapter;
    use crate::ports::network::{NetworkEvent, P2pNetwork};

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
}
