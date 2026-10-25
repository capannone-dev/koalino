<!-- This directory contains libp2p infrastructure adapters and never exposes libp2p types above the ports layer. -->

# Network adapters

`libp2p_adapter.rs` builds the decentralized local-network transport: TCP is secured with Noise,
connections are multiplexed with Yamux, and mDNS supplies local peer-discovery events. Its private
`/note-taker/sync-message/1` request-response behaviour frames bounded opaque `SyncMessage`
payloads and translates authenticated inbound requests to `NetworkEvent::MessageReceived`.
Application code consequently has no dependency on libp2p types or socket APIs. Binding listener
addresses and driving the asynchronous swarm are composition-root responsibilities. The adapter's
integration contract uses Noise over libp2p `MemoryTransport`, never TCP or UDP sockets, to verify
directed and broadcast payload delivery.
