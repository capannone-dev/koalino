//! Specifies meeting-title replication through the Automerge meeting document (Issue #39).
//!
//! Two in-memory SQLite repositories play peers A and B. Their `SyncManager`s exchange real
//! Automerge sync messages through a linked in-process network double, so these tests cover the
//! CRDT schema, the SQLite projection, conflict resolution, and legacy-document migration without
//! opening a socket.

use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use automerge::{AutoCommit, ROOT, transaction::Transactable};

use crate::{
    adapters::storage::sqlite_adapter::SqliteNoteRepository,
    application::{
        crypto::voice_encryptor::ephemeral_cipher, meeting_crdt::MeetingCrdtService,
        sync_manager::SyncManager,
    },
    domain::{
        errors::NetworkError,
        models::{DeviceId, MeetingId, MeetingTitle},
    },
    ports::{
        network::{NetworkEvent, P2pNetwork},
        storage::NoteRepository,
    },
};

/// Upper bound for every sync payload exchanged after one rename of an already-synced meeting.
/// A title is a single scalar `put`, so a rename must cost a few hundred bytes, not a document.
const RENAME_SYNC_BYTES_BUDGET: usize = 1024;

/// One direction of the in-process link: events waiting to be polled by a peer.
type Inbox = Rc<RefCell<VecDeque<NetworkEvent>>>;

/// Delivers every encrypted send or broadcast straight into the other peer's inbox.
struct LinkedNetwork {
    /// Identity this side reports as the sender of its payloads.
    own_device_id: DeviceId,
    /// Events this side will poll.
    own_inbox: Inbox,
    /// Events the other side will poll.
    peer_inbox: Inbox,
    /// Sizes of every payload this side sent, for the payload-budget test.
    sent_payload_sizes: Rc<RefCell<Vec<usize>>>,
}

impl LinkedNetwork {
    /// Queues one payload for the other peer and records its size.
    fn deliver(&self, payload: &[u8]) {
        self.sent_payload_sizes.borrow_mut().push(payload.len());
        self.peer_inbox
            .borrow_mut()
            .push_back(NetworkEvent::MessageReceived {
                peer_device_id: self.own_device_id.clone(),
                payload: payload.to_vec(),
            });
    }
}

impl P2pNetwork for LinkedNetwork {
    fn send_encrypted(
        &mut self,
        _peer_device_id: &DeviceId,
        payload: &[u8],
    ) -> Result<(), NetworkError> {
        self.deliver(payload);
        Ok(())
    }

    fn broadcast_encrypted(&mut self, payload: &[u8]) -> Result<(), NetworkError> {
        self.deliver(payload);
        Ok(())
    }

    fn poll_event(&mut self) -> Result<Option<NetworkEvent>, NetworkError> {
        Ok(self.own_inbox.borrow_mut().pop_front())
    }
}

/// One simulated device: its SQLite repository plus handles into its network link.
struct Peer {
    repository: SqliteNoteRepository,
    device_id: DeviceId,
    inbox: Inbox,
    sent_payload_sizes: Rc<RefCell<Vec<usize>>>,
}

impl Peer {
    /// Opens an in-memory peer that already holds `meeting_id` as an empty CRDT document,
    /// matching how a device learns of a meeting before syncing its content.
    fn with_empty_meeting(device_name: &str, meeting_id: &MeetingId) -> Self {
        let peer = Self::without_meeting(device_name);
        MeetingCrdtService::new(&peer.repository)
            .create_meeting(meeting_id)
            .unwrap_or_else(|error| panic!("meeting creation must succeed: {error}"));
        peer
    }

    /// Opens an in-memory peer with no meetings.
    fn without_meeting(device_name: &str) -> Self {
        Self {
            repository: SqliteNoteRepository::open_in_memory(ephemeral_cipher())
                .unwrap_or_else(|error| panic!("in-memory SQLite must open: {error}")),
            device_id: DeviceId::new(device_name),
            inbox: Rc::new(RefCell::new(VecDeque::new())),
            sent_payload_sizes: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Borrows a CRDT service over this peer's repository.
    fn crdt(&self) -> MeetingCrdtService<&SqliteNoteRepository> {
        MeetingCrdtService::new(&self.repository)
    }

    /// Renames the meeting locally through the CRDT service.
    fn rename(&self, meeting_id: &MeetingId, title_text: &str) {
        self.crdt()
            .set_meeting_title(meeting_id, &title(title_text))
            .unwrap_or_else(|error| panic!("local rename must succeed: {error}"));
    }

    /// Reads the title projected into this peer's SQLite metadata.
    fn stored_title(&self, meeting_id: &MeetingId) -> Option<String> {
        self.repository
            .load_meeting_metadata(meeting_id)
            .unwrap_or_else(|error| panic!("metadata must load: {error}"))
            .unwrap_or_else(|| panic!("meeting must exist"))
            .title()
            .map(|stored_title| stored_title.as_str().to_owned())
    }

    /// Reads the title held in this peer's CRDT document.
    fn crdt_title(&self, meeting_id: &MeetingId) -> Option<String> {
        self.crdt()
            .load_title(meeting_id)
            .unwrap_or_else(|error| panic!("CRDT title must load: {error}"))
            .map(|crdt_title| crdt_title.as_str().to_owned())
    }

    /// Replaces this peer's stored CRDT document with `edit` applied to it, bypassing the
    /// service, to model documents written by older or misbehaving builds.
    fn edit_raw_document(&self, meeting_id: &MeetingId, edit: impl FnOnce(&mut AutoCommit)) {
        let stored_document = self
            .repository
            .load_crdt_document(meeting_id)
            .unwrap_or_else(|error| panic!("document must load: {error}"));
        let mut raw_document = AutoCommit::load(&stored_document)
            .unwrap_or_else(|error| panic!("document must deserialize: {error}"));
        edit(&mut raw_document);
        self.repository
            .save_crdt_document(meeting_id, &raw_document.save())
            .unwrap_or_else(|error| panic!("document must save: {error}"));
    }
}

/// Parses a test title.
fn title(title_text: &str) -> MeetingTitle {
    MeetingTitle::parse(title_text).unwrap_or_else(|error| panic!("test title is valid: {error}"))
}

/// Runs a full sync session between two peers: both discover each other, then every queued
/// message is processed until neither side has anything left to say.
///
/// Returns the sizes of every payload exchanged during the session.
fn sync_peers(meeting_id: &MeetingId, peer_a: &Peer, peer_b: &Peer) -> Vec<usize> {
    let network_a = LinkedNetwork {
        own_device_id: peer_a.device_id.clone(),
        own_inbox: Rc::clone(&peer_a.inbox),
        peer_inbox: Rc::clone(&peer_b.inbox),
        sent_payload_sizes: Rc::clone(&peer_a.sent_payload_sizes),
    };
    let network_b = LinkedNetwork {
        own_device_id: peer_b.device_id.clone(),
        own_inbox: Rc::clone(&peer_b.inbox),
        peer_inbox: Rc::clone(&peer_a.inbox),
        sent_payload_sizes: Rc::clone(&peer_b.sent_payload_sizes),
    };
    peer_a.sent_payload_sizes.borrow_mut().clear();
    peer_b.sent_payload_sizes.borrow_mut().clear();
    let mut manager_a = SyncManager::new(meeting_id.clone(), &peer_a.repository, network_a);
    let mut manager_b = SyncManager::new(meeting_id.clone(), &peer_b.repository, network_b);
    peer_a
        .inbox
        .borrow_mut()
        .push_back(NetworkEvent::PeerDiscovered(peer_b.device_id.clone()));
    peer_b
        .inbox
        .borrow_mut()
        .push_back(NetworkEvent::PeerDiscovered(peer_a.device_id.clone()));

    for _round in 0..64 {
        if peer_a.inbox.borrow().is_empty() && peer_b.inbox.borrow().is_empty() {
            let mut exchanged_sizes = peer_a.sent_payload_sizes.borrow().clone();
            exchanged_sizes.extend(peer_b.sent_payload_sizes.borrow().iter());
            return exchanged_sizes;
        }
        manager_a
            .process_next_network_event()
            .unwrap_or_else(|error| panic!("peer A must process sync: {error}"));
        manager_b
            .process_next_network_event()
            .unwrap_or_else(|error| panic!("peer B must process sync: {error}"));
    }
    panic!("sync session did not reach quiescence");
}

/// Verifies a meeting created with a title carries it in the CRDT document and SQLite metadata.
#[test]
fn creating_a_titled_meeting_stores_the_title_in_the_crdt_document() {
    let meeting_id = MeetingId::new("meeting-title-create");
    let peer = Peer::without_meeting("peer-turin");

    peer.crdt()
        .create_titled_meeting(&meeting_id, &title("Quarterly planning"))
        .unwrap_or_else(|error| panic!("titled creation must succeed: {error}"));

    assert_eq!(
        peer.crdt_title(&meeting_id).as_deref(),
        Some("Quarterly planning")
    );
    assert_eq!(
        peer.stored_title(&meeting_id).as_deref(),
        Some("Quarterly planning")
    );
}

/// Verifies a rename replaces the CRDT title and the projected SQLite title.
#[test]
fn renaming_updates_the_crdt_title_and_the_local_metadata() {
    let meeting_id = MeetingId::new("meeting-title-rename");
    let peer = Peer::without_meeting("peer-turin");
    peer.crdt()
        .create_titled_meeting(&meeting_id, &title("Draft name"))
        .unwrap_or_else(|error| panic!("titled creation must succeed: {error}"));

    peer.rename(&meeting_id, "Final name");

    assert_eq!(peer.crdt_title(&meeting_id).as_deref(), Some("Final name"));
    assert_eq!(
        peer.stored_title(&meeting_id).as_deref(),
        Some("Final name")
    );
}

/// Verifies re-applying the current title records no new CRDT change, so it costs no sync bytes.
#[test]
fn renaming_to_the_current_title_adds_no_crdt_change() {
    let meeting_id = MeetingId::new("meeting-title-noop");
    let peer = Peer::without_meeting("peer-turin");
    peer.crdt()
        .create_titled_meeting(&meeting_id, &title("Standup"))
        .unwrap_or_else(|error| panic!("titled creation must succeed: {error}"));
    let document_before = peer
        .repository
        .load_crdt_document(&meeting_id)
        .unwrap_or_else(|error| panic!("document must load: {error}"));

    peer.rename(&meeting_id, "  Standup ");

    let document_after = peer
        .repository
        .load_crdt_document(&meeting_id)
        .unwrap_or_else(|error| panic!("document must load: {error}"));
    assert_eq!(document_before, document_after);
}

/// Verifies a title created on peer A reaches peer B's CRDT document and SQLite metadata.
#[test]
fn title_syncs_from_peer_a_to_peer_b() {
    let meeting_id = MeetingId::new("meeting-title-sync");
    let peer_a = Peer::without_meeting("peer-milan");
    peer_a
        .crdt()
        .create_titled_meeting(&meeting_id, &title("Design review"))
        .unwrap_or_else(|error| panic!("titled creation must succeed: {error}"));
    let peer_b = Peer::with_empty_meeting("peer-rome", &meeting_id);

    sync_peers(&meeting_id, &peer_a, &peer_b);

    assert_eq!(
        peer_b.crdt_title(&meeting_id).as_deref(),
        Some("Design review")
    );
    assert_eq!(
        peer_b.stored_title(&meeting_id).as_deref(),
        Some("Design review")
    );
}

/// Verifies a rename made after an initial sync propagates within one further sync session.
#[test]
fn rename_after_sync_reaches_the_peer() {
    let meeting_id = MeetingId::new("meeting-title-resync");
    let peer_a = Peer::without_meeting("peer-milan");
    peer_a
        .crdt()
        .create_titled_meeting(&meeting_id, &title("Old name"))
        .unwrap_or_else(|error| panic!("titled creation must succeed: {error}"));
    let peer_b = Peer::with_empty_meeting("peer-rome", &meeting_id);
    sync_peers(&meeting_id, &peer_a, &peer_b);

    peer_a.rename(&meeting_id, "New name");
    sync_peers(&meeting_id, &peer_a, &peer_b);

    assert_eq!(
        peer_b.stored_title(&meeting_id).as_deref(),
        Some("New name")
    );
}

/// Verifies concurrent offline renames converge to the same single title on both peers.
#[test]
fn concurrent_renames_converge_to_one_title_on_both_peers() {
    let meeting_id = MeetingId::new("meeting-title-conflict");
    let peer_a = Peer::without_meeting("peer-milan");
    peer_a
        .crdt()
        .create_titled_meeting(&meeting_id, &title("Shared meeting"))
        .unwrap_or_else(|error| panic!("titled creation must succeed: {error}"));
    let peer_b = Peer::with_empty_meeting("peer-rome", &meeting_id);
    sync_peers(&meeting_id, &peer_a, &peer_b);

    peer_a.rename(&meeting_id, "Budget review");
    peer_b.rename(&meeting_id, "Q3 planning");
    sync_peers(&meeting_id, &peer_a, &peer_b);

    let converged_title = peer_a.crdt_title(&meeting_id);
    assert!(matches!(
        converged_title.as_deref(),
        Some("Budget review" | "Q3 planning")
    ));
    assert_eq!(peer_b.crdt_title(&meeting_id), converged_title);
    assert_eq!(peer_a.stored_title(&meeting_id), converged_title);
    assert_eq!(peer_b.stored_title(&meeting_id), converged_title);
}

/// Verifies a rename made after seeing the other peer's rename wins on both peers.
#[test]
fn causally_later_rename_wins_on_both_peers() {
    let meeting_id = MeetingId::new("meeting-title-causal");
    let peer_a = Peer::without_meeting("peer-milan");
    peer_a
        .crdt()
        .create_titled_meeting(&meeting_id, &title("First"))
        .unwrap_or_else(|error| panic!("titled creation must succeed: {error}"));
    let peer_b = Peer::with_empty_meeting("peer-rome", &meeting_id);
    sync_peers(&meeting_id, &peer_a, &peer_b);

    peer_a.rename(&meeting_id, "Second");
    sync_peers(&meeting_id, &peer_a, &peer_b);
    peer_b.rename(&meeting_id, "Third");
    sync_peers(&meeting_id, &peer_a, &peer_b);

    assert_eq!(peer_a.stored_title(&meeting_id).as_deref(), Some("Third"));
    assert_eq!(peer_b.stored_title(&meeting_id).as_deref(), Some("Third"));
}

/// Verifies a pre-#39 document with notes but no `title` key still syncs its content.
#[test]
fn legacy_document_without_a_title_still_syncs() {
    let meeting_id = MeetingId::new("meeting-title-legacy-doc");
    let peer_a = Peer::with_empty_meeting("peer-milan", &meeting_id);
    peer_a.edit_raw_document(&meeting_id, |legacy_document| {
        legacy_document
            .put(ROOT, "note_text", "Notes from an older build")
            .unwrap_or_else(|error| panic!("legacy edit must apply: {error}"));
    });
    let peer_b = Peer::with_empty_meeting("peer-rome", &meeting_id);

    sync_peers(&meeting_id, &peer_a, &peer_b);

    assert_eq!(
        peer_b
            .crdt()
            .load_note_text(&meeting_id)
            .unwrap_or_else(|error| panic!("note must load: {error}"))
            .as_deref(),
        Some("Notes from an older build")
    );
    assert_eq!(peer_b.crdt_title(&meeting_id), None);
}

/// Verifies an untitled legacy meeting shows the dated default title after syncing, and that the
/// default is derived on read rather than written into the shared document.
#[test]
fn legacy_untitled_meeting_gets_the_dated_default_title_after_sync() {
    let meeting_id = MeetingId::new("meeting-title-legacy-default");
    let peer_a = Peer::with_empty_meeting("peer-milan", &meeting_id);
    let peer_b = Peer::with_empty_meeting("peer-rome", &meeting_id);

    sync_peers(&meeting_id, &peer_a, &peer_b);

    let peer_b_metadata = peer_b
        .repository
        .load_meeting_metadata(&meeting_id)
        .unwrap_or_else(|error| panic!("metadata must load: {error}"))
        .unwrap_or_else(|| panic!("meeting must exist"));
    assert_eq!(peer_b_metadata.title(), None);
    assert_eq!(
        peer_b_metadata.display_title(),
        format!(
            "Meeting {}",
            peer_b_metadata.created_at().format("%Y-%m-%d %H:%M UTC")
        )
    );
    assert_eq!(peer_a.crdt_title(&meeting_id), None);
    assert_eq!(peer_b.crdt_title(&meeting_id), None);
}

/// Verifies a title stored only in SQLite by Issue #35 builds is copied into the CRDT document
/// the first time the meeting syncs, so it reaches peers.
#[test]
fn sqlite_only_title_is_copied_into_the_crdt_when_syncing() {
    let meeting_id = MeetingId::new("meeting-title-legacy-sqlite");
    let peer_a = Peer::with_empty_meeting("peer-milan", &meeting_id);
    peer_a
        .repository
        .save_meeting_title(&meeting_id, &title("Named before #39"))
        .unwrap_or_else(|error| panic!("legacy title must save: {error}"));
    let peer_b = Peer::with_empty_meeting("peer-rome", &meeting_id);

    sync_peers(&meeting_id, &peer_a, &peer_b);

    assert_eq!(
        peer_a.crdt_title(&meeting_id).as_deref(),
        Some("Named before #39")
    );
    assert_eq!(
        peer_b.stored_title(&meeting_id).as_deref(),
        Some("Named before #39")
    );
}

/// Verifies a peer's invalid or non-text title is ignored instead of failing sync or reaching
/// local metadata, while the rest of the document still merges.
#[test]
fn invalid_peer_titles_are_not_projected_into_local_metadata() {
    let oversized_title = "x".repeat(MeetingTitle::MAX_CHARACTERS + 1);
    let invalid_titles: [&dyn Fn(&mut AutoCommit); 3] = [
        &|document| {
            document
                .put(ROOT, "title", oversized_title.as_str())
                .unwrap_or_else(|error| panic!("raw edit must apply: {error}"));
        },
        &|document| {
            document
                .put(ROOT, "title", "line\nbreak")
                .unwrap_or_else(|error| panic!("raw edit must apply: {error}"));
        },
        &|document| {
            document
                .put(ROOT, "title", 42_i64)
                .unwrap_or_else(|error| panic!("raw edit must apply: {error}"));
        },
    ];

    for (case_index, write_invalid_title) in invalid_titles.into_iter().enumerate() {
        let meeting_id = MeetingId::new(format!("meeting-title-invalid-{case_index}"));
        let peer_a = Peer::with_empty_meeting("peer-milan", &meeting_id);
        peer_a.edit_raw_document(&meeting_id, |document| {
            write_invalid_title(document);
            document
                .put(ROOT, "note_text", "Still synced")
                .unwrap_or_else(|error| panic!("raw edit must apply: {error}"));
        });
        let peer_b = Peer::with_empty_meeting("peer-rome", &meeting_id);

        sync_peers(&meeting_id, &peer_a, &peer_b);

        assert_eq!(peer_b.stored_title(&meeting_id), None, "case {case_index}");
        assert_eq!(peer_b.crdt_title(&meeting_id), None, "case {case_index}");
        assert_eq!(
            peer_b
                .crdt()
                .load_note_text(&meeting_id)
                .unwrap_or_else(|error| panic!("note must load: {error}"))
                .as_deref(),
            Some("Still synced"),
            "case {case_index}"
        );
    }
}

/// Verifies a rename of an already-synced meeting costs only a small, bounded sync payload.
#[test]
fn rename_sync_payloads_stay_within_budget() {
    let meeting_id = MeetingId::new("meeting-title-payload");
    let peer_a = Peer::without_meeting("peer-milan");
    peer_a
        .crdt()
        .create_titled_meeting(&meeting_id, &title("Initial"))
        .unwrap_or_else(|error| panic!("titled creation must succeed: {error}"));
    peer_a.edit_raw_document(&meeting_id, |document| {
        document
            .put(ROOT, "note_text", "n".repeat(20_000))
            .unwrap_or_else(|error| panic!("raw edit must apply: {error}"));
    });
    let peer_b = Peer::with_empty_meeting("peer-rome", &meeting_id);
    sync_peers(&meeting_id, &peer_a, &peer_b);

    peer_a.rename(&meeting_id, "Renamed after a long meeting");
    let rename_payload_sizes = sync_peers(&meeting_id, &peer_a, &peer_b);

    let total_rename_bytes: usize = rename_payload_sizes.iter().sum();
    assert!(
        total_rename_bytes <= RENAME_SYNC_BYTES_BUDGET,
        "rename sync used {total_rename_bytes} bytes: {rename_payload_sizes:?}"
    );
    assert_eq!(
        peer_b.stored_title(&meeting_id).as_deref(),
        Some("Renamed after a long meeting")
    );
}
