//! Receives Teams bot audio, slides, and meeting metadata over a localhost WebSocket.
//!
//! Frames follow `core/protocol/teams_bridge.proto`. `message_decoder` turns them into domain
//! values (`ExternalMediaChunk`, `SlideCapture`, meeting starts), `teams_websocket_server` accepts the bot on 127.0.0.1, and `bridge_buffer` holds them
//! until the meeting pump consumes them, with audio beyond its memory bound waiting sealed in
//! `sealed_audio_spool` (#133). See `README.md`.

/// Holds decoded events between the WebSocket reader and the meeting pump.
pub mod bridge_buffer;
/// Decodes Protobuf frames into validated domain values.
pub mod message_decoder;
/// Keeps audio the pump hasn't reached yet in sealed files (#133).
pub mod sealed_audio_spool;
/// Accepts the bot's localhost WebSocket connection and feeds the buffer.
pub mod teams_websocket_server;

#[cfg(test)]
mod bridge_buffer_tests;
#[cfg(test)]
pub(crate) mod log_capture;
#[cfg(test)]
mod message_decoder_tests;
#[cfg(test)]
mod sealed_audio_spool_tests;
#[cfg(test)]
mod teams_websocket_server_tests;
