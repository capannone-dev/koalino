//! Groups libp2p-backed implementations of encrypted P2P network ports.

/// Defines the attachment-only request-response protocol and its framing codec.
pub mod file_transfer_behaviour;
/// Configures a Noise-encrypted, Yamux-multiplexed libp2p swarm with mDNS discovery.
pub mod libp2p_adapter;
/// Admits connections only from paired peers, or for pairing while a window is open (Issue #43).
mod peer_gate;
/// Receives Teams bot media over a localhost WebSocket carrying Protobuf frames (Issue #33).
pub mod teams_bridge;
