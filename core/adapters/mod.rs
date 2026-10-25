//! Exposes infrastructure adapters; this initial scaffold only configures observability.

/// Implements opt-in local AI ports without exposing model types outside adapters.
#[cfg(feature = "local-ai")]
pub mod ai;
/// Implements audio-capture ports with local, platform-facing infrastructure.
pub mod audio;
/// Implements local attachment metadata extraction behind the media port.
pub mod media;
/// Implements encrypted libp2p transport and decentralized local discovery.
pub mod network;
/// Configures structured, redaction-aware logging sinks.
pub mod observability;
/// Implements persistence ports with local storage technologies.
pub mod storage;
