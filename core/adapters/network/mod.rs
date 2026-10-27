//! Groups libp2p-backed implementations of encrypted P2P network ports.

/// Configures a Noise-encrypted, Yamux-multiplexed libp2p swarm with mDNS discovery.
pub mod file_transfer_behaviour;
/// Configures a Noise-encrypted, Yamux-multiplexed libp2p swarm with mDNS discovery.
pub mod libp2p_adapter;
/// Receives Teams bot media over a localhost WebSocket carrying Protobuf frames (Issue #33).
pub mod teams_bridge;
