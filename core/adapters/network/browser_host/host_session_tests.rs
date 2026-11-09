//! Specifies one host session end to end: browser messages in, bridge frames out, and status and
//! rejections reported back to the extension.

use std::{
    io::{Cursor, Write},
    sync::{Arc, Mutex},
    time::Duration,
};

use serde_json::json;

use super::{
    bridge_client::BridgeClientConfig,
    host_session::run_host_session,
    native_messaging::{read_message, write_message},
};
use crate::adapters::network::teams_bridge::{
    bridge_buffer::BridgeBuffer,
    message_decoder::BridgeEvent,
    teams_websocket_server::{
        DEFAULT_AUTHENTICATION_TIMEOUT, TeamsBridgeServerConfig, TeamsWebSocketServer,
    },
};

/// A writer whose bytes the test can read afterwards.
#[derive(Clone, Default)]
struct SharedOutput(Arc<Mutex<Vec<u8>>>);

impl Write for SharedOutput {
    /// Appends to the shared buffer.
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("poisoned"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    /// Nothing to flush.
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn messages_become_frames_and_the_extension_hears_back() {
    let buffer = BridgeBuffer::new(Duration::from_secs(60));
    let server = TeamsWebSocketServer::start(
        TeamsBridgeServerConfig {
            port: 0,
            shared_secret: None,
            authentication_timeout: DEFAULT_AUTHENTICATION_TIMEOUT,
        },
        buffer.clone(),
    )
    .await
    .unwrap_or_else(|error| panic!("server should start: {error}"));
    let mut input = Vec::new();
    for message in [
        json!({"type": "meeting", "meetingId": "19:m@thread.v2", "startTimeMs": 0}),
        json!({"type": "unknown"}),
        json!({"type": "ended", "meetingId": "19:m@thread.v2"}),
    ] {
        write_message(&mut input, &message)
            .unwrap_or_else(|error| panic!("input should encode: {error}"));
    }
    let output = SharedOutput::default();

    run_host_session(
        Cursor::new(input),
        output.clone(),
        BridgeClientConfig::new(server.local_addr().port(), None)
            .with_reconnect_interval(Duration::from_millis(50)),
    )
    .await;

    let mut events = Vec::new();
    for _attempt in 0..100 {
        while let Some(event) = buffer.next_event(Duration::ZERO) {
            events.push(event);
        }
        if events.len() >= 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        matches!(events.first(), Some(BridgeEvent::MeetingStarted(_))),
        "{events:?}"
    );
    assert!(matches!(events.get(1), Some(BridgeEvent::MeetingEnded(_))));

    let written = output
        .0
        .lock()
        .map(|bytes| bytes.clone())
        .unwrap_or_default();
    let mut replies = Vec::new();
    let mut reader = Cursor::new(written);
    while let Ok(Some(reply)) = read_message(&mut reader) {
        replies.push(reply);
    }
    assert!(replies.contains(&json!({"type": "status", "bridge": "connected"})));
    assert!(replies.iter().any(|reply| reply["type"] == "rejected"));
    server.shutdown().await;
}
