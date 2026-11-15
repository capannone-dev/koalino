//! Admits libp2p connections only from paired peers, or for pairing while a window is open.
//!
//! The gate runs when a connection has been authenticated by Noise, so `peer` is the verified
//! identity of the remote key, not a claim. Its rules are:
//!
//! - **Inbound:** admit a trusted peer, or any peer while the owner keeps a pairing window open.
//! - **Outbound:** admit a trusted peer, the one peer the owner chose to pair with, or a nearby
//!   peer asked for its name (#121), which the other side admits only during its own window.
//! - **Revocation, and the end of pairing:** close connections to peers that are not trusted.
//! - **Keep-alive:** a connection to a trusted peer stays open while idle, so a change reaches it
//!   at once; other connections close after the swarm's idle timeout.
//!
//! Admission alone never grants sync access: the adapter separately drops sync and file payloads
//! from untrusted peers, so a pairing connection can only carry the pairing protocol.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    convert::Infallible,
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};

use libp2p::core::upgrade::DeniedUpgrade;
use libp2p::{
    Multiaddr, PeerId,
    core::{Endpoint, transport::PortUse},
    swarm::{
        CloseConnection, ConnectionDenied, ConnectionHandler, ConnectionHandlerEvent, ConnectionId,
        FromSwarm, NetworkBehaviour, NotifyHandler, SubstreamProtocol, THandler, THandlerInEvent,
        THandlerOutEvent, ToSwarm,
        handler::{ConnectionEvent, FullyNegotiatedInbound, FullyNegotiatedOutbound},
    },
};

/// Why the gate refused a connection; reported to libp2p, never to the remote peer.
#[derive(Debug, thiserror::Error)]
#[error("peer is not paired with this device")]
struct NotPaired;

/// Connection-level allowlist for paired peers.
#[derive(Default)]
pub(super) struct PeerGate {
    /// Peers the owner paired with this node.
    trusted_peers: HashSet<PeerId>,
    /// While set and in the future, inbound connections from unknown peers are admitted.
    pairing_window_ends_at: Option<Instant>,
    /// Unknown peers this node may dial because the owner started pairing with them.
    pairing_dial_targets: HashSet<PeerId>,
    /// Nearby unknown peers this node may dial to ask their name (#121); they admit the
    /// connection only while their own pairing window is open.
    name_probe_targets: HashSet<PeerId>,
    /// Open connections by peer, so untrusted ones can be closed and newly trusted ones kept.
    connections: HashMap<PeerId, HashSet<ConnectionId>>,
    /// Swarm actions `poll` must emit: disconnects and keep-alive changes.
    pending_actions: VecDeque<ToSwarm<Infallible, bool>>,
    /// Wakes the swarm when a disconnect is queued.
    waker: Option<Waker>,
}

impl PeerGate {
    /// Reports whether `peer` is trusted.
    pub(super) fn is_trusted(&self, peer: &PeerId) -> bool {
        self.trusted_peers.contains(peer)
    }

    /// Lists the trusted peers.
    pub(super) fn trusted_peers(&self) -> impl Iterator<Item = &PeerId> {
        self.trusted_peers.iter()
    }

    /// Adds `peer` to the allowlist and ends any pending pairing dial to it.
    ///
    /// Side effects: Keeps its open connections (from pairing) alive from now on.
    pub(super) fn trust(&mut self, peer: PeerId) {
        self.pairing_dial_targets.remove(&peer);
        self.trusted_peers.insert(peer);
        let open_connections: Vec<ConnectionId> = self
            .connections
            .get(&peer)
            .map(|connections| connections.iter().copied().collect())
            .unwrap_or_default();
        for connection in open_connections {
            self.queue_action(ToSwarm::NotifyHandler {
                peer_id: peer,
                handler: NotifyHandler::One(connection),
                event: true,
            });
        }
    }

    /// Removes `peer` from the allowlist and closes its connections.
    ///
    /// Side effects: Queues a disconnect and wakes the swarm.
    pub(super) fn revoke(&mut self, peer: &PeerId) {
        self.trusted_peers.remove(peer);
        self.pairing_dial_targets.remove(peer);
        if self.connections.contains_key(peer) {
            self.queue_disconnect(*peer);
        }
    }

    /// Closes `peer`'s connections unless it is trusted, for a stranger admitted only to pair that
    /// asked for something else.
    ///
    /// Side effects: Queues a disconnect and wakes the swarm.
    pub(super) fn disconnect_if_untrusted(&mut self, peer: PeerId) {
        if !self.is_trusted(&peer) && self.connections.contains_key(&peer) {
            self.queue_disconnect(peer);
        }
    }

    /// Admits inbound connections from unknown peers for `duration`.
    pub(super) fn open_pairing_window(&mut self, duration: Duration) {
        self.pairing_window_ends_at = Instant::now().checked_add(duration);
    }

    /// Lets this node dial the unknown `peer` to pair with it.
    pub(super) fn allow_pairing_dial(&mut self, peer: PeerId) {
        self.pairing_dial_targets.insert(peer);
    }

    /// Lets this node dial the nearby unknown `peer` to ask its name (#121).
    pub(super) fn allow_name_probe(&mut self, peer: PeerId) {
        self.name_probe_targets.insert(peer);
    }

    /// Stops dialing `peer` for its name: it left the network or was paired.
    pub(super) fn forget_name_probe(&mut self, peer: &PeerId) {
        self.name_probe_targets.remove(peer);
    }

    /// Ends pairing: closes the window, forgets dial targets, and disconnects untrusted peers.
    ///
    /// Side effects: Queues disconnects and wakes the swarm.
    pub(super) fn end_pairing(&mut self) {
        self.pairing_window_ends_at = None;
        self.pairing_dial_targets.clear();
        let untrusted_connected: Vec<PeerId> = self
            .connections
            .keys()
            .filter(|peer| !self.trusted_peers.contains(peer))
            .copied()
            .collect();
        for peer in untrusted_connected {
            self.queue_disconnect(peer);
        }
    }

    /// Reports whether the pairing window is open now.
    fn pairing_window_is_open(&self) -> bool {
        self.pairing_window_ends_at
            .is_some_and(|ends_at| Instant::now() < ends_at)
    }

    /// Queues `peer` for disconnection.
    fn queue_disconnect(&mut self, peer: PeerId) {
        self.queue_action(ToSwarm::CloseConnection {
            peer_id: peer,
            connection: CloseConnection::All,
        });
    }

    /// Queues a swarm action and wakes the swarm.
    fn queue_action(&mut self, action: ToSwarm<Infallible, bool>) {
        self.pending_actions.push_back(action);
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }
}

/// The gate's per-connection handler: it speaks no protocol, like libp2p's `dummy` handler,
/// and keeps the connection open while the peer is trusted.
///
/// The gate sends `true` when a connected peer becomes trusted. Revocation closes the
/// connection instead, so the flag never needs to go back to `false`.
pub(super) struct PairedKeepAlive {
    /// Whether the connection is to a trusted peer.
    keep_alive: bool,
}

impl ConnectionHandler for PairedKeepAlive {
    type FromBehaviour = bool;
    type ToBehaviour = Infallible;
    type InboundProtocol = DeniedUpgrade;
    type OutboundProtocol = DeniedUpgrade;
    type InboundOpenInfo = ();
    type OutboundOpenInfo = ();

    /// Accepts no inbound streams; the other behaviours carry all traffic.
    fn listen_protocol(&self) -> SubstreamProtocol<Self::InboundProtocol, Self::InboundOpenInfo> {
        SubstreamProtocol::new(DeniedUpgrade, ())
    }

    /// Keeps a trusted peer's connection open while idle.
    fn connection_keep_alive(&self) -> bool {
        self.keep_alive
    }

    /// Records that the peer became trusted.
    fn on_behaviour_event(&mut self, keep_alive: bool) {
        self.keep_alive = keep_alive;
    }

    /// Never opens streams or emits events.
    fn poll(
        &mut self,
        _: &mut Context<'_>,
    ) -> Poll<ConnectionHandlerEvent<Self::OutboundProtocol, (), Self::ToBehaviour>> {
        Poll::Pending
    }

    /// No stream is ever negotiated through `DeniedUpgrade`, so there is nothing to handle.
    fn on_connection_event(
        &mut self,
        event: ConnectionEvent<Self::InboundProtocol, Self::OutboundProtocol>,
    ) {
        match event {
            ConnectionEvent::FullyNegotiatedInbound(FullyNegotiatedInbound {
                protocol, ..
            }) => match protocol {},
            ConnectionEvent::FullyNegotiatedOutbound(FullyNegotiatedOutbound {
                protocol, ..
            }) => match protocol {},
            _ => {}
        }
    }
}

impl NetworkBehaviour for PeerGate {
    type ConnectionHandler = PairedKeepAlive;
    type ToSwarm = Infallible;

    /// Admits a trusted peer, or any peer while the pairing window is open.
    fn handle_established_inbound_connection(
        &mut self,
        _: ConnectionId,
        peer: PeerId,
        _: &Multiaddr,
        _: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        if self.is_trusted(&peer) || self.pairing_window_is_open() {
            Ok(PairedKeepAlive {
                keep_alive: self.is_trusted(&peer),
            })
        } else {
            tracing::debug!("refused an inbound connection from an unpaired peer");
            Err(ConnectionDenied::new(NotPaired))
        }
    }

    /// Admits a trusted peer, the peer the owner chose to pair with, or a nearby peer asked for its
    /// name.
    fn handle_established_outbound_connection(
        &mut self,
        _: ConnectionId,
        peer: PeerId,
        _: &Multiaddr,
        _: Endpoint,
        _: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        if self.is_trusted(&peer)
            || self.pairing_dial_targets.contains(&peer)
            || self.name_probe_targets.contains(&peer)
        {
            Ok(PairedKeepAlive {
                keep_alive: self.is_trusted(&peer),
            })
        } else {
            tracing::debug!("refused an outbound connection to an unpaired peer");
            Err(ConnectionDenied::new(NotPaired))
        }
    }

    /// Tracks each peer's open connections.
    fn on_swarm_event(&mut self, event: FromSwarm) {
        match event {
            FromSwarm::ConnectionEstablished(established) => {
                self.connections
                    .entry(established.peer_id)
                    .or_default()
                    .insert(established.connection_id);
            }
            FromSwarm::ConnectionClosed(closed) => {
                if let Some(connections) = self.connections.get_mut(&closed.peer_id) {
                    connections.remove(&closed.connection_id);
                    if connections.is_empty() {
                        self.connections.remove(&closed.peer_id);
                    }
                }
            }
            _ => {}
        }
    }

    /// The handler emits no events.
    fn on_connection_handler_event(
        &mut self,
        _: PeerId,
        _: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        match event {}
    }

    /// Emits queued disconnects and keep-alive changes.
    fn poll(
        &mut self,
        context: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        if let Some(action) = self.pending_actions.pop_front() {
            return Poll::Ready(action);
        }
        self.waker = Some(context.waker().clone());
        Poll::Pending
    }
}
