//! Specifies the host's connection to the local Teams bridge, against the real bridge server:
//! authentication, frame order, queueing while the core is down, and the queue's drop policy.

use std::time::Duration;

use serde_json::json;
use tokio::sync::mpsc;

use super::{
    bridge_client::{BridgeClientConfig, BridgeStatus, FrameQueue, run_bridge_client},
    extension_message::{BridgeFrame, to_bridge_frame},
};
use crate::adapters::network::teams_bridge::{
    bridge_buffer::BridgeBuffer,
    message_decoder::BridgeEvent,
    teams_websocket_server::{
        BridgeSecret, DEFAULT_AUTHENTICATION_TIMEOUT, TeamsBridgeServerConfig, TeamsWebSocketServer,
    },
};

/// The meeting the test frames belong to.
const MEETING: &str = "19:meeting_browser@thread.v2";

/// A meeting frame.
fn meeting_frame() -> BridgeFrame {
    to_bridge_frame(json!({"type": "meeting", "meetingId": MEETING, "startTimeMs": 0}))
        .unwrap_or_else(|error| panic!("meeting should convert: {error}"))
}

/// A one-second remote audio frame at `timestamp_ms`.
fn audio_frame(timestamp_ms: u64) -> BridgeFrame {
    use base64::Engine as _;
    to_bridge_frame(json!({
        "type": "audio",
        "track": "remote",
        "timestampMs": timestamp_ms,
        "pcm": base64::engine::general_purpose::STANDARD.encode(vec![0_u8; 32_000])
    }))
    .unwrap_or_else(|error| panic!("audio should convert: {error}"))
}

/// Starts the real bridge server on `port` (0 for any), with an optional secret.
async fn start_server(
    port: u16,
    secret: Option<&str>,
) -> (
    crate::adapters::network::teams_bridge::teams_websocket_server::RunningTeamsBridge,
    BridgeBuffer,
) {
    let buffer = BridgeBuffer::new(Duration::from_secs(60));
    let server = TeamsWebSocketServer::start(
        TeamsBridgeServerConfig {
            port,
            shared_secret: secret.map(|secret| BridgeSecret::new(secret.to_owned())),
            authentication_timeout: DEFAULT_AUTHENTICATION_TIMEOUT,
        },
        buffer.clone(),
    )
    .await
    .unwrap_or_else(|error| panic!("server should start: {error}"));
    (server, buffer)
}

/// Waits (up to 5 s) for `count` events in the buffer.
async fn collect_events(buffer: &BridgeBuffer, count: usize) -> Vec<BridgeEvent> {
    let mut events = Vec::new();
    for _attempt in 0..250 {
        while let Some(event) = buffer.next_event(Duration::ZERO) {
            events.push(event);
        }
        if events.len() >= count {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    events
}

/// A client configuration for `port`.
fn client_config(port: u16, secret: Option<&str>) -> BridgeClientConfig {
    BridgeClientConfig::new(port, secret.map(str::to_owned))
        .with_reconnect_interval(Duration::from_millis(50))
}

#[tokio::test]
async fn frames_reach_the_bridge_in_order_after_authenticating() {
    let (server, buffer) = start_server(0, Some("host-secret")).await;
    let (frame_sender, frame_receiver) = mpsc::channel(16);
    let (status_sender, mut status_receiver) = mpsc::unbounded_channel();
    let client = tokio::spawn(run_bridge_client(
        client_config(server.local_addr().port(), Some("host-secret")),
        frame_receiver,
        status_sender,
    ));
    for frame in [meeting_frame(), audio_frame(0), audio_frame(1_000)] {
        frame_sender
            .send(frame)
            .await
            .unwrap_or_else(|_| panic!("client should accept frames"));
    }

    let events = collect_events(&buffer, 3).await;
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], BridgeEvent::MeetingStarted(_)));
    assert!(
        matches!(&events[2], BridgeEvent::Audio(audio) if audio.media_chunk().timestamp_ms() == 1_000)
    );
    assert_eq!(status_receiver.recv().await, Some(BridgeStatus::Connected));

    drop(frame_sender);
    client
        .await
        .unwrap_or_else(|error| panic!("client should stop: {error}"));
    server.shutdown().await;
}

#[tokio::test]
async fn frames_wait_for_the_core_to_start() {
    // Reserve a port, then free it so the bridge is not listening yet.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .unwrap_or_else(|error| panic!("a port should be free: {error}"));
    let (frame_sender, frame_receiver) = mpsc::channel(16);
    let (status_sender, mut status_receiver) = mpsc::unbounded_channel();
    let client = tokio::spawn(run_bridge_client(
        client_config(port, None),
        frame_receiver,
        status_sender,
    ));
    frame_sender
        .send(meeting_frame())
        .await
        .unwrap_or_else(|_| panic!("client should accept frames"));
    frame_sender
        .send(audio_frame(0))
        .await
        .unwrap_or_else(|_| panic!("client should accept frames"));
    assert_eq!(
        status_receiver.recv().await,
        Some(BridgeStatus::Unavailable)
    );

    let (server, buffer) = start_server(port, None).await;
    let events = collect_events(&buffer, 2).await;
    assert_eq!(
        events.len(),
        2,
        "queued frames are delivered once the core listens"
    );
    assert_eq!(status_receiver.recv().await, Some(BridgeStatus::Connected));

    drop(frame_sender);
    client
        .await
        .unwrap_or_else(|error| panic!("client should stop: {error}"));
    server.shutdown().await;
}

#[test]
fn the_queue_drops_the_oldest_audio_but_keeps_everything_else() {
    let mut queue = FrameQueue::new(2_000);
    queue.push(meeting_frame());
    for second in 0..4 {
        queue.push(audio_frame(second * 1_000));
    }
    assert_eq!(queue.len(), 3, "the meeting and the newest 2 s of audio");
    assert_eq!(queue.dropped_audio_frames(), 2);
    let first = queue.pop();
    assert!(first.is_some_and(|frame| frame.audio_duration_ms().is_none()));
}
