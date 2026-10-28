<!-- This directory contains network infrastructure adapters: libp2p peer sync and the localhost Teams bot bridge. Neither exposes its library types above the ports layer. -->

# Network adapters

`NodeIdentity` (in `libp2p_adapter.rs`) is the node's Ed25519 keypair. `load_or_create` keeps it
in a `NodeIdentityStore`, so the peer ID, and therefore every pairing, survives restarts.
`Libp2pNetworkAdapter::new` takes one. `NodeIdentity::ephemeral()` exists only for tests and
throwaway nodes.

`peer_gate.rs` is the first behaviour in the swarm. It decides on each connection after Noise has
authenticated the remote key:

- **Inbound:** a trusted peer, or any peer while `open_pairing_window(duration)` is in effect.
- **Outbound:** a trusted peer, or the peer passed to `allow_pairing_dial`.
- **Disconnects:** `revoke_peer` closes that peer's connections, and `end_pairing` closes every
  connection that is still untrusted.

Being admitted grants nothing beyond the connection. The adapter drops sync and file-transfer
requests from untrusted peers, `send_encrypted` refuses an untrusted target, and
`broadcast_encrypted` reaches only trusted peers. mDNS discoveries of trusted peers are dialed
and reported as `PeerDiscovered`. Untrusted ones are listed only by `nearby_unpaired_peers()`, as
pairing candidates. The trusted set is loaded from `TrustedPeerStore` by the composition layer.

`pairing_behaviour.rs` defines `/note-taker/pairing/1`, which carries version-1 JSON in 4 KiB
length-prefixed frames. The initiator sends `PairingRequest { device_name }`. The responder replies
`PairingResponse { accepted_as: Some(name) | None }` once its owner has compared the codes. The
request timeout is two minutes, to leave time for that.

The adapter implements `PairingTransport` as follows:

- An incoming request with a valid name becomes `PairingEvent::Requested`, and its response
  channel is held until `answer_pairing`. A request with an invalid name is rejected at once.
- Answers and delivery failures become `Answered` or `Failed`.
- Pairing events have their own queue, `poll_pairing_event`, separate from `NetworkEvent`.
- The adapter only carries messages. Deciding when to trust a peer is the application's job.

`libp2p_adapter.rs` builds the decentralized local-network transport: TCP secured with Noise,
connections multiplexed with Yamux, and mDNS events for local peer discovery. Its private
`/note-taker/sync-message/1` request-response behaviour frames bounded opaque `SyncMessage`
payloads and translates authenticated inbound requests to `NetworkEvent::MessageReceived`, so
application code has no dependency on libp2p types or socket APIs. Binding listener addresses
and driving the asynchronous swarm are composition-root responsibilities. The adapter's
integration contract uses Noise over libp2p `MemoryTransport`, never TCP or UDP sockets, to
verify directed and broadcast payload delivery.

`file_transfer/` and `file_transfer_behaviour.rs` carry content-addressed attachment transfers
between peers.

`teams_bridge/` (Issue #33) is a separate, local-only inbound channel. A Teams bot process
connects over a WebSocket on `127.0.0.1`, which is never exposed to the network, and streams
Protobuf frames that the adapter decodes into `ExternalMediaChunk` and `SlideCapture`. See
`teams_bridge/README.md`.
