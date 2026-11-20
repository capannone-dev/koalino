//! Composes and runs the daemon's P2P task: pairing and meeting sync with the owner's devices.
//!
//! The task runs on its own OS thread with a single-threaded Tokio runtime, so libp2p's event loop
//! and the SQLite work of merging documents never run on the HTTP runtime. It owns:
//!
//! - a `Libp2pNetworkAdapter` with the persisted node identity, listening on
//!   `NOTE_TAKER_P2P_LISTEN`;
//! - its own SQLite connections: a document-backed note repository and the peer store;
//! - the `P2pService` that joins pairing and sync.
//!
//! Callers talk to it through a `P2pHandle`:
//!
//! - `command` sends a `P2pCommand` and awaits its outcome;
//! - `notify_local_change` reports a local write without waiting;
//! - `snapshot` reads the latest status.
//!
//! Addresses saved in Settings replace the relay and peer variables (`p2p_network_routes`).
//!
//! Environment: `NOTE_TAKER_P2P_ENABLED` (`true` by default, `false` turns P2P off),
//! `NOTE_TAKER_P2P_LISTEN` (default `/ip4/0.0.0.0/tcp/0`), `NOTE_TAKER_DEVICE_NAME` (default: the
//! host name), and `NOTE_TAKER_P2P_PEERS` (comma-separated `…/p2p/<peer id>` addresses of devices
//! mDNS cannot find, redialed every 10 s), `NOTE_TAKER_P2P_RELAY` (the `…/p2p/<peer id>` address of
//! the owner's relay, for reaching devices across NATs; it must be paired with this device), and
//! `NOTE_TAKER_P2P_RELAY_SERVER_ADDRESS` (on the always-on daemon: its public address, such as
//! `/ip4/203.0.113.5/tcp/4001`, which also turns the relay service on; use a fixed port in
//! `NOTE_TAKER_P2P_LISTEN`). Unpaired devices on the LAN can see this node's peer ID through mDNS, but they
//! cannot connect except while the owner keeps a pairing window open, and even then they cannot
//! sync.

use crate::domain::transcription_language::TranscriptionLanguage;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
    thread::JoinHandle,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use tokio::sync::{mpsc, oneshot, watch};

use crate::{
    adapters::{
        network::libp2p_adapter::{Libp2pNetworkAdapter, NodeIdentity},
        storage::{
            attachment_sync_store::AttachmentSyncStore, sqlite_adapter::SqliteNoteRepository,
            sqlite_peer_store::SqlitePeerStore, sqlite_voice_vector_db::SqliteVoiceVectorDB,
        },
    },
    application::{
        compute_router::ComputePreference,
        crypto::pair_key::PairAgreementSecret,
        p2p_service::{
            FinishedPairedChat, LocalCapabilities, NetworkCommand, NetworkCommandError,
            NetworkCommandOutcome, P2pCommand, P2pCommandOutcome, P2pControl, P2pControlError,
            P2pService, P2pSnapshot, PAIRING_WINDOW, PairedChatHost, REMOTE_TRANSCRIPTION_TIMEOUT,
            RemoteTranscriptionJob, SPEECH_TRANSFER_TIMEOUT, SpeechTransferJob,
        },
        paired_chat_exchange::{PairedChatRequest, PairedChatTurn},
        paired_chat_relay::{PairedChatJob, PairedChatResult},
        pairing_manager::PairingManager,
        repository_sync_manager::RepositorySyncManager,
        transcription_routing::RemoteTranscriptionGateway,
    },
    domain::{
        chat::ChatScope,
        errors::TranscriptionError,
        models::{DeviceId, DeviceManifest, MeetingId},
        network_addresses::NetworkAddresses,
        peer_trust::DeviceName,
    },
    llm_settings::{FileSharedPrompts, llm_settings_file_for},
    p2p_network_routes::{NetworkRoutes, run_network_command},
    pipeline_composition::{PipelineConfig, select_transcription_engine},
    ports::{
        crypto::EmbeddingCipher,
        peer_trust::PairAgreementSecretStore,
        speech_transfer::{SpeechInbox, SpeechLink, SpeechPart, SpeechSender, SpeechTransferError},
    },
    storage_composition::open_note_repository,
    transcription_host::ThreadedTranscriptionHost,
};

#[cfg(test)]
#[path = "p2p_composition_tests.rs"]
mod tests;

/// Listen address used when `NOTE_TAKER_P2P_LISTEN` is unset: every interface, a free port.
const DEFAULT_LISTEN_ADDRESS: &str = "/ip4/0.0.0.0/tcp/0";

/// Device name used when neither `NOTE_TAKER_DEVICE_NAME` nor a usable host name is available.
const FALLBACK_DEVICE_NAME: &str = "Koalino device";

/// How many chunks may wait to be sent; the pipeline handles one at a time.
const REMOTE_TRANSCRIPTION_QUEUE_CAPACITY: usize = 4;

/// How many speech parts may wait to be sent; the after-meeting worker sends one at a time.
const SPEECH_TRANSFER_QUEUE_CAPACITY: usize = 2;

/// Longest host name used as a default device name, matching `DeviceName`'s limit.
const MAX_DEFAULT_NAME_CHARACTERS: usize = 64;

/// How often the task looks for meetings written locally (by any connection or process) and
/// sends them to connected devices, so a new transcript reaches them about a second after it
/// is saved.
const LOCAL_CHANGE_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// How often paired devices that are not connected are dialed again, so a device that comes
/// back online reconnects within this time, without waiting for a new mDNS announcement.
const PAIRED_PEER_REDIAL_INTERVAL: Duration = Duration::from_secs(10);

/// Chat questions that may wait for the task (#107).
const PAIRED_CHAT_QUEUE_CAPACITY: usize = 8;
/// How long `ask_paired_computer` waits: every computer's accept window plus one answer.
const PAIRED_CHAT_WAIT: Duration = Duration::from_secs(8 * 60);

/// Commands that may wait for the task before senders see back-pressure.
const COMMAND_QUEUE_CAPACITY: usize = 64;

/// The P2P task's settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct P2pConfig {
    /// Multiaddress to listen on.
    pub listen_address: String,
    /// Name announced to devices during pairing.
    pub device_name: DeviceName,
    /// Addresses of peers that mDNS cannot find, each ending in `/p2p/<peer id>`.
    pub static_peers: Vec<String>,
    /// Address of the relay this device uses, ending in `/p2p/<relay id>`.
    pub relay_address: Option<String>,
    /// Public address of this device's relay service; `None` means it offers none.
    pub relay_server_address: Option<String>,
    /// Where attachments are kept; `None` means an `attachments` folder beside the database.
    pub attachment_directory: Option<PathBuf>,
    /// What this device advertises in its manifest; `from_lookup` leaves the default (no
    /// models), and callers set the models their pipeline configuration can run.
    pub capabilities: LocalCapabilities,
}

impl P2pConfig {
    /// Reads the settings from the process environment.
    ///
    /// Outputs: `None` when P2P is disabled, the settings, or an error for an invalid value.
    /// Side effects: Reads environment variables and, for the default name, `/etc/hostname`.
    pub fn from_environment() -> Result<Option<Self>> {
        Self::from_lookup(|name| {
            let value = std::env::var(name).ok();
            if name == "HOSTNAME" {
                value
                    .or_else(|| std::env::var("COMPUTERNAME").ok())
                    .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
            } else {
                value
            }
        })
    }

    /// Reads the settings through `lookup`, which maps a variable name to its value.
    ///
    /// Outputs: `None` when `NOTE_TAKER_P2P_ENABLED` is `false`; otherwise the settings. An
    /// enable flag other than `true`/`false`, or an invalid explicit device name, is an error.
    /// A host name that is not a valid device name falls back to `FALLBACK_DEVICE_NAME`.
    /// Side effects: Only those of `lookup`.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Option<Self>> {
        match lookup("NOTE_TAKER_P2P_ENABLED").as_deref().map(str::trim) {
            None | Some("true") => {}
            Some("false") => return Ok(None),
            Some(_) => bail!("NOTE_TAKER_P2P_ENABLED must be true or false"),
        }
        let listen_address =
            lookup("NOTE_TAKER_P2P_LISTEN").unwrap_or_else(|| DEFAULT_LISTEN_ADDRESS.to_owned());
        let device_name = match lookup("NOTE_TAKER_DEVICE_NAME") {
            Some(explicit_name) => DeviceName::parse(&explicit_name)
                .context("NOTE_TAKER_DEVICE_NAME is not a valid device name")?,
            None => default_device_name(lookup("HOSTNAME").as_deref())?,
        };
        let static_peers = lookup("NOTE_TAKER_P2P_PEERS")
            .map(|peer_list| {
                peer_list
                    .split(',')
                    .map(str::trim)
                    .filter(|peer_address| !peer_address.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        Ok(Some(Self {
            listen_address,
            device_name,
            static_peers,
            relay_address: lookup("NOTE_TAKER_P2P_RELAY").map(|address| address.trim().to_owned()),
            relay_server_address: lookup("NOTE_TAKER_P2P_RELAY_SERVER_ADDRESS")
                .map(|address| address.trim().to_owned()),
            attachment_directory: lookup("NOTE_TAKER_ATTACHMENT_DIR").map(PathBuf::from),
            capabilities: LocalCapabilities::default(),
        }))
    }
}

/// Derives a device name from the host name, or the fallback when it is missing or unusable.
///
/// Outputs: The name, or an error only if `FALLBACK_DEVICE_NAME` itself were invalid.
fn default_device_name(host_name: Option<&str>) -> Result<DeviceName> {
    let host_device_name = host_name
        .map(|host_name| {
            host_name
                .trim()
                .chars()
                .take(MAX_DEFAULT_NAME_CHARACTERS)
                .collect::<String>()
        })
        .and_then(|candidate| DeviceName::parse(&candidate).ok());
    match host_device_name {
        Some(device_name) => Ok(device_name),
        None => DeviceName::parse(FALLBACK_DEVICE_NAME).context("invalid fallback device name"),
    }
}

/// One command and where to send its outcome (errors as text, without peer payloads).
type NetworkRequest = (
    NetworkCommand,
    oneshot::Sender<Result<NetworkCommandOutcome, NetworkCommandError>>,
);

/// A command for the task and where to send its outcome.
type P2pRequest = (
    P2pCommand,
    oneshot::Sender<std::result::Result<P2pCommandOutcome, String>>,
);

/// The running P2P task's controls. Cheap to share behind an `Arc`.
pub struct P2pHandle {
    /// Queue of commands to the task.
    commands: mpsc::Sender<P2pRequest>,
    network_commands: mpsc::Sender<NetworkRequest>,
    /// Latest snapshot published by the task.
    snapshots: watch::Receiver<P2pSnapshot>,
    /// Queue of chunks to send to paired devices.
    remote_transcriptions: mpsc::Sender<RemoteTranscriptionJob>,
    /// Queue of retained speech parts to send to the device that combines a meeting (#88).
    speech_transfers: mpsc::Sender<SpeechTransferJob>,
    /// Where speech paired devices send is stored, once the after-meeting worker runs.
    speech_inbox: Arc<Mutex<Option<Box<dyn SpeechInbox>>>>,
    /// Chat questions for the paired computers (#107).
    paired_chats: mpsc::Sender<PairedChatJob>,
    /// Answers paired devices' chat questions, once the host attaches it (#107).
    paired_chat_host: Arc<Mutex<Option<Box<dyn PairedChatHost>>>>,
    /// Tells the task to stop; taken by `shutdown`.
    stop: Mutex<Option<oneshot::Sender<()>>>,
    /// The task's thread; taken by `shutdown`.
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl P2pHandle {
    /// Sends `command` to the task and waits for its outcome.
    ///
    /// Outputs: The outcome, or an error when the command failed or the task has stopped.
    /// Side effects: Those of the command, carried out on the P2P thread.
    pub async fn command(&self, command: P2pCommand) -> Result<P2pCommandOutcome> {
        Ok(P2pControl::command(self, command).await?)
    }

    /// Reports that `meeting_id` was written locally, without waiting; callable from any thread.
    ///
    /// Side effects: Queues a `LocalChange`. A full queue or a stopped task is logged and the
    /// change waits for the next connection, when the handshake sends all meetings anyway.
    pub fn notify_local_change(&self, meeting_id: &MeetingId) {
        let (reply_sender, _ignored_reply) = oneshot::channel();
        if self
            .commands
            .try_send((P2pCommand::LocalChange(meeting_id.clone()), reply_sender))
            .is_err()
        {
            tracing::warn!(meeting_id = %meeting_id.as_str(), "could not queue a local change for sync");
        }
    }

    /// Returns the latest snapshot.
    pub fn snapshot(&self) -> P2pSnapshot {
        self.snapshots.borrow().clone()
    }

    /// Stops the task and waits for its thread; later calls do nothing.
    ///
    /// Side effects: Blocks until the thread exits, so async callers run it in `spawn_blocking`.
    /// The swarm and its connections close when the thread drops them.
    pub fn shutdown(&self) {
        if let Some(stop) = self
            .stop
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            let _ = stop.send(());
        }
        let worker = self
            .worker
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(worker) = worker
            && worker.join().is_err()
        {
            tracing::error!("the P2P task panicked");
        }
    }
}

impl Drop for P2pHandle {
    /// Tells the task to stop without waiting for its thread, so dropping a handle (for example a
    /// test's FFI runtime) never blocks and never leaves the task running. `shutdown` waits.
    fn drop(&mut self) {
        if let Some(stop) = self
            .stop
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            let _ = stop.send(());
        }
    }
}

impl RemoteTranscriptionGateway for P2pHandle {
    /// Reads the choice from the latest snapshot.
    fn route(&self) -> ComputePreference {
        self.snapshots.borrow().status.transcription_route.clone()
    }

    /// Reads this device's manifest from the latest snapshot.
    fn local_manifest(&self) -> DeviceManifest {
        self.snapshots.borrow().status.local_manifest.clone()
    }

    /// Reads the connected devices' manifests from the latest snapshot.
    fn connected_manifests(&self) -> Vec<DeviceManifest> {
        self.snapshots.borrow().status.peer_manifests.clone()
    }

    /// Hands the chunk to the P2P task and blocks until the answer or the deadline.
    ///
    /// Side effects: See the trait. A full queue or a stopped task is `Unavailable` at once.
    fn transcribe_on_peer(
        &self,
        peer: &DeviceId,
        samples: Vec<i16>,
        previous_text: Option<String>,
        language: Option<TranscriptionLanguage>,
    ) -> Result<String, TranscriptionError> {
        let (reply, answer) = std::sync::mpsc::channel();
        self.remote_transcriptions
            .try_send(RemoteTranscriptionJob {
                peer: peer.clone(),
                samples,
                previous_text,
                language,
                reply,
            })
            .map_err(|_| TranscriptionError::Unavailable)?;
        answer
            .recv_timeout(REMOTE_TRANSCRIPTION_TIMEOUT + Duration::from_secs(5))
            .map_err(|_| TranscriptionError::Unavailable)?
    }
}

impl SpeechSender for P2pHandle {
    /// Hands the part to the P2P task and blocks until its receipt or the deadline. `to` is the
    /// device's storage ID, as written on its meetings.
    ///
    /// Side effects: See the trait. A full queue or a stopped task is `Unreachable` at once.
    fn send_part(&self, to: &DeviceId, part: SpeechPart) -> Result<(), SpeechTransferError> {
        let (reply, answer) = std::sync::mpsc::channel();
        self.speech_transfers
            .try_send(SpeechTransferJob {
                device: to.clone(),
                part,
                reply,
            })
            .map_err(|_| SpeechTransferError::Unreachable)?;
        answer
            .recv_timeout(SPEECH_TRANSFER_TIMEOUT + Duration::from_secs(5))
            .map_err(|_| SpeechTransferError::Unreachable)?
    }
}

impl P2pHandle {
    /// Asks the connected paired computers a chat question and blocks until one answers (#107).
    ///
    /// Inputs: The scope, the question and the chat so far (oldest first).
    /// Outputs: The answer, or an error code (`no_paired_computer`, `paired_computer_timeout`,
    /// `paired_computer_left`, a computer's own code, or `p2p_unavailable` when the task is not
    /// running). Side effects: Sends the question sealed to paired computers. Blocks for up to
    /// about seven minutes, so callers run it off the UI thread and outside async tasks.
    pub fn ask_paired_computer(
        &self,
        scope: ChatScope,
        question: String,
        history: Vec<PairedChatTurn>,
    ) -> PairedChatResult {
        let (reply, answer) = std::sync::mpsc::channel();
        self.paired_chats
            .try_send(PairedChatJob {
                scope,
                question,
                history,
                reply,
            })
            .map_err(|_| "p2p_unavailable".to_owned())?;
        answer
            .recv_timeout(PAIRED_CHAT_WAIT)
            .map_err(|_| "paired_computer_timeout".to_owned())?
    }

    /// Answers paired devices' chat questions with `host` from now on (#107).
    ///
    /// Side effects: Replaces any host attached before.
    pub fn answer_paired_chats_with(&self, host: Box<dyn PairedChatHost>) {
        *self
            .paired_chat_host
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(host);
    }
}

/// The P2P service's chat host: forwards to the host attached later, and declines until then.
struct AttachedPairedChatHost(Arc<Mutex<Option<Box<dyn PairedChatHost>>>>);

impl PairedChatHost for AttachedPairedChatHost {
    fn submit(&mut self, peer: DeviceId, request: PairedChatRequest) -> Result<(), String> {
        match self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            Some(host) => host.submit(peer, request),
            None => Err("not_offered".to_owned()),
        }
    }

    fn poll_finished(&mut self) -> Option<FinishedPairedChat> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
            .and_then(|host| host.poll_finished())
    }
}

impl SpeechLink for P2pHandle {
    fn accept_speech_with(&self, inbox: Box<dyn SpeechInbox>) {
        *self
            .speech_inbox
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(inbox);
    }
}

/// The P2P service's inbox: forwards to the inbox the after-meeting worker attaches later, and
/// refuses speech until then (the sender tries again on its next round).
struct AttachedSpeechInbox(Arc<Mutex<Option<Box<dyn SpeechInbox>>>>);

impl SpeechInbox for AttachedSpeechInbox {
    fn receive_part(
        &mut self,
        from: &DeviceId,
        part: SpeechPart,
    ) -> Result<(), SpeechTransferError> {
        match self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            Some(inbox) => inbox.receive_part(from, part),
            None => Err(SpeechTransferError::Refused),
        }
    }
}

#[async_trait::async_trait]
impl P2pControl for P2pHandle {
    fn snapshot(&self) -> P2pSnapshot {
        P2pHandle::snapshot(self)
    }

    async fn network_command(
        &self,
        command: NetworkCommand,
    ) -> Result<NetworkCommandOutcome, NetworkCommandError> {
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.network_commands
            .send((command, reply_sender))
            .await
            .map_err(|_| NetworkCommandError::Stopped)?;
        reply_receiver
            .await
            .map_err(|_| NetworkCommandError::Stopped)?
    }

    async fn command(&self, command: P2pCommand) -> Result<P2pCommandOutcome, P2pControlError> {
        let (reply_sender, reply_receiver) = oneshot::channel();
        self.commands
            .send((command, reply_sender))
            .await
            .map_err(|_| P2pControlError::Stopped)?;
        reply_receiver
            .await
            .map_err(|_| P2pControlError::Stopped)?
            .map_err(P2pControlError::Refused)
    }

    fn notify_local_change(&self, meeting_id: &MeetingId) {
        P2pHandle::notify_local_change(self, meeting_id);
    }

    fn ask_paired_computer(
        &self,
        scope: ChatScope,
        question: String,
        history: Vec<PairedChatTurn>,
    ) -> PairedChatResult {
        P2pHandle::ask_paired_computer(self, scope, question, history)
    }
}

/// The service type the task runs.
type DaemonP2pService = P2pService<Libp2pNetworkAdapter, SqlitePeerStore, SqliteNoteRepository>;

/// Starts the P2P task and returns once it listens, or with the reason it could not start.
///
/// Inputs: The database path (the task opens its own connections), this device's ID, the
/// verified embedding cipher (it seals the node identity and the pair-agreement secret), the
/// settings, and how to transcribe for paired devices (`None` refuses their chunks).
/// Outputs: The handle, or an error naming the failed step.
/// Side effects: Spawns the `note-taker-p2p` thread, opens SQLite connections, creates or loads the
/// node identity and the pair-agreement secret, and binds the listen address. Blocks the caller while starting, so async callers
/// run it in `spawn_blocking`.
pub fn spawn_p2p_task(
    database_path: &Path,
    device_id: DeviceId,
    embedding_cipher: Arc<dyn EmbeddingCipher>,
    mut config: P2pConfig,
    hosted_pipeline: Option<PipelineConfig>,
) -> Result<P2pHandle> {
    // The name the owner chose wins over the host name and the app's default (#121).
    if let Some(saved_name) = crate::device_name_settings::load_saved_device_name(database_path) {
        config.device_name = saved_name;
    }
    let (startup_sender, startup_receiver) = std::sync::mpsc::sync_channel(1);
    let (command_sender, command_receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let (network_sender, network_receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let (stop_sender, stop_receiver) = oneshot::channel();
    let (remote_sender, remote_receiver) = mpsc::channel(REMOTE_TRANSCRIPTION_QUEUE_CAPACITY);
    let (speech_sender, speech_receiver) = mpsc::channel(SPEECH_TRANSFER_QUEUE_CAPACITY);
    let speech_inbox: Arc<Mutex<Option<Box<dyn SpeechInbox>>>> = Arc::new(Mutex::new(None));
    let service_inbox = AttachedSpeechInbox(Arc::clone(&speech_inbox));
    let (chat_sender, chat_receiver) = mpsc::channel(PAIRED_CHAT_QUEUE_CAPACITY);
    let paired_chat_host: Arc<Mutex<Option<Box<dyn PairedChatHost>>>> = Arc::new(Mutex::new(None));
    let service_chat_host = AttachedPairedChatHost(Arc::clone(&paired_chat_host));
    let database_path = database_path.to_path_buf();
    let worker = std::thread::Builder::new()
        .name("note-taker-p2p".to_owned())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = startup_sender.send(Err(anyhow!(error).context("no P2P runtime")));
                    return;
                }
            };
            runtime.block_on(async move {
                let started = open_service(
                    &database_path,
                    device_id,
                    embedding_cipher,
                    config,
                    hosted_pipeline,
                );
                let (service, mut network_routes) = match started {
                    Ok(started) => started,
                    Err(error) => {
                        let _ = startup_sender.send(Err(error));
                        return;
                    }
                };
                let mut service = service
                    .with_speech_inbox(Box::new(service_inbox))
                    .with_paired_chat_host(Box::new(service_chat_host));
                let (snapshot_sender, snapshot_receiver) =
                    watch::channel(snapshot_of(&service, &network_routes));
                if startup_sender.send(Ok(snapshot_receiver)).is_err() {
                    return;
                }
                run_service(
                    &mut service,
                    &mut network_routes,
                    snapshot_sender,
                    TaskQueues {
                        commands: command_receiver,
                        network: network_receiver,
                        remote_transcriptions: remote_receiver,
                        speech_transfers: speech_receiver,
                        paired_chats: chat_receiver,
                        stop: stop_receiver,
                    },
                )
                .await;
            });
        })
        .context("unable to start the P2P thread")?;
    let snapshots = startup_receiver
        .recv()
        .map_err(|_| anyhow!("the P2P thread exited while starting"))??;
    tracing::info!("P2P sync started");
    Ok(P2pHandle {
        commands: command_sender,
        network_commands: network_sender,
        remote_transcriptions: remote_sender,
        speech_transfers: speech_sender,
        speech_inbox,
        paired_chats: chat_sender,
        paired_chat_host,
        snapshots,
        stop: Mutex::new(Some(stop_sender)),
        worker: Mutex::new(Some(worker)),
    })
}

/// Opens the stores, builds the adapter and service, and starts listening.
///
/// Must run inside the task's runtime, because the adapter registers Tokio sockets.
/// Side effects: See `spawn_p2p_task`.
fn open_service(
    database_path: &Path,
    device_id: DeviceId,
    embedding_cipher: Arc<dyn EmbeddingCipher>,
    config: P2pConfig,
    hosted_pipeline: Option<PipelineConfig>,
) -> Result<(DaemonP2pService, NetworkRoutes)> {
    let note_repository = open_note_repository(
        database_path,
        device_id.clone(),
        Arc::clone(&embedding_cipher),
    )?;
    let peer_store = SqlitePeerStore::open(database_path, Arc::clone(&embedding_cipher))
        .context("unable to open the peer store")?;
    let node_identity =
        NodeIdentity::load_or_create(&peer_store).context("unable to load the node identity")?;
    let pair_agreement_secret = peer_store
        .load_or_insert_pair_agreement_secret(PairAgreementSecret::generate().to_bytes().as_slice())
        .context("unable to load the pair-agreement secret")
        .and_then(|secret_bytes| {
            PairAgreementSecret::from_bytes(&secret_bytes)
                .context("the stored pair-agreement secret is malformed")
        })?;
    let mut adapter = if config.relay_server_address.is_some() {
        Libp2pNetworkAdapter::new_relay_server(node_identity)
    } else {
        Libp2pNetworkAdapter::new(node_identity)
    }
    .context("unable to configure libp2p")?;
    if let Some(public_address) = &config.relay_server_address {
        adapter
            .advertise_relay_address(public_address)
            .context("NOTE_TAKER_P2P_RELAY_SERVER_ADDRESS is invalid")?;
    }
    adapter
        .listen_on(&config.listen_address)
        .context("unable to listen for P2P connections")?;
    let address_store = SqlitePeerStore::open(database_path, Arc::clone(&embedding_cipher))
        .context("unable to open the network addresses")?;
    let network_routes = NetworkRoutes::start(
        Box::new(address_store),
        NetworkAddresses {
            relay: config.relay_address.clone(),
            peers: config.static_peers.clone(),
        },
        config.relay_server_address.is_some(),
        &mut adapter,
    )?;
    let attachment_directory = config.attachment_directory.unwrap_or_else(|| {
        database_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("attachments")
    });
    let project_library = open_note_repository(
        database_path,
        device_id.clone(),
        Arc::clone(&embedding_cipher),
    )?;
    let storage_device_id = device_id.clone();
    let attachment_library = AttachmentSyncStore::new(
        open_note_repository(database_path, device_id, Arc::clone(&embedding_cipher))?,
        attachment_directory,
    );
    let route_store = SqlitePeerStore::open(database_path, Arc::clone(&embedding_cipher))
        .context("unable to open the transcription choice")?;
    let voice_library = SqliteVoiceVectorDB::open(database_path, embedding_cipher)
        .context("unable to open the voice library for sync")?;
    let mut service = P2pService::new(
        PairingManager::new(
            adapter,
            peer_store,
            config.device_name,
            Box::new(unix_millis),
        ),
        RepositorySyncManager::new(note_repository),
    )
    .with_local_capabilities(LocalCapabilities {
        storage_device_id: Some(storage_device_id),
        ..config.capabilities
    })
    .with_pair_agreement_secret(pair_agreement_secret)
    .with_voice_library(Box::new(voice_library))
    .with_project_library(Box::new(project_library))
    // The owner's summary and chat prompts, kept with the AI settings (#145).
    .with_shared_prompts(Box::new(FileSharedPrompts::new(llm_settings_file_for(
        database_path,
    ))))
    .with_attachment_library(Box::new(attachment_library))
    .with_transcription_route_store(Box::new(route_store));
    if let Some(pipeline_config) = hosted_pipeline {
        let host = ThreadedTranscriptionHost::spawn(move |store| {
            select_transcription_engine(&pipeline_config, store)
        })
        .context("unable to start transcribing for paired devices")?;
        service = service.with_transcription_host(Box::new(host));
    }
    service
        .start()
        .context("unable to load the paired devices")?;
    Ok((service, network_routes))
}

/// The queues the task reads, filled through its `P2pHandle`.
struct TaskQueues {
    /// Commands from the API, CLI and FFI.
    commands: mpsc::Receiver<P2pRequest>,
    /// Network-address commands.
    network: mpsc::Receiver<NetworkRequest>,
    /// Chunks to transcribe on paired devices.
    remote_transcriptions: mpsc::Receiver<RemoteTranscriptionJob>,
    /// Retained speech parts to send to the device that combines a meeting (#88).
    speech_transfers: mpsc::Receiver<SpeechTransferJob>,
    /// Chat questions for the paired computers (#107).
    paired_chats: mpsc::Receiver<PairedChatJob>,
    /// Fires once to stop the task.
    stop: oneshot::Receiver<()>,
}

/// Runs the event loop until `stop` fires, all handles are gone, or the swarm ends.
///
/// Side effects: Drives the swarm, applies events and commands, and publishes snapshots.
async fn run_service(
    service: &mut DaemonP2pService,
    network_routes: &mut NetworkRoutes,
    snapshot_sender: watch::Sender<P2pSnapshot>,
    queues: TaskQueues,
) {
    let TaskQueues {
        commands: mut command_receiver,
        network: mut network_receiver,
        remote_transcriptions: mut remote_receiver,
        speech_transfers: mut speech_receiver,
        paired_chats: mut chat_receiver,
        mut stop,
    } = queues;
    let mut change_poll = tokio::time::interval(LOCAL_CHANGE_POLL_INTERVAL);
    change_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut paired_peer_redial = tokio::time::interval(PAIRED_PEER_REDIAL_INTERVAL);
    paired_peer_redial.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        service.process_pending_events();
        let snapshot = snapshot_of(service, network_routes);
        snapshot_sender.send_if_modified(|current| {
            let changed = *current != snapshot;
            if changed {
                *current = snapshot;
            }
            changed
        });
        let next_retry_at = service.next_retry_at();
        tokio::select! {
            _ = &mut stop => break,
            () = sleep_until_retry(next_retry_at) => {}
            _ = change_poll.tick() => {
                service.poll_local_changes();
                service.poll_trust_changes();
                settle_invites(service, network_routes);
            }
            _ = paired_peer_redial.tick() => {
                service.transport_mut().redial_paired_peers();
                // Nearby devices open to pairing say their name (#121).
                service.transport_mut().probe_nearby_names();
            }
            swarm_result = service.transport_mut().drive_next_swarm_event() => {
                if let Err(error) = swarm_result {
                    tracing::error!(error = %error, "the P2P swarm stopped");
                    break;
                }
            }
            Some(job) = remote_receiver.recv() => service.request_remote_transcription(job),
            Some(job) = speech_receiver.recv() => service.request_speech_transfer(job),
            Some(job) = chat_receiver.recv() => service.request_paired_chat(job),
            request = command_receiver.recv() => {
                let Some((command, reply_sender)) = request else {
                    break;
                };
                let revoked_peer = match &command {
                    P2pCommand::Revoke(peer_id) => Some(peer_id.clone()),
                    _ => None,
                };
                let outcome = service
                    .handle_command(command)
                    .map_err(|error| error.to_string());
                if let (Some(peer_id), true) = (revoked_peer, outcome.is_ok()) {
                    network_routes.forget_peer(&peer_id, service.transport_mut());
                }
                let _ = reply_sender.send(outcome);
            }
            Some((command, reply_sender)) = network_receiver.recv() => {
                let _ = reply_sender.send(handle_network_command(service, network_routes, command));
            }
        }
    }
    tracing::info!("P2P sync stopped");
}

/// Runs a network command on the task.
///
/// Outputs: Its outcome. `CreateInvite` also opens the pairing window; the invite lasts as long.
/// Side effects: Those of the command.
fn handle_network_command(
    service: &mut DaemonP2pService,
    network_routes: &mut NetworkRoutes,
    command: NetworkCommand,
) -> Result<NetworkCommandOutcome, NetworkCommandError> {
    let trusted_devices = service.status().trusted_devices;
    let is_paired = |peer_id: &DeviceId| {
        trusted_devices
            .iter()
            .any(|device| device.peer_id() == peer_id)
    };
    let opens_window = command == NetworkCommand::CreateInvite;
    let invite_lifetime_ms = u64::try_from(PAIRING_WINDOW.as_millis()).unwrap_or(u64::MAX);
    let outcome = run_network_command(
        command,
        network_routes,
        service.transport_mut(),
        unix_millis(),
        invite_lifetime_ms,
        is_paired,
    )?;
    if opens_window {
        service
            .handle_command(P2pCommand::MakePairable)
            .map_err(|_| NetworkCommandError::Stopped)?;
    }
    Ok(outcome)
}

/// Stores invited devices that are now paired, and drops expired invites.
///
/// Side effects: See `NetworkRoutes::settle_invites`.
fn settle_invites(service: &mut DaemonP2pService, network_routes: &mut NetworkRoutes) {
    let status = service.status();
    network_routes.settle_invites(
        unix_millis(),
        |peer_id| {
            status
                .trusted_devices
                .iter()
                .any(|device| device.peer_id() == peer_id)
        },
        |peer_id| {
            status
                .outgoing_pairings
                .iter()
                .any(|pairing| pairing.peer_id == *peer_id)
        },
        service.transport_mut(),
    );
}

/// Sleeps until `retry_at`, or forever when no retry is scheduled.
async fn sleep_until_retry(retry_at: Option<std::time::Instant>) {
    match retry_at {
        Some(retry_at) => tokio::time::sleep_until(tokio::time::Instant::from_std(retry_at)).await,
        None => std::future::pending().await,
    }
}

/// Captures the service's status and listen addresses.
fn snapshot_of(service: &DaemonP2pService, network_routes: &NetworkRoutes) -> P2pSnapshot {
    P2pSnapshot {
        status: service.status(),
        listen_addresses: service.transport().listen_addresses(),
        network: network_routes.status(service.transport()),
    }
}

/// The current time in milliseconds since the Unix epoch, or 0 before it.
fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}
