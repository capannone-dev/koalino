//! The browser extension's native messaging host (Issue #102): the process a browser starts for
//! the Koalino extension, which turns the extension's JSON messages into Teams bridge frames
//! and sends them to the local core over the loopback WebSocket. See `README.md`.

/// Starts Koalino when a meeting needs it and it isn't running (#135).
pub mod app_launcher;
/// Connects to the local Teams bridge, authenticates, queues and reconnects.
pub mod bridge_client;
/// The extension's JSON messages and their conversion into Teams bridge frames.
pub mod extension_message;
/// Registers the host with Firefox, Chrome, Chromium and Edge.
pub mod host_install;
/// One session: stdin to the bridge, status and rejections back to stdout.
pub mod host_session;
/// Native messaging framing: a 32-bit native-endian length followed by UTF-8 JSON.
pub mod native_messaging;
/// Frames Koalino hasn't received yet, sealed on disk until delivered (#135).
pub mod outbox;
/// Replays outbox sessions whose browser host has gone, in order (#135).
pub mod outbox_delivery;

#[cfg(test)]
mod app_launcher_tests;
#[cfg(test)]
mod bridge_client_tests;
#[cfg(test)]
mod extension_message_tests;
#[cfg(test)]
mod host_install_tests;
#[cfg(test)]
mod host_session_tests;
#[cfg(test)]
mod native_messaging_tests;
#[cfg(test)]
mod outbox_tests;
