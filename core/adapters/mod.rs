//! Exposes infrastructure adapters; this initial scaffold only configures observability.

/// Reads who is speaking from call windows' accessibility trees (#113).
pub mod accessibility;
/// Implements opt-in local AI ports without exposing model types outside adapters.
pub mod ai;
/// Implements audio-capture ports with local, platform-facing infrastructure.
pub mod audio;
/// Waits for child processes that the Flutter engine may reap first.
pub mod child_process;
/// Reads the text of attached documents for summaries, chat and search (#128).
pub mod documents;
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
/// Shows the system notifications the core owns (the call offer's Windows toast, #111).
pub mod notification;
/// Configures structured, redaction-aware logging sinks.
pub mod observability;
/// Implements persistence ports with local storage technologies.
pub mod storage;
