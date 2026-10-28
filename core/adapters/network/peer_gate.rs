//! Admits libp2p connections only from paired peers, or for pairing while a window is open.
//!
//! The gate runs when a connection has been authenticated by Noise, so `peer` is the verified
//! identity of the remote key, not a claim. Its rules are:
//!
//! - **Inbound:** admit a trusted peer, or any peer while the owner keeps a pairing window open.
//! - **Outbound:** admit a trusted peer, or the one peer the owner chose to pair with.
//! - **Revocation, and the end of pairing:** close connections to peers that are not trusted.
//!
//! Admission alone never grants sync access: the adapter separately drops sync and file payloads
//! from untrusted peers, so a pairing connection can only carry the pairing protocol.

use std::{
    collections::{HashSet, VecDeque},
    convert::Infallible,
    task::{Context, Poll, Waker},
    time::{Duration, Instant},
};

use libp2p::{
    Multiaddr, PeerId,
    core::{Endpoint, transport::PortUse},
    swarm::{
        CloseConnection, ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour, THandler,
        THandlerInEvent, THandlerOutEvent, ToSwarm, dummy,
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
    /// Peers with at least one open connection, so untrusted ones can be closed later.
    connected_peers: HashSet<PeerId>,
    /// Peers whose connections `poll` must close.
    peers_to_disconnect: VecDeque<PeerId>,
    /// Wakes the swarm when a disconnect is queued.
    waker: Option<Waker>,
}

impl PeerGate {
    /// Reports whether `peer` is trusted.
    pub(super) fn is_trusted(&self, peer: &PeerId) -> bool {
        self.trusted_peers.contains(peer)
    }

    /// Adds `peer` to the allowlist and ends any pending pairing dial to it.
    pub(super) fn trust(&mut self, peer: PeerId) {
        self.pairing_dial_targets.remove(&peer);
        self.trusted_peers.insert(peer);
    }

    /// Removes `peer` from the allowlist and closes its connections.
    ///
    /// Side effects: Queues a disconnect and wakes the swarm.
    pub(super) fn revoke(&mut self, peer: &PeerId) {
        self.trusted_peers.remove(peer);
        self.pairing_dial_targets.remove(peer);
        if self.connected_peers.contains(peer) {
            self.queue_disconnect(*peer);
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

    /// Ends pairing: closes the window, forgets dial targets, and disconnects untrusted peers.
    ///
    /// Side effects: Queues disconnects and wakes the swarm.
    pub(super) fn end_pairing(&mut self) {
        self.pairing_window_ends_at = None;
        self.pairing_dial_targets.clear();
        let untrusted_connected: Vec<PeerId> = self
            .connected_peers
            .iter()
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

    /// Queues `peer` for disconnection and wakes the swarm.
    fn queue_disconnect(&mut self, peer: PeerId) {
        self.peers_to_disconnect.push_back(peer);
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }
}

impl NetworkBehaviour for PeerGate {
    type ConnectionHandler = dummy::ConnectionHandler;
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
            Ok(dummy::ConnectionHandler)
        } else {
            tracing::debug!("refused an inbound connection from an unpaired peer");
            Err(ConnectionDenied::new(NotPaired))
        }
    }

    /// Admits a trusted peer, or the peer the owner chose to pair with.
    fn handle_established_outbound_connection(
        &mut self,
        _: ConnectionId,
        peer: PeerId,
        _: &Multiaddr,
        _: Endpoint,
        _: PortUse,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        if self.is_trusted(&peer) || self.pairing_dial_targets.contains(&peer) {
            Ok(dummy::ConnectionHandler)
        } else {
            tracing::debug!("refused an outbound connection to an unpaired peer");
            Err(ConnectionDenied::new(NotPaired))
        }
    }

    /// Tracks which peers are connected.
    fn on_swarm_event(&mut self, event: FromSwarm) {
        match event {
            FromSwarm::ConnectionEstablished(established) => {
                self.connected_peers.insert(established.peer_id);
            }
            FromSwarm::ConnectionClosed(closed) if closed.remaining_established == 0 => {
                self.connected_peers.remove(&closed.peer_id);
            }
            _ => {}
        }
    }

    /// The dummy handler emits no events.
    fn on_connection_handler_event(
        &mut self,
        _: PeerId,
        _: ConnectionId,
        event: THandlerOutEvent<Self>,
    ) {
        match event {}
    }

    /// Closes queued connections.
    fn poll(
        &mut self,
        context: &mut Context<'_>,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        if let Some(peer) = self.peers_to_disconnect.pop_front() {
            return Poll::Ready(ToSwarm::CloseConnection {
                peer_id: peer,
                connection: CloseConnection::All,
            });
        }
        self.waker = Some(context.waker().clone());
        Poll::Pending
    }
}
