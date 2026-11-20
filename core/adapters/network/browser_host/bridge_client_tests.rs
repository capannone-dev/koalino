//! Specifies the host's connection to the local Teams bridge, against the real bridge server:
//! authentication, frame order, queueing while the core is down, and the queue's drop policy.

use std::time::Duration;

use serde_json::json;
use tokio::sync::mpsc;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use super::{
    bridge_client::{
        BridgeClientConfig, BridgeStatus, FrameQueue, OutboxSettings, run_bridge_client,
    },
    extension_message::{BridgeFrame, to_bridge_frame},
    outbox::{OutboxKey, try_delivery_lock},
    outbox_delivery::deliver_waiting_sessions,
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
    // Room for a replayed five-minute meeting (the app spools beyond its bound, #133).
    let buffer = BridgeBuffer::new(Duration::from_secs(600));
    let server = TeamsWebSocketServer::start(
        TeamsBridgeServerConfig {
            port,
            shared_secret: secret.map(|secret| BridgeSecret::new(secret.to_owned())),
            authentication_timeout: DEFAULT_AUTHENTICATION_TIMEOUT,
            spoken_language: None,
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
    assert_eq!(
        status_receiver.recv().await,
        Some(BridgeStatus::Connected { language: None })
    );

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
    assert_eq!(
        status_receiver.recv().await,
        Some(BridgeStatus::Connected { language: None })
    );

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

/// A port nothing listens on yet.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .unwrap_or_else(|error| panic!("a port should be free: {error}"))
}

/// A fresh outbox folder.
fn outbox_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("host-outbox-{}", uuid::Uuid::new_v4()))
}

fn outbox_key() -> OutboxKey {
    OutboxKey::from_bridge_secret("host-secret")
}

/// A client that keeps undelivered frames in `root` and gives up 200 ms after the extension left.
fn outbox_config(port: u16, root: &std::path::Path) -> BridgeClientConfig {
    client_config(port, Some("host-secret"))
        .with_outbox(OutboxSettings::new(root.to_path_buf(), outbox_key()))
        .with_final_delivery_timeout(Duration::from_millis(200))
}

/// Frames waiting in the outbox, in every session.
fn waiting_frames(root: &std::path::Path) -> usize {
    std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .flat_map(|session| {
            std::fs::read_dir(session.path())
                .into_iter()
                .flatten()
                .flatten()
        })
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "frame")
        })
        .count()
}

#[tokio::test]
async fn with_an_outbox_a_whole_meeting_waits_for_koalino_even_after_the_browser_left() {
    let port = free_port();
    let root = outbox_root();
    let (frame_sender, frame_receiver) = mpsc::channel(16);
    let (status_sender, _status_receiver) = mpsc::unbounded_channel();
    let client = tokio::spawn(run_bridge_client(
        outbox_config(port, &root),
        frame_receiver,
        status_sender,
    ));
    frame_sender
        .send(meeting_frame())
        .await
        .unwrap_or_else(|_| panic!("client should accept frames"));
    // Five minutes of audio: more than the 4 minutes the memory queue keeps.
    for second in 0..300 {
        frame_sender
            .send(audio_frame(second * 1_000))
            .await
            .unwrap_or_else(|_| panic!("client should accept frames"));
    }
    drop(frame_sender);
    client
        .await
        .unwrap_or_else(|error| panic!("client should stop: {error}"));
    assert_eq!(waiting_frames(&root), 301, "nothing is dropped or lost");

    // Koalino starts later and takes the meeting over.
    let (server, buffer) = start_server(port, Some("host-secret")).await;
    let delivered = deliver_waiting_sessions(
        &root,
        &outbox_key(),
        &client_config(port, Some("host-secret")),
        Duration::ZERO,
    )
    .await;
    assert_eq!(delivered, 301);
    let events = collect_events(&buffer, 301).await;
    assert_eq!(events.len(), 301);
    assert!(matches!(events[0], BridgeEvent::MeetingStarted(_)));
    assert!(
        matches!(&events[300], BridgeEvent::Audio(audio) if audio.media_chunk().timestamp_ms() == 299_000),
        "in order"
    );
    assert_eq!(waiting_frames(&root), 0, "delivered frames are deleted");
    server.shutdown().await;
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn a_host_waits_while_someone_else_delivers() {
    let root = outbox_root();
    let (server, buffer) = start_server(0, Some("host-secret")).await;
    let other_deliverer = try_delivery_lock(&root);
    assert!(other_deliverer.is_some());
    let (frame_sender, frame_receiver) = mpsc::channel(16);
    let (status_sender, _status_receiver) = mpsc::unbounded_channel();
    let client = tokio::spawn(run_bridge_client(
        outbox_config(server.local_addr().port(), &root),
        frame_receiver,
        status_sender,
    ));
    for frame in [meeting_frame(), audio_frame(0)] {
        frame_sender
            .send(frame)
            .await
            .unwrap_or_else(|_| panic!("client should accept frames"));
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        buffer.next_event(Duration::ZERO).is_none(),
        "nothing interleaves"
    );
    assert_eq!(waiting_frames(&root), 2);

    drop(other_deliverer);
    let events = collect_events(&buffer, 2).await;
    assert_eq!(events.len(), 2);
    drop(frame_sender);
    client
        .await
        .unwrap_or_else(|error| panic!("client should stop: {error}"));
    server.shutdown().await;
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn koalino_is_started_once_when_there_is_something_to_deliver() {
    let root = outbox_root();
    let starts = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&starts);
    let (frame_sender, frame_receiver) = mpsc::channel(16);
    let (status_sender, mut status_receiver) = mpsc::unbounded_channel();
    let client = tokio::spawn(run_bridge_client(
        outbox_config(free_port(), &root).with_core_starter(Arc::new(move || {
            counted.fetch_add(1, Ordering::SeqCst);
        })),
        frame_receiver,
        status_sender,
    ));
    assert_eq!(
        status_receiver.recv().await,
        Some(BridgeStatus::Unavailable)
    );
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert_eq!(starts.load(Ordering::SeqCst), 0, "not without a meeting");
    for second in 0..3 {
        frame_sender
            .send(audio_frame(second * 1_000))
            .await
            .unwrap_or_else(|_| panic!("client should accept frames"));
        tokio::time::sleep(Duration::from_millis(120)).await;
    }
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    drop(frame_sender);
    client
        .await
        .unwrap_or_else(|error| panic!("client should stop: {error}"));
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn the_status_carries_koalinos_spoken_language() {
    use crate::adapters::network::teams_bridge::teams_websocket_server::SpokenLanguageSource;
    let server = TeamsWebSocketServer::start(
        TeamsBridgeServerConfig {
            port: 0,
            shared_secret: Some(BridgeSecret::new("host-secret".to_owned())),
            authentication_timeout: DEFAULT_AUTHENTICATION_TIMEOUT,
            spoken_language: Some(SpokenLanguageSource::new(|| Some("it"))),
        },
        BridgeBuffer::new(Duration::from_secs(10)),
    )
    .await
    .unwrap_or_else(|error| panic!("server should start: {error}"));
    let (frame_sender, frame_receiver) = mpsc::channel(16);
    let (status_sender, mut status_receiver) = mpsc::unbounded_channel();
    let client = tokio::spawn(run_bridge_client(
        client_config(server.local_addr().port(), Some("host-secret")),
        frame_receiver,
        status_sender,
    ));

    assert_eq!(
        status_receiver.recv().await,
        Some(BridgeStatus::Connected {
            language: Some("it".to_owned())
        })
    );

    drop(frame_sender);
    client
        .await
        .unwrap_or_else(|error| panic!("client should stop: {error}"));
    server.shutdown().await;
}
