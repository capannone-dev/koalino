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

use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::{Duration, Instant},
};

use uuid::Uuid;

use crate::{
    application::{
        cluster_share_exchange::{
            decode_cluster_share_message, encode_cluster_share_messages, is_cluster_share_message,
        },
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
        voice_library_exchange::{
            decode_voice_library_message, encode_voice_library_messages, is_voice_library_message,
        },
    },
    domain::{
        models::{
            ClusterDigest, ClusterShare, ClusterShareImport, DeviceId, DeviceManifest, MeetingId,
            TranscriptionModel,
        },
        peer_trust::{DeviceName, PairingCode, TrustedPeer},
    },
    ports::{
        network::{NetworkEvent, P2pNetwork},
        peer_trust::{PairKeyPinning, PairingTransport, TrustedPeerStore},
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
}

/// What the Devices screens show.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct P2pStatus {
    /// This device's peer ID.
    pub local_peer_id: DeviceId,
    /// This device's announced name.
    pub device_name: DeviceName,
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
    /// The voice library to keep in step with paired devices; `None` leaves it on this device.
    voice_library: Option<Box<dyn VoiceLibrarySync>>,
    /// The library digest each connected device was last sent.
    shared_voice_digests: HashMap<DeviceId, [u8; 32]>,
}

impl<Transport, Store, Repository> P2pService<Transport, Store, Repository>
where
    Transport: PairingTransport + P2pNetwork,
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
                Some(peer_device_id.clone())
            }
            NetworkEvent::PeerExpired(peer_device_id) => {
                self.peer_manifests.remove(peer_device_id);
                self.connected_peers.remove(peer_device_id);
                self.shared_clusters.remove(peer_device_id);
                self.shared_voice_digests.remove(peer_device_id);
                None
            }
            // The device may have missed the voice library, so it is sent again.
            NetworkEvent::DeliveryFailed(peer_device_id) => {
                self.shared_voice_digests.remove(peer_device_id);
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
