//! Orchestrates Automerge sync handshakes across portable encrypted network and storage ports.

use std::collections::HashMap;

use automerge::sync;

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

/// Describes a failed application-level CRDT synchronization operation.
#[derive(Debug, thiserror::Error)]
pub enum SyncManagerError {
    /// The encrypted P2P port could not supply or transmit a sync message.
    #[error(transparent)]
    Network(#[from] NetworkError),
    /// The local meeting document could not be read, merged, or persisted.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// The local meeting identifier cannot be serialized into the sync protocol envelope.
    #[error(transparent)]
    Protocol(#[from] SyncMessageEncodeError),
    /// A peer supplied bytes that do not satisfy the sync-envelope or Automerge message contract.
    #[error("malformed sync message: {reason}")]
    MalformedMessage {
        /// Describes the failed validation without retaining peer payload contents.
        reason: String,
    },
}

/// Synchronizes one meeting document through injected encrypted-network and repository ports.
pub struct SyncManager<RepositoryPort, NetworkPort> {
    meeting_id: MeetingId,
    meeting_crdt_service: MeetingCrdtService<RepositoryPort>,
    p2p_network: NetworkPort,
    peer_sync_states: HashMap<DeviceId, sync::State>,
}

impl<RepositoryPort, NetworkPort> SyncManager<RepositoryPort, NetworkPort>
where
    RepositoryPort: NoteRepository,
    NetworkPort: P2pNetwork,
{
    /// Creates a Sync Manager for one locally persisted meeting document.
    ///
    /// Inputs: `meeting_id` must exist in `note_repository`; `p2p_network` must provide encrypted events.
    /// Outputs: A manager with no established peer sync states.
    /// Side effects: Takes ownership of the supplied ports but performs no I/O until invoked.
    pub fn new(
        meeting_id: MeetingId,
        note_repository: RepositoryPort,
        p2p_network: NetworkPort,
    ) -> Self {
        Self {
            meeting_id,
            meeting_crdt_service: MeetingCrdtService::new(note_repository),
            p2p_network,
            peer_sync_states: HashMap::new(),
        }
    }

    /// Processes at most one portable encrypted-network event.
    ///
    /// Inputs: None; the injected P2P port provides an already-observed event when available.
    /// Outputs: Success when no event is pending or an event is handled; otherwise a portable error.
    /// Side effects: May send a sync handshake, merge and persist a document, or update peer state.
    pub fn process_next_network_event(&mut self) -> Result<(), SyncManagerError> {
        match self.p2p_network.poll_event()? {
            Some(NetworkEvent::PeerDiscovered(peer_device_id)) => {
                tracing::info!(peer_id = %peer_device_id.as_str(), "Initiating CRDT sync");
                self.send_sync_message_to_peer(&peer_device_id)
            }
            Some(NetworkEvent::MessageReceived {
                peer_device_id,
                payload,
            }) => self.apply_remote_sync_message(&peer_device_id, &payload),
            Some(NetworkEvent::PeerExpired(peer_device_id)) => {
                self.peer_sync_states.remove(&peer_device_id);
                Ok(())
            }
            None => Ok(()),
        }
    }

    /// Broadcasts a fresh Automerge sync message after a local edit has been persisted.
    ///
    /// Inputs: None; callers invoke this only after the meeting's local CRDT document changes.
    /// Outputs: Success when no update is needed or the encrypted broadcast is accepted.
    /// Side effects: Reads the local document and broadcasts opaque sync bytes to connected peers.
    pub fn notify_local_change(&mut self) -> Result<(), SyncManagerError> {
        let mut initial_sync_state = sync::State::new();
        let generated_message = self
            .meeting_crdt_service
            .generate_sync_message(&self.meeting_id, &mut initial_sync_state)?;
        if let Some(automerge_sync_message) = generated_message {
            let sync_payload =
                SyncMessage::new(self.meeting_id.clone(), automerge_sync_message.encode());
            let serialized_sync_payload = sync_payload.to_bytes()?;
            self.p2p_network
                .broadcast_encrypted(&serialized_sync_payload)?;
            tracing::info!(meeting_id = %self.meeting_id.as_str(), "Broadcast CRDT sync update");
        }
        Ok(())
    }

    /// Generates and sends the next peer-specific handshake message when Automerge has one.
    ///
    /// Inputs: `peer_device_id` identifies an authenticated discovered peer.
    /// Outputs: Success when no message is required or when the P2P port accepts the message.
    /// Side effects: Reads local CRDT state, updates peer protocol state, and sends encrypted bytes.
    fn send_sync_message_to_peer(
        &mut self,
        peer_device_id: &DeviceId,
    ) -> Result<(), SyncManagerError> {
        let peer_sync_state = self
            .peer_sync_states
            .entry(peer_device_id.clone())
            .or_default();
        let generated_message = self
            .meeting_crdt_service
            .generate_sync_message(&self.meeting_id, peer_sync_state)?;
        if let Some(automerge_sync_message) = generated_message {
            let sync_payload =
                SyncMessage::new(self.meeting_id.clone(), automerge_sync_message.encode());
            let serialized_sync_payload = sync_payload.to_bytes()?;
            self.p2p_network
                .send_encrypted(peer_device_id, &serialized_sync_payload)?;
        }
        Ok(())
    }

    /// Validates, applies, persists, and acknowledges one peer-provided sync envelope.
    ///
    /// Inputs: `peer_device_id` identifies the authenticated sender; `payload` is opaque transport data.
    /// Outputs: Success when the message is applied or a validation/storage/network error.
    /// Side effects: May update per-peer state, persist the merged document, and send an acknowledgement.
    fn apply_remote_sync_message(
        &mut self,
        peer_device_id: &DeviceId,
        payload: &[u8],
    ) -> Result<(), SyncManagerError> {
        let received_sync_message = SyncMessage::from_bytes(payload).map_err(|error| {
            tracing::warn!(peer_id = %peer_device_id.as_str(), error = %error, "Rejected malformed CRDT sync envelope");
            SyncManagerError::MalformedMessage {
                reason: error.to_string(),
            }
        })?;
        if received_sync_message.meeting_id() != &self.meeting_id {
            let reason = String::from("sync envelope targets a different meeting");
            tracing::warn!(peer_id = %peer_device_id.as_str(), "Rejected sync message for a different meeting");
            return Err(SyncManagerError::MalformedMessage { reason });
        }
        let automerge_sync_message = sync::Message::decode(received_sync_message.sync_bytes()).map_err(|error| {
            tracing::warn!(peer_id = %peer_device_id.as_str(), error = %error, "Rejected malformed Automerge sync message");
            SyncManagerError::MalformedMessage {
                reason: error.to_string(),
            }
        })?;
        let peer_sync_state = self
            .peer_sync_states
            .entry(peer_device_id.clone())
            .or_default();
        self.meeting_crdt_service.apply_sync_message(
            &self.meeting_id,
            peer_sync_state,
            automerge_sync_message,
        )?;
        tracing::debug!(peer_id = %peer_device_id.as_str(), "Applied remote sync message");
        self.send_sync_message_to_peer(peer_device_id)
    }
}
