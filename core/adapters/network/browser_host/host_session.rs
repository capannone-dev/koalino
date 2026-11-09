//! One native messaging session: from the moment the browser starts the host until the extension
//! disconnects (the browser closes the host's stdin).
//!
//! ```text
//! stdin ──read_message──▶ to_bridge_frame ──▶ run_bridge_client ──WebSocket──▶ Teams bridge
//!   (blocking thread)          │ invalid                │ status
//!                              ▼                        ▼
//! stdout ◀──write_message── {"type":"rejected"}   {"type":"status","bridge":"connected"|"unavailable"}
//! ```
//!
//! Replies never contain message contents.

use std::io::{Read, Write};

use serde_json::json;
use tokio::sync::mpsc;

use super::{
    bridge_client::{BridgeClientConfig, BridgeStatus, run_bridge_client},
    extension_message::to_bridge_frame,
    native_messaging::{read_message, write_message},
};

/// Frames waiting between the stdin reader and the WebSocket client.
const FRAME_CHANNEL_CAPACITY: usize = 64;

/// Runs one session.
///
/// Inputs: The browser's messages (stdin), where replies go (stdout), and the bridge settings.
/// Outputs: None, once the input ended and queued frames were delivered (or given up).
/// Side effects: Reads the input on a blocking thread, connects to the bridge, writes replies.
pub async fn run_host_session<Input, Output>(
    input: Input,
    output: Output,
    config: BridgeClientConfig,
) where
    Input: Read + Send + 'static,
    Output: Write + Send + 'static,
{
    let (reply_sender, reply_receiver) = std::sync::mpsc::channel::<serde_json::Value>();
    let writer = std::thread::spawn(move || write_replies(output, &reply_receiver));
    let (frame_sender, frame_receiver) = mpsc::channel(FRAME_CHANNEL_CAPACITY);
    let (status_sender, mut status_receiver) = mpsc::unbounded_channel();

    let reader_replies = reply_sender.clone();
    let reader = tokio::task::spawn_blocking(move || {
        read_messages(input, &frame_sender, &reader_replies);
    });
    let status_replies = reply_sender.clone();
    let status_forwarder = tokio::spawn(async move {
        while let Some(status) = status_receiver.recv().await {
            let bridge = match status {
                BridgeStatus::Connected => "connected",
                BridgeStatus::Unavailable => "unavailable",
            };
            let _ = status_replies.send(json!({"type": "status", "bridge": bridge}));
        }
    });
    run_bridge_client(config, frame_receiver, status_sender).await;
    let _ = reader.await;
    let _ = status_forwarder.await;
    drop(reply_sender);
    let _ = writer.join();
}

/// Reads messages until the input ends, converting each into a frame.
///
/// Side effects: Blocks on the input; sends frames and rejection replies.
fn read_messages(
    mut input: impl Read,
    frame_sender: &mpsc::Sender<super::extension_message::BridgeFrame>,
    replies: &std::sync::mpsc::Sender<serde_json::Value>,
) {
    loop {
        let message = match read_message(&mut input) {
            Ok(Some(message)) => message,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!(error = %error, "Browser message unreadable; closing the session");
                return;
            }
        };
        match to_bridge_frame(message) {
            Ok(frame) => {
                if frame_sender.blocking_send(frame).is_err() {
                    return;
                }
            }
            Err(error) => {
                tracing::warn!(error = %error, "Browser message rejected");
                let _ = replies.send(json!({"type": "rejected", "reason": error.to_string()}));
            }
        }
    }
}

/// Writes replies until every sender is gone.
///
/// Side effects: Writes to the output; stops at the first write error (the browser left).
fn write_replies(mut output: impl Write, replies: &std::sync::mpsc::Receiver<serde_json::Value>) {
    while let Ok(reply) = replies.recv() {
        if write_message(&mut output, &reply).is_err() {
            return;
        }
    }
}
