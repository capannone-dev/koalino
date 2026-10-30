//! Combines pairing and repository sync on one transport for the daemon's P2P task (Issue #43).
//!
//! The task owns one `P2pService`. It drives the transport's event loop, calls
//! `process_pending_events` after every transport event, and passes the owner's requests (from the
//! HTTP routes) to `handle_command`. `status` returns a snapshot for the Devices screens without
//! touching storage: the paired-device list is cached and refreshed whenever pairing changes it.
//!
//! A device whose pairing completes starts syncing at once: the transport reports the connected,
//! newly trusted peer, so meetings flow without waiting for the next discovery announcement.
//!
//! Every connected paired device is also sent this device's `DeviceManifest` (REQUIREMENTS 3.4):
//! its CPU cores, GPU memory, and the transcription models it can run. Manifests received from
//! paired devices are kept while they stay connected and paired, for the Devices screens and,
//! later, workload routing.
//!
//! When configured with a pair-agreement secret, the service also sends every connected paired
//! device its X25519 public key (`NTK1`, Issue #50). The public key received from a paired device
//! yields the pair key, which is pinned in the allowlist the first time; review clips sent to that
//! device are encrypted with it.
//!
//! On the same one-second tick as local changes, unresolved unknown-speaker clusters are copied
//! to connected paired devices whose pair key is agreed (`NTC1`, Issue #50), so their speakers
//! can be named there. Each device is sent only the chunks and clips it has not been sent and has
//! not sent here, so copies never echo. A share whose meeting has not synced yet is held (bounded
//! in size and time) and retried on later ticks.

use crate::domain::transcription_language::TranscriptionLanguage;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::{Duration, Instant},
};

use uuid::Uuid;

use crate::domain::network_addresses::{NetworkAddresses, RelayState};
use crate::{
    application::{
        attachment_exchange::{
            decode_attachment_manifest, encode_attachment_manifests, is_attachment_manifest,
        },
        attachment_sync::AttachmentSync,
        cluster_share_exchange::{
            decode_cluster_share_message, encode_cluster_share_messages, is_cluster_share_message,
        },
        compute_router::ComputePreference,
        crypto::pair_key::PairAgreementSecret,
        device_manifest_exchange::{decode_manifest_message, encode_manifest_message},
        pair_key_exchange::{
            PairKeyMessageError, decode_pair_key_message, encode_pair_key_message,
        },
        pairing_manager::{
            IncomingPairingRequest, OutgoingPairingStatus, PairingError, PairingManager,
            PairingUpdate,
        },
        repository_sync_manager::{RepositorySyncManager, RepositorySyncManagerError},
        transcription_exchange::{
            TranscriptionRequest, TranscriptionResponse, decode_transcription_request,
            decode_transcription_response, encode_transcription_request,
            encode_transcription_response, is_transcription_request, is_transcription_response,
            wipe_request,
        },
        transcription_routing::{parse_route, route_text},
        voice_library_exchange::{
            decode_voice_library_message, encode_voice_library_messages, is_voice_library_message,
        },
    },
    domain::{
        errors::{StorageError, TranscriptionError},
        models::{
            ClusterDigest, ClusterShare, ClusterShareImport, DeviceId, DeviceManifest, MeetingId,
            TranscriptionModel,
        },
        peer_trust::{DeviceName, PairKey, PairingCode, TrustedPeer},
    },
    ports::{
        file_transfer::{AttachmentLibrary, FileTransferNetwork},
        network::{NetworkEvent, P2pNetwork},
        peer_trust::{PairKeyPinning, PairingTransport, TrustedPeerStore},
        remote_transcription::{
            FinishedTranscription, HostedTranscription, TranscriptionHost, TranscriptionRouteStore,
        },
        storage::NoteRepository,
        voice::VoiceLibrarySync,
    },
};

/// How long `MakePairable` admits connections from unpaired devices.
pub const PAIRING_WINDOW: Duration = Duration::from_secs(5 * 60);

/// How long after a failed delivery sync with that peer restarts. Failures in between collapse
/// into this one restart, so a peer that drops messages costs one retry per delay.
pub const DEFAULT_RETRY_DELAY: Duration = Duration::from_secs(2);

/// Most pairing updates kept for the Devices screens, newest last.
const MAX_RECENT_UPDATES: usize = 16;

/// Most queued network events handled per `process_pending_events` call, so one call always ends.
const MAX_EVENTS_PER_PASS: usize = 1024;
/// How long a paired device may take to transcribe one chunk before this device gives up on it.
pub const REMOTE_TRANSCRIPTION_TIMEOUT: Duration = Duration::from_secs(180);

/// Most review-clip audio one cluster-share message carries, well under the transport's limit.
const CLUSTER_SHARE_AUDIO_BUDGET: usize = 8 * 1024 * 1024;

/// Most clip audio held in memory for shares whose meeting has not synced yet.
const MAX_HELD_SHARE_AUDIO_BYTES: usize = 32 * 1024 * 1024;

/// How long a held share is retried before it is dropped (the sender sends it again at the next
/// connection).
const HELD_SHARE_LIFETIME: Duration = Duration::from_secs(5 * 60);

/// What one paired device already has of one cluster, as far as this device knows.
#[derive(Default)]
struct SharedCluster {
    /// The local digest when the device was last brought up to date.
    digest: Option<ClusterDigest>,
    /// Member chunks sent to or received from the device.
    chunk_ids: HashSet<String>,
    /// Review clips sent to or received from the device.
    clip_ids: HashSet<Uuid>,
}

/// A share received for a meeting that has not synced here yet.
struct HeldShare {
    /// The share; it holds raw audio and embeddings and is never logged.
    share: ClusterShare,
    /// When it arrived.
    received_at: Instant,
}
/// One request from the owner, or from a local writer, to the P2P task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum P2pCommand {
    /// Admits pairing requests from nearby devices for `PAIRING_WINDOW`.
    MakePairable,
    /// Asks a nearby device to pair; the outcome carries the code to show.
    StartPairing(DeviceId),
    /// Records that the owner saw matching codes for a pairing this device started.
    ConfirmCode(DeviceId),
    /// Abandons a pairing this device started.
    CancelPairing(DeviceId),
    /// Accepts another device's request after the owner compared codes.
    AcceptIncoming(DeviceId),
    /// Declines another device's request.
    RejectIncoming(DeviceId),
    /// Removes a paired device and closes its connections.
    Revoke(DeviceId),
    /// Sends a meeting that was just written locally to connected paired devices.
    LocalChange(MeetingId),
    /// Chooses where this device's chunks are transcribed.
    SetTranscriptionRoute(ComputePreference),
}

/// The result of one `P2pCommand`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum P2pCommandOutcome {
    /// The command took effect and has nothing to report.
    Done,
    /// A pairing started; both devices show this code.
    PairingStarted(PairingCode),
    /// A pairing step finished with this update.
    Pairing(PairingUpdate),
    /// Whether the revoked device was paired.
    Revoked(bool),
}

/// Why a P2P command failed.
#[derive(Debug, thiserror::Error)]
pub enum P2pServiceError {
    /// A pairing step failed, for example for a pairing that is not in progress.
    #[error(transparent)]
    Pairing(#[from] PairingError),
    /// Syncing a local change failed.
    #[error(transparent)]
    Sync(#[from] RepositorySyncManagerError),
    /// The chosen transcription device is not paired.
    #[error("the chosen transcription device is not paired")]
    RouteDeviceNotPaired,
    /// The transcription choice could not be stored.
    #[error(transparent)]
    RouteStorage(#[from] StorageError),
}

/// A request about this device's routes to other networks (Issues #64 and #65).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NetworkCommand {
    /// Replaces the relay and static peers, live.
    SetAddresses(NetworkAddresses),
    /// Opens the pairing window and makes an invite that lasts as long as it does.
    CreateInvite,
    /// Lists the device an invite names as a pairing candidate.
    JoinInvite(String),
}

/// The result of one `NetworkCommand`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NetworkCommandOutcome {
    /// The command took effect and has nothing to report.
    Done,
    /// The invite to show, and when it ends.
    Invite {
        /// Invite text, for a link or a QR code.
        text: String,
        /// When it ends, in milliseconds since the Unix epoch.
        expires_at_ms: u64,
    },
    /// The device the invite named, now listed as nearby.
    Joined(DeviceId),
}

/// Why a `NetworkCommand` was refused; nothing changed in any of these cases.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum NetworkCommandError {
    /// The relay address does not parse, lacks `/p2p/<id>`, is a circuit, or names this device.
    #[error("the relay address is invalid")]
    InvalidRelayAddress,
    /// A peer address does not parse, lacks `/p2p/<id>`, or names this device.
    #[error("a peer address is invalid")]
    InvalidPeerAddress,
    /// The invite is not one this version understands.
    #[error("the invite is malformed")]
    InviteMalformed,
    /// The invite's time has passed.
    #[error("the invite has expired")]
    InviteExpired,
    /// The invite was made on this device.
    #[error("the invite was made on this device")]
    InviteFromThisDevice,
    /// The invite's device is already paired with this one.
    #[error("that device is already paired")]
    AlreadyPaired,
    /// This device has no address to put in an invite.
    #[error("this device has no address to share")]
    NoAddresses,
    /// The addresses could not be stored.
    #[error("the addresses could not be stored")]
    Storage,
    /// The P2P task is not running.
    #[error("P2P sync is not running")]
    Stopped,
}

impl NetworkCommandError {
    /// Returns the machine-readable code the apps turn into their own wording.
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidRelayAddress => "invalid_relay_address",
            Self::InvalidPeerAddress => "invalid_peer_address",
            Self::InviteMalformed => "invite_malformed",
            Self::InviteExpired => "invite_expired",
            Self::InviteFromThisDevice => "invite_from_this_device",
            Self::AlreadyPaired => "already_paired",
            Self::NoAddresses => "no_addresses",
            Self::Storage => "storage_failed",
            Self::Stopped => "unavailable",
        }
    }
}

/// This device's routes to other networks, for the status.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NetworkRouteStatus {
    /// The relay in effect, ending in `/p2p/<relay id>`.
    pub relay_address: Option<String>,
    /// How far this device has got with that relay.
    pub relay_state: RelayState,
    /// Static peers in effect; devices paired through invites are not listed.
    pub peers: Vec<String>,
    /// Whether this device is itself a relay for the owner's other devices.
    pub offers_relay: bool,
}

/// A chunk this device wants a paired device to transcribe, and where the answer goes.
pub struct RemoteTranscriptionJob {
    /// The device to ask.
    pub peer: DeviceId,
    /// 16 kHz mono samples; wiped once sealed.
    pub samples: Vec<i16>,
    /// The previous chunk's text.
    pub previous_text: Option<String>,
    /// The language chosen for the chunk.
    pub language: Option<TranscriptionLanguage>,
    /// Receives the text, or `Unavailable` on any failure.
    pub reply: std::sync::mpsc::Sender<Result<String, TranscriptionError>>,
}

/// A job waiting for its answer.
struct PendingTranscription {
    peer: DeviceId,
    reply: std::sync::mpsc::Sender<Result<String, TranscriptionError>>,
    deadline: Instant,
}

/// What the Devices screens show.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct P2pStatus {
    /// This device's peer ID.
    pub local_peer_id: DeviceId,
    /// This device's announced name.
    pub device_name: DeviceName,
    /// Where this device's chunks are transcribed.
    pub transcription_route: ComputePreference,
    /// Paired devices, oldest first.
    pub trusted_devices: Vec<TrustedPeer>,
    /// Unpaired devices visible on the local network.
    pub nearby_devices: Vec<DeviceId>,
    /// Requests awaiting the owner's answer.
    pub incoming_requests: Vec<IncomingPairingRequest>,
    /// Pairings this device started that have not finished.
    pub outgoing_pairings: Vec<OutgoingPairingStatus>,
    /// The latest pairing updates, newest last.
    pub recent_updates: Vec<PairingUpdate>,
    /// The manifest this device sends to its paired devices.
    pub local_manifest: DeviceManifest,
    /// Manifests received from connected paired devices, ordered by peer ID.
    pub peer_manifests: Vec<DeviceManifest>,
}

/// What this device can run, advertised in its manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalCapabilities {
    /// CPU cores available to the app.
    pub cpu_cores: u16,
    /// GPU memory usable for AI work, in megabytes; 0 when none is known.
    pub gpu_vram_megabytes: u32,
    /// Transcription models this device's pipeline is configured to run.
    pub transcription_models: Vec<TranscriptionModel>,
}

impl Default for LocalCapabilities {
    /// A device that runs no models: the available CPU cores and no GPU, as a "dumb" node.
    fn default() -> Self {
        let cpu_cores = std::thread::available_parallelism()
            .map_or(1, |cores| u16::try_from(cores.get()).unwrap_or(u16::MAX));
        Self {
            cpu_cores,
            gpu_vram_megabytes: 0,
            transcription_models: Vec::new(),
        }
    }
}

/// What the Devices screens show, plus the node's listen addresses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct P2pSnapshot {
    /// Pairing and paired-device state.
    pub status: P2pStatus,
    /// Addresses the node listens on, as multiaddress text.
    pub listen_addresses: Vec<String>,
    /// Routes to other networks.
    pub network: NetworkRouteStatus,
}

/// Why a driving adapter's P2P request failed.
#[derive(Debug, thiserror::Error)]
pub enum P2pControlError {
    /// The service refused the command, for example for a pairing that is not in progress.
    #[error("{0}")]
    Refused(String),
    /// The P2P task is not running.
    #[error("P2P sync is not running")]
    Stopped,
}

/// Lets driving adapters, such as the HTTP routes, reach the running P2P task.
#[async_trait::async_trait]
pub trait P2pControl: Send + Sync {
    /// Returns the latest snapshot. Side effects: None.
    fn snapshot(&self) -> P2pSnapshot;

    /// Runs `command` on the P2P task and returns its outcome.
    ///
    /// Outputs: The outcome, `Refused` when the command failed, or `Stopped`.
    /// Side effects: Those of the command.
    async fn command(&self, command: P2pCommand) -> Result<P2pCommandOutcome, P2pControlError>;

    /// Runs a network `command` on the P2P task and returns its outcome.
    ///
    /// Outputs: The outcome or why it was refused; by default `Stopped`.
    /// Side effects: Those of the command.
    async fn network_command(
        &self,
        _command: NetworkCommand,
    ) -> Result<NetworkCommandOutcome, NetworkCommandError> {
        Err(NetworkCommandError::Stopped)
    }

    /// Reports that `meeting_id` was just written locally, without waiting.
    ///
    /// Side effects: Queues a `LocalChange` for the task; a full queue is logged, and the change
    /// then reaches peers at their next connection.
    fn notify_local_change(&self, meeting_id: &MeetingId);
}

/// Pairing plus whole-repository sync over one shared transport.
pub struct P2pService<Transport, Store, Repository> {
    /// Owns the transport and the allowlist.
    pairing_manager: PairingManager<Transport, Store>,
    /// Syncs meetings over the pairing manager's transport.
    sync_manager: RepositorySyncManager<Repository>,
    /// The allowlist as last read, so `status` needs no storage access.
    trusted_devices: Vec<TrustedPeer>,
    /// The latest pairing updates, newest last, at most `MAX_RECENT_UPDATES`.
    recent_updates: VecDeque<PairingUpdate>,
    /// Delay between a failed delivery and the restart it schedules.
    retry_delay: Duration,
    /// Peers whose sync restarts at the given instant.
    scheduled_restarts: HashMap<DeviceId, Instant>,
    /// The newest document revision already sent to connected peers.
    last_seen_revision: u64,
    /// This device's manifest, sent to every paired device when it connects.
    local_manifest: DeviceManifest,
    /// Manifests of connected paired devices, by peer.
    peer_manifests: HashMap<DeviceId, DeviceManifest>,
    /// This device's X25519 secret; without it no pair keys are agreed and no clusters shared.
    pair_agreement_secret: Option<PairAgreementSecret>,
    /// Paired devices currently connected.
    connected_peers: HashSet<DeviceId>,
    /// What each connected device has of each cluster.
    shared_clusters: HashMap<DeviceId, HashMap<Uuid, SharedCluster>>,
    /// Shares waiting for their meeting, oldest first.
    held_shares: Vec<HeldShare>,
    /// Where this device's chunks are transcribed.
    transcription_route: ComputePreference,
    /// Keeps the choice across restarts; `None` keeps it in memory only.
    transcription_route_store: Option<Box<dyn TranscriptionRouteStore>>,
    /// Runs paired devices' transcriptions; `None` refuses them.
    transcription_host: Option<Box<dyn TranscriptionHost>>,
    /// Chunks sent to paired devices, by job ID.
    pending_transcriptions: HashMap<Uuid, PendingTranscription>,
    /// Attachment exchange with paired devices; `None` leaves attachments on this device.
    attachment_sync: Option<AttachmentSync>,
    /// The voice library to keep in step with paired devices; `None` leaves it on this device.
    voice_library: Option<Box<dyn VoiceLibrarySync>>,
    /// The library digest each connected device was last sent.
    shared_voice_digests: HashMap<DeviceId, [u8; 32]>,
}

impl<Transport, Store, Repository> P2pService<Transport, Store, Repository>
where
    Transport: PairingTransport + P2pNetwork + FileTransferNetwork,
    Store: TrustedPeerStore,
    Repository: NoteRepository,
{
    /// Combines the two managers. Call `start` before driving the transport.
    ///
    /// Side effects: None.
    pub fn new(
        pairing_manager: PairingManager<Transport, Store>,
        sync_manager: RepositorySyncManager<Repository>,
    ) -> Self {
        let local_manifest = local_manifest(
            pairing_manager.transport().local_peer_id(),
            LocalCapabilities::default(),
        );
        Self {
            pairing_manager,
            sync_manager,
            trusted_devices: Vec::new(),
            recent_updates: VecDeque::new(),
            retry_delay: DEFAULT_RETRY_DELAY,
            scheduled_restarts: HashMap::new(),
            last_seen_revision: 0,
            local_manifest,
            peer_manifests: HashMap::new(),
            pair_agreement_secret: None,
            connected_peers: HashSet::new(),
            shared_clusters: HashMap::new(),
            held_shares: Vec::new(),
            transcription_route: ComputePreference::LocalOnly,
            transcription_route_store: None,
            transcription_host: None,
            pending_transcriptions: HashMap::new(),
            attachment_sync: None,
            voice_library: None,
            shared_voice_digests: HashMap::new(),
        }
    }

    /// Agrees pair keys with paired devices using `secret` (Issue #50).
    ///
    /// Inputs: This device's stored X25519 secret.
    /// Outputs: The service. Side effects: None; public keys are sent as devices connect.
    pub fn with_pair_agreement_secret(mut self, secret: PairAgreementSecret) -> Self {
        self.pair_agreement_secret = Some(secret);
        self
    }

    /// Keeps `voice_library` in step with paired devices (Issue #56). It needs a pair agreement
    /// secret too, since embeddings travel only under a pair key.
    ///
    /// Inputs: The voice library. Outputs: The service. Side effects: None until devices connect.
    pub fn with_voice_library(mut self, voice_library: Box<dyn VoiceLibrarySync>) -> Self {
        self.voice_library = Some(voice_library);
        self
    }

    /// Exchanges attachments with paired devices (Issue #57). It needs a pair agreement secret
    /// too, since the lists of attachments travel only under a pair key.
    ///
    /// Inputs: The local attachments. Outputs: The service. Side effects: None until devices
    /// connect.
    pub fn with_attachment_library(mut self, library: Box<dyn AttachmentLibrary>) -> Self {
        self.attachment_sync = Some(AttachmentSync::new(library));
        self
    }

    /// Runs paired devices' transcriptions on `host` (Issue #59). It needs a pair agreement secret
    /// too, since audio travels only under a pair key.
    ///
    /// Inputs: The host. Outputs: The service. Side effects: None until a device asks.
    pub fn with_transcription_host(mut self, host: Box<dyn TranscriptionHost>) -> Self {
        self.transcription_host = Some(host);
        self
    }

    /// Keeps the transcription choice in `store` and starts from what it holds (Issue #59).
    ///
    /// Inputs: The store. Outputs: The service.
    /// Side effects: Reads the store; a failure is logged and the device stays on Local, the
    /// choice that sends no audio anywhere.
    pub fn with_transcription_route_store(
        mut self,
        store: Box<dyn TranscriptionRouteStore>,
    ) -> Self {
        match store.load_transcription_route() {
            Ok(Some(route)) => self.transcription_route = parse_route(&route),
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(error = %error, "the transcription choice could not be read");
            }
        }
        self.transcription_route_store = Some(store);
        self
    }

    /// Sets what this device advertises to its paired devices.
    ///
    /// Inputs: The capabilities (normally from the pipeline configuration).
    /// Outputs: The service. Side effects: None; the manifest is sent as devices connect.
    pub fn with_local_capabilities(mut self, capabilities: LocalCapabilities) -> Self {
        self.local_manifest = local_manifest(
            self.pairing_manager.transport().local_peer_id(),
            capabilities,
        );
        self
    }

    /// Replaces `DEFAULT_RETRY_DELAY`, for example with a short delay in tests.
    pub fn with_retry_delay(mut self, retry_delay: Duration) -> Self {
        self.retry_delay = retry_delay;
        self
    }

    /// When the earliest scheduled restart is due, so the task loop can wake for it.
    ///
    /// Side effects: None.
    pub fn next_retry_at(&self) -> Option<Instant> {
        self.scheduled_restarts.values().min().copied()
    }

    /// Admits the stored paired devices into the transport and caches the allowlist.
    ///
    /// Outputs: Success, or a storage or network error.
    /// Side effects: Reads the allowlist and updates the transport's connection gate.
    pub fn start(&mut self) -> Result<(), P2pServiceError> {
        self.pairing_manager.load_trusted_peers()?;
        self.refresh_trusted_devices()?;
        // Earlier writes reach each peer through the handshake when it connects.
        self.last_seen_revision = self
            .sync_manager
            .repository()
            .document_changes_since(0)
            .map_err(RepositorySyncManagerError::from)?
            .latest_revision;
        Ok(())
    }

    /// Sends every meeting written since the last call to the connected peers.
    ///
    /// The task calls this about once a second. It finds writes from any connection or process:
    /// the recording pipeline saving a transcript, the HTTP API, the CLI, the app. It also finds
    /// documents this task merged from one peer, so they are forwarded to the other connected
    /// peers. A merge that adds nothing writes nothing, so devices never echo each other.
    ///
    /// It also copies new unknown-speaker clusters, members, and clips to connected paired devices
    /// and retries shares held for meetings that have since synced.
    ///
    /// Side effects: Reads document revisions and sends sync messages; reads clusters and sends
    /// sealed shares; may store held shares. Failures are logged and retried on the next call, or
    /// at the peer's next connection.
    pub fn poll_local_changes(&mut self) {
        self.send_document_changes();
        self.share_clusters();
        self.retry_held_shares();
        self.share_voice_library();
        self.share_attachments();
        self.expire_pending_transcriptions();
    }

    /// Sends every meeting document written since the last call to the connected peers.
    ///
    /// Side effects: Reads document revisions and sends sync messages; failures are logged.
    fn send_document_changes(&mut self) {
        let changes = match self
            .sync_manager
            .repository()
            .document_changes_since(self.last_seen_revision)
        {
            Ok(changes) => changes,
            Err(error) => {
                tracing::warn!(error = %error, "local changes could not be listed");
                return;
            }
        };
        self.last_seen_revision = changes.latest_revision;
        for meeting_id in changes.meeting_ids {
            if let Err(error) = self
                .sync_manager
                .notify_local_change(self.pairing_manager.transport_mut(), &meeting_id)
            {
                tracing::warn!(error = %error, "a local change could not be sent");
            }
        }
    }

    /// Applies allowlist changes made outside this task, such as the CLI's `devices revoke`.
    ///
    /// The task calls this about once a second, with `poll_local_changes`. A device removed from
    /// the stored allowlist is dropped from the transport's gate and disconnected; the status
    /// snapshot is refreshed when anything changed.
    /// Side effects: Reads the allowlist; may revoke or admit peers in the transport. Failures are
    /// logged and retried on the next call.
    pub fn poll_trust_changes(&mut self) {
        match self.pairing_manager.reconcile_trusted_peers() {
            Ok(false) => {}
            Ok(true) => {
                if let Err(error) = self.refresh_trusted_devices() {
                    tracing::warn!(error = %error, "paired devices could not be listed");
                }
            }
            Err(error) => {
                tracing::warn!(error = %error, "allowlist changes could not be applied");
            }
        }
    }

    /// Handles everything the transport queued since the last call.
    ///
    /// Side effects: Applies sync messages (persisting merged documents and creating meetings),
    /// completes pairings (persisting trust), and starts syncing with newly paired devices.
    /// Errors concern one event or one peer, so they are logged and processing continues.
    pub fn process_pending_events(&mut self) {
        // Pairing first: completing a pairing trusts the peer, which queues the `PeerDiscovered`
        // that the sync pass below must see in this same call. Nothing else would wake the task.
        match self.pairing_manager.process_events() {
            Ok(updates) => {
                for update in updates {
                    self.record_update(update);
                }
            }
            Err(error) => tracing::warn!(error = %error, "a pairing event could not be applied"),
        }
        for _ in 0..MAX_EVENTS_PER_PASS {
            let network_event = match self.pairing_manager.transport_mut().poll_event() {
                Ok(Some(network_event)) => network_event,
                Ok(None) => break,
                Err(error) => {
                    tracing::warn!(error = %error, "a network event could not be read");
                    break;
                }
            };
            self.handle_network_event(network_event);
        }
        self.process_file_transfers();
        self.send_finished_transcriptions();
        self.run_scheduled_restarts();
    }

    /// Routes one network event: manifests and pair keys are handled here, everything else goes
    /// to sync.
    ///
    /// Side effects: On a connected peer, starts sync and sends this device's manifest and public
    /// key; on an expired peer, forgets its manifest; on a manifest message, stores it when it
    /// names its sender; on a public key, pins the pair key. Failures are logged without payload
    /// contents.
    fn handle_network_event(&mut self, network_event: NetworkEvent) {
        let connected_peer = match &network_event {
            NetworkEvent::MessageReceived {
                peer_device_id,
                payload,
            } => {
                if let Some(decoded) = decode_manifest_message(payload) {
                    self.accept_manifest(peer_device_id, decoded);
                    return;
                }
                if let Some(decoded) = decode_pair_key_message(payload) {
                    self.accept_pair_public_key(peer_device_id, decoded);
                    return;
                }
                if is_transcription_request(payload) {
                    self.accept_transcription_request(peer_device_id, payload);
                    return;
                }
                if is_transcription_response(payload) {
                    self.accept_transcription_response(peer_device_id, payload);
                    return;
                }
                if is_attachment_manifest(payload) {
                    self.accept_attachment_manifest(peer_device_id, payload);
                    return;
                }
                if is_voice_library_message(payload) {
                    self.accept_voice_library(peer_device_id, payload);
                    return;
                }
                if is_cluster_share_message(payload) {
                    self.accept_cluster_share(peer_device_id, payload);
                    return;
                }
                None
            }
            NetworkEvent::PeerDiscovered(peer_device_id) => {
                // A (re)connected device may have lost nothing, but sending again is idempotent
                // and covers shares it could not store before.
                self.connected_peers.insert(peer_device_id.clone());
                self.shared_clusters.remove(peer_device_id);
                self.shared_voice_digests.remove(peer_device_id);
                if let Some(attachment_sync) = self.attachment_sync.as_mut() {
                    attachment_sync.forget_shared(peer_device_id);
                }
                Some(peer_device_id.clone())
            }
            NetworkEvent::PeerExpired(peer_device_id) => {
                self.peer_manifests.remove(peer_device_id);
                self.connected_peers.remove(peer_device_id);
                self.shared_clusters.remove(peer_device_id);
                self.shared_voice_digests.remove(peer_device_id);
                if let Some(attachment_sync) = self.attachment_sync.as_mut() {
                    attachment_sync.peer_lost(peer_device_id);
                }
                self.fail_pending_transcriptions(peer_device_id);
                None
            }
            // The device may have missed the voice library or attachment list, so they are sent
            // again.
            NetworkEvent::DeliveryFailed(peer_device_id) => {
                self.shared_voice_digests.remove(peer_device_id);
                if let Some(attachment_sync) = self.attachment_sync.as_mut() {
                    attachment_sync.forget_shared(peer_device_id);
                }
                // A request may be what was lost, so nobody would ever answer it.
                self.fail_pending_transcriptions(peer_device_id);
                None
            }
        };
        if let Err(error) = self
            .sync_manager
            .handle_network_event(self.pairing_manager.transport_mut(), network_event)
        {
            tracing::warn!(error = %error, "a sync message could not be applied");
        }
        if let Some(peer_device_id) = connected_peer {
            self.send_manifest(&peer_device_id);
            self.send_pair_public_key(&peer_device_id);
        }
    }

    /// Derives and pins the pair key from a public key `peer_device_id` sent.
    ///
    /// Side effects: May store the sealed key in the allowlist. A malformed or low-order key, a
    /// key from an unpaired device, and a key that differs from the pinned one are logged (peer ID
    /// only) and ignored; the pinned key stays.
    fn accept_pair_public_key(
        &mut self,
        peer_device_id: &DeviceId,
        decoded: Result<[u8; 32], PairKeyMessageError>,
    ) {
        let Some(secret) = &self.pair_agreement_secret else {
            return;
        };
        let local_peer_id = self.pairing_manager.transport().local_peer_id();
        let pinning = decoded
            .map_err(|error| error.to_string())
            .and_then(|public_key| {
                secret
                    .derive_pair_key(&local_peer_id, peer_device_id, &public_key)
                    .map_err(|error| error.to_string())
            })
            .and_then(|pair_key| {
                self.pairing_manager
                    .store()
                    .pin_pair_key(peer_device_id, &pair_key)
                    .map_err(|error| error.to_string())
            });
        match pinning {
            Ok(PairKeyPinning::Stored | PairKeyPinning::AlreadyPinned) => {}
            Ok(PairKeyPinning::Mismatch) => tracing::warn!(
                peer = %peer_device_id.as_str(),
                "a paired device sent a different agreement key; the pinned pair key is kept"
            ),
            Ok(PairKeyPinning::UnknownPeer) => tracing::warn!(
                peer = %peer_device_id.as_str(),
                "an agreement key from an unpaired device was ignored"
            ),
            Err(error) => tracing::warn!(
                peer = %peer_device_id.as_str(),
                error = %error,
                "an agreement key was rejected"
            ),
        }
    }

    /// Sends the voice library to every connected device that was not sent this version of it.
    ///
    /// Side effects: Reads the digest each call and, when a device needs it, decrypts the library
    /// and sends sealed messages. Failures are logged without content and retried next call.
    fn share_voice_library(&mut self) {
        if self.connected_peers.is_empty() || self.pair_agreement_secret.is_none() {
            return;
        }
        let Some(voice_library) = self.voice_library.as_ref() else {
            return;
        };
        let digest = match voice_library.voice_library_digest() {
            Ok(digest) => digest,
            Err(error) => {
                tracing::warn!(error = %error, "the voice library could not be read");
                return;
            }
        };
        let waiting: Vec<DeviceId> = self
            .connected_peers
            .iter()
            .filter(|peer| self.shared_voice_digests.get(*peer) != Some(&digest))
            .cloned()
            .collect();
        if waiting.is_empty() {
            return;
        }
        let library = match voice_library.export_voice_library() {
            Ok(library) => library,
            Err(error) => {
                tracing::warn!(error = %error, "the voice library could not be exported");
                return;
            }
        };
        let local_peer_id = self.pairing_manager.transport().local_peer_id();
        for peer_device_id in waiting {
            let pair_key = match self.pairing_manager.store().pair_key(&peer_device_id) {
                Ok(Some(pair_key)) => pair_key,
                Ok(None) => continue,
                Err(error) => {
                    tracing::warn!(
                        peer = %peer_device_id.as_str(),
                        error = %error,
                        "the pair key could not be loaded"
                    );
                    continue;
                }
            };
            let messages = match encode_voice_library_messages(
                &library,
                &pair_key,
                &local_peer_id,
                &peer_device_id,
            ) {
                Ok(messages) => messages,
                Err(error) => {
                    tracing::warn!(error = %error, "the voice library could not be sealed");
                    continue;
                }
            };
            let transport = self.pairing_manager.transport_mut();
            let all_sent = messages.iter().all(|message| {
                match transport.send_encrypted(&peer_device_id, message) {
                    Ok(()) => true,
                    Err(error) => {
                        tracing::warn!(
                            peer = %peer_device_id.as_str(),
                            error = %error,
                            "the voice library could not be sent"
                        );
                        false
                    }
                }
            });
            if all_sent {
                self.shared_voice_digests.insert(peer_device_id, digest);
            }
        }
    }

    /// Opens and merges a voice library `peer_device_id` sent.
    ///
    /// Side effects: May write identities and variants and delete others. A message from a device
    /// without an agreed pair key, or one that fails to open or merge, is logged (peer ID only) and
    /// dropped.
    fn accept_voice_library(&mut self, peer_device_id: &DeviceId, payload: &[u8]) {
        let Some(voice_library) = self.voice_library.as_ref() else {
            return;
        };
        let pair_key = match self.pairing_manager.store().pair_key(peer_device_id) {
            Ok(Some(pair_key)) => pair_key,
            Ok(None) => {
                tracing::warn!(
                    peer = %peer_device_id.as_str(),
                    "a voice library arrived before the pair key was agreed and was dropped"
                );
                return;
            }
            Err(error) => {
                tracing::warn!(
                    peer = %peer_device_id.as_str(),
                    error = %error,
                    "the pair key could not be loaded"
                );
                return;
            }
        };
        let local_peer_id = self.pairing_manager.transport().local_peer_id();
        let merged =
            decode_voice_library_message(payload, &pair_key, peer_device_id, &local_peer_id)
                .map_err(|error| error.to_string())
                .and_then(|library| {
                    voice_library
                        .import_voice_library(&library)
                        .map_err(|error| error.to_string())
                });
        if let Err(error) = merged {
            tracing::warn!(
                peer = %peer_device_id.as_str(),
                error = %error,
                "a voice library was rejected"
            );
        }
    }

    /// Sends a chunk to a paired device.
    ///
    /// Inputs: The job, whose answer goes to its `reply`.
    /// Side effects: Seals and sends the audio, then wipes this copy. A device that is not
    /// connected, or has no agreed pair key, gets nothing and the reply is `Unavailable`.
    pub fn request_remote_transcription(&mut self, job: RemoteTranscriptionJob) {
        let RemoteTranscriptionJob {
            peer,
            samples,
            previous_text,
            language,
            reply,
        } = job;
        let job_id = Uuid::new_v4();
        let mut request = TranscriptionRequest {
            job_id,
            previous_text,
            language,
            samples,
        };
        let sent = self.seal_and_send_request(&peer, &request);
        wipe_request(&mut request);
        if sent {
            self.pending_transcriptions.insert(
                job_id,
                PendingTranscription {
                    peer,
                    reply,
                    deadline: Instant::now() + REMOTE_TRANSCRIPTION_TIMEOUT,
                },
            );
        } else {
            let _ = reply.send(Err(TranscriptionError::Unavailable));
        }
    }

    /// Seals and sends a request.
    ///
    /// Outputs: Whether it was queued. Side effects: Reads the pair key, sends; failures logged.
    fn seal_and_send_request(&mut self, peer: &DeviceId, request: &TranscriptionRequest) -> bool {
        if !self.connected_peers.contains(peer) {
            return false;
        }
        let Some(pair_key) = self.usable_pair_key(peer) else {
            return false;
        };
        let local_peer_id = self.pairing_manager.transport().local_peer_id();
        let sent = encode_transcription_request(request, &pair_key, &local_peer_id, peer)
            .map_err(|error| error.to_string())
            .and_then(|message| {
                self.pairing_manager
                    .transport_mut()
                    .send_encrypted(peer, &message)
                    .map_err(|error| error.to_string())
            });
        if let Err(error) = &sent {
            tracing::warn!(peer = %peer.as_str(), error = %error, "a chunk could not be sent for transcription");
        }
        sent.is_ok()
    }

    /// Loads the pair key for `peer`.
    ///
    /// Outputs: The key, or `None` when none is agreed or it cannot be read (logged, peer ID only).
    /// Side effects: Reads the store.
    fn usable_pair_key(&self, peer: &DeviceId) -> Option<PairKey> {
        match self.pairing_manager.store().pair_key(peer) {
            Ok(pair_key) => pair_key,
            Err(error) => {
                tracing::warn!(peer = %peer.as_str(), error = %error, "the pair key could not be loaded");
                None
            }
        }
    }

    /// Opens a chunk a paired device sent and queues it on the host.
    ///
    /// Side effects: Holds the audio in the host's queue. A device without an agreed key, or a
    /// message that does not open, is logged and dropped; a device this one cannot serve (no
    /// host, or a full queue) is told so at once and its audio is wiped.
    fn accept_transcription_request(&mut self, peer: &DeviceId, payload: &[u8]) {
        let Some(pair_key) = self.usable_pair_key(peer) else {
            tracing::warn!(peer = %peer.as_str(), "a chunk arrived without an agreed pair key and was dropped");
            return;
        };
        let local_peer_id = self.pairing_manager.transport().local_peer_id();
        let mut request =
            match decode_transcription_request(payload, &pair_key, peer, &local_peer_id) {
                Ok(request) => request,
                Err(error) => {
                    tracing::warn!(peer = %peer.as_str(), error = %error, "a chunk was rejected");
                    return;
                }
            };
        let job_id = request.job_id;
        let accepted = match self.transcription_host.as_mut() {
            Some(host) => host.submit(HostedTranscription {
                peer: peer.clone(),
                job_id,
                samples: std::mem::take(&mut request.samples),
                previous_text: request.previous_text.take(),
                language: request.language,
            }),
            None => {
                wipe_request(&mut request);
                false
            }
        };
        if !accepted {
            self.send_transcription_response(FinishedTranscription {
                peer: peer.clone(),
                job_id,
                text: None,
            });
        }
    }

    /// Sends the answers of finished hosted jobs.
    ///
    /// Side effects: Seals and sends each; a device that left simply misses it.
    fn send_finished_transcriptions(&mut self) {
        for _ in 0..MAX_EVENTS_PER_PASS {
            let Some(finished) = self
                .transcription_host
                .as_mut()
                .and_then(|host| host.poll_finished())
            else {
                return;
            };
            self.send_transcription_response(finished);
        }
    }

    /// Seals and sends one answer.
    ///
    /// Side effects: Reads the pair key and sends; failures are logged without text.
    fn send_transcription_response(&mut self, finished: FinishedTranscription) {
        let FinishedTranscription { peer, job_id, text } = finished;
        let Some(pair_key) = self.usable_pair_key(&peer) else {
            return;
        };
        let local_peer_id = self.pairing_manager.transport().local_peer_id();
        let sent = encode_transcription_response(
            &TranscriptionResponse { job_id, text },
            &pair_key,
            &local_peer_id,
            &peer,
        )
        .map_err(|error| error.to_string())
        .and_then(|message| {
            self.pairing_manager
                .transport_mut()
                .send_encrypted(&peer, &message)
                .map_err(|error| error.to_string())
        });
        if let Err(error) = sent {
            tracing::warn!(peer = %peer.as_str(), error = %error, "a transcript could not be sent back");
        }
    }

    /// Hands a paired device's answer to whoever waits for it.
    ///
    /// Side effects: Ends the pending job. An answer from a device that was not asked, or to an
    /// unknown job, is ignored.
    fn accept_transcription_response(&mut self, peer: &DeviceId, payload: &[u8]) {
        let Some(pair_key) = self.usable_pair_key(peer) else {
            return;
        };
        let local_peer_id = self.pairing_manager.transport().local_peer_id();
        let response = match decode_transcription_response(payload, &pair_key, peer, &local_peer_id)
        {
            Ok(response) => response,
            Err(error) => {
                tracing::warn!(peer = %peer.as_str(), error = %error, "a transcript was rejected");
                return;
            }
        };
        if self
            .pending_transcriptions
            .get(&response.job_id)
            .is_none_or(|pending| pending.peer != *peer)
        {
            return;
        }
        if let Some(pending) = self.pending_transcriptions.remove(&response.job_id) {
            let _ = pending
                .reply
                .send(response.text.ok_or(TranscriptionError::Unavailable));
        }
    }

    /// Fails every job waiting on `peer`, so the pipeline falls back at once.
    ///
    /// Side effects: Answers those jobs with `Unavailable`.
    fn fail_pending_transcriptions(&mut self, peer: &DeviceId) {
        self.pending_transcriptions.retain(|_, pending| {
            let keep = pending.peer != *peer;
            if !keep {
                let _ = pending.reply.send(Err(TranscriptionError::Unavailable));
            }
            keep
        });
    }

    /// Fails jobs past their deadline. Side effects: Answers them with `Unavailable`.
    fn expire_pending_transcriptions(&mut self) {
        let now = Instant::now();
        self.pending_transcriptions.retain(|_, pending| {
            let keep = pending.deadline > now;
            if !keep {
                let _ = pending.reply.send(Err(TranscriptionError::Unavailable));
            }
            keep
        });
    }

    /// Tells connected devices which attachments exist, then advances transfers.
    ///
    /// Side effects: May hash one new attachment file; sends sealed lists to devices that were
    /// not sent the current one; may queue chunk requests. Failures are logged without names and
    /// retried on the next call.
    fn share_attachments(&mut self) {
        if self.pair_agreement_secret.is_none() {
            return;
        }
        let Some(attachment_sync) = self.attachment_sync.as_mut() else {
            return;
        };
        if let Some(pending) = attachment_sync.pending_manifest(&self.connected_peers) {
            let local_peer_id = self.pairing_manager.transport().local_peer_id();
            for peer_device_id in pending.peers {
                let pair_key = match self.pairing_manager.store().pair_key(&peer_device_id) {
                    Ok(Some(pair_key)) => pair_key,
                    Ok(None) => continue,
                    Err(error) => {
                        tracing::warn!(
                            peer = %peer_device_id.as_str(),
                            error = %error,
                            "the pair key could not be loaded"
                        );
                        continue;
                    }
                };
                let messages = match encode_attachment_manifests(
                    &pending.attachments,
                    &pair_key,
                    &local_peer_id,
                    &peer_device_id,
                ) {
                    Ok(messages) => messages,
                    Err(error) => {
                        tracing::warn!(error = %error, "an attachment list could not be sealed");
                        continue;
                    }
                };
                let transport = self.pairing_manager.transport_mut();
                let all_sent = messages.iter().all(|message| {
                    transport
                        .send_encrypted(&peer_device_id, message)
                        .map_err(|error| {
                            tracing::warn!(
                                peer = %peer_device_id.as_str(),
                                error = %error,
                                "an attachment list could not be sent"
                            );
                        })
                        .is_ok()
                });
                if all_sent {
                    attachment_sync.mark_shared(peer_device_id, pending.digest);
                }
            }
        }
        attachment_sync.pump(self.pairing_manager.transport_mut(), Instant::now());
    }

    /// Opens an attachment list `peer_device_id` sent and notes what this device lacks.
    ///
    /// Side effects: Reads rows and updates the wanted list; transfers start on the next tick. A
    /// message that does not open or validate is logged (peer ID only) and dropped.
    fn accept_attachment_manifest(&mut self, peer_device_id: &DeviceId, payload: &[u8]) {
        let Some(attachment_sync) = self.attachment_sync.as_mut() else {
            return;
        };
        let pair_key = match self.pairing_manager.store().pair_key(peer_device_id) {
            Ok(Some(pair_key)) => pair_key,
            Ok(None) => {
                tracing::warn!(
                    peer = %peer_device_id.as_str(),
                    "an attachment list arrived before the pair key was agreed and was dropped"
                );
                return;
            }
            Err(error) => {
                tracing::warn!(
                    peer = %peer_device_id.as_str(),
                    error = %error,
                    "the pair key could not be loaded"
                );
                return;
            }
        };
        let local_peer_id = self.pairing_manager.transport().local_peer_id();
        match decode_attachment_manifest(payload, &pair_key, peer_device_id, &local_peer_id) {
            Ok(attachments) => attachment_sync.accept_manifest(peer_device_id, attachments),
            Err(error) => tracing::warn!(
                peer = %peer_device_id.as_str(),
                error = %error,
                "an attachment list was rejected"
            ),
        }
    }

    /// Serves and absorbs what arrived on the file-transfer protocol.
    ///
    /// Side effects: Reads and writes attachment files and queues frames. Without attachment sync
    /// the frames are dropped.
    fn process_file_transfers(&mut self) {
        for _ in 0..MAX_EVENTS_PER_PASS {
            let transport = self.pairing_manager.transport_mut();
            let Some(event) = transport.poll_file_transfer() else {
                return;
            };
            if let Some(attachment_sync) = self.attachment_sync.as_mut() {
                attachment_sync.handle_event(transport, event);
            }
        }
    }

    /// Copies each changed unresolved cluster to every connected device with an agreed pair key.
    ///
    /// Side effects: Reads cluster digests, and for changed clusters their shares (decrypting
    /// embeddings and audio); sends sealed messages. Failures are logged and retried next tick.
    fn share_clusters(&mut self) {
        if self.connected_peers.is_empty() || self.pair_agreement_secret.is_none() {
            return;
        }
        let digests = match self.sync_manager.repository().unresolved_cluster_digests() {
            Ok(digests) => digests,
            Err(error) => {
                tracing::warn!(error = %error, "unknown-speaker clusters could not be listed");
                return;
            }
        };
        let mut peers: Vec<DeviceId> = self.connected_peers.iter().cloned().collect();
        peers.sort_by(|first, second| first.as_str().cmp(second.as_str()));
        for peer_device_id in peers {
            let known_clusters = self.shared_clusters.get(&peer_device_id);
            let changed: Vec<ClusterDigest> = digests
                .iter()
                .filter(|digest| {
                    known_clusters
                        .and_then(|clusters| clusters.get(&digest.cluster_id()))
                        .and_then(|cluster| cluster.digest)
                        != Some(**digest)
                })
                .copied()
                .collect();
            if changed.is_empty() {
                continue;
            }
            let pair_key = match self.pairing_manager.store().pair_key(&peer_device_id) {
                Ok(Some(pair_key)) => pair_key,
                Ok(None) => continue,
                Err(error) => {
                    tracing::warn!(
                        peer = %peer_device_id.as_str(),
                        error = %error,
                        "the pair key could not be loaded"
                    );
                    continue;
                }
            };
            for digest in changed {
                self.share_cluster(&peer_device_id, &pair_key, digest);
            }
        }
    }

    /// Sends one device the chunks and clips of one cluster it does not have yet.
    ///
    /// Side effects: Reads the cluster's share; sends sealed messages; records what was sent.
    /// A failure is logged and leaves the record unchanged, so the next tick retries.
    fn share_cluster(
        &mut self,
        peer_device_id: &DeviceId,
        pair_key: &crate::domain::peer_trust::PairKey,
        digest: ClusterDigest,
    ) {
        let cluster_id = digest.cluster_id();
        let share = match self
            .sync_manager
            .repository()
            .load_cluster_share(cluster_id)
        {
            Ok(Some(share)) => share,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!(
                    cluster_id = %cluster_id,
                    error = %error,
                    "an unknown-speaker cluster could not be loaded for sharing"
                );
                return;
            }
        };
        let known = self
            .shared_clusters
            .get(peer_device_id)
            .and_then(|clusters| clusters.get(&cluster_id));
        let has_new_chunks = share
            .chunks()
            .iter()
            .any(|chunk| known.is_none_or(|known| !known.chunk_ids.contains(chunk.chunk_id())));
        let new_clips: Vec<_> = share
            .clips()
            .iter()
            .filter(|clip| known.is_none_or(|known| !known.clip_ids.contains(&clip.clip_id())))
            .cloned()
            .collect();
        if has_new_chunks || !new_clips.is_empty() {
            let outgoing =
                ClusterShare::new(share.cluster().clone(), share.chunks().to_vec(), new_clips);
            let local_peer_id = self.pairing_manager.transport().local_peer_id();
            let sent = encode_cluster_share_messages(
                &outgoing,
                pair_key,
                &local_peer_id,
                peer_device_id,
                CLUSTER_SHARE_AUDIO_BUDGET,
            )
            .map_err(|error| error.to_string())
            .and_then(|messages| {
                messages.iter().try_for_each(|message| {
                    self.pairing_manager
                        .transport_mut()
                        .send_encrypted(peer_device_id, message)
                        .map_err(|error| error.to_string())
                })
            });
            if let Err(error) = sent {
                tracing::warn!(
                    peer = %peer_device_id.as_str(),
                    cluster_id = %cluster_id,
                    error = %error,
                    "an unknown-speaker cluster could not be shared"
                );
                return;
            }
            self.remember_shared(peer_device_id, &outgoing);
            tracing::debug!(
                peer = %peer_device_id.as_str(),
                cluster_id = %cluster_id,
                clip_count = outgoing.clips().len(),
                "unknown-speaker cluster shared"
            );
        }
        self.shared_clusters
            .entry(peer_device_id.clone())
            .or_default()
            .entry(cluster_id)
            .or_default()
            .digest = Some(digest);
    }

    /// Records that `peer_device_id` has the chunks and clips of `share`.
    ///
    /// Side effects: Updates the in-memory record only.
    fn remember_shared(&mut self, peer_device_id: &DeviceId, share: &ClusterShare) {
        let known = self
            .shared_clusters
            .entry(peer_device_id.clone())
            .or_default()
            .entry(share.cluster().cluster_id())
            .or_default();
        known.chunk_ids.extend(
            share
                .chunks()
                .iter()
                .map(|chunk| chunk.chunk_id().to_owned()),
        );
        known
            .clip_ids
            .extend(share.clips().iter().map(|clip| clip.clip_id()));
    }

    /// Opens and imports a share `peer_device_id` sent.
    ///
    /// Side effects: May store the cluster, chunks, and clips, or hold the share until its
    /// meeting syncs. A share from a device without an agreed pair key, or one that fails to
    /// open or validate, is logged (peer ID only) and dropped.
    fn accept_cluster_share(&mut self, peer_device_id: &DeviceId, payload: &[u8]) {
        let pair_key = match self.pairing_manager.store().pair_key(peer_device_id) {
            Ok(Some(pair_key)) => pair_key,
            Ok(None) => {
                tracing::warn!(
                    peer = %peer_device_id.as_str(),
                    "a cluster share arrived before the pair key was agreed and was dropped"
                );
                return;
            }
            Err(error) => {
                tracing::warn!(
                    peer = %peer_device_id.as_str(),
                    error = %error,
                    "the pair key could not be loaded"
                );
                return;
            }
        };
        let local_peer_id = self.pairing_manager.transport().local_peer_id();
        match decode_cluster_share_message(payload, &pair_key, peer_device_id, &local_peer_id) {
            Ok(share) => {
                self.remember_shared(peer_device_id, &share);
                if let Some(held) = self.import_share(share) {
                    self.hold_share(held);
                }
            }
            Err(error) => tracing::warn!(
                peer = %peer_device_id.as_str(),
                error = %error,
                "a cluster share was rejected"
            ),
        }
    }

    /// Imports a share into the local repository.
    ///
    /// Outputs: The share back when its meeting has not synced yet, so it can be held.
    /// Side effects: May store the cluster, chunks, and clips; logs failures without content.
    fn import_share(&mut self, share: ClusterShare) -> Option<ClusterShare> {
        match self.sync_manager.repository().import_cluster_share(&share) {
            Ok(ClusterShareImport::Imported | ClusterShareImport::AlreadyResolved) => None,
            Ok(ClusterShareImport::UnknownMeeting) => Some(share),
            Err(error) => {
                tracing::warn!(
                    cluster_id = %share.cluster().cluster_id(),
                    error = %error,
                    "a cluster share could not be imported"
                );
                None
            }
        }
    }

    /// Holds a share until its meeting syncs, within `MAX_HELD_SHARE_AUDIO_BYTES`.
    ///
    /// Side effects: Keeps the share in memory, or drops it with a warning when the bound is hit.
    fn hold_share(&mut self, share: ClusterShare) {
        let audio_bytes = |share: &ClusterShare| -> usize {
            share
                .clips()
                .iter()
                .map(|clip| clip.audio_bytes().len())
                .sum()
        };
        let held_audio_bytes: usize = self
            .held_shares
            .iter()
            .map(|held| audio_bytes(&held.share))
            .sum();
        if held_audio_bytes + audio_bytes(&share) > MAX_HELD_SHARE_AUDIO_BYTES {
            tracing::warn!(
                cluster_id = %share.cluster().cluster_id(),
                "a cluster share for a meeting not synced yet was dropped: too many are waiting"
            );
            return;
        }
        self.held_shares.push(HeldShare {
            share,
            received_at: Instant::now(),
        });
    }

    /// Imports held shares whose meeting has synced, and drops expired ones.
    ///
    /// Side effects: May store clusters, chunks, and clips; frees expired shares.
    fn retry_held_shares(&mut self) {
        if self.held_shares.is_empty() {
            return;
        }
        let now = Instant::now();
        let held_shares = std::mem::take(&mut self.held_shares);
        for held in held_shares {
            if now.duration_since(held.received_at) > HELD_SHARE_LIFETIME {
                tracing::debug!(
                    cluster_id = %held.share.cluster().cluster_id(),
                    "a held cluster share expired before its meeting synced"
                );
                continue;
            }
            if let Some(share) = self.import_share(held.share) {
                self.held_shares.push(HeldShare {
                    share,
                    received_at: held.received_at,
                });
            }
        }
    }

    /// Sends this device's X25519 public key to one connected paired device.
    ///
    /// Side effects: Sends one encrypted message when a secret is configured; a failure is
    /// logged, and the key is sent again at the peer's next connection.
    fn send_pair_public_key(&mut self, peer_device_id: &DeviceId) {
        let Some(secret) = &self.pair_agreement_secret else {
            return;
        };
        let message = encode_pair_key_message(&secret.public_key());
        if let Err(error) = self
            .pairing_manager
            .transport_mut()
            .send_encrypted(peer_device_id, &message)
        {
            tracing::warn!(
                peer = %peer_device_id.as_str(),
                error = %error,
                "this device's agreement key could not be sent"
            );
        }
    }

    /// Stores a manifest received from `peer_device_id`.
    ///
    /// Side effects: Replaces that peer's manifest. A rejected manifest, or one naming another
    /// device, is logged and ignored.
    fn accept_manifest(
        &mut self,
        peer_device_id: &DeviceId,
        decoded: Result<
            DeviceManifest,
            crate::application::device_manifest_exchange::ManifestMessageError,
        >,
    ) {
        match decoded {
            Ok(manifest) if manifest.device_id() == peer_device_id => {
                self.peer_manifests.insert(peer_device_id.clone(), manifest);
            }
            Ok(_) => tracing::warn!(
                peer = %peer_device_id.as_str(),
                "a manifest naming another device was ignored"
            ),
            Err(error) => tracing::warn!(
                peer = %peer_device_id.as_str(),
                error = %error,
                "a manifest was rejected"
            ),
        }
    }

    /// Sends this device's manifest to one connected paired device.
    ///
    /// Side effects: Sends one encrypted message; a failure is logged, and the manifest is sent
    /// again at the peer's next connection.
    fn send_manifest(&mut self, peer_device_id: &DeviceId) {
        let sent = encode_manifest_message(&self.local_manifest)
            .map_err(|error| error.to_string())
            .and_then(|message| {
                self.pairing_manager
                    .transport_mut()
                    .send_encrypted(peer_device_id, &message)
                    .map_err(|error| error.to_string())
            });
        if let Err(error) = sent {
            tracing::warn!(
                peer = %peer_device_id.as_str(),
                error = %error,
                "this device's manifest could not be sent"
            );
        }
    }

    /// Schedules a restart for each peer whose delivery failed, then runs the restarts that are
    /// due for peers still connected.
    ///
    /// Side effects: May send one handshake per local meeting to each restarted peer. Failures
    /// are logged; a disconnected peer starts over at its next connection.
    fn run_scheduled_restarts(&mut self) {
        let now = Instant::now();
        let retry_at = now.checked_add(self.retry_delay).unwrap_or(now);
        for failed_peer in self.sync_manager.take_failed_peers() {
            self.scheduled_restarts
                .entry(failed_peer)
                .or_insert(retry_at);
        }
        let due_peers: Vec<DeviceId> = self
            .scheduled_restarts
            .iter()
            .filter(|(_, restart_at)| **restart_at <= now)
            .map(|(peer_id, _)| peer_id.clone())
            .collect();
        for peer_id in due_peers {
            self.scheduled_restarts.remove(&peer_id);
            if !self.sync_manager.is_connected(&peer_id) {
                continue;
            }
            if let Err(error) = self
                .sync_manager
                .restart_peer(self.pairing_manager.transport_mut(), peer_id)
            {
                tracing::warn!(error = %error, "sync with a paired device could not restart");
            }
        }
    }

    /// Carries out one command.
    ///
    /// Outputs: The command's outcome, or the pairing or sync error that stopped it; a failed
    /// command changes nothing beyond what the error describes.
    /// Side effects: Depends on the command: opens the pairing window, sends pairing messages,
    /// persists or removes trust, or sends sync messages.
    pub fn handle_command(
        &mut self,
        command: P2pCommand,
    ) -> Result<P2pCommandOutcome, P2pServiceError> {
        match command {
            P2pCommand::MakePairable => {
                self.pairing_manager.make_pairable(PAIRING_WINDOW);
                Ok(P2pCommandOutcome::Done)
            }
            P2pCommand::StartPairing(peer_id) => Ok(P2pCommandOutcome::PairingStarted(
                self.pairing_manager.start_pairing(&peer_id)?,
            )),
            P2pCommand::ConfirmCode(peer_id) => {
                let update = self.pairing_manager.confirm_code(&peer_id)?;
                self.record_update(update.clone());
                Ok(P2pCommandOutcome::Pairing(update))
            }
            P2pCommand::CancelPairing(peer_id) => {
                self.pairing_manager.cancel(&peer_id)?;
                Ok(P2pCommandOutcome::Done)
            }
            P2pCommand::AcceptIncoming(peer_id) => {
                let update = self.pairing_manager.accept_incoming(&peer_id)?;
                self.record_update(update.clone());
                Ok(P2pCommandOutcome::Pairing(update))
            }
            P2pCommand::RejectIncoming(peer_id) => {
                self.pairing_manager.reject_incoming(&peer_id)?;
                Ok(P2pCommandOutcome::Done)
            }
            P2pCommand::Revoke(peer_id) => {
                let was_paired = self.pairing_manager.revoke(&peer_id)?;
                self.refresh_trusted_devices()?;
                Ok(P2pCommandOutcome::Revoked(was_paired))
            }
            P2pCommand::SetTranscriptionRoute(route) => {
                if let ComputePreference::SpecificPeer(peer_id) = &route
                    && !self
                        .trusted_devices
                        .iter()
                        .any(|device| device.peer_id() == peer_id)
                {
                    return Err(P2pServiceError::RouteDeviceNotPaired);
                }
                if let Some(store) = &self.transcription_route_store {
                    store.save_transcription_route(&route_text(&route))?;
                }
                self.transcription_route = route;
                Ok(P2pCommandOutcome::Done)
            }
            P2pCommand::LocalChange(meeting_id) => {
                self.sync_manager
                    .notify_local_change(self.pairing_manager.transport_mut(), &meeting_id)?;
                Ok(P2pCommandOutcome::Done)
            }
        }
    }

    /// Returns the current snapshot for the Devices screens.
    ///
    /// Side effects: None; the allowlist comes from the cache.
    pub fn status(&self) -> P2pStatus {
        P2pStatus {
            local_peer_id: self.pairing_manager.transport().local_peer_id(),
            device_name: self.pairing_manager.local_name().clone(),
            transcription_route: self.transcription_route.clone(),
            trusted_devices: self.trusted_devices.clone(),
            nearby_devices: self.pairing_manager.nearby_devices(),
            incoming_requests: self.pairing_manager.incoming_requests(),
            outgoing_pairings: self.pairing_manager.outgoing_pairings(),
            recent_updates: self.recent_updates.iter().cloned().collect(),
            local_manifest: self.local_manifest.clone(),
            peer_manifests: {
                let mut peer_manifests: Vec<DeviceManifest> =
                    self.peer_manifests.values().cloned().collect();
                peer_manifests.sort_by(|first, second| {
                    first.device_id().as_str().cmp(second.device_id().as_str())
                });
                peer_manifests
            },
        }
    }

    /// Borrows the transport, for example to read its listen addresses.
    pub fn transport(&self) -> &Transport {
        self.pairing_manager.transport()
    }

    /// Mutably borrows the transport, to drive its event loop.
    pub fn transport_mut(&mut self) -> &mut Transport {
        self.pairing_manager.transport_mut()
    }

    /// Borrows the local repository.
    pub fn repository(&self) -> &Repository {
        self.sync_manager.repository()
    }

    /// Keeps `update` for the screens. A completed pairing also refreshes the allowlist cache and
    /// restarts sync with the device from fresh state: if this device already trusted it (it
    /// re-paired after the other side revoked), its first handshake went out while the other side
    /// still dropped it, and only a restart sends it again.
    ///
    /// Side effects: May read the allowlist and send sync messages. Failures are logged, since
    /// the trust decision itself is already persisted.
    fn record_update(&mut self, update: PairingUpdate) {
        if let PairingUpdate::Paired { peer_id, .. } = &update {
            if let Err(error) = self.refresh_trusted_devices() {
                tracing::warn!(error = %error, "the paired-device list could not be refreshed");
            }
            if let Err(error) = self
                .sync_manager
                .restart_peer(self.pairing_manager.transport_mut(), peer_id.clone())
            {
                tracing::warn!(error = %error, "sync with a newly paired device could not start");
            }
        }
        if self.recent_updates.len() == MAX_RECENT_UPDATES {
            self.recent_updates.pop_front();
        }
        self.recent_updates.push_back(update);
    }

    /// Re-reads the allowlist into the cache.
    ///
    /// Side effects: Reads the trusted-peer store.
    fn refresh_trusted_devices(&mut self) -> Result<(), P2pServiceError> {
        self.trusted_devices = self.pairing_manager.trusted_devices()?;
        // A device no longer paired is no longer a routing candidate.
        let trusted_devices = &self.trusted_devices;
        self.peer_manifests.retain(|peer_id, _| {
            trusted_devices
                .iter()
                .any(|device| device.peer_id() == peer_id)
        });
        Ok(())
    }
}

/// Builds this device's manifest under its peer ID.
fn local_manifest(local_peer_id: DeviceId, capabilities: LocalCapabilities) -> DeviceManifest {
    DeviceManifest::new(
        local_peer_id,
        capabilities.cpu_cores,
        capabilities.gpu_vram_megabytes,
        capabilities.transcription_models,
    )
}
