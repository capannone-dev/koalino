<!-- This directory contains libp2p infrastructure adapters and never exposes libp2p types above the ports layer. -->

# Network adapters

`libp2p_adapter.rs` builds the decentralized local-network transport: TCP is secured with Noise,
connections are multiplexed with Yamux, and mDNS supplies local peer-discovery events. The adapter
translates those events to `ports::network::NetworkEvent`; application code consequently has no
dependency on libp2p types or socket APIs. Binding listener addresses and driving the asynchronous
swarm are composition-root responsibilities and are intentionally not exercised by unit tests.
