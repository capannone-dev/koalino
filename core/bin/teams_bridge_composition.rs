//! Composes the Teams bridge for the daemon and, since #123, the desktop app (the Teams bot, or
//! the browser extension through its native messaging host, #102): configuration, the localhost
//! WebSocket server, the drop-oldest buffer, and the meeting pump that feeds `MeetingSession`.
//!
//! The daemon hands the bridge its ready pipeline. The desktop app hands it a builder instead
//! (`BridgePipeline::on_first_meeting`), so an app that never records from the browser loads no
//! transcription model. The app's bridge is on unless `NOTE_TAKER_TEAMS_BRIDGE_ENABLED=false`
//! and never runs without a secret (`app_bridge_config`).
//!
//! ```text
//! Teams bot ──WebSocket (127.0.0.1)──▶ TeamsWebSocketServer ──decode──▶ BridgeBuffer
//!                                                                          │ (pump thread)
//!            MeetingStarted ─▶ RecordingManager::begin_external_meeting    ▼
//!            Audio ─▶ ChunkAudioStore + RecordingManager::process_external_chunk (waits)
//!            Slide ─▶ AttachmentWorkflow (SlideAttachmentStore)
//!            Document ─▶ AttachmentWorkflow::handle_meeting_document (transcript, captions)
//!            MeetingEnded ─▶ RecordingManager::end_external_meeting (after-meeting pass)
//! ```
//!
//! The pump runs on its own OS thread and waits for each chunk's result, so transcription speed
//! is the only backpressure: the WebSocket reader never blocks. When the pump falls more than
//! `NOTE_TAKER_TEAMS_BRIDGE_BUFFER_SECONDS` behind, newer audio waits sealed in `bridge-spool`
//! next to the database (#133), up to 1 GiB, and is transcribed late instead of dropped; only past
//! that, or without a spool, does the buffer drop the oldest audio. Chunk audio is keyed
//! `teams-<uuid>` in the store shared with local recordings and is always discarded after
//! processing; spool files are deleted when read back and at start-up, so raw audio never
//! outlives its transcription.
//!
//! | Variable | Values | Default |
//! | --- | --- | --- |
//! | `NOTE_TAKER_TEAMS_BRIDGE_ENABLED` | boolean | `false` |
//! | `NOTE_TAKER_TEAMS_BRIDGE_PORT` | 1–65535 (always bound on 127.0.0.1) | `8765` |
//! | `NOTE_TAKER_TEAMS_BRIDGE_SECRET` | shared secret the bot sends first (never logged) | the keychain's `teams-bridge-secret`, else unset |
//! | `NOTE_TAKER_TEAMS_BRIDGE_BUFFER_SECONDS` | 1–300 | `10` |
//! | `NOTE_TAKER_TEAMS_BRIDGE_SLIDE_DIR` | directory for slide images | `slides` next to the database |

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    thread::JoinHandle,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use uuid::Uuid;

use crate::adapters::network::browser_host::{
    bridge_client::BridgeClientConfig,
    outbox::{OutboxKey, browser_outbox_root},
    outbox_delivery::run_outbox_importer,
};
use crate::{
    adapters::{
        audio::chunk_audio_store::ChunkAudioStore,
        keychain::bridge_secret::{
            bridge_secret_keychain, ensure_bridge_secret, load_bridge_secret,
        },
        network::teams_bridge::{
            bridge_buffer::{AudioSpool, BridgeBuffer},
            message_decoder::{BridgeAudio, BridgeEvent, BridgeMeeting},
            sealed_audio_spool::{DEFAULT_SPOOL_CAP_BYTES, SealedAudioSpool},
            teams_websocket_server::{
                BridgeSecret, DEFAULT_AUTHENTICATION_TIMEOUT, DEFAULT_TEAMS_BRIDGE_PORT,
                RunningTeamsBridge, TeamsBridgeServerConfig, TeamsWebSocketServer,
            },
        },
        storage::{
            slide_attachment_store::SlideAttachmentStore, sqlite_adapter::SqliteNoteRepository,
            sqlite_voice_vector_db::SqliteVoiceVectorDB,
        },
    },
    application::{
        meeting_crdt::MeetingCrdtService,
        recording_manager::{ExternalChunkRequest, RecordingManager, SharedIdentityMapper},
        teams_processor::TeamsIdentityMapper,
    },
    domain::models::{AudioRecording, DeviceId, MeetingId, MeetingTitle, RecordingId},
    pipeline_composition::parse_flag,
    ports::{
        crypto::{EmbeddingCipher, KeychainStore},
        external_bridge::AttachmentWorkflow,
    },
    recording_controller::RecordingController,
    storage_composition::open_note_repository,
    transcription_settings::{keeps_meeting_voices, settings_file_for},
};

#[cfg(test)]
#[path = "teams_bridge_composition_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "teams_bridge_app_tests.rs"]
mod app_tests;

/// Enables the bridge.
pub const TEAMS_BRIDGE_ENABLED_VARIABLE: &str = "NOTE_TAKER_TEAMS_BRIDGE_ENABLED";
/// Loopback port of the WebSocket server.
pub const TEAMS_BRIDGE_PORT_VARIABLE: &str = "NOTE_TAKER_TEAMS_BRIDGE_PORT";
/// Shared secret the bot must present.
pub const TEAMS_BRIDGE_SECRET_VARIABLE: &str = "NOTE_TAKER_TEAMS_BRIDGE_SECRET";
/// Audio the buffer holds before dropping the oldest.
/// Folder next to the database where audio waits for transcription, sealed (#133).
pub const AUDIO_SPOOL_DIRECTORY: &str = "bridge-spool";
/// Variable naming the most audio held in memory before it waits in the spool.
pub const TEAMS_BRIDGE_BUFFER_SECONDS_VARIABLE: &str = "NOTE_TAKER_TEAMS_BRIDGE_BUFFER_SECONDS";
/// Directory receiving slide images.
pub const TEAMS_BRIDGE_SLIDE_DIR_VARIABLE: &str = "NOTE_TAKER_TEAMS_BRIDGE_SLIDE_DIR";
/// Default buffer length in seconds.
const DEFAULT_BUFFER_SECONDS: u64 = 10;
/// Largest accepted buffer length in seconds.
const MAXIMUM_BUFFER_SECONDS: u64 = 300;
/// How often the pump re-checks for shutdown while the buffer is empty.
const PUMP_POLL_INTERVAL: Duration = Duration::from_millis(200);
/// Prefix of the chunk-audio IDs the pump stores (local recordings use `<uuid>-chunk-<n>`).
const TEAMS_CHUNK_ID_PREFIX: &str = "teams-";

/// Bridge settings read from the environment; the secret is redacted in `Debug`.
#[derive(Clone, Debug)]
pub struct TeamsBridgeConfig {
    enabled: bool,
    port: u16,
    shared_secret: Option<BridgeSecret>,
    buffer_duration: Duration,
    slide_directory: Option<PathBuf>,
}

impl TeamsBridgeConfig {
    /// Reads the configuration from the process environment.
    ///
    /// Outputs: The configuration, or an error naming an invalid variable.
    /// Side effects: Reads environment variables; never logs their values.
    pub fn from_environment() -> Result<Self> {
        let config = Self::from_lookup(|name| std::env::var(name).ok())?;
        if !config.enabled || config.shared_secret.is_some() {
            return Ok(config);
        }
        Ok(config.with_keychain_secret(&bridge_secret_keychain()))
    }

    /// Uses the keychain's bridge secret (created by `browser-host install`, #102) when no
    /// secret came from the environment.
    ///
    /// Inputs: The keychain entry.
    /// Outputs: The configuration; unchanged when a secret is already set, none is stored, or
    /// the keychain cannot be read (logged without the secret; the bridge then accepts any
    /// local client, as without a secret).
    /// Side effects: Queries the credential store.
    pub fn with_keychain_secret(mut self, keychain: &dyn KeychainStore) -> Self {
        if self.shared_secret.is_some() {
            return self;
        }
        match load_bridge_secret(keychain) {
            Ok(Some(secret)) => {
                self.shared_secret = Some(BridgeSecret::new(secret.as_str().to_owned()));
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(error = %error, "Teams bridge secret could not be read from the keychain");
            }
        }
        self
    }

    /// Reads the configuration through `lookup`, so tests never touch the environment.
    ///
    /// Inputs: A function returning a variable's value, or `None` when unset. Blank values
    /// count as unset.
    /// Outputs: The configuration, or an error naming an invalid variable.
    /// Side effects: None beyond `lookup`.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let read = |name: &str| {
            lookup(name)
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        let port = match read(TEAMS_BRIDGE_PORT_VARIABLE) {
            None => DEFAULT_TEAMS_BRIDGE_PORT,
            Some(value) => match value.parse::<u16>() {
                Ok(port) if port != 0 => port,
                _ => bail!("{TEAMS_BRIDGE_PORT_VARIABLE} must be a port between 1 and 65535"),
            },
        };
        let buffer_seconds = match read(TEAMS_BRIDGE_BUFFER_SECONDS_VARIABLE) {
            None => DEFAULT_BUFFER_SECONDS,
            Some(value) => match value.parse::<u64>() {
                Ok(seconds) if (1..=MAXIMUM_BUFFER_SECONDS).contains(&seconds) => seconds,
                _ => bail!(
                    "{TEAMS_BRIDGE_BUFFER_SECONDS_VARIABLE} must be between 1 and \
                     {MAXIMUM_BUFFER_SECONDS} seconds"
                ),
            },
        };
        Ok(Self {
            enabled: parse_flag(
                TEAMS_BRIDGE_ENABLED_VARIABLE,
                read(TEAMS_BRIDGE_ENABLED_VARIABLE),
                false,
            )?,
            port,
            shared_secret: read(TEAMS_BRIDGE_SECRET_VARIABLE).map(BridgeSecret::new),
            buffer_duration: Duration::from_secs(buffer_seconds),
            slide_directory: read(TEAMS_BRIDGE_SLIDE_DIR_VARIABLE).map(PathBuf::from),
        })
    }

    /// Overrides the port (tests use 0 for an ephemeral port).
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Reports whether the bridge should start.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the loopback port.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Reports whether the bot must authenticate with the shared secret.
    pub fn requires_authentication(&self) -> bool {
        self.shared_secret.is_some()
    }

    /// Returns how much audio the buffer holds before dropping the oldest.
    pub fn buffer_duration(&self) -> Duration {
        self.buffer_duration
    }

    /// Returns the configured slide directory, if one was set.
    pub fn slide_directory(&self) -> Option<&Path> {
        self.slide_directory.as_deref()
    }

    /// Builds the WebSocket server settings.
    fn server_config(&self) -> TeamsBridgeServerConfig {
        TeamsBridgeServerConfig {
            port: self.port,
            shared_secret: self.shared_secret.clone(),
            authentication_timeout: DEFAULT_AUTHENTICATION_TIMEOUT,
        }
    }
}

/// The pipeline worker and the chunk audio store its transcription engine reads.
pub type PipelineHandles = (Arc<RecordingManager>, Arc<ChunkAudioStore>);

/// Builds the pipeline worker and the chunk audio store its engine reads.
pub type PipelineBuilder = Box<dyn FnMut() -> Result<PipelineHandles> + Send>;

/// The pipeline worker the bridge feeds: ready, or built when the first meeting starts (#123).
pub struct BridgePipeline {
    /// The worker and its audio store, once built.
    ready: Option<(Arc<RecordingManager>, Arc<ChunkAudioStore>)>,
    /// Builds them; `None` once built, or for a pipeline handed over ready.
    builder: Option<PipelineBuilder>,
}

impl BridgePipeline {
    /// A pipeline that already runs (the daemon's).
    pub fn ready(
        recording_manager: Arc<RecordingManager>,
        chunk_audio_store: Arc<ChunkAudioStore>,
    ) -> Self {
        Self {
            ready: Some((recording_manager, chunk_audio_store)),
            builder: None,
        }
    }

    /// A pipeline built when the first meeting starts (the desktop app's), so an idle app loads
    /// no model. A failed build is retried when the next meeting starts.
    pub fn on_first_meeting(builder: PipelineBuilder) -> Self {
        Self {
            ready: None,
            builder: Some(builder),
        }
    }

    /// Builds the pipeline unless it runs already.
    ///
    /// Outputs: The worker and its audio store, or why they could not be built.
    /// Side effects: May run the builder (which may load a model); keeps what it built.
    fn build(&mut self) -> Result<(Arc<RecordingManager>, Arc<ChunkAudioStore>)> {
        if let Some((recording_manager, chunk_audio_store)) = &self.ready {
            return Ok((Arc::clone(recording_manager), Arc::clone(chunk_audio_store)));
        }
        let builder = self
            .builder
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("the bridge has no pipeline"))?;
        let built = builder()?;
        self.ready = Some((Arc::clone(&built.0), Arc::clone(&built.1)));
        self.builder = None;
        Ok(built)
    }

    /// The worker, once built.
    fn recording_manager(&self) -> Option<&Arc<RecordingManager>> {
        self.ready
            .as_ref()
            .map(|(recording_manager, _)| recording_manager)
    }

    /// The chunk audio store, once built.
    fn chunk_audio_store(&self) -> Option<&Arc<ChunkAudioStore>> {
        self.ready
            .as_ref()
            .map(|(_, chunk_audio_store)| chunk_audio_store)
    }
}

/// Everything the pump needs to turn bridge events into meeting data.
pub struct TeamsBridgePorts<SlideWorkflow> {
    /// The pipeline worker, whose external meeting session receives the chunks, and the store
    /// its transcription engine reads chunk audio from.
    pub pipeline: BridgePipeline,
    /// Maps Teams participants to persistent local identities.
    pub identity_mapper: SharedIdentityMapper,
    /// Stores screen-share captures as meeting attachments.
    pub slide_workflow: SlideWorkflow,
    /// Recorded as the acquiring device of each chunk.
    pub device_id: DeviceId,
    /// Holds audio transcription hasn't reached yet, sealed (#133); `None` drops it instead.
    pub audio_spool: Option<Arc<dyn AudioSpool>>,
}

/// Counters reported when the bridge shuts down (IDs and counts only).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TeamsBridgeStatistics {
    /// Meeting starts (including resumptions after reconnects).
    pub meetings_started: usize,
    /// Audio chunks persisted as segments.
    pub processed_audio_chunks: usize,
    /// Audio chunks that failed in the pipeline.
    pub failed_audio_chunks: usize,
    /// Slides attached to their meeting.
    pub stored_slides: usize,
    /// Slides that could not be stored.
    pub failed_slides: usize,
    /// Meeting documents (transcripts, captions) attached to their meeting.
    pub stored_documents: usize,
    /// Meeting documents that could not be stored.
    pub failed_documents: usize,
    /// Meetings the sender reported as over.
    pub meetings_ended: usize,
}

/// A running bridge: server, buffer, and pump thread.
pub struct TeamsBridgeService {
    server: RunningTeamsBridge,
    buffer: BridgeBuffer,
    pump_thread: JoinHandle<TeamsBridgeStatistics>,
}

impl TeamsBridgeService {
    /// Returns the server's bound loopback address.
    pub fn local_addr(&self) -> SocketAddr {
        self.server.local_addr()
    }

    /// Stops the server, lets the pump drain the buffer, and ends the external meeting.
    ///
    /// Outputs: The pump's counters (default counters if the pump panicked, which is logged).
    /// Side effects: Closes bot connections (1001); processes buffered events on the pump thread
    /// (awaited via `spawn_blocking`, so the runtime is not blocked).
    pub async fn shutdown(self) -> TeamsBridgeStatistics {
        self.server.shutdown().await;
        self.buffer.close();
        let pump_thread = self.pump_thread;
        match tokio::task::spawn_blocking(move || pump_thread.join()).await {
            Ok(Ok(statistics)) => statistics,
            Ok(Err(_)) | Err(_) => {
                tracing::error!("Teams bridge pump panicked");
                TeamsBridgeStatistics::default()
            }
        }
    }
}

/// Starts the WebSocket server and the pump thread.
///
/// Inputs: The configuration (its `enabled` flag is not consulted) and the pump's ports.
/// Outputs: The running service, or an error when the port cannot be bound or the thread cannot
/// start.
/// Side effects: Listens on 127.0.0.1; spawns the pump thread and server tasks.
pub async fn start_teams_bridge<SlideWorkflow>(
    config: &TeamsBridgeConfig,
    ports: TeamsBridgePorts<SlideWorkflow>,
) -> Result<TeamsBridgeService>
where
    SlideWorkflow: AttachmentWorkflow + Send + 'static,
{
    let buffer = match ports.audio_spool.clone() {
        Some(spool) => BridgeBuffer::with_spool(config.buffer_duration(), spool),
        None => BridgeBuffer::new(config.buffer_duration()),
    };
    let server = TeamsWebSocketServer::start(config.server_config(), buffer.clone())
        .await
        .context("unable to start the Teams bridge server")?;
    let pump_buffer = buffer.clone();
    let pump_thread = std::thread::Builder::new()
        .name("teams-bridge-pump".to_owned())
        .spawn(move || MeetingPump::new(ports).run(&pump_buffer))
        .context("unable to start the Teams bridge pump")?;
    tracing::info!(
        local_addr = %server.local_addr(),
        authentication_required = config.requires_authentication(),
        buffer_seconds = config.buffer_duration().as_secs(),
        "Teams bridge started"
    );
    Ok(TeamsBridgeService {
        server,
        buffer,
        pump_thread,
    })
}

/// Opens the SQLite-backed ports for the bridge next to an existing recording controller.
///
/// Inputs: The database path, the device ID, the verified cipher, the slide directory (default:
/// `slides` next to the database), and the controller's shared worker and chunk audio store.
/// Outputs: The ports, or an error naming the store that failed to open.
/// Side effects: Opens a voice-store and a note-repository connection (blocking I/O; call it
/// from `spawn_blocking`).
pub fn open_teams_bridge_ports(
    database_path: &Path,
    device_id: DeviceId,
    embedding_cipher: Arc<dyn EmbeddingCipher>,
    slide_directory: Option<&Path>,
    pipeline: BridgePipeline,
) -> Result<TeamsBridgePorts<SlideAttachmentStore<SqliteNoteRepository>>> {
    let voice_store = SqliteVoiceVectorDB::open(database_path, Arc::clone(&embedding_cipher))
        .context("unable to open the voice store for Teams identities")?;
    let attachment_repository = open_note_repository(
        database_path,
        device_id.clone(),
        Arc::clone(&embedding_cipher),
    )
    .context("unable to open the note repository for Teams slides")?;
    let slide_directory = slide_directory.map_or_else(
        || {
            database_path
                .parent()
                .map_or_else(PathBuf::new, Path::to_path_buf)
                .join("slides")
        },
        Path::to_path_buf,
    );
    let sealed_files = attachment_repository.sealed_files();
    let spool_directory = database_path
        .parent()
        .map_or_else(PathBuf::new, Path::to_path_buf)
        .join(AUDIO_SPOOL_DIRECTORY);
    let audio_spool: Option<Arc<dyn AudioSpool>> = match SealedAudioSpool::open(
        &spool_directory,
        Arc::clone(&embedding_cipher),
        DEFAULT_SPOOL_CAP_BYTES,
    ) {
        Ok(spool) => Some(Arc::new(spool)),
        Err(error) => {
            tracing::warn!(error = %error, "Teams audio spool unavailable; late audio is dropped");
            None
        }
    };
    // Read at every offer, so switching it in Settings applies without a restart (#103).
    let settings_file = settings_file_for(database_path);
    Ok(TeamsBridgePorts {
        pipeline,
        identity_mapper: Arc::new(
            TeamsIdentityMapper::new(std::sync::Mutex::new(voice_store))
                .with_voice_keeping(Box::new(move || keeps_meeting_voices(&settings_file))),
        ),
        slide_workflow: SlideAttachmentStore::new(
            slide_directory,
            attachment_repository,
            device_id.clone(),
            sealed_files,
        ),
        device_id,
        audio_spool,
    })
}

/// Starts the bridge when `NOTE_TAKER_TEAMS_BRIDGE_ENABLED` is set and the pipeline is ready.
///
/// Inputs: The database path, the verified cipher, and the recording controller (`None` when
/// no transcription engine is configured).
/// Outputs: The running service, or `None` when disabled or when start-up failed (logged; the
/// daemon keeps running without the bridge).
/// Side effects: Reads the environment; opens SQLite connections on a blocking thread; listens
/// on 127.0.0.1.
pub async fn start_teams_bridge_from_environment(
    database_path: &Path,
    device_id: DeviceId,
    embedding_cipher: Arc<dyn EmbeddingCipher>,
    recording_controller: Option<&RecordingController>,
) -> Option<TeamsBridgeService> {
    let config = match TeamsBridgeConfig::from_environment() {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(error = %format!("{error:#}"), "Teams bridge disabled: invalid configuration");
            return None;
        }
    };
    if !config.enabled() {
        return None;
    }
    let Some(recording_controller) = recording_controller else {
        tracing::warn!("Teams bridge disabled: the meeting pipeline is unavailable");
        return None;
    };
    let pipeline = BridgePipeline::ready(
        recording_controller.shared_recording_manager(),
        recording_controller.shared_chunk_audio_store(),
    );
    start_teams_bridge_with_pipeline(
        &config,
        database_path,
        device_id,
        embedding_cipher,
        pipeline,
    )
    .await
}

/// Reads the desktop app's bridge configuration (#123): on unless
/// `NOTE_TAKER_TEAMS_BRIDGE_ENABLED=false`, and always with a secret.
///
/// Inputs: The app's settings (`NOTE_TAKER_TEAMS_BRIDGE_*`, as for the daemon) and the keychain
/// entry of the bridge secret.
/// Outputs: The configuration, or `None` when turned off, misconfigured, or when no secret is
/// configured and the keychain cannot keep one, or is turned off (`NOTE_TAKER_KEYCHAIN_ENABLED`,
/// then never queried) (the bridge would otherwise accept any local program). Each `None` is
/// logged without values.
/// Side effects: Creates the keychain secret when none exists, so the browser host (which reads
/// it there) and the app agree without an install step.
pub fn app_bridge_config(
    setting: &dyn Fn(&str) -> Option<String>,
    keychain: &dyn KeychainStore,
) -> Option<TeamsBridgeConfig> {
    let config = TeamsBridgeConfig::from_lookup(|name| match name {
        TEAMS_BRIDGE_ENABLED_VARIABLE => Some(setting(name).unwrap_or_else(|| "true".to_owned())),
        _ => setting(name),
    });
    let mut config = match config {
        Ok(config) if config.enabled() => config,
        Ok(_) => return None,
        Err(error) => {
            tracing::warn!(error = %format!("{error:#}"), "Teams bridge disabled: invalid configuration");
            return None;
        }
    };
    // With the keychain turned off, asking it would raise a keyring prompt: the bridge then needs
    // a configured secret.
    let keychain_enabled =
        crate::application::crypto::key_manager::KeyManagerConfig::from_lookup(|name| {
            setting(name)
        })
        .keychain_enabled();
    if config.shared_secret.is_none() && !keychain_enabled {
        tracing::info!("Teams bridge off: the keychain is turned off and no secret is configured");
        return None;
    }
    if config.shared_secret.is_none() {
        match ensure_bridge_secret(keychain) {
            Ok(secret) => {
                config.shared_secret = Some(BridgeSecret::new(secret.as_str().to_owned()))
            }
            Err(error) => {
                tracing::warn!(error = %error, "Teams bridge disabled: no secret could be kept");
                return None;
            }
        }
    }
    Some(config)
}

/// Starts the bridge with `config` over `pipeline`.
///
/// Inputs: An enabled configuration, the database, device and cipher the stores open with, and
/// the pipeline (ready, or built at the first meeting).
/// Outputs: The running service, or `None` when start-up failed (logged).
/// Side effects: Opens SQLite connections on a blocking thread; listens on 127.0.0.1.
pub async fn start_teams_bridge_with_pipeline(
    config: &TeamsBridgeConfig,
    database_path: &Path,
    device_id: DeviceId,
    embedding_cipher: Arc<dyn EmbeddingCipher>,
    pipeline: BridgePipeline,
) -> Option<TeamsBridgeService> {
    let database_path = database_path.to_path_buf();
    let slide_directory = config.slide_directory().map(Path::to_path_buf);
    let ports = tokio::task::spawn_blocking(move || {
        open_teams_bridge_ports(
            &database_path,
            device_id,
            embedding_cipher,
            slide_directory.as_deref(),
            pipeline,
        )
    })
    .await;
    let started = match ports {
        Ok(Ok(ports)) => start_teams_bridge(config, ports).await,
        Ok(Err(error)) => Err(error),
        Err(join_error) => Err(anyhow::anyhow!(
            "opening the Teams bridge stores panicked: {join_error}"
        )),
    };
    match started {
        Ok(service) => {
            start_outbox_importer(config, service.local_addr().port());
            Some(service)
        }
        Err(error) => {
            tracing::warn!(error = %format!("{error:#}"), "Teams bridge unavailable");
            None
        }
    }
}

/// Takes over meetings the browser host kept because Koalino wasn't there (#135): replays them
/// to this bridge now and every 30 s.
///
/// Inputs: The bridge's configuration (its secret seals the outbox) and its port.
/// Side effects: Spawns a task on the current runtime; without a secret or an outbox location
/// nothing is started (the host then keeps nothing on disk either).
fn start_outbox_importer(config: &TeamsBridgeConfig, port: u16) {
    let Some(secret) = config
        .shared_secret
        .as_ref()
        .map(|secret| secret.expose().to_owned())
    else {
        return;
    };
    let Some(root) = browser_outbox_root(|name| std::env::var(name).ok()) else {
        return;
    };
    let key = OutboxKey::from_bridge_secret(&secret);
    let client = BridgeClientConfig::new(port, Some(secret));
    tokio::spawn(run_outbox_importer(root, key, client));
}

/// Feeds buffered bridge events into the pipeline worker, one at a time.
struct MeetingPump<SlideWorkflow> {
    ports: TeamsBridgePorts<SlideWorkflow>,
    /// The meeting the worker's external session is open for.
    active_meeting_id: Option<MeetingId>,
    statistics: TeamsBridgeStatistics,
}

impl<SlideWorkflow> MeetingPump<SlideWorkflow>
where
    SlideWorkflow: AttachmentWorkflow,
{
    /// Creates an idle pump.
    fn new(ports: TeamsBridgePorts<SlideWorkflow>) -> Self {
        Self {
            ports,
            active_meeting_id: None,
            statistics: TeamsBridgeStatistics::default(),
        }
    }

    /// Processes events until the buffer is closed and drained, then ends the external meeting.
    ///
    /// Outputs: The counters.
    /// Side effects: Blocks the calling (pump) thread; see the per-event handlers.
    fn run(mut self, buffer: &BridgeBuffer) -> TeamsBridgeStatistics {
        loop {
            match buffer.next_event(PUMP_POLL_INTERVAL) {
                Some(event) => self.handle_event(event),
                None if buffer.is_closed() => break,
                None => {}
            }
        }
        if self.active_meeting_id.is_some()
            && let Some(recording_manager) = self.ports.pipeline.recording_manager()
            && let Err(error) = recording_manager.end_external_meeting()
        {
            tracing::warn!(error = %error, "Teams meeting could not be ended cleanly");
        }
        tracing::info!(
            meetings_started = self.statistics.meetings_started,
            processed_audio_chunks = self.statistics.processed_audio_chunks,
            failed_audio_chunks = self.statistics.failed_audio_chunks,
            stored_slides = self.statistics.stored_slides,
            failed_slides = self.statistics.failed_slides,
            stored_documents = self.statistics.stored_documents,
            failed_documents = self.statistics.failed_documents,
            meetings_ended = self.statistics.meetings_ended,
            "Teams bridge pump stopped"
        );
        self.statistics
    }

    /// Routes one event.
    fn handle_event(&mut self, event: BridgeEvent) {
        match event {
            BridgeEvent::MeetingStarted(meeting) => self.begin_meeting(&meeting),
            BridgeEvent::Audio(audio) => self.process_audio(audio),
            BridgeEvent::Slide(slide_capture) => {
                match self
                    .ports
                    .slide_workflow
                    .handle_slide_capture(slide_capture)
                {
                    Ok(()) => self.statistics.stored_slides += 1,
                    Err(error) => {
                        self.statistics.failed_slides += 1;
                        tracing::warn!(error = %error, "Teams slide could not be stored");
                    }
                }
            }
            BridgeEvent::IgnoredVideo => {}
            BridgeEvent::Document(document) => {
                match self.ports.slide_workflow.handle_meeting_document(document) {
                    Ok(()) => self.statistics.stored_documents += 1,
                    Err(error) => {
                        self.statistics.failed_documents += 1;
                        tracing::warn!(error = %error, "Meeting document could not be stored");
                    }
                }
            }
            BridgeEvent::MeetingEnded(meeting_uuid) => self.end_meeting(meeting_uuid),
        }
    }

    /// Closes the external session when the active meeting ended, so its after-meeting pass runs.
    ///
    /// Side effects: Ends the worker's external meeting; logs IDs only.
    fn end_meeting(&mut self, meeting_uuid: Uuid) {
        let is_active = self
            .active_meeting_id
            .as_ref()
            .is_some_and(|meeting_id| meeting_id.as_str() == meeting_uuid.to_string());
        if !is_active {
            return;
        }
        self.active_meeting_id = None;
        let Some(recording_manager) = self.ports.pipeline.recording_manager() else {
            return;
        };
        match recording_manager.end_external_meeting() {
            Ok(_summary) => self.statistics.meetings_ended += 1,
            Err(error) => {
                tracing::warn!(error = %error, "Online meeting could not be ended cleanly")
            }
        }
    }

    /// Opens (or resumes) the worker's external session for `meeting`.
    ///
    /// Side effects: May create the meeting in SQLite, store its Teams title, and end a previous
    /// external meeting.
    fn begin_meeting(&mut self, meeting: &BridgeMeeting) {
        let recording_manager = match self.ports.pipeline.build() {
            Ok((recording_manager, _)) => recording_manager,
            Err(error) => {
                self.active_meeting_id = None;
                tracing::warn!(
                    meeting_id = meeting.meeting_id().as_str(),
                    error = %format!("{error:#}"),
                    "the meeting pipeline could not be built; this meeting's media will be dropped"
                );
                return;
            }
        };
        match recording_manager.begin_external_meeting(
            meeting.meeting_id().clone(),
            Arc::clone(&self.ports.identity_mapper),
        ) {
            Ok(()) => {
                self.statistics.meetings_started += 1;
                self.active_meeting_id = Some(meeting.meeting_id().clone());
                self.store_title_if_untitled(meeting);
            }
            Err(error) => {
                self.active_meeting_id = None;
                tracing::warn!(
                    meeting_id = meeting.meeting_id().as_str(),
                    error = %error,
                    "Teams meeting could not be opened; its media will be dropped"
                );
            }
        }
    }

    /// Stores the Teams meeting title when the local meeting has none yet (Issue #35).
    ///
    /// A title already stored (from an earlier metadata frame or set locally) is kept, so
    /// re-sent metadata never overwrites it. An empty or invalid Teams title is ignored.
    /// Side effects: Reads meeting metadata and may write the title into the CRDT document and its
    /// SQLite projection (Issue #39) on the worker thread; never logs the title. Failures are
    /// logged and do not affect media processing.
    fn store_title_if_untitled(&self, meeting: &BridgeMeeting) {
        let Ok(teams_title) = MeetingTitle::parse(meeting.title()) else {
            return;
        };
        let Some(recording_manager) = self.ports.pipeline.recording_manager() else {
            return;
        };
        let meeting_id = meeting.meeting_id().clone();
        let stored = recording_manager.with_note_repository(move |note_repository| {
            match note_repository.load_meeting_metadata(&meeting_id)? {
                Some(metadata) if metadata.title().is_none() => {
                    MeetingCrdtService::new(note_repository)
                        .set_meeting_title(&meeting_id, &teams_title)
                }
                _ => Ok(()),
            }
        });
        if let Err(error) = stored
            .map_err(|error| error.to_string())
            .and_then(|stored| stored.map_err(|error| error.to_string()))
        {
            tracing::warn!(error = %error, "Teams meeting title could not be stored");
        }
    }

    /// Transcribes one chunk in the open meeting and waits for the result.
    ///
    /// Side effects: Stores the chunk PCM in the shared audio store (always discarded afterwards),
    /// runs the pipeline on the worker, and persists segments.
    fn process_audio(&mut self, audio: BridgeAudio) {
        let Some(meeting_id) = self
            .active_meeting_id
            .clone()
            .filter(|meeting_id| meeting_id.as_str() == audio.meeting_uuid().to_string())
        else {
            self.statistics.failed_audio_chunks += 1;
            tracing::warn!("Teams audio dropped: its meeting is not open");
            return;
        };
        // An open meeting implies a built pipeline.
        let (Some(recording_manager), Some(chunk_audio_store)) = (
            self.ports.pipeline.recording_manager().cloned(),
            self.ports.pipeline.chunk_audio_store().cloned(),
        ) else {
            self.statistics.failed_audio_chunks += 1;
            return;
        };
        let duration_ms = audio.duration_ms();
        let media_chunk = audio.into_media_chunk();
        let recording_id = RecordingId::new(format!("{TEAMS_CHUNK_ID_PREFIX}{}", Uuid::new_v4()));
        let samples = media_chunk
            .audio_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|sample_bytes| i16::from_le_bytes(*sample_bytes))
            .collect();
        chunk_audio_store.insert(&recording_id, samples);
        let outcome = recording_manager.process_external_chunk(ExternalChunkRequest {
            media_chunk,
            audio_recording: AudioRecording::new(
                recording_id.clone(),
                meeting_id,
                self.ports.device_id.clone(),
            ),
            duration_ms,
        });
        chunk_audio_store.discard(&recording_id);
        match outcome {
            Ok(_segment_count) => self.statistics.processed_audio_chunks += 1,
            Err(error) => {
                self.statistics.failed_audio_chunks += 1;
                tracing::warn!(error = %error, "Teams audio chunk failed in the pipeline");
            }
        }
    }
}
