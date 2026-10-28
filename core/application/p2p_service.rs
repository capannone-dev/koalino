//! Combines pairing and repository sync on one transport for the daemon's P2P task (Issue #43).
//!
//! The task owns one `P2pService`. It drives the transport's event loop, calls
//! `process_pending_events` after every transport event, and passes the owner's requests (from the
//! HTTP routes) to `handle_command`. `status` returns a snapshot for the Devices screens without
//! touching storage: the paired-device list is cached and refreshed whenever pairing changes it.
//!
//! A device whose pairing completes starts syncing at once: the transport reports the connected,
//! newly trusted peer, so meetings flow without waiting for the next discovery announcement.

use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

use crate::{
    application::{
        pairing_manager::{
            IncomingPairingRequest, OutgoingPairingStatus, PairingError, PairingManager,
            PairingUpdate,
        },
        repository_sync_manager::{RepositorySyncManager, RepositorySyncManagerError},
    },
    domain::{
        models::{DeviceId, MeetingId},
        peer_trust::{DeviceName, PairingCode, TrustedPeer},
    },
    ports::{
        network::P2pNetwork,
        peer_trust::{PairingTransport, TrustedPeerStore},
        storage::NoteRepository,
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
        Self {
            pairing_manager,
            sync_manager,
            trusted_devices: Vec::new(),
            recent_updates: VecDeque::new(),
            retry_delay: DEFAULT_RETRY_DELAY,
            scheduled_restarts: HashMap::new(),
            last_seen_revision: 0,
        }
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
    /// Side effects: Reads document revisions and sends sync messages. Failures are logged and
    /// retried on the next call, or at the peer's next connection.
    pub fn poll_local_changes(&mut self) {
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
            match self
                .sync_manager
                .process_next_network_event(self.pairing_manager.transport_mut())
            {
                Ok(true) => {}
                Ok(false) => break,
                Err(error) => tracing::warn!(error = %error, "a sync message could not be applied"),
            }
        }
        self.run_scheduled_restarts();
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
        Ok(())
    }
}
