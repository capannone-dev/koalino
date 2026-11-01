//! Synchronizes every meeting of the local repository with paired devices (Issue #43).
//!
//! When a paired peer connects, the manager starts an Automerge sync handshake for each local
//! meeting, keeping one `sync::State` per (peer, meeting). A peer that names a meeting this device
//! does not have is answered from an empty in-memory document; once the peer's changes arrive, the
//! meeting is created locally from that document, so it keeps the peer's origin and transcript.
//! Local writes reach connected peers when the caller reports them through `notify_local_change`.
//!
//! The manager borrows the network for each call instead of owning it, so the P2P task can share
//! one adapter with `PairingManager`.
//!
//! Only paired peers reach this manager: the network adapter drops sync payloads from anyone else.
//! Even so, the number of unknown meetings held in memory at once is bounded, so a misbehaving
//! peer cannot grow memory without limit.

use std::collections::{HashMap, HashSet};

use automerge::{
    AutoCommit,
    sync::{self, SyncDoc},
};

use crate::{
    application::meeting_crdt::MeetingCrdtService,
    domain::{
        errors::{NetworkError, StorageError},
        models::{DeviceId, MeetingId, SyncMessage, SyncMessageEncodeError},
    },
    ports::{
        network::{NetworkEvent, P2pNetwork},
        storage::NoteRepository,
    },
};

/// Most meetings that exist only on a peer and are still being received, across all peers.
pub const MAX_PENDING_REMOTE_MEETINGS: usize = 64;

/// Describes a failed whole-repository synchronization step.
#[derive(Debug, thiserror::Error)]
pub enum RepositorySyncManagerError {
    /// The encrypted P2P port could not supply or transmit a sync message.
    #[error(transparent)]
    Network(#[from] NetworkError),
    /// A meeting document could not be read, merged, created, or persisted.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// A local meeting identifier cannot be serialized into the sync envelope.
    #[error(transparent)]
    Protocol(#[from] SyncMessageEncodeError),
    /// A peer sent bytes that are not a valid sync envelope or Automerge sync message.
    #[error("malformed sync message: {reason}")]
    MalformedMessage {
        /// Describes the failed validation without peer payload contents.
        reason: String,
    },
    /// A peer named a new unknown meeting while `MAX_PENDING_REMOTE_MEETINGS` were in progress.
    #[error("too many meetings from peers are still being received")]
    TooManyPendingMeetings,
}

/// Synchronizes all local meetings with connected paired peers.
pub struct RepositorySyncManager<RepositoryPort> {
    /// Reads, merges, and creates meeting documents in the local repository.
    meeting_crdt_service: MeetingCrdtService<RepositoryPort>,
    /// Paired peers currently connected, which receive local changes.
    connected_peers: HashSet<DeviceId>,
    /// Automerge protocol state per (peer, meeting).
    peer_sync_states: HashMap<(DeviceId, MeetingId), sync::State>,
    /// Documents of meetings that exist only on a peer, until their first changes arrive.
    pending_remote_meetings: HashMap<MeetingId, AutoCommit>,
    /// Connected peers that failed to acknowledge a sync payload since the last
    /// `take_failed_peers`.
    failed_peers: HashSet<DeviceId>,
}

impl<RepositoryPort> RepositorySyncManager<RepositoryPort>
where
    RepositoryPort: NoteRepository,
{
    /// Creates a manager over the local repository.
    ///
    /// Outputs: A manager with no connected peers.
    /// Side effects: None until invoked.
    pub fn new(note_repository: RepositoryPort) -> Self {
        Self {
            meeting_crdt_service: MeetingCrdtService::new(note_repository),
            connected_peers: HashSet::new(),
            peer_sync_states: HashMap::new(),
            pending_remote_meetings: HashMap::new(),
            failed_peers: HashSet::new(),
        }
    }

    /// Borrows the local repository.
    pub fn repository(&self) -> &RepositoryPort {
        self.meeting_crdt_service.repository()
    }

    /// Processes at most one event from `p2p_network`.
    ///
    /// Outputs: `true` when an event was handled, `false` when none was pending; otherwise an
    /// error for that event only, after which later events can still be processed.
    /// Side effects: On a connected peer, sends one handshake per local meeting. On a received
    /// message, may merge and persist a document, create a meeting, and send a reply. On an
    /// expired peer, forgets its protocol state.
    pub fn process_next_network_event(
        &mut self,
        p2p_network: &mut impl P2pNetwork,
    ) -> Result<bool, RepositorySyncManagerError> {
        let Some(network_event) = p2p_network.poll_event()? else {
            return Ok(false);
        };
        self.handle_network_event(p2p_network, network_event)?;
        Ok(true)
    }

    /// Handles one event already taken from `p2p_network`, for callers that route other message
    /// kinds (such as device manifests) themselves.
    ///
    /// Outputs: Success, or an error for this event only.
    /// Side effects: As `process_next_network_event`.
    pub fn handle_network_event(
        &mut self,
        p2p_network: &mut impl P2pNetwork,
        network_event: NetworkEvent,
    ) -> Result<(), RepositorySyncManagerError> {
        match network_event {
            NetworkEvent::PeerDiscovered(peer_device_id) => {
                self.peer_connected(p2p_network, peer_device_id)?;
            }
            NetworkEvent::MessageReceived {
                peer_device_id,
                payload,
            } => self.apply_remote_sync_message(p2p_network, &peer_device_id, &payload)?,
            NetworkEvent::PeerExpired(peer_device_id) => self.forget_peer(&peer_device_id),
            NetworkEvent::DeliveryFailed(peer_device_id) => {
                if self.connected_peers.contains(&peer_device_id) {
                    self.failed_peers.insert(peer_device_id);
                }
            }
        }
        Ok(())
    }

    /// Sends a local change of `meeting_id`, or a new local meeting, to every connected peer.
    ///
    /// Inputs: `meeting_id` exists locally and was just written (recorded, renamed, re-labelled).
    /// Outputs: Success, or the first storage, protocol, or network error.
    /// Side effects: Reads the document and sends one sync message per connected peer.
    pub fn notify_local_change(
        &mut self,
        p2p_network: &mut impl P2pNetwork,
        meeting_id: &MeetingId,
    ) -> Result<(), RepositorySyncManagerError> {
        let connected_peers: Vec<DeviceId> = self.connected_peers.iter().cloned().collect();
        for peer_device_id in connected_peers {
            self.send_local_sync_message(p2p_network, &peer_device_id, meeting_id)?;
        }
        Ok(())
    }

    /// Records `peer_device_id` as connected and starts a handshake for every local meeting.
    ///
    /// Inputs: A paired peer that just connected. A peer already recorded as connected is
    /// ignored, so a repeated report never sends a second handshake.
    /// Outputs: Success, or the first storage, protocol, or network error.
    /// Side effects: Lists local meetings and sends up to one message per meeting.
    pub fn peer_connected(
        &mut self,
        p2p_network: &mut impl P2pNetwork,
        peer_device_id: DeviceId,
    ) -> Result<(), RepositorySyncManagerError> {
        if self.connected_peers.contains(&peer_device_id) {
            return Ok(());
        }
        tracing::info!(peer_id = %peer_device_id.as_str(), "Starting repository sync");
        let local_meeting_ids = self.repository().list_meeting_ids()?;
        self.connected_peers.insert(peer_device_id.clone());
        for meeting_id in local_meeting_ids {
            if let Err(error) =
                self.send_local_sync_message(p2p_network, &peer_device_id, &meeting_id)
            {
                // Not connected after all: the next report of this peer starts over.
                self.forget_peer(&peer_device_id);
                return Err(error);
            }
        }
        Ok(())
    }

    /// Reports whether `peer_device_id` is recorded as connected.
    pub fn is_connected(&self, peer_device_id: &DeviceId) -> bool {
        self.connected_peers.contains(peer_device_id)
    }

    /// Returns the connected peers whose deliveries failed since the last call, by peer ID, and
    /// clears the set.
    ///
    /// Side effects: Clears the in-memory set.
    pub fn take_failed_peers(&mut self) -> Vec<DeviceId> {
        let mut failed_peers: Vec<DeviceId> = self.failed_peers.drain().collect();
        failed_peers.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        failed_peers
    }

    /// Restarts sync with `peer_device_id` from fresh protocol state.
    ///
    /// Inputs: A connected peer whose earlier messages may have been dropped, such as a device
    /// that did not trust this one until a pairing just completed.
    /// Outputs: Success, or the first storage, protocol, or network error.
    /// Side effects: Drops the peer's sync states, then sends one handshake per local meeting.
    pub fn restart_peer(
        &mut self,
        p2p_network: &mut impl P2pNetwork,
        peer_device_id: DeviceId,
    ) -> Result<(), RepositorySyncManagerError> {
        self.forget_peer(&peer_device_id);
        self.peer_connected(p2p_network, peer_device_id)
    }

    /// Forgets a disconnected peer's protocol state and abandons meetings only it was sending.
    ///
    /// Side effects: Updates in-memory state only.
    fn forget_peer(&mut self, peer_device_id: &DeviceId) {
        self.connected_peers.remove(peer_device_id);
        self.failed_peers.remove(peer_device_id);
        self.peer_sync_states
            .retain(|(state_peer, _), _| state_peer != peer_device_id);
        let peer_sync_states = &self.peer_sync_states;
        self.pending_remote_meetings.retain(|pending_meeting, _| {
            peer_sync_states
                .keys()
                .any(|(_, state_meeting)| state_meeting == pending_meeting)
        });
    }

    /// Generates and sends the next message for a local meeting when Automerge has one.
    ///
    /// Side effects: Reads the document, updates the (peer, meeting) state, and sends bytes.
    fn send_local_sync_message(
        &mut self,
        p2p_network: &mut impl P2pNetwork,
        peer_device_id: &DeviceId,
        meeting_id: &MeetingId,
    ) -> Result<(), RepositorySyncManagerError> {
        let peer_sync_state = self
            .peer_sync_states
            .entry((peer_device_id.clone(), meeting_id.clone()))
            .or_default();
        let generated_message = self
            .meeting_crdt_service
            .generate_sync_message(meeting_id, peer_sync_state)?;
        send(p2p_network, peer_device_id, meeting_id, generated_message)
    }

    /// Validates one peer envelope and applies it to a local or pending meeting.
    ///
    /// Side effects: See `apply_to_local_meeting` and `apply_to_pending_meeting`. A message about a
    /// deleted meeting is dropped, whatever it holds.
    fn apply_remote_sync_message(
        &mut self,
        p2p_network: &mut impl P2pNetwork,
        peer_device_id: &DeviceId,
        payload: &[u8],
    ) -> Result<(), RepositorySyncManagerError> {
        let envelope = SyncMessage::from_bytes(payload).map_err(|error| {
            tracing::warn!(peer_id = %peer_device_id.as_str(), error = %error, "Rejected malformed CRDT sync envelope");
            RepositorySyncManagerError::MalformedMessage {
                reason: error.to_string(),
            }
        })?;
        let automerge_sync_message = sync::Message::decode(envelope.sync_bytes()).map_err(|error| {
            tracing::warn!(peer_id = %peer_device_id.as_str(), error = %error, "Rejected malformed Automerge sync message");
            RepositorySyncManagerError::MalformedMessage {
                reason: error.to_string(),
            }
        })?;
        let meeting_id = envelope.meeting_id().clone();
        if self.repository().is_meeting_deleted(&meeting_id)? {
            // A deletion always wins: the peer gets the record and deletes its copy too.
            self.pending_remote_meetings.remove(&meeting_id);
            self.peer_sync_states
                .remove(&(peer_device_id.clone(), meeting_id));
            return Ok(());
        }
        if self.meeting_crdt_service.meeting_exists(&meeting_id)? {
            self.apply_to_local_meeting(
                p2p_network,
                peer_device_id,
                &meeting_id,
                automerge_sync_message,
            )
        } else {
            self.apply_to_pending_meeting(
                p2p_network,
                peer_device_id,
                &meeting_id,
                automerge_sync_message,
            )
        }
    }

    /// Merges a message into an existing meeting and replies.
    ///
    /// Side effects: Persists the merged document (the repository re-projects its index) and
    /// sends the next handshake message when there is one.
    fn apply_to_local_meeting(
        &mut self,
        p2p_network: &mut impl P2pNetwork,
        peer_device_id: &DeviceId,
        meeting_id: &MeetingId,
        automerge_sync_message: sync::Message,
    ) -> Result<(), RepositorySyncManagerError> {
        let peer_sync_state = self
            .peer_sync_states
            .entry((peer_device_id.clone(), meeting_id.clone()))
            .or_default();
        self.meeting_crdt_service.apply_sync_message(
            meeting_id,
            peer_sync_state,
            automerge_sync_message,
        )?;
        tracing::debug!(peer_id = %peer_device_id.as_str(), meeting_id = %meeting_id.as_str(), "Applied remote sync message");
        self.send_local_sync_message(p2p_network, peer_device_id, meeting_id)
    }

    /// Applies a message about a meeting this device lacks, creating it once changes arrive.
    ///
    /// Outputs: Success, `TooManyPendingMeetings` for a new unknown meeting over the bound, a
    /// malformed-message error when Automerge rejects the message, or a storage error.
    /// Side effects: Holds the document in memory until it has changes, then creates the meeting
    /// from it in the repository and continues the handshake from there.
    fn apply_to_pending_meeting(
        &mut self,
        p2p_network: &mut impl P2pNetwork,
        peer_device_id: &DeviceId,
        meeting_id: &MeetingId,
        automerge_sync_message: sync::Message,
    ) -> Result<(), RepositorySyncManagerError> {
        if !self.pending_remote_meetings.contains_key(meeting_id)
            && self.pending_remote_meetings.len() >= MAX_PENDING_REMOTE_MEETINGS
        {
            tracing::warn!(peer_id = %peer_device_id.as_str(), "Refused a sync message for one unknown meeting too many");
            return Err(RepositorySyncManagerError::TooManyPendingMeetings);
        }
        let peer_sync_state = self
            .peer_sync_states
            .entry((peer_device_id.clone(), meeting_id.clone()))
            .or_default();
        let pending_document = self
            .pending_remote_meetings
            .entry(meeting_id.clone())
            .or_default();
        pending_document
            .sync()
            .receive_sync_message(peer_sync_state, automerge_sync_message)
            .map_err(|error| RepositorySyncManagerError::MalformedMessage {
                reason: error.to_string(),
            })?;
        if pending_document.get_heads().is_empty() {
            let reply = pending_document
                .sync()
                .generate_sync_message(peer_sync_state);
            return send(p2p_network, peer_device_id, meeting_id, reply);
        }
        let received_document = pending_document.save();
        self.pending_remote_meetings.remove(meeting_id);
        self.meeting_crdt_service
            .create_meeting_from_peer_document(meeting_id, &received_document)?;
        tracing::info!(peer_id = %peer_device_id.as_str(), meeting_id = %meeting_id.as_str(), "Created a meeting received from a paired device");
        self.send_local_sync_message(p2p_network, peer_device_id, meeting_id)
    }
}

/// Wraps an optional Automerge message in an envelope and sends it to `peer_device_id`.
///
/// Side effects: Sends encrypted bytes when there is a message.
fn send(
    p2p_network: &mut impl P2pNetwork,
    peer_device_id: &DeviceId,
    meeting_id: &MeetingId,
    generated_message: Option<sync::Message>,
) -> Result<(), RepositorySyncManagerError> {
    if let Some(automerge_sync_message) = generated_message {
        let serialized_envelope =
            SyncMessage::new(meeting_id.clone(), automerge_sync_message.encode()).to_bytes()?;
        p2p_network.send_encrypted(peer_device_id, &serialized_envelope)?;
    }
    Ok(())
}
