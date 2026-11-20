//! Localhost WebSocket server that receives the Teams bot's Protobuf frames.
//!
//! The server binds `127.0.0.1:<NOTE_TAKER_TEAMS_BRIDGE_PORT>` only and refuses handshakes that
//! carry an `Origin` header, so web pages cannot reach it (cross-site WebSocket hijacking). When a
//! shared secret is configured, the bot's first frame must be the text `{"auth":"<secret>"}`; the
//! server answers `{"auth":"ok"}` or closes with 1008 (policy violation). Binary frames are decoded
//! by `message_decoder` and pushed into the `BridgeBuffer`, which never blocks, so a slow
//! transcription pipeline cannot stall the socket. One bot is active at a time: a newer
//! authenticated connection replaces the older one. Disconnects are logged, the active meeting is
//! kept, and the server keeps listening for the bot to reconnect. Secrets, meeting titles, e-mail
//! addresses, and media are never logged.

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{oneshot, watch},
    task::{JoinHandle, JoinSet},
};
use tokio_tungstenite::{
    WebSocketStream, accept_hdr_async_with_config,
    tungstenite::{
        Message,
        handshake::server::{ErrorResponse, Request, Response},
        http::{StatusCode, header::ORIGIN},
        protocol::{CloseFrame, WebSocketConfig, frame::coding::CloseCode},
    },
};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::adapters::network::teams_bridge::{
    bridge_buffer::BridgeBuffer,
    message_decoder::{BridgeEvent, decode_frame},
};

/// Default listening port (`NOTE_TAKER_TEAMS_BRIDGE_PORT`).
pub const DEFAULT_TEAMS_BRIDGE_PORT: u16 = 8765;
/// Default time a connection has to complete the handshake and authenticate.
pub const DEFAULT_AUTHENTICATION_TIMEOUT: Duration = Duration::from_secs(5);
/// Largest accepted WebSocket message (a 30 s chunk is ~960 KiB; slides may be larger).
pub const MAXIMUM_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
/// The acknowledgement sent after a successful authentication.
const AUTHENTICATION_ACCEPTED_REPLY: &str = r#"{"auth":"ok"}"#;

/// The shared secret; its `Debug` output is redacted so it cannot leak into logs.
#[derive(Clone)]
pub struct BridgeSecret(String);

impl BridgeSecret {
    /// Wraps the secret read from `NOTE_TAKER_TEAMS_BRIDGE_SECRET`.
    pub fn new(secret: String) -> Self {
        Self(secret)
    }

    /// The secret itself, for a local client of this bridge (the outbox replay, #135) and the
    /// outbox key. Never log it. Side effects: None.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Compares `candidate` with the secret in constant time.
    ///
    /// Inputs: The secret presented by the bot.
    /// Outputs: Whether it matches. Both sides are hashed first so the comparison time does not
    /// depend on the length or contents of either value.
    /// Side effects: None.
    fn matches(&self, candidate: &str) -> bool {
        let expected_digest = Sha256::digest(self.0.as_bytes());
        let candidate_digest = Sha256::digest(candidate.as_bytes());
        let differing_bits = expected_digest
            .iter()
            .zip(candidate_digest.iter())
            .fold(0_u8, |accumulated, (expected, actual)| {
                accumulated | (expected ^ actual)
            });
        differing_bits == 0
    }
}

impl std::fmt::Debug for BridgeSecret {
    /// Prints a placeholder instead of the secret.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("BridgeSecret(<redacted>)")
    }
}

/// Server settings, normally read from `NOTE_TAKER_TEAMS_BRIDGE_*` variables.
#[derive(Clone, Debug)]
pub struct TeamsBridgeServerConfig {
    /// Loopback port to listen on (0 picks an ephemeral port).
    pub port: u16,
    /// Secret the bot must present first; `None` accepts any local process.
    pub shared_secret: Option<BridgeSecret>,
    /// Time allowed for the handshake and, when required, the authentication frame.
    pub authentication_timeout: Duration,
    /// Koalino's fixed spoken language, named in the authentication acknowledgement (#141) so
    /// the browser extension can set Teams' language to it; `None` names none.
    pub spoken_language: Option<SpokenLanguageSource>,
}

/// Reads Koalino's default transcription language when a client authenticates (#141).
///
/// The function answers a Whisper code (`"it"`), or `None` when the language is Auto. It is read
/// at each connection, so a change in the app reaches the next one.
#[derive(Clone)]
pub struct SpokenLanguageSource(Arc<dyn Fn() -> Option<&'static str> + Send + Sync>);

impl SpokenLanguageSource {
    /// Wraps the reading function.
    ///
    /// Inputs: A function answering the current fixed language. Outputs: The source.
    /// Side effects: None (the function may read a settings file when called).
    pub fn new(read: impl Fn() -> Option<&'static str> + Send + Sync + 'static) -> Self {
        Self(Arc::new(read))
    }

    /// The current fixed language, if any. Side effects: Those of the function.
    fn current(&self) -> Option<&'static str> {
        (self.0)()
    }
}

impl std::fmt::Debug for SpokenLanguageSource {
    /// Names the type only; the language is read on demand.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SpokenLanguageSource")
    }
}

/// Errors starting the server.
#[derive(Debug, thiserror::Error)]
pub enum TeamsBridgeServerError {
    /// The loopback port could not be bound (for example, it is already in use).
    #[error("unable to listen for the Teams bot on 127.0.0.1:{port}: {source}")]
    Bind {
        /// The requested port.
        port: u16,
        /// The operating-system error.
        source: std::io::Error,
    },
}

/// The `{"auth":"<secret>"}` frame; deliberately not `Debug`.
#[derive(Deserialize)]
struct AuthenticationRequest {
    auth: String,
}

/// The currently active bot connection.
struct ActiveConnection {
    /// Identifies the connection so it only clears its own registration.
    connection_number: u64,
    /// Tells the connection it has been replaced.
    eviction_sender: oneshot::Sender<()>,
}

/// State shared by the accept loop and every connection.
struct BridgeState {
    buffer: BridgeBuffer,
    shared_secret: Option<BridgeSecret>,
    authentication_timeout: Duration,
    spoken_language: Option<SpokenLanguageSource>,
    /// The local meeting of the last metadata, kept across reconnects.
    active_meeting: Mutex<Option<Uuid>>,
    active_connection: Mutex<Option<ActiveConnection>>,
    next_connection_number: AtomicU64,
}

impl BridgeState {
    /// Returns the active meeting.
    fn active_meeting(&self) -> Option<Uuid> {
        *lock(&self.active_meeting)
    }

    /// Registers connection `connection_number` as active and evicts the previous one.
    fn activate(&self, connection_number: u64) -> oneshot::Receiver<()> {
        let (eviction_sender, eviction_receiver) = oneshot::channel();
        let previous_connection = lock(&self.active_connection).replace(ActiveConnection {
            connection_number,
            eviction_sender,
        });
        if let Some(previous_connection) = previous_connection {
            let _already_gone = previous_connection.eviction_sender.send(());
        }
        eviction_receiver
    }

    /// Clears the active registration if it still belongs to `connection_number`.
    fn deactivate(&self, connection_number: u64) {
        let mut active_connection = lock(&self.active_connection);
        if active_connection
            .as_ref()
            .is_some_and(|active| active.connection_number == connection_number)
        {
            active_connection.take();
        }
    }
}

/// Entry point for starting the server.
pub struct TeamsWebSocketServer;

impl TeamsWebSocketServer {
    /// Binds the loopback listener and starts accepting the bot in the background.
    ///
    /// Inputs: The server settings and the buffer that receives decoded events.
    /// Outputs: A handle for the address and shutdown, or `Bind` if the port is unavailable.
    /// Side effects: Listens on 127.0.0.1; spawns Tokio tasks; logs a warning when no secret is
    /// configured.
    pub async fn start(
        config: TeamsBridgeServerConfig,
        buffer: BridgeBuffer,
    ) -> Result<RunningTeamsBridge, TeamsBridgeServerError> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, config.port))
            .await
            .map_err(|source| TeamsBridgeServerError::Bind {
                port: config.port,
                source,
            })?;
        let local_addr = listener
            .local_addr()
            .map_err(|source| TeamsBridgeServerError::Bind {
                port: config.port,
                source,
            })?;
        if config.shared_secret.is_none() {
            warn!(
                %local_addr,
                "Teams bridge has no NOTE_TAKER_TEAMS_BRIDGE_SECRET; any local process can connect"
            );
        }
        info!(%local_addr, "Teams bridge listening");
        let state = Arc::new(BridgeState {
            buffer,
            shared_secret: config.shared_secret,
            authentication_timeout: config.authentication_timeout,
            spoken_language: config.spoken_language,
            active_meeting: Mutex::new(None),
            active_connection: Mutex::new(None),
            next_connection_number: AtomicU64::new(0),
        });
        let (shutdown_sender, shutdown_receiver) = watch::channel(false);
        let accept_task = tokio::spawn(accept_connections(listener, state, shutdown_receiver));
        Ok(RunningTeamsBridge {
            local_addr,
            shutdown_sender,
            accept_task,
        })
    }
}

/// A running server.
pub struct RunningTeamsBridge {
    local_addr: SocketAddr,
    shutdown_sender: watch::Sender<bool>,
    accept_task: JoinHandle<()>,
}

impl RunningTeamsBridge {
    /// Returns the bound loopback address.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Stops listening and closes every connection (close code 1001).
    ///
    /// Side effects: Ends the background tasks; the buffer is left open for the pump to drain.
    pub async fn shutdown(self) {
        let _no_receivers_left = self.shutdown_sender.send(true);
        if let Err(error) = self.accept_task.await {
            warn!(%error, "Teams bridge accept task ended abnormally");
        }
        info!(local_addr = %self.local_addr, "Teams bridge stopped");
    }
}

/// Accepts connections until shutdown, then waits for every connection to close.
async fn accept_connections(
    listener: TcpListener,
    state: Arc<BridgeState>,
    mut shutdown_receiver: watch::Receiver<bool>,
) {
    let mut connections = JoinSet::new();
    loop {
        while connections.try_join_next().is_some() {}
        tokio::select! {
            _ = shutdown_receiver.changed() => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, peer_address)) if peer_address.ip().is_loopback() => {
                    connections.spawn(handle_connection(
                        stream,
                        peer_address,
                        Arc::clone(&state),
                        shutdown_receiver.clone(),
                    ));
                }
                Ok((_stream, peer_address)) => {
                    warn!(%peer_address, "Teams bridge refused a non-loopback peer");
                }
                Err(error) => warn!(%error, "Teams bridge failed to accept a connection"),
            },
        }
    }
    while connections.join_next().await.is_some() {}
}

/// Runs one bot connection: handshake, optional authentication, then the frame loop.
///
/// Side effects: Pushes decoded events into the buffer; updates the active meeting; logs the
/// connection status.
async fn handle_connection(
    stream: TcpStream,
    peer_address: SocketAddr,
    state: Arc<BridgeState>,
    mut shutdown_receiver: watch::Receiver<bool>,
) {
    let websocket_config = WebSocketConfig::default()
        .max_message_size(Some(MAXIMUM_MESSAGE_BYTES))
        .max_frame_size(Some(MAXIMUM_MESSAGE_BYTES));
    let handshake =
        accept_hdr_async_with_config(stream, refuse_browser_origins, Some(websocket_config));
    let mut socket = match tokio::time::timeout(state.authentication_timeout, handshake).await {
        Ok(Ok(socket)) => socket,
        Ok(Err(error)) => {
            warn!(%peer_address, %error, "Teams bridge handshake rejected");
            return;
        }
        Err(_) => {
            warn!(%peer_address, "Teams bridge handshake timed out");
            return;
        }
    };
    if let Some(shared_secret) = &state.shared_secret
        && !authenticate(
            &mut socket,
            shared_secret,
            state.authentication_timeout,
            state.spoken_language.as_ref(),
        )
        .await
    {
        warn!(%peer_address, "Teams bot authentication rejected");
        close_socket(&mut socket, CloseCode::Policy, "authentication required").await;
        return;
    }

    let connection_number = state.next_connection_number.fetch_add(1, Ordering::Relaxed);
    let mut eviction_receiver = state.activate(connection_number);
    info!(%peer_address, connection_number, "Teams bot connected");
    loop {
        tokio::select! {
            _ = &mut eviction_receiver => {
                info!(%peer_address, connection_number, "Teams bot connection replaced by a newer one");
                close_socket(&mut socket, CloseCode::Normal, "replaced by a newer connection").await;
                break;
            }
            _ = shutdown_receiver.changed() => {
                close_socket(&mut socket, CloseCode::Away, "note taker is shutting down").await;
                break;
            }
            message = socket.next() => match message {
                Some(Ok(Message::Binary(frame))) => handle_binary_frame(&frame, &state),
                Some(Ok(Message::Text(text))) => {
                    if state.shared_secret.is_none() && parse_authentication(&text).is_some() {
                        let reply = authentication_reply(state.spoken_language.as_ref());
                        send_authentication_reply(&mut socket, &reply).await;
                    } else {
                        warn!(connection_number, "Teams bridge skipped an unexpected text frame");
                    }
                }
                Some(Ok(Message::Close(_))) | None => {
                    info!(%peer_address, connection_number, "Teams bot disconnected; waiting for reconnection");
                    break;
                }
                Some(Ok(_control_frame)) => {}
                Some(Err(error)) => {
                    info!(%peer_address, connection_number, %error, "Teams bot disconnected with an error; waiting for reconnection");
                    break;
                }
            },
        }
    }
    state.deactivate(connection_number);
}

/// Handshake callback refusing requests with an `Origin` header (sent by browsers, not bots).
#[allow(clippy::result_large_err)]
fn refuse_browser_origins(
    request: &Request,
    response: Response,
) -> Result<Response, ErrorResponse> {
    if request.headers().contains_key(ORIGIN) {
        let mut refusal = ErrorResponse::new(Some(
            "browser origins may not connect to the Teams bridge".to_owned(),
        ));
        *refusal.status_mut() = StatusCode::FORBIDDEN;
        return Err(refusal);
    }
    Ok(response)
}

/// Waits for the authentication frame and acknowledges a correct secret.
///
/// Outputs: Whether the bot authenticated within `authentication_timeout`.
/// Side effects: Reads one frame; sends `{"auth":"ok"}` on success, with `"language"` when
/// `spoken_language` names one (#141). Never logs the secret.
async fn authenticate<S>(
    socket: &mut WebSocketStream<S>,
    shared_secret: &BridgeSecret,
    authentication_timeout: Duration,
    spoken_language: Option<&SpokenLanguageSource>,
) -> bool
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let first_frame = tokio::time::timeout(authentication_timeout, socket.next()).await;
    let Ok(Some(Ok(Message::Text(text)))) = first_frame else {
        return false;
    };
    let authenticated =
        parse_authentication(&text).is_some_and(|request| shared_secret.matches(&request.auth));
    if authenticated {
        send_authentication_reply(socket, &authentication_reply(spoken_language)).await;
    }
    authenticated
}

/// The acknowledgement: `{"auth":"ok"}`, with `"language"` when Koalino has a fixed one (#141).
///
/// Side effects: Reads the language source.
fn authentication_reply(spoken_language: Option<&SpokenLanguageSource>) -> String {
    match spoken_language.and_then(SpokenLanguageSource::current) {
        Some(language) => serde_json::json!({"auth": "ok", "language": language}).to_string(),
        None => AUTHENTICATION_ACCEPTED_REPLY.to_owned(),
    }
}

/// Parses `{"auth":"..."}`, returning `None` for anything else.
fn parse_authentication(text: &str) -> Option<AuthenticationRequest> {
    serde_json::from_str(text).ok()
}

/// Sends the authentication acknowledgement, logging transport failures.
async fn send_authentication_reply<S>(socket: &mut WebSocketStream<S>, reply: &str)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    if let Err(error) = socket.send(Message::text(reply)).await {
        warn!(%error, "Teams bridge failed to acknowledge authentication");
    }
}

/// Decodes one binary frame and buffers it, skipping invalid frames with a warning.
///
/// Side effects: Updates the active meeting on metadata; pushes into the buffer (non-blocking).
fn handle_binary_frame(frame: &[u8], state: &BridgeState) {
    match decode_frame(frame, state.active_meeting()) {
        Ok(BridgeEvent::MeetingStarted(meeting)) => {
            *lock(&state.active_meeting) = Some(meeting.meeting_uuid());
            info!(
                meeting_id = meeting.meeting_id().as_str(),
                teams_meeting_id = meeting.teams_meeting_id(),
                participant_count = meeting.participants().len(),
                start_time_ms = meeting.start_time_ms(),
                "Teams meeting started"
            );
            state.buffer.push(BridgeEvent::MeetingStarted(meeting));
        }
        Ok(event) => {
            debug!("Teams bridge buffered a media frame");
            state.buffer.push(event);
        }
        Err(error) => warn!(%error, "Teams bridge frame skipped"),
    }
}

/// Sends a close frame, ignoring a peer that is already gone.
async fn close_socket<S>(socket: &mut WebSocketStream<S>, code: CloseCode, reason: &'static str)
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let close_frame = CloseFrame {
        code,
        reason: reason.into(),
    };
    if let Err(error) = socket.close(Some(close_frame)).await {
        debug!(%error, "Teams bridge close frame not delivered");
    }
}

/// Locks a mutex, recovering from poisoning (the guarded values are always consistent).
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
