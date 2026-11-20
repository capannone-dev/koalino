//! The host's WebSocket connection to the local Teams bridge (`ws://127.0.0.1:<port>`).
//!
//! Frames from the extension are sent in order. When the core is not running (or restarts), the
//! client reports `Unavailable`, keeps the frames, and retries every `reconnect_interval`.
//!
//! With an outbox (#135, whenever there is a bridge secret to seal with), undelivered frames are
//! kept sealed on disk, all of them, and stay there for Koalino when the browser disconnects
//! first; frames are sent only while holding the outbox's delivery lock, so they never interleave
//! with another host's or with Koalino replaying an earlier session. The client also asks once
//! per session for Koalino to be started (`with_core_starter`) when it has something to deliver
//! and can't connect. Without an outbox, frames wait in memory: audio beyond `queue_audio_ms` is
//! dropped oldest first, while meeting starts, slides, documents and meeting ends are kept. With a shared secret the
//! client authenticates first (`{"auth":"<secret>"}`, answered `{"auth":"ok"}`). The handshake
//! sends no `Origin` header, which the bridge requires. The secret and frame contents are never
//! logged.

use std::{collections::VecDeque, path::PathBuf, sync::Arc, time::Duration};

use futures::{SinkExt, StreamExt};
use tokio::{net::TcpStream, sync::mpsc};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use zeroize::Zeroizing;

use super::{
    extension_message::{BridgeFrame, MeetingEdge},
    outbox::{
        DEFAULT_OUTBOX_CAP_BYTES, ExclusiveLock, OutboxKey, OutboxSession, STALE_SESSION_AFTER,
        try_delivery_lock,
    },
    outbox_delivery::{left_behind_sessions, send_sessions},
};

/// Default audio held while the core is unreachable (two minutes of both tracks).
pub const DEFAULT_QUEUE_AUDIO_MS: u64 = 240_000;
/// Default wait between connection attempts.
const DEFAULT_RECONNECT_INTERVAL: Duration = Duration::from_secs(2);
/// How long the server may take to answer the authentication.
const AUTHENTICATION_REPLY_TIMEOUT: Duration = Duration::from_secs(5);
/// How long the client keeps trying to deliver queued frames after the extension disconnected.
const FINAL_DELIVERY_TIMEOUT: Duration = Duration::from_secs(10);

/// Where undelivered frames are kept on disk (#135).
#[derive(Clone, Debug)]
pub struct OutboxSettings {
    root: PathBuf,
    key: OutboxKey,
    cap_bytes: u64,
}

impl OutboxSettings {
    /// Keeps frames under `root`, sealed with `key`, up to the default cap.
    /// Side effects: None.
    pub fn new(root: PathBuf, key: OutboxKey) -> Self {
        Self {
            root,
            key,
            cap_bytes: DEFAULT_OUTBOX_CAP_BYTES,
        }
    }
}

/// Starts Koalino; called at most once per session.
pub type CoreStarter = Arc<dyn Fn() + Send + Sync>;

/// Where and how to connect.
#[derive(Clone)]
pub struct BridgeClientConfig {
    port: u16,
    secret: Option<Zeroizing<String>>,
    reconnect_interval: Duration,
    queue_audio_ms: u64,
    outbox: Option<OutboxSettings>,
    core_starter: Option<CoreStarter>,
    final_delivery_timeout: Duration,
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
            outbox: None,
            core_starter: None,
            final_delivery_timeout: FINAL_DELIVERY_TIMEOUT,
        }
    }

    /// Keeps undelivered frames on disk instead of in memory (#135). Side effects: None.
    pub fn with_outbox(mut self, outbox: OutboxSettings) -> Self {
        self.outbox = Some(outbox);
        self
    }

    /// Starts Koalino when there is something to deliver and it can't be reached (#135).
    /// Side effects: None.
    pub fn with_core_starter(mut self, core_starter: CoreStarter) -> Self {
        self.core_starter = Some(core_starter);
        self
    }

    /// Overrides how long delivery continues after the extension disconnected.
    /// Side effects: None.
    pub fn with_final_delivery_timeout(mut self, timeout: Duration) -> Self {
        self.final_delivery_timeout = timeout;
        self
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
            .field("outbox", &self.outbox.is_some())
            .field("starts_core", &self.core_starter.is_some())
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

    /// The oldest frame's bytes. Side effects: None.
    fn front_bytes(&self) -> Option<Vec<u8>> {
        self.frames.front().map(|frame| frame.bytes().to_vec())
    }

    /// Takes the oldest frame. Side effects: Removes it.
    pub fn pop(&mut self) -> Option<BridgeFrame> {
        let frame = self.frames.pop_front()?;
        self.queued_audio_ms -= frame.audio_duration_ms().unwrap_or(0);
        Some(frame)
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
pub(crate) type BridgeSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Frames waiting for delivery: in memory, or sealed on disk (#135).
enum PendingFrames {
    Memory(FrameQueue),
    Disk(OutboxSession),
}

impl PendingFrames {
    /// Keeps a frame. Side effects: May write a sealed file or drop old audio.
    fn push(&mut self, frame: BridgeFrame) {
        match self {
            Self::Memory(queue) => queue.push(frame),
            Self::Disk(session) => {
                session.push(frame.bytes(), frame.audio_duration_ms().is_some());
            }
        }
    }

    /// The oldest frame's bytes. Side effects: May read a file.
    fn front(&mut self) -> Option<Vec<u8>> {
        match self {
            Self::Memory(queue) => queue.front_bytes(),
            Self::Disk(session) => session.front(),
        }
    }

    /// Forgets the oldest frame after delivery. Side effects: May delete a file.
    fn remove_front(&mut self) {
        match self {
            Self::Memory(queue) => {
                let _ = queue.pop();
            }
            Self::Disk(session) => session.remove_front(),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::Memory(queue) => queue.is_empty(),
            Self::Disk(session) => session.is_empty(),
        }
    }

    /// Keeps a disk session from being claimed. Side effects: Rewrites its heartbeat.
    fn heartbeat(&self) {
        if let Self::Disk(session) = self {
            session.heartbeat();
        }
    }

    /// Ends the session, logging what didn't reach Koalino. Side effects: See `finish`.
    fn finish(self) {
        match self {
            Self::Memory(queue) => {
                if queue.dropped_audio_frames() > 0 || !queue.is_empty() {
                    tracing::warn!(
                        dropped_audio_frames = queue.dropped_audio_frames(),
                        undelivered_frames = queue.len(),
                        "Some meeting data did not reach Koalino"
                    );
                }
            }
            Self::Disk(session) => {
                if !session.is_empty() {
                    tracing::info!(
                        waiting_frames = session.len(),
                        "Meeting data kept for Koalino to take when it runs"
                    );
                }
                if session.dropped_audio_frames() > 0 {
                    tracing::warn!(
                        dropped_audio_frames = session.dropped_audio_frames(),
                        "The outbox was full; some audio was dropped"
                    );
                }
                session.finish();
            }
        }
    }
}

/// Opens the session's pending store: the outbox when configured and usable, memory otherwise.
fn pending_frames(config: &BridgeClientConfig) -> PendingFrames {
    if let Some(outbox) = &config.outbox {
        match OutboxSession::create(&outbox.root, outbox.key.clone(), outbox.cap_bytes) {
            Ok(session) => return PendingFrames::Disk(session),
            Err(error) => {
                tracing::warn!(error = %error, "Outbox unavailable; frames wait in memory");
            }
        }
    }
    PendingFrames::Memory(FrameQueue::new(config.queue_audio_ms))
}

/// Sends the extension's frames to the bridge until the extension disconnects.
///
/// Inputs: The configuration, the frames (the channel closes when the browser closes the port),
/// and where to report status changes.
/// Outputs: None, once the frame channel is closed and the frames were delivered (or the final
/// delivery timeout passed: frames in the outbox then stay there for Koalino).
/// Side effects: Connects to 127.0.0.1, retries while the core is down, may start Koalino once,
/// writes and deletes outbox files, logs status changes and counts (never contents or the
/// secret).
pub async fn run_bridge_client(
    config: BridgeClientConfig,
    mut frames: mpsc::Receiver<BridgeFrame>,
    status: mpsc::UnboundedSender<BridgeStatus>,
) {
    let mut pending = pending_frames(&config);
    let mut socket: Option<BridgeSocket> = None;
    // Held while delivering from a disk outbox, so no other stream interleaves.
    let mut delivery_lock: Option<ExclusiveLock> = None;
    let mut reported: Option<BridgeStatus> = None;
    let mut report = |new_status: BridgeStatus| {
        if reported != Some(new_status) {
            reported = Some(new_status);
            tracing::info!(status = ?new_status, "Teams bridge status");
            let _ = status.send(new_status);
        }
    };
    let mut asked_to_start_core = false;
    // Whether the extension's stream is inside a meeting: the lock is kept until it ends.
    let mut in_meeting = false;
    let mut extension_open = true;
    let mut final_deadline: Option<tokio::time::Instant> = None;
    loop {
        pending.heartbeat();
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
        if socket.is_none()
            && !asked_to_start_core
            && !pending.is_empty()
            && let Some(start_core) = &config.core_starter
        {
            asked_to_start_core = true;
            start_core();
        }
        if delivery_lock.is_none()
            && !pending.is_empty()
            && let (Some(open_socket), PendingFrames::Disk(_), Some(outbox)) =
                (socket.as_mut(), &pending, &config.outbox)
        {
            delivery_lock = try_delivery_lock(&outbox.root);
            if delivery_lock.is_some() {
                // Meetings an earlier host left behind go first, so Koalino gets them in order.
                let earlier = left_behind_sessions(&outbox.root, STALE_SESSION_AFTER);
                if !earlier.is_empty() {
                    let (_, finished) = send_sessions(open_socket, earlier, &outbox.key).await;
                    if !finished {
                        socket = None;
                        report(BridgeStatus::Unavailable);
                    }
                }
            }
        }
        let may_send = socket.is_some()
            && (matches!(pending, PendingFrames::Memory(_)) || delivery_lock.is_some());
        if may_send && let Some(open_socket) = socket.as_mut() {
            while let Some(frame_bytes) = pending.front() {
                if open_socket
                    .send(Message::binary(frame_bytes))
                    .await
                    .is_err()
                {
                    socket = None;
                    report(BridgeStatus::Unavailable);
                    break;
                }
                pending.remove_front();
            }
        }
        // Between meetings with nothing waiting, let others deliver (a second browser, or Koalino
        // replaying a meeting left behind).
        if !in_meeting && pending.is_empty() {
            delivery_lock = None;
        }
        if !extension_open {
            let deadline = *final_deadline
                .get_or_insert_with(|| tokio::time::Instant::now() + config.final_delivery_timeout);
            if pending.is_empty() || tokio::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(config.reconnect_interval).await;
            continue;
        }
        tokio::select! {
            received = frames.recv() => match received {
                Some(frame) => {
                    match frame.meeting_edge() {
                        Some(MeetingEdge::Started) => in_meeting = true,
                        Some(MeetingEdge::Ended) => in_meeting = false,
                        None => {}
                    }
                    let may_send = may_send && (delivery_lock.is_some() || matches!(pending, PendingFrames::Memory(_)));
                    let sent_directly = match socket.as_mut() {
                        Some(open_socket) if may_send && pending.is_empty() => {
                            if open_socket.send(Message::binary(frame.bytes().to_vec())).await.is_ok() {
                                true
                            } else {
                                socket = None;
                                report(BridgeStatus::Unavailable);
                                false
                            }
                        }
                        _ => false,
                    };
                    if !sent_directly {
                        pending.push(frame);
                    }
                }
                None => extension_open = false,
            },
            () = tokio::time::sleep(config.reconnect_interval) => {}
        }
    }
    if let Some(mut open_socket) = socket {
        let _ = open_socket.close(None).await;
    }
    drop(delivery_lock);
    pending.finish();
}

/// Opens and, when configured, authenticates one connection.
///
/// Outputs: The socket, or a privacy-safe reason.
/// Side effects: Connects to 127.0.0.1 and sends the secret to the bridge only.
pub(crate) async fn connect(config: &BridgeClientConfig) -> Result<BridgeSocket, &'static str> {
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
