//! Exposes infrastructure adapters; this initial scaffold only configures observability.

/// Implements opt-in local AI ports without exposing model types outside adapters.
pub mod ai;
/// Implements audio-capture ports with local, platform-facing infrastructure.
pub mod audio;
/// Waits for child processes that the Flutter engine may reap first.
pub mod child_process;
/// Reads imported e-mails with `mail-parser` behind the e-mail port (Issue #85).
pub mod email;
/// Implements portable meeting export renderers behind the export port.
pub mod export;
/// Stores the random embedding key in the operating system's credential store.
pub mod keychain;
/// Implements local attachment metadata extraction behind the media port.
pub mod media;
/// Implements encrypted libp2p transport and decentralized local discovery.
pub mod network;
/// Configures structured, redaction-aware logging sinks.
pub mod observability;
/// Implements persistence ports with local storage technologies.
pub mod storage;
