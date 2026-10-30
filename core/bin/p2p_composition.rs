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

use std::{
    path::Path,
    sync::{Arc, Mutex, PoisonError},
    thread::JoinHandle,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use tokio::sync::{mpsc, oneshot, watch};

use crate::{
    adapters::{
        network::libp2p_adapter::{Libp2pNetworkAdapter, NodeIdentity},
        storage::{sqlite_adapter::SqliteNoteRepository, sqlite_peer_store::SqlitePeerStore},
    },
    application::{
        crypto::pair_key::PairAgreementSecret,
        p2p_service::{
            LocalCapabilities, P2pCommand, P2pCommandOutcome, P2pControl, P2pControlError,
            P2pService, P2pSnapshot,
        },
        pairing_manager::PairingManager,
        repository_sync_manager::RepositorySyncManager,
    },
    domain::{
        models::{DeviceId, MeetingId},
        peer_trust::DeviceName,
    },
    ports::{crypto::EmbeddingCipher, peer_trust::PairAgreementSecretStore},
    storage_composition::open_note_repository,
};

#[cfg(test)]
#[path = "p2p_composition_tests.rs"]
mod tests;

/// Listen address used when `NOTE_TAKER_P2P_LISTEN` is unset: every interface, a free port.
const DEFAULT_LISTEN_ADDRESS: &str = "/ip4/0.0.0.0/tcp/0";

/// Device name used when neither `NOTE_TAKER_DEVICE_NAME` nor a usable host name is available.
const FALLBACK_DEVICE_NAME: &str = "Note Taker device";

/// Longest host name used as a default device name, matching `DeviceName`'s limit.
const MAX_DEFAULT_NAME_CHARACTERS: usize = 64;

/// How often the task looks for meetings written locally (by any connection or process) and
/// sends them to connected devices, so a new transcript reaches them about a second after it
/// is saved.
const LOCAL_CHANGE_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// How often paired devices that are not connected are dialed again, so a device that comes
/// back online reconnects within this time, without waiting for a new mDNS announcement.
const PAIRED_PEER_REDIAL_INTERVAL: Duration = Duration::from_secs(10);

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
type P2pRequest = (
    P2pCommand,
    oneshot::Sender<std::result::Result<P2pCommandOutcome, String>>,
);

/// The running P2P task's controls. Cheap to share behind an `Arc`.
pub struct P2pHandle {
    /// Queue of commands to the task.
    commands: mpsc::Sender<P2pRequest>,
    /// Latest snapshot published by the task.
    snapshots: watch::Receiver<P2pSnapshot>,
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

#[async_trait::async_trait]
impl P2pControl for P2pHandle {
    fn snapshot(&self) -> P2pSnapshot {
        P2pHandle::snapshot(self)
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
}

/// The service type the task runs.
type DaemonP2pService = P2pService<Libp2pNetworkAdapter, SqlitePeerStore, SqliteNoteRepository>;

/// Starts the P2P task and returns once it listens, or with the reason it could not start.
///
/// Inputs: The database path (the task opens its own connections), this device's ID, the
/// verified embedding cipher (it seals the node identity and the pair-agreement secret), and the
/// settings.
/// Outputs: The handle, or an error naming the failed step.
/// Side effects: Spawns the `note-taker-p2p` thread, opens SQLite connections, creates or loads the
/// node identity and the pair-agreement secret, and binds the listen address. Blocks the caller while starting, so async callers
/// run it in `spawn_blocking`.
pub fn spawn_p2p_task(
    database_path: &Path,
    device_id: DeviceId,
    embedding_cipher: Arc<dyn EmbeddingCipher>,
    config: P2pConfig,
) -> Result<P2pHandle> {
    let (startup_sender, startup_receiver) = std::sync::mpsc::sync_channel(1);
    let (command_sender, command_receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let (stop_sender, stop_receiver) = oneshot::channel();
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
                let started = open_service(&database_path, device_id, embedding_cipher, config);
                let mut service = match started {
                    Ok(service) => service,
                    Err(error) => {
                        let _ = startup_sender.send(Err(error));
                        return;
                    }
                };
                let (snapshot_sender, snapshot_receiver) = watch::channel(snapshot_of(&service));
                if startup_sender.send(Ok(snapshot_receiver)).is_err() {
                    return;
                }
                run_service(
                    &mut service,
                    snapshot_sender,
                    command_receiver,
                    stop_receiver,
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
) -> Result<DaemonP2pService> {
    let note_repository =
        open_note_repository(database_path, device_id, Arc::clone(&embedding_cipher))?;
    let peer_store = SqlitePeerStore::open(database_path, embedding_cipher)
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
    for peer_address in &config.static_peers {
        adapter
            .add_static_peer(peer_address)
            .context("NOTE_TAKER_P2P_PEERS contains an invalid peer address")?;
    }
    if let Some(relay_address) = &config.relay_address {
        adapter
            .use_relay(relay_address)
            .context("NOTE_TAKER_P2P_RELAY is invalid")?;
    }
    let mut service = P2pService::new(
        PairingManager::new(
            adapter,
            peer_store,
            config.device_name,
            Box::new(unix_millis),
        ),
        RepositorySyncManager::new(note_repository),
    )
    .with_local_capabilities(config.capabilities)
    .with_pair_agreement_secret(pair_agreement_secret);
    service
        .start()
        .context("unable to load the paired devices")?;
    Ok(service)
}

/// Runs the event loop until `stop` fires, all handles are gone, or the swarm ends.
///
/// Side effects: Drives the swarm, applies events and commands, and publishes snapshots.
async fn run_service(
    service: &mut DaemonP2pService,
    snapshot_sender: watch::Sender<P2pSnapshot>,
    mut command_receiver: mpsc::Receiver<P2pRequest>,
    mut stop: oneshot::Receiver<()>,
) {
    let mut change_poll = tokio::time::interval(LOCAL_CHANGE_POLL_INTERVAL);
    change_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut paired_peer_redial = tokio::time::interval(PAIRED_PEER_REDIAL_INTERVAL);
    paired_peer_redial.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        service.process_pending_events();
        let snapshot = snapshot_of(service);
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
            }
            _ = paired_peer_redial.tick() => service.transport_mut().redial_paired_peers(),
            swarm_result = service.transport_mut().drive_next_swarm_event() => {
                if let Err(error) = swarm_result {
                    tracing::error!(error = %error, "the P2P swarm stopped");
                    break;
                }
            }
            request = command_receiver.recv() => {
                let Some((command, reply_sender)) = request else {
                    break;
                };
                let outcome = service
                    .handle_command(command)
                    .map_err(|error| error.to_string());
                let _ = reply_sender.send(outcome);
            }
        }
    }
    tracing::info!("P2P sync stopped");
}

/// Sleeps until `retry_at`, or forever when no retry is scheduled.
async fn sleep_until_retry(retry_at: Option<std::time::Instant>) {
    match retry_at {
        Some(retry_at) => tokio::time::sleep_until(tokio::time::Instant::from_std(retry_at)).await,
        None => std::future::pending().await,
    }
}

/// Captures the service's status and listen addresses.
fn snapshot_of(service: &DaemonP2pService) -> P2pSnapshot {
    P2pSnapshot {
        status: service.status(),
        listen_addresses: service.transport().listen_addresses(),
    }
}

/// The current time in milliseconds since the Unix epoch, or 0 before it.
fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}
