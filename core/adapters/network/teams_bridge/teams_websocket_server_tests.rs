//! Specifies the localhost Teams bridge WebSocket server against a mock bot built on
//! `tokio-tungstenite` that sends Protobuf frames.

use std::{net::SocketAddr, time::Duration};

use futures::{SinkExt, StreamExt};
use protobuf::Message as _;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{
        Message, client::IntoClientRequest, http::HeaderValue, protocol::frame::coding::CloseCode,
    },
};

use crate::{
    adapters::network::teams_bridge::{
        bridge_buffer::BridgeBuffer,
        log_capture::capture_logs,
        message_decoder::{BridgeEvent, local_meeting_uuid},
        teams_websocket_server::{
            BridgeSecret, SpokenLanguageSource, TeamsBridgeServerConfig, TeamsWebSocketServer,
        },
    },
    protocol::teams_bridge::{
        AudioChunk, ExternalParticipant, MeetingMetadata, TeamsBridgeMessage, VideoFrame,
    },
};

/// The mock bot's connection type.
type BotSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// The Teams meeting the mock bot joins.
const TEAMS_MEETING: &str = "19:meeting_mock@thread.v2";
/// How long tests wait for the server to react.
const REACTION_TIMEOUT: Duration = Duration::from_secs(5);
/// How long tests wait to confirm that nothing reached the buffer.
const NOTHING_ARRIVES_WAIT: Duration = Duration::from_millis(300);

/// Returns a config on an ephemeral port with an optional secret and a short auth timeout.
fn test_config(shared_secret: Option<&str>) -> TeamsBridgeServerConfig {
    TeamsBridgeServerConfig {
        port: 0,
        shared_secret: shared_secret.map(|secret| BridgeSecret::new(secret.to_owned())),
        authentication_timeout: Duration::from_millis(300),
        spoken_language: None,
    }
}

/// Starts a server over a fresh 10 s buffer.
async fn start_server(
    shared_secret: Option<&str>,
) -> (
    crate::adapters::network::teams_bridge::teams_websocket_server::RunningTeamsBridge,
    BridgeBuffer,
) {
    let buffer = BridgeBuffer::new(Duration::from_secs(10));
    let server = TeamsWebSocketServer::start(test_config(shared_secret), buffer.clone())
        .await
        .unwrap_or_else(|error| panic!("server should start: {error}"));
    (server, buffer)
}

/// Connects the mock bot to `address`.
async fn connect_bot(address: SocketAddr) -> BotSocket {
    let (socket, _response) = connect_async(format!("ws://{address}"))
        .await
        .unwrap_or_else(|error| panic!("bot should connect: {error}"));
    socket
}

/// Serializes a protocol message.
fn encode(message: &TeamsBridgeMessage) -> Message {
    let frame = message
        .write_to_bytes()
        .unwrap_or_else(|error| panic!("message should encode: {error}"));
    Message::binary(frame)
}

/// Builds the meeting metadata frame for `TEAMS_MEETING`.
fn metadata_frame() -> Message {
    let mut participant = ExternalParticipant::new();
    participant.external_id = "teams-user-1".to_owned();
    participant.display_name = "Ada Lovelace".to_owned();
    let mut metadata = MeetingMetadata::new();
    metadata.meeting_id = TEAMS_MEETING.to_owned();
    metadata.meeting_title = "Mock sync".to_owned();
    metadata.start_time_ms = 1_793_000_000_000;
    metadata.participants.push(participant);
    let mut message = TeamsBridgeMessage::new();
    message.set_metadata(metadata);
    encode(&message)
}

/// Builds a 1 s audio frame from "teams-user-1" stamped `timestamp_ms`.
fn audio_frame(timestamp_ms: u64) -> Message {
    let mut participant = ExternalParticipant::new();
    participant.external_id = "teams-user-1".to_owned();
    participant.display_name = "Ada Lovelace".to_owned();
    let mut audio = AudioChunk::new();
    audio.participant = protobuf::MessageField::some(participant);
    audio.audio_data = vec![0; 32_000];
    audio.timestamp_ms = timestamp_ms;
    audio.sample_rate = 16_000;
    let mut message = TeamsBridgeMessage::new();
    message.set_audio(audio);
    encode(&message)
}

/// Builds a screen-share frame stamped `timestamp_ms`.
fn slide_frame(timestamp_ms: u64) -> Message {
    let mut video = VideoFrame::new();
    video.image_data = vec![0x89, b'P', b'N', b'G'];
    video.timestamp_ms = timestamp_ms;
    video.source = "screen_share".to_owned();
    let mut message = TeamsBridgeMessage::new();
    message.set_video(video);
    encode(&message)
}

/// Sends `message`, panicking on transport errors.
async fn send(socket: &mut BotSocket, message: Message) {
    socket
        .send(message)
        .await
        .unwrap_or_else(|error| panic!("bot should send: {error}"));
}

/// Waits (off the runtime thread) for the next buffered event.
async fn next_buffered(buffer: &BridgeBuffer, wait: Duration) -> Option<BridgeEvent> {
    let buffer = buffer.clone();
    tokio::task::spawn_blocking(move || buffer.next_event(wait))
        .await
        .unwrap_or_else(|error| panic!("buffer wait should not panic: {error}"))
}

/// Waits for the next buffered event, panicking if none arrives.
async fn expect_buffered(buffer: &BridgeBuffer) -> BridgeEvent {
    next_buffered(buffer, REACTION_TIMEOUT)
        .await
        .unwrap_or_else(|| panic!("an event should reach the buffer"))
}

/// Reads until the server closes the socket and returns the close code, if any.
async fn expect_close(socket: &mut BotSocket) -> Option<CloseCode> {
    let read_until_close = async {
        while let Some(message) = socket.next().await {
            match message {
                Ok(Message::Close(frame)) => return frame.map(|frame| frame.code),
                Ok(_) => {}
                Err(_) => return None,
            }
        }
        None
    };
    tokio::time::timeout(REACTION_TIMEOUT, read_until_close)
        .await
        .unwrap_or_else(|_| panic!("the server should close the connection"))
}

/// Polls `predicate` over the captured logs until it holds or the timeout elapses.
async fn wait_for_log(
    captured_logs: &crate::adapters::network::teams_bridge::log_capture::CapturedLogs,
    needle: &str,
) {
    let deadline = tokio::time::Instant::now() + REACTION_TIMEOUT;
    while !captured_logs.text().contains(needle) {
        assert!(
            tokio::time::Instant::now() < deadline,
            "expected log containing {needle:?}, got: {}",
            captured_logs.text()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Verifies a connection is accepted and an AudioChunk becomes an `ExternalMediaChunk`.
#[tokio::test]
async fn audio_chunk_is_converted_into_an_external_media_chunk() {
    let (server, buffer) = start_server(None).await;
    let mut bot = connect_bot(server.local_addr()).await;
    send(&mut bot, metadata_frame()).await;
    send(&mut bot, audio_frame(4_000)).await;

    assert!(matches!(
        expect_buffered(&buffer).await,
        BridgeEvent::MeetingStarted(_)
    ));
    match expect_buffered(&buffer).await {
        BridgeEvent::Audio(audio) => {
            let chunk = audio.media_chunk();
            assert_eq!(chunk.external_participant().external_id(), "teams-user-1");
            assert_eq!(chunk.timestamp_ms(), 4_000);
            assert_eq!(audio.duration_ms(), 1_000);
            assert_eq!(audio.meeting_uuid(), local_meeting_uuid(TEAMS_MEETING));
        }
        other => panic!("expected audio, got {other:?}"),
    }
    server.shutdown().await;
}

/// Verifies a screen-share VideoFrame becomes a `SlideCapture` of the meeting.
#[tokio::test]
async fn video_frame_is_converted_into_a_slide_capture() {
    let (server, buffer) = start_server(None).await;
    let mut bot = connect_bot(server.local_addr()).await;
    send(&mut bot, metadata_frame()).await;
    send(&mut bot, slide_frame(9_000)).await;

    expect_buffered(&buffer).await;
    match expect_buffered(&buffer).await {
        BridgeEvent::Slide(slide) => {
            assert_eq!(slide.meeting_id(), local_meeting_uuid(TEAMS_MEETING));
            assert_eq!(slide.timestamp_ms(), 9_000);
            assert_eq!(slide.image_bytes(), &[0x89, b'P', b'N', b'G']);
        }
        other => panic!("expected a slide, got {other:?}"),
    }
    server.shutdown().await;
}

/// Verifies MeetingMetadata logs the meeting start without logging the title.
#[tokio::test]
async fn meeting_metadata_logs_the_meeting_start() {
    let captured_logs = capture_logs();
    let (server, buffer) = start_server(None).await;
    let mut bot = connect_bot(server.local_addr()).await;
    send(&mut bot, metadata_frame()).await;

    match expect_buffered(&buffer).await {
        BridgeEvent::MeetingStarted(meeting) => {
            assert_eq!(meeting.teams_meeting_id(), TEAMS_MEETING);
            assert_eq!(meeting.participants().len(), 1);
        }
        other => panic!("expected a meeting start, got {other:?}"),
    }
    wait_for_log(&captured_logs, "Teams meeting started").await;
    assert!(
        !captured_logs.text().contains("Mock sync"),
        "titles must not be logged"
    );
    server.shutdown().await;
}

/// Verifies a connection that sends media before authenticating is closed with 1008.
#[tokio::test]
async fn unauthenticated_connection_is_rejected_when_a_secret_is_configured() {
    let (server, buffer) = start_server(Some("s3cret-token")).await;
    let mut bot = connect_bot(server.local_addr()).await;
    send(&mut bot, metadata_frame()).await;

    assert_eq!(expect_close(&mut bot).await, Some(CloseCode::Policy));
    assert!(next_buffered(&buffer, NOTHING_ARRIVES_WAIT).await.is_none());
    server.shutdown().await;
}

/// Verifies a wrong secret is rejected with 1008 and never logged.
#[tokio::test]
async fn wrong_secret_is_rejected_and_not_logged() {
    let captured_logs = capture_logs();
    let (server, _buffer) = start_server(Some("s3cret-token")).await;
    let mut bot = connect_bot(server.local_addr()).await;
    send(&mut bot, Message::text(r#"{"auth":"guessed-token"}"#)).await;

    assert_eq!(expect_close(&mut bot).await, Some(CloseCode::Policy));
    wait_for_log(&captured_logs, "rejected").await;
    let logs = captured_logs.text();
    assert!(!logs.contains("guessed-token") && !logs.contains("s3cret-token"));
    server.shutdown().await;
}

/// Verifies a silent connection is closed with 1008 once the auth timeout elapses.
#[tokio::test]
async fn silent_connection_is_rejected_after_the_authentication_timeout() {
    let (server, _buffer) = start_server(Some("s3cret-token")).await;
    let mut bot = connect_bot(server.local_addr()).await;
    assert_eq!(expect_close(&mut bot).await, Some(CloseCode::Policy));
    server.shutdown().await;
}

/// Verifies the correct secret is acknowledged and media then flows.
#[tokio::test]
async fn correct_secret_is_acknowledged_and_media_flows() {
    let (server, buffer) = start_server(Some("s3cret-token")).await;
    let mut bot = connect_bot(server.local_addr()).await;
    send(&mut bot, Message::text(r#"{"auth":"s3cret-token"}"#)).await;

    let acknowledgement = tokio::time::timeout(REACTION_TIMEOUT, bot.next())
        .await
        .unwrap_or_else(|_| panic!("the server should acknowledge"));
    match acknowledgement {
        Some(Ok(Message::Text(text))) => assert_eq!(text.as_str(), r#"{"auth":"ok"}"#),
        other => panic!("expected the auth acknowledgement, got {other:?}"),
    }
    send(&mut bot, metadata_frame()).await;
    assert!(matches!(
        expect_buffered(&buffer).await,
        BridgeEvent::MeetingStarted(_)
    ));
    server.shutdown().await;
}

/// Verifies a disconnect is logged and a reconnecting bot resumes the same meeting.
#[tokio::test]
async fn disconnect_is_logged_and_the_bot_can_reconnect() {
    let captured_logs = capture_logs();
    let (server, buffer) = start_server(None).await;
    let mut bot = connect_bot(server.local_addr()).await;
    send(&mut bot, metadata_frame()).await;
    expect_buffered(&buffer).await;
    bot.close(None)
        .await
        .unwrap_or_else(|error| panic!("bot should close: {error}"));
    drop(bot);
    wait_for_log(&captured_logs, "Teams bot disconnected").await;

    let mut reconnected_bot = connect_bot(server.local_addr()).await;
    send(&mut reconnected_bot, audio_frame(60_000)).await;
    match expect_buffered(&buffer).await {
        BridgeEvent::Audio(audio) => {
            assert_eq!(audio.meeting_uuid(), local_meeting_uuid(TEAMS_MEETING));
        }
        other => panic!("expected audio after reconnecting, got {other:?}"),
    }
    server.shutdown().await;
}

/// Verifies a newer connection replaces the older one, which is closed.
#[tokio::test]
async fn newer_connection_replaces_the_older_one() {
    let (server, buffer) = start_server(None).await;
    let mut first_bot = connect_bot(server.local_addr()).await;
    send(&mut first_bot, metadata_frame()).await;
    expect_buffered(&buffer).await;

    let mut second_bot = connect_bot(server.local_addr()).await;
    assert!(expect_close(&mut first_bot).await.is_some());
    send(&mut second_bot, audio_frame(1_000)).await;
    assert!(matches!(
        expect_buffered(&buffer).await,
        BridgeEvent::Audio(_)
    ));
    server.shutdown().await;
}

/// Verifies malformed frames and audio before metadata are skipped with warnings.
#[tokio::test]
async fn invalid_frames_are_skipped_and_the_connection_stays_open() {
    let captured_logs = capture_logs();
    let (server, buffer) = start_server(None).await;
    let mut bot = connect_bot(server.local_addr()).await;
    send(&mut bot, audio_frame(0)).await;
    send(&mut bot, Message::binary(vec![0xFF, 0xFF, 0xFF])).await;
    send(&mut bot, metadata_frame()).await;

    assert!(matches!(
        expect_buffered(&buffer).await,
        BridgeEvent::MeetingStarted(_)
    ));
    wait_for_log(&captured_logs, "MeetingMetadata").await;
    wait_for_log(&captured_logs, "skipped").await;
    server.shutdown().await;
}

/// Verifies browser-originated handshakes (with an Origin header) are refused.
#[tokio::test]
async fn handshakes_with_an_origin_header_are_refused() {
    let (server, _buffer) = start_server(None).await;
    let mut request = format!("ws://{}", server.local_addr())
        .into_client_request()
        .unwrap_or_else(|error| panic!("request should build: {error}"));
    request
        .headers_mut()
        .insert("Origin", HeaderValue::from_static("http://evil.example"));
    assert!(connect_async(request).await.is_err());
    server.shutdown().await;
}

/// Verifies the server only listens on the loopback interface.
#[tokio::test]
async fn server_binds_to_loopback_only() {
    let (server, _buffer) = start_server(None).await;
    assert!(server.local_addr().ip().is_loopback());
    server.shutdown().await;
}

/// Verifies the acknowledgement names Koalino's fixed language (#141), read when the client
/// authenticates, so a change in the app reaches the next connection.
#[tokio::test]
async fn the_acknowledgement_names_koalinos_spoken_language() {
    let mut config = test_config(Some("s3cret-token"));
    config.spoken_language = Some(SpokenLanguageSource::new(|| Some("it")));
    let server = TeamsWebSocketServer::start(config, BridgeBuffer::new(Duration::from_secs(10)))
        .await
        .unwrap_or_else(|error| panic!("server should start: {error}"));
    let mut bot = connect_bot(server.local_addr()).await;
    send(&mut bot, Message::text(r#"{"auth":"s3cret-token"}"#)).await;

    let acknowledgement = tokio::time::timeout(REACTION_TIMEOUT, bot.next())
        .await
        .unwrap_or_else(|_| panic!("the server should acknowledge"));
    match acknowledgement {
        Some(Ok(Message::Text(text))) => {
            assert_eq!(text.as_str(), r#"{"auth":"ok","language":"it"}"#)
        }
        other => panic!("expected the auth acknowledgement, got {other:?}"),
    }
    server.shutdown().await;
}
