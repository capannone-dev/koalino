//! Synchronizes every meeting of the local repository with paired devices (Issue #43).
//!
//! When a paired peer connects, the manager starts an Automerge sync handshake for each local
//! meeting, keeping one `sync::State` per (peer, meeting). A peer that names a meeting this device
//! does not have is answered from an empty in-memory document; once the peer's changes arrive, the
//! meeting is created locally from that document, so it keeps the peer's origin and transcript.
//! Local writes reach connected peers when the caller reports them through `notify_local_change`.
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
pub struct RepositorySyncManager<RepositoryPort, NetworkPort> {
    /// Reads, merges, and creates meeting documents in the local repository.
    meeting_crdt_service: MeetingCrdtService<RepositoryPort>,
    /// Encrypted transport to paired peers.
    p2p_network: NetworkPort,
    /// Paired peers currently connected, which receive local changes.
    connected_peers: HashSet<DeviceId>,
    /// Automerge protocol state per (peer, meeting).
    peer_sync_states: HashMap<(DeviceId, MeetingId), sync::State>,
    /// Documents of meetings that exist only on a peer, until their first changes arrive.
    pending_remote_meetings: HashMap<MeetingId, AutoCommit>,
}

impl<RepositoryPort, NetworkPort> RepositorySyncManager<RepositoryPort, NetworkPort>
where
    RepositoryPort: NoteRepository,
    NetworkPort: P2pNetwork,
{
    /// Creates a manager over the local repository and the encrypted network.
    ///
    /// Outputs: A manager with no connected peers.
    /// Side effects: None until invoked.
    pub fn new(note_repository: RepositoryPort, p2p_network: NetworkPort) -> Self {
        Self {
            meeting_crdt_service: MeetingCrdtService::new(note_repository),
            p2p_network,
            connected_peers: HashSet::new(),
            peer_sync_states: HashMap::new(),
            pending_remote_meetings: HashMap::new(),
        }
    }

    /// Borrows the local repository.
    pub fn repository(&self) -> &RepositoryPort {
        self.meeting_crdt_service.repository()
    }

    /// Borrows the network port, for example to manage pairing on the same adapter.
    pub fn network(&self) -> &NetworkPort {
        &self.p2p_network
    }

    /// Mutably borrows the network port, for example to drive the swarm.
    pub fn network_mut(&mut self) -> &mut NetworkPort {
        &mut self.p2p_network
    }

    /// Processes at most one network event.
    ///
    /// Outputs: Success when no event is pending or the event was handled; otherwise an error
    /// for that event only, after which later events can still be processed.
    /// Side effects: On a connected peer, sends one handshake per local meeting. On a received
    /// message, may merge and persist a document, create a meeting, and send a reply. On an
    /// expired peer, forgets its protocol state.
    pub fn process_next_network_event(&mut self) -> Result<(), RepositorySyncManagerError> {
        match self.p2p_network.poll_event()? {
            Some(NetworkEvent::PeerDiscovered(peer_device_id)) => {
                self.start_syncing_with(peer_device_id)
            }
            Some(NetworkEvent::MessageReceived {
                peer_device_id,
                payload,
            }) => self.apply_remote_sync_message(&peer_device_id, &payload),
            Some(NetworkEvent::PeerExpired(peer_device_id)) => {
                self.forget_peer(&peer_device_id);
                Ok(())
            }
            None => Ok(()),
        }
    }

    /// Sends a local change of `meeting_id`, or a new local meeting, to every connected peer.
    ///
    /// Inputs: `meeting_id` exists locally and was just written (recorded, renamed, re-labelled).
    /// Outputs: Success, or the first storage, protocol, or network error.
    /// Side effects: Reads the document and sends one sync message per connected peer.
    pub fn notify_local_change(
        &mut self,
        meeting_id: &MeetingId,
    ) -> Result<(), RepositorySyncManagerError> {
        let connected_peers: Vec<DeviceId> = self.connected_peers.iter().cloned().collect();
        for peer_device_id in connected_peers {
            self.send_local_sync_message(&peer_device_id, meeting_id)?;
        }
        Ok(())
    }

    /// Records `peer_device_id` as connected and starts a handshake for every local meeting.
    ///
    /// Side effects: Lists local meetings and sends up to one message per meeting.
    fn start_syncing_with(
        &mut self,
        peer_device_id: DeviceId,
    ) -> Result<(), RepositorySyncManagerError> {
        tracing::info!(peer_id = %peer_device_id.as_str(), "Starting repository sync");
        let local_meeting_ids = self.repository().list_meeting_ids()?;
        self.connected_peers.insert(peer_device_id.clone());
        for meeting_id in local_meeting_ids {
            self.send_local_sync_message(&peer_device_id, &meeting_id)?;
        }
        Ok(())
    }

    /// Forgets a disconnected peer's protocol state and abandons meetings only it was sending.
    ///
    /// Side effects: Updates in-memory state only.
    fn forget_peer(&mut self, peer_device_id: &DeviceId) {
        self.connected_peers.remove(peer_device_id);
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
        self.send(peer_device_id, meeting_id, generated_message)
    }

    /// Wraps an optional Automerge message in an envelope and sends it to `peer_device_id`.
    ///
    /// Side effects: Sends encrypted bytes when there is a message.
    fn send(
        &mut self,
        peer_device_id: &DeviceId,
        meeting_id: &MeetingId,
        generated_message: Option<sync::Message>,
    ) -> Result<(), RepositorySyncManagerError> {
        if let Some(automerge_sync_message) = generated_message {
            let serialized_envelope =
                SyncMessage::new(meeting_id.clone(), automerge_sync_message.encode()).to_bytes()?;
            self.p2p_network
                .send_encrypted(peer_device_id, &serialized_envelope)?;
        }
        Ok(())
    }

    /// Validates one peer envelope and applies it to a local or pending meeting.
    ///
    /// Side effects: See `apply_to_local_meeting` and `apply_to_pending_meeting`.
    fn apply_remote_sync_message(
        &mut self,
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
        if self.meeting_crdt_service.meeting_exists(&meeting_id)? {
            self.apply_to_local_meeting(peer_device_id, &meeting_id, automerge_sync_message)
        } else {
            self.apply_to_pending_meeting(peer_device_id, &meeting_id, automerge_sync_message)
        }
    }

    /// Merges a message into an existing meeting and replies.
    ///
    /// Side effects: Persists the merged document (the repository re-projects its index) and
    /// sends the next handshake message when there is one.
    fn apply_to_local_meeting(
        &mut self,
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
        self.send_local_sync_message(peer_device_id, meeting_id)
    }

    /// Applies a message about a meeting this device lacks, creating it once changes arrive.
    ///
    /// Outputs: Success, `TooManyPendingMeetings` for a new unknown meeting over the bound, a
    /// malformed-message error when Automerge rejects the message, or a storage error.
    /// Side effects: Holds the document in memory until it has changes, then creates the meeting
    /// from it in the repository and continues the handshake from there.
    fn apply_to_pending_meeting(
        &mut self,
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
            return self.send(peer_device_id, meeting_id, reply);
        }
        let received_document = pending_document.save();
        self.pending_remote_meetings.remove(meeting_id);
        self.meeting_crdt_service
            .create_meeting_from_peer_document(meeting_id, &received_document)?;
        tracing::info!(peer_id = %peer_device_id.as_str(), meeting_id = %meeting_id.as_str(), "Created a meeting received from a paired device");
        self.send_local_sync_message(peer_device_id, meeting_id)
    }
}
