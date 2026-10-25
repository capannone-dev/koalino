//! Defines mocked-port contracts for offline Automerge synchronization before its implementation.

use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use automerge::{
    AutoCommit, ROOT, ReadDoc, Value,
    sync::{self, SyncDoc},
    transaction::Transactable,
};

use crate::{
    application::sync_manager::{SyncManager, SyncManagerError},
    domain::{
        errors::{NetworkError, StorageError},
        models::{DeviceId, MeetingId, SyncMessage, Transcript},
    },
    ports::{
        network::{NetworkEvent, P2pNetwork},
        storage::NoteRepository,
    },
};

/// Shortens the mocked queued-port result type used to drive deterministic network events.
type QueuedNetworkEvent = Result<Option<NetworkEvent>, NetworkError>;

/// Records persistence calls while serving one deterministic local meeting document.
#[derive(Clone)]
struct MockNoteRepository {
    stored_document: Rc<RefCell<Vec<u8>>>,
    saved_documents: Rc<RefCell<Vec<Vec<u8>>>>,
}

impl MockNoteRepository {
    /// Creates a mock repository initialized with a valid serialized Automerge document.
    fn new(initial_document: Vec<u8>) -> Self {
        Self {
            stored_document: Rc::new(RefCell::new(initial_document)),
            saved_documents: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Returns every document passed to the save port by the manager.
    fn saved_documents(&self) -> Vec<Vec<u8>> {
        self.saved_documents.borrow().clone()
    }
}

impl NoteRepository for MockNoteRepository {
    fn create_meeting(
        &self,
        _meeting_id: &MeetingId,
        _initial_crdt_document: &[u8],
    ) -> Result<(), StorageError> {
        Ok(())
    }

    fn save_transcript(&self, _transcript: &Transcript) -> Result<(), StorageError> {
        Ok(())
    }

    fn load_transcript(&self, _meeting_id: &MeetingId) -> Result<Option<Transcript>, StorageError> {
        Ok(None)
    }

    fn save_crdt_document(
        &self,
        _meeting_id: &MeetingId,
        crdt_document: &[u8],
    ) -> Result<(), StorageError> {
        let persisted_document = crdt_document.to_vec();
        *self.stored_document.borrow_mut() = persisted_document.clone();
        self.saved_documents.borrow_mut().push(persisted_document);
        Ok(())
    }

    fn load_crdt_document(&self, _meeting_id: &MeetingId) -> Result<Vec<u8>, StorageError> {
        Ok(self.stored_document.borrow().clone())
    }

    fn save_unassigned_clip(
        &self,
        _clip: crate::domain::models::UnassignedVoiceClip,
    ) -> Result<(), StorageError> {
        Ok(())
    }

    fn get_unassigned_clips(
        &self,
        _meeting_id: &MeetingId,
    ) -> Result<Vec<crate::domain::models::UnassignedVoiceClip>, StorageError> {
        Ok(Vec::new())
    }
}

/// Records encrypted sends and broadcast payloads without opening a network connection.
#[derive(Clone)]
struct MockP2pNetwork {
    queued_events: Rc<RefCell<VecDeque<QueuedNetworkEvent>>>,
    broadcast_payloads: Rc<RefCell<Vec<Vec<u8>>>>,
}

impl MockP2pNetwork {
    /// Creates a mock network that yields the supplied events in FIFO order.
    fn new(events: impl IntoIterator<Item = QueuedNetworkEvent>) -> Self {
        Self {
            queued_events: Rc::new(RefCell::new(events.into_iter().collect())),
            broadcast_payloads: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Returns all payloads passed to the encrypted broadcast port.
    fn broadcast_payloads(&self) -> Vec<Vec<u8>> {
        self.broadcast_payloads.borrow().clone()
    }
}

impl P2pNetwork for MockP2pNetwork {
    fn send_encrypted(
        &mut self,
        _peer_device_id: &DeviceId,
        _payload: &[u8],
    ) -> Result<(), NetworkError> {
        Ok(())
    }

    fn broadcast_encrypted(&mut self, payload: &[u8]) -> Result<(), NetworkError> {
        self.broadcast_payloads.borrow_mut().push(payload.to_vec());
        Ok(())
    }

    fn poll_event(&mut self) -> Result<Option<NetworkEvent>, NetworkError> {
        self.queued_events
            .borrow_mut()
            .pop_front()
            .unwrap_or(Ok(None))
    }
}

/// Creates a valid handshake response containing an offline edit from a simulated peer.
fn remote_edit_message(meeting_id: &MeetingId, initial_document: &[u8]) -> SyncMessage {
    let mut remote_document = AutoCommit::load(initial_document)
        .unwrap_or_else(|error| panic!("test document must load: {error}"));
    let mut local_document = AutoCommit::load(initial_document)
        .unwrap_or_else(|error| panic!("test document must load: {error}"));
    let mut local_sync_state = sync::State::new();
    let initial_handshake_message = local_document
        .sync()
        .generate_sync_message(&mut local_sync_state)
        .unwrap_or_else(|| panic!("initial handshake must generate a sync message"));
    let mut remote_sync_state = sync::State::new();
    remote_document
        .sync()
        .receive_sync_message(&mut remote_sync_state, initial_handshake_message)
        .unwrap_or_else(|error| panic!("peer must accept the initial handshake: {error}"));
    remote_document
        .put(ROOT, "note_text", "Remote peer edit")
        .unwrap_or_else(|error| panic!("test edit must apply: {error}"));
    let automerge_message = remote_document
        .sync()
        .generate_sync_message(&mut remote_sync_state)
        .unwrap_or_else(|| panic!("remote edit must generate a sync message"));

    SyncMessage::new(meeting_id.clone(), automerge_message.encode())
}

/// Verifies that a valid remote sync message is merged and its resulting document is persisted.
#[test]
fn remote_sync_message_merges_and_persists_the_local_document() {
    let meeting_id = MeetingId::new("meeting-sync-receive");
    let mut initial_document = AutoCommit::new();
    let local_document = initial_document.save();
    let sync_message = remote_edit_message(&meeting_id, &local_document);
    let mock_repository = MockNoteRepository::new(local_document);
    let peer_device_id = DeviceId::new("peer-naples");
    let mock_network = MockP2pNetwork::new([
        Ok(Some(NetworkEvent::PeerDiscovered(peer_device_id.clone()))),
        Ok(Some(NetworkEvent::MessageReceived {
            peer_device_id,
            payload: sync_message
                .to_bytes()
                .unwrap_or_else(|error| panic!("test envelope must serialize: {error}")),
        })),
    ]);
    let repository_inspector = mock_repository.clone();
    let mut sync_manager = SyncManager::new(meeting_id, mock_repository, mock_network);

    sync_manager
        .process_next_network_event()
        .unwrap_or_else(|error| panic!("peer discovery must start a handshake: {error}"));
    sync_manager
        .process_next_network_event()
        .unwrap_or_else(|error| panic!("valid remote sync must succeed: {error}"));

    assert_eq!(repository_inspector.saved_documents().len(), 1);
    let saved_documents = repository_inspector.saved_documents();
    let merged_document = AutoCommit::load(&saved_documents[0])
        .unwrap_or_else(|error| panic!("saved document must remain valid Automerge: {error}"));
    match merged_document
        .get(ROOT, "note_text")
        .unwrap_or_else(|error| panic!("merged note text must be readable: {error}"))
    {
        Some((Value::Scalar(note_text), _)) => {
            assert_eq!(note_text.to_str(), Some("Remote peer edit"));
        }
        _ => panic!("remote sync must persist the remote note text"),
    }
}

/// Verifies that a local edit produces an envelope with an Automerge sync payload for broadcast.
#[test]
fn local_change_broadcasts_an_automerge_sync_message() {
    let meeting_id = MeetingId::new("meeting-sync-broadcast");
    let mut edited_document = AutoCommit::new();
    edited_document
        .put(ROOT, "note_text", "Local edit")
        .unwrap_or_else(|error| panic!("test edit must apply: {error}"));
    let mock_repository = MockNoteRepository::new(edited_document.save());
    let mock_network = MockP2pNetwork::new([]);
    let network_inspector = mock_network.clone();
    let mut sync_manager = SyncManager::new(meeting_id.clone(), mock_repository, mock_network);

    sync_manager
        .notify_local_change()
        .unwrap_or_else(|error| panic!("local change broadcast must succeed: {error}"));

    let broadcast_payloads = network_inspector.broadcast_payloads();
    assert_eq!(broadcast_payloads.len(), 1);
    let broadcast_message = SyncMessage::from_bytes(&broadcast_payloads[0])
        .unwrap_or_else(|error| panic!("broadcast payload must be a sync envelope: {error}"));
    assert_eq!(broadcast_message.meeting_id(), &meeting_id);
    assert!(automerge::sync::Message::decode(broadcast_message.sync_bytes()).is_ok());
}

/// Verifies that malformed peer data is rejected without persisting a partial document or panicking.
#[test]
fn malformed_sync_message_is_rejected_without_persisting() {
    let meeting_id = MeetingId::new("meeting-sync-malformed");
    let mut initial_document = AutoCommit::new();
    let mock_repository = MockNoteRepository::new(initial_document.save());
    let mock_network = MockP2pNetwork::new([Ok(Some(NetworkEvent::MessageReceived {
        peer_device_id: DeviceId::new("peer-bologna"),
        payload: vec![0, 255, 1],
    }))]);
    let repository_inspector = mock_repository.clone();
    let mut sync_manager = SyncManager::new(meeting_id, mock_repository, mock_network);

    let received_error = sync_manager
        .process_next_network_event()
        .expect_err("malformed sync data must be rejected");

    assert!(matches!(
        received_error,
        SyncManagerError::MalformedMessage { .. }
    ));
    assert!(repository_inspector.saved_documents().is_empty());
}
