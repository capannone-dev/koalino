<!-- This directory contains network infrastructure adapters: libp2p peer sync and the localhost Teams bridge and the browser extension host that feeds it. Neither exposes its library types above the ports layer. -->

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
- **Keep-alive:** a connection to a trusted peer stays open while idle, including a pairing
  connection once its peer becomes trusted. Other connections close after libp2p's 10-second idle
  timeout. Without this, a paired connection closed 10 s after its last sync, and the device was
  not dialed again until mDNS re-announced it, minutes later.

Being admitted grants nothing beyond the connection. The adapter drops sync and file-transfer
requests from untrusted peers, `send_encrypted` refuses an untrusted target, and
`broadcast_encrypted` reaches only trusted peers. mDNS discoveries of trusted peers are dialed.
Untrusted ones are listed only by `nearby_unpaired_peers()`, as pairing candidates.

Reachability follows connections, not announcements. A trusted peer is reported as
`PeerDiscovered` when its first connection is established, or at once when an already-connected
(pairing) peer becomes trusted. Its first sync message therefore never races the dial. The peer is
reported as `PeerExpired` when its last connection closes or it is revoked. A sync payload that a
trusted peer does not acknowledge is reported as `DeliveryFailed`. This happens when the peer dropped
it because it did not trust this device yet, or when the request timed out, so the sender knows to
send again.

`add_static_peer` takes a `…/p2p/<peer id>` multiaddress for a device that mDNS cannot find (another
subnet, NAT, a VPN, a tunnel). An unpaired one is listed as nearby so it can be paired; a paired one
is dialed. `redial_paired_peers` dials every trusted peer that is not connected (configured or
found by mDNS, whose address stays cached) and lists unpaired configured ones again, so a device
that comes back online reconnects. The
address is only a hint: Noise still authenticates the peer ID, and the gate still decides
admission. The trusted set is loaded from `TrustedPeerStore` by the composition layer.

**Relay.** A node built with `new_relay_server` and given `advertise_relay_address` offers Circuit
Relay v2 to its paired devices. `use_relay` makes a device hold a reservation there and teaches the
swarm a `<relay>/p2p-circuit/p2p/<peer>` address for each paired peer; `redial_paired_peers` keeps
both up, so a relay that returns is used again and one that is gone costs only this route. DCUtR
then tries to turn a relayed connection into a direct one, using addresses Identify observed.

- **The relay reads nothing.** Noise runs end to end through the circuit, so it forwards ciphertext.
  The daemon that hosts it is still a paired device, and syncs meetings as one.
- **Paired devices only.** The gate refuses everyone else. A pairing window admits strangers to the
  node, so the adapter disconnects any untrusted peer the moment it reserves or opens a circuit.
- **The destination decides.** A relayed connection passes the destination's gate like any other.
- **Limits** are one hour and 1 GiB per circuit, after which the devices dial again; libp2p's
  defaults (2 minutes, 128 KiB) would not fit one sync message.
- **Pairing still needs a direct route**: `nearby_unpaired_peers` comes from mDNS or configured
  addresses, and the relay address serves paired devices.

`pairing_behaviour.rs` defines `/note-taker/pairing/1`, which carries version-1 JSON in 4 KiB
length-prefixed frames. The initiator sends `PairingRequest { device_name }`. The responder replies
`PairingResponse { accepted_as: Some(name) | None }` once its owner has compared the codes. The
request timeout is two minutes, to leave time for that.

`device_name_behaviour.rs` defines `/note-taker/device-name/1` (Issue #121), framed like the
pairing protocol. A `DeviceNameQuestion` gets a `DeviceNameAnswer { device_name }`, with a
10-second timeout. Every `NAME_PROBE_INTERVAL` (30 s) the adapter asks each nearby, unpaired device
it has no name for; `peer_gate.rs` admits outbound dials to those probe targets. The other device
answers only over connections its own gate admits, so only while its pairing window is open. A
name that fails `DeviceName::parse` is dropped. `nearby_peer_name` reads the learned name;
forgetting or expiring a nearby peer drops it. `set_local_name` changes the name this device
answers with and puts in its pairing requests.

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

`file_transfer_behaviour.rs` carries attachment chunks, requested one at a time,
between peers.

`teams_bridge/` (Issue #33) is a separate, local-only inbound channel. A Teams bot process
connects over a WebSocket on `127.0.0.1`, which is never exposed to the network, and streams
Protobuf frames that the adapter decodes into `ExternalMediaChunk` and `SlideCapture`. See
`teams_bridge/README.md`.

`browser_host/` (Issue #102) is the other end of that channel for the browser extension: the
`browser-host` native messaging host turns the extension's JSON messages into the same Protobuf
frames and sends them to the bridge as a local client. See `browser_host/README.md`.
