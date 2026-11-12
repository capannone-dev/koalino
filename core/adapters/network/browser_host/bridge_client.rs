//! The host's WebSocket connection to the local Teams bridge (`ws://127.0.0.1:<port>`).
//!
//! Frames from the extension go into a `FrameQueue` and are sent in order. When the core is not
//! running (or restarts), the client reports `Unavailable`, keeps queueing, and retries every
//! `reconnect_interval`; queued audio beyond `queue_audio_ms` is dropped oldest first, while
//! meeting starts, slides, documents and meeting ends are always kept. With a shared secret the
//! client authenticates first (`{"auth":"<secret>"}`, answered `{"auth":"ok"}`). The handshake
//! sends no `Origin` header, which the bridge requires. The secret and frame contents are never
//! logged.

use std::{collections::VecDeque, time::Duration};

use futures::{SinkExt, StreamExt};
use tokio::{net::TcpStream, sync::mpsc};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use zeroize::Zeroizing;

use super::extension_message::BridgeFrame;

/// Default audio held while the core is unreachable (two minutes of both tracks).
pub const DEFAULT_QUEUE_AUDIO_MS: u64 = 240_000;
/// Default wait between connection attempts.
const DEFAULT_RECONNECT_INTERVAL: Duration = Duration::from_secs(2);
/// How long the server may take to answer the authentication.
const AUTHENTICATION_REPLY_TIMEOUT: Duration = Duration::from_secs(5);
/// How long the client keeps trying to deliver queued frames after the extension disconnected.
const FINAL_DELIVERY_TIMEOUT: Duration = Duration::from_secs(10);

/// Where and how to connect.
#[derive(Clone)]
pub struct BridgeClientConfig {
    port: u16,
    secret: Option<Zeroizing<String>>,
    reconnect_interval: Duration,
    queue_audio_ms: u64,
}

impl BridgeClientConfig {
    /// Creates a configuration with the default retry interval and queue.
    ///
    /// Inputs: The bridge's loopback port and its shared secret, if it requires one.
    /// Outputs: The configuration. Side effects: None.
    pub fn new(port: u16, secret: Option<String>) -> Self {
        Self {
            port,
            secret: secret.map(Zeroizing::new),
            reconnect_interval: DEFAULT_RECONNECT_INTERVAL,
            queue_audio_ms: DEFAULT_QUEUE_AUDIO_MS,
        }
    }

    /// Overrides the wait between connection attempts. Side effects: None.
    pub fn with_reconnect_interval(mut self, reconnect_interval: Duration) -> Self {
        self.reconnect_interval = reconnect_interval;
        self
    }
}

impl std::fmt::Debug for BridgeClientConfig {
    /// Prints the configuration without the secret.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BridgeClientConfig")
            .field("port", &self.port)
            .field("authenticates", &self.secret.is_some())
            .finish()
    }
}

/// Whether the core can currently be reached.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BridgeStatus {
    /// Connected (and authenticated).
    Connected,
    /// Not running, refusing, or the connection dropped; frames are queued.
    Unavailable,
}

/// Frames waiting to be sent, bounded by their audio length.
pub struct FrameQueue {
    frames: VecDeque<BridgeFrame>,
    queued_audio_ms: u64,
    audio_capacity_ms: u64,
    dropped_audio_frames: usize,
}

impl FrameQueue {
    /// Creates an empty queue holding at most `audio_capacity_ms` of audio. Side effects: None.
    pub fn new(audio_capacity_ms: u64) -> Self {
        Self {
            frames: VecDeque::new(),
            queued_audio_ms: 0,
            audio_capacity_ms,
            dropped_audio_frames: 0,
        }
    }

    /// Appends a frame, dropping the oldest audio while the queue holds too much.
    ///
    /// Inputs: The frame. Outputs: None. Side effects: May drop older audio frames.
    pub fn push(&mut self, frame: BridgeFrame) {
        self.queued_audio_ms += frame.audio_duration_ms().unwrap_or(0);
        self.frames.push_back(frame);
        while self.queued_audio_ms > self.audio_capacity_ms {
            let Some(oldest_audio) = self
                .frames
                .iter()
                .position(|frame| frame.audio_duration_ms().is_some())
            else {
                break;
            };
            if let Some(dropped) = self.frames.remove(oldest_audio) {
                self.queued_audio_ms -= dropped.audio_duration_ms().unwrap_or(0);
                self.dropped_audio_frames += 1;
            }
        }
    }

    /// Takes the oldest frame. Side effects: Removes it.
    pub fn pop(&mut self) -> Option<BridgeFrame> {
        let frame = self.frames.pop_front()?;
        self.queued_audio_ms -= frame.audio_duration_ms().unwrap_or(0);
        Some(frame)
    }

    /// Puts a frame that could not be sent back at the front. Side effects: Updates the queue.
    fn unpop(&mut self, frame: BridgeFrame) {
        self.queued_audio_ms += frame.audio_duration_ms().unwrap_or(0);
        self.frames.push_front(frame);
    }

    /// The number of queued frames. Side effects: None.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Whether nothing is queued. Side effects: None.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Audio frames dropped so far. Side effects: None.
    pub fn dropped_audio_frames(&self) -> usize {
        self.dropped_audio_frames
    }
}

/// The open connection.
type BridgeSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Sends the extension's frames to the bridge until the extension disconnects.
///
/// Inputs: The configuration, the frames (the channel closes when the browser closes the port),
/// and where to report status changes.
/// Outputs: None, once the frame channel is closed and the queue was delivered (or
/// `FINAL_DELIVERY_TIMEOUT` passed).
/// Side effects: Connects to 127.0.0.1, retries while the core is down, logs status changes and
/// dropped-audio counts (never contents or the secret).
pub async fn run_bridge_client(
    config: BridgeClientConfig,
    mut frames: mpsc::Receiver<BridgeFrame>,
    status: mpsc::UnboundedSender<BridgeStatus>,
) {
    let mut queue = FrameQueue::new(config.queue_audio_ms);
    let mut socket: Option<BridgeSocket> = None;
    let mut reported: Option<BridgeStatus> = None;
    let mut report = |new_status: BridgeStatus| {
        if reported != Some(new_status) {
            reported = Some(new_status);
            tracing::info!(status = ?new_status, "Teams bridge status");
            let _ = status.send(new_status);
        }
    };
    let mut extension_open = true;
    let mut final_deadline: Option<tokio::time::Instant> = None;
    loop {
        if socket.is_none() {
            match connect(&config).await {
                Ok(connected) => {
                    socket = Some(connected);
                    report(BridgeStatus::Connected);
                }
                Err(reason) => {
                    tracing::debug!(reason, "Teams bridge not reachable");
                    report(BridgeStatus::Unavailable);
                }
            }
        }
        if let Some(open_socket) = socket.as_mut() {
            while let Some(frame) = queue.pop() {
                if open_socket
                    .send(Message::binary(frame.bytes().to_vec()))
                    .await
                    .is_err()
                {
                    queue.unpop(frame);
                    socket = None;
                    report(BridgeStatus::Unavailable);
                    break;
                }
            }
        }
        if !extension_open {
            let deadline = *final_deadline
                .get_or_insert_with(|| tokio::time::Instant::now() + FINAL_DELIVERY_TIMEOUT);
            if queue.is_empty() || tokio::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(config.reconnect_interval).await;
            continue;
        }
        if socket.is_some() {
            match frames.recv().await {
                Some(frame) => queue.push(frame),
                None => extension_open = false,
            }
        } else {
            tokio::select! {
                received = frames.recv() => match received {
                    Some(frame) => queue.push(frame),
                    None => extension_open = false,
                },
                () = tokio::time::sleep(config.reconnect_interval) => {}
            }
        }
    }
    if let Some(mut open_socket) = socket {
        let _ = open_socket.close(None).await;
    }
    if queue.dropped_audio_frames() > 0 || !queue.is_empty() {
        tracing::warn!(
            dropped_audio_frames = queue.dropped_audio_frames(),
            undelivered_frames = queue.len(),
            "Some meeting data did not reach Koalino"
        );
    }
}

/// Opens and, when configured, authenticates one connection.
///
/// Outputs: The socket, or a privacy-safe reason.
/// Side effects: Connects to 127.0.0.1 and sends the secret to the bridge only.
async fn connect(config: &BridgeClientConfig) -> Result<BridgeSocket, &'static str> {
    let request = format!("ws://127.0.0.1:{}", config.port)
        .into_client_request()
        .map_err(|_| "invalid bridge address")?;
    let (mut socket, _response) = connect_async(request)
        .await
        .map_err(|_| "connection refused")?;
    if let Some(secret) = &config.secret {
        let authentication =
            Zeroizing::new(serde_json::json!({ "auth": secret.as_str() }).to_string());
        socket
            .send(Message::text(authentication.as_str()))
            .await
            .map_err(|_| "authentication could not be sent")?;
        let reply = tokio::time::timeout(AUTHENTICATION_REPLY_TIMEOUT, socket.next())
            .await
            .map_err(|_| "authentication timed out")?;
        match reply {
            Some(Ok(Message::Text(text))) if text.as_str().contains("\"ok\"") => {}
            _ => return Err("authentication refused"),
        }
    }
    Ok(socket)
}
