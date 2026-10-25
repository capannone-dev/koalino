//! Exposes infrastructure adapters; this initial scaffold only configures observability.

/// Implements audio-capture ports with local, platform-facing infrastructure.
pub mod audio;
/// Implements encrypted libp2p transport and decentralized local discovery.
pub mod network;
/// Configures structured, redaction-aware logging sinks.
pub mod observability;
/// Implements persistence ports with local storage technologies.
pub mod storage;
