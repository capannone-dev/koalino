//! The browser extension's native messaging host (Issue #102): the process a browser starts for
//! the Note Taker extension, which turns the extension's JSON messages into Teams bridge frames
//! and sends them to the local core over the loopback WebSocket. See `README.md`.

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
