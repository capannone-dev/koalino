<!-- This directory contains network infrastructure adapters: libp2p peer sync and the localhost Teams bot bridge. Neither exposes its library types above the ports layer. -->

# Network adapters

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
