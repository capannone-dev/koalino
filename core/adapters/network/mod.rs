//! Groups libp2p-backed implementations of encrypted P2P network ports.

/// Configures a Noise-encrypted, Yamux-multiplexed libp2p swarm with mDNS discovery.
pub mod libp2p_adapter;
