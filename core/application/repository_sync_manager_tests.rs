//! Specifies whole-repository sync between paired devices (Issue #43).
//!
//! Two file-backed SQLite repositories with the Automerge document codec play a laptop and a
//! phone. Their `RepositorySyncManager`s exchange real Automerge sync messages through a linked
//! in-process network double, so these tests cover meeting discovery, creation of meetings that
//! exist only on the other device, the origin and transcript carried by the document, and local
//! changes made after the devices connected, without opening a socket.

use std::{cell::RefCell, collections::VecDeque, path::PathBuf, rc::Rc, sync::Arc};

use automerge::{AutoCommit, sync::SyncDoc};
use uuid::Uuid;

use crate::{
    adapters::storage::sqlite_adapter::SqliteNoteRepository,
    application::{
        crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
        meeting_crdt::MeetingCrdtService,
        meeting_document::AutomergeMeetingDocumentCodec,
        repository_sync_manager::{
            MAX_PENDING_REMOTE_MEETINGS, RepositorySyncManager, RepositorySyncManagerError,
        },
    },
    domain::{
        errors::NetworkError,
        models::{DeviceId, MeetingId, MeetingTitle, SyncMessage, Transcript, TranscriptSegment},
    },
    ports::{
        network::{NetworkEvent, P2pNetwork},
        storage::NoteRepository,
    },
};

/// Upper bound on manager steps while two devices converge; a livelock fails instead of hanging.
const MAX_SYNC_STEPS: usize = 200;

/// One direction of the in-process link: events waiting to be polled by a device.
type Inbox = Rc<RefCell<VecDeque<NetworkEvent>>>;

/// Delivers every encrypted send or broadcast straight into the other device's inbox.
struct LinkedNetwork {
    /// Identity this side reports as the sender of its payloads.
    own_device_id: DeviceId,
    /// Events this side will poll.
    own_inbox: Inbox,
    /// Events the other side will poll.
    peer_inbox: Inbox,
}

impl LinkedNetwork {
    /// Queues one payload for the other device.
    fn deliver(&self, payload: &[u8]) {
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

/// Network double that only serves queued events and drops everything sent.
struct ScriptedNetwork {
    /// Events the manager will poll, in order.
    events: VecDeque<NetworkEvent>,
}

impl P2pNetwork for ScriptedNetwork {
    fn send_encrypted(&mut self, _: &DeviceId, _: &[u8]) -> Result<(), NetworkError> {
        Ok(())
    }

    fn broadcast_encrypted(&mut self, _: &[u8]) -> Result<(), NetworkError> {
        Ok(())
    }

    fn poll_event(&mut self) -> Result<Option<NetworkEvent>, NetworkError> {
        Ok(self.events.pop_front())
    }
}

/// A device's sync manager over its own repository.
type DeviceManager = RepositorySyncManager<SqliteNoteRepository, LinkedNetwork>;

/// One simulated device: its sync manager (which owns the repository) and its inbox.
struct Device {
    device_id: DeviceId,
    manager: DeviceManager,
    inbox: Inbox,
}

impl Device {
    /// Borrows this device's repository from its manager.
    fn repository(&self) -> &SqliteNoteRepository {
        self.manager.repository()
    }

    /// Borrows a CRDT service over this device's repository.
    fn crdt(&self) -> MeetingCrdtService<&SqliteNoteRepository> {
        MeetingCrdtService::new(self.repository())
    }

    /// Records a titled meeting with a transcript and segments, as the recording pipeline does.
    fn record_meeting(&self, meeting_id: &MeetingId, title_text: &str) {
        self.crdt()
            .create_titled_meeting(meeting_id, &title(title_text))
            .and_then(|()| {
                self.repository()
                    .save_transcript(&Transcript::new(meeting_id.clone(), "Ciao Hello"))
            })
            .and_then(|()| {
                self.repository()
                    .save_transcript_segments(meeting_id, &segments(meeting_id))
            })
            .unwrap_or_else(|error| panic!("meeting must be recorded: {error}"));
    }

    /// Tells this device's manager that `peer` connected.
    fn discover(&self, peer: &Device) {
        self.inbox
            .borrow_mut()
            .push_back(NetworkEvent::PeerDiscovered(peer.device_id.clone()));
    }

    /// Lists this device's meetings in a stable order.
    fn meeting_ids(&self) -> Vec<MeetingId> {
        let mut meeting_ids = self
            .repository()
            .list_meeting_ids()
            .unwrap_or_else(|error| panic!("meetings must list: {error}"));
        meeting_ids.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        meeting_ids
    }

    /// Reads the title projected into this device's SQLite metadata.
    fn stored_title(&self, meeting_id: &MeetingId) -> Option<String> {
        self.repository()
            .load_meeting_metadata(meeting_id)
            .unwrap_or_else(|error| panic!("metadata must load: {error}"))
            .unwrap_or_else(|| panic!("meeting must exist"))
            .title()
            .map(|stored_title| stored_title.as_str().to_owned())
    }
}

/// A fixed-key cipher shared by both devices' repositories.
fn cipher() -> Arc<VoiceEncryptor> {
    Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([9; 32])))
}

/// A database path in a fresh temporary directory (left for the OS to clean).
fn scratch_database(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("repository-sync-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory)
        .unwrap_or_else(|error| panic!("scratch directory must be created: {error}"));
    directory.join(name)
}

/// Opens a document-backed repository for `device_name`.
fn device_repository(device_name: &str) -> SqliteNoteRepository {
    SqliteNoteRepository::open(
        scratch_database(&format!("{device_name}.sqlite3")),
        DeviceId::new(device_name),
        cipher(),
    )
    .and_then(|repository| repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec)))
    .unwrap_or_else(|error| panic!("repository must open: {error}"))
}

/// Creates two devices whose networks deliver into each other's inbox.
fn linked_devices(first_name: &str, second_name: &str) -> (Device, Device) {
    let first_inbox: Inbox = Rc::new(RefCell::new(VecDeque::new()));
    let second_inbox: Inbox = Rc::new(RefCell::new(VecDeque::new()));
    let device = |name: &str, own_inbox: &Inbox, peer_inbox: &Inbox| {
        let network = LinkedNetwork {
            own_device_id: DeviceId::new(name),
            own_inbox: Rc::clone(own_inbox),
            peer_inbox: Rc::clone(peer_inbox),
        };
        Device {
            manager: RepositorySyncManager::new(device_repository(name), network),
            device_id: DeviceId::new(name),
            inbox: Rc::clone(own_inbox),
        }
    };
    (
        device(first_name, &first_inbox, &second_inbox),
        device(second_name, &second_inbox, &first_inbox),
    )
}

/// Steps both managers until neither inbox holds an event.
fn run_until_quiet(first: &mut Device, second: &mut Device) {
    for _ in 0..MAX_SYNC_STEPS {
        if first.inbox.borrow().is_empty() && second.inbox.borrow().is_empty() {
            return;
        }
        for device in [&mut *first, &mut *second] {
            device
                .manager
                .process_next_network_event()
                .unwrap_or_else(|error| panic!("sync step must succeed: {error}"));
        }
    }
    panic!("devices did not converge within {MAX_SYNC_STEPS} steps");
}

/// Parses a fixture title.
fn title(title_text: &str) -> MeetingTitle {
    MeetingTitle::parse(title_text).unwrap_or_else(|error| panic!("title must parse: {error}"))
}

/// Builds two segments of `meeting_id`.
fn segments(meeting_id: &MeetingId) -> Vec<TranscriptSegment> {
    vec![
        TranscriptSegment::new(meeting_id.clone(), 0, 1_000, None, "Ciao"),
        TranscriptSegment::new(meeting_id.clone(), 1_000, 2_000, None, "Hello"),
    ]
}

/// A meeting recorded only on the laptop appears on the phone with its title, transcript,
/// segments, and the laptop as its acquisition device.
#[test]
fn a_meeting_that_exists_only_on_one_device_is_created_on_the_other() {
    let (mut laptop, mut phone) = linked_devices("laptop", "phone");
    let meeting_id = MeetingId::new("meeting-standup");
    laptop.record_meeting(&meeting_id, "Standup");

    laptop.discover(&phone);
    phone.discover(&laptop);
    run_until_quiet(&mut laptop, &mut phone);

    assert_eq!(phone.meeting_ids(), vec![meeting_id.clone()]);
    assert_eq!(phone.stored_title(&meeting_id).as_deref(), Some("Standup"));
    let transcript = phone
        .repository()
        .load_transcript(&meeting_id)
        .unwrap_or_else(|error| panic!("transcript must load: {error}"))
        .unwrap_or_else(|| panic!("transcript must exist"));
    assert_eq!(
        transcript,
        Transcript::new(meeting_id.clone(), "Ciao Hello")
    );
    let synced_segments = phone
        .repository()
        .load_transcript_segments(&meeting_id)
        .unwrap_or_else(|error| panic!("segments must load: {error}"));
    assert_eq!(synced_segments, segments(&meeting_id));
    let acquisition_device = phone
        .repository()
        .acquisition_device_id(&meeting_id)
        .unwrap_or_else(|error| panic!("origin must load: {error}"));
    assert_eq!(acquisition_device, Some(DeviceId::new("laptop")));
}

/// Each device ends with the meetings of both when they recorded different ones offline.
#[test]
fn devices_exchange_the_meetings_each_recorded_offline() {
    let (mut laptop, mut phone) = linked_devices("laptop", "phone");
    let laptop_meeting = MeetingId::new("meeting-a-laptop");
    let phone_meeting = MeetingId::new("meeting-b-phone");
    laptop.record_meeting(&laptop_meeting, "Design review");
    phone.record_meeting(&phone_meeting, "Call with Rome");

    laptop.discover(&phone);
    phone.discover(&laptop);
    run_until_quiet(&mut laptop, &mut phone);

    let both = vec![laptop_meeting.clone(), phone_meeting.clone()];
    assert_eq!(laptop.meeting_ids(), both);
    assert_eq!(phone.meeting_ids(), both);
    assert_eq!(
        laptop.stored_title(&phone_meeting).as_deref(),
        Some("Call with Rome")
    );
    assert_eq!(
        phone.stored_title(&laptop_meeting).as_deref(),
        Some("Design review")
    );
}

/// A rename made after the devices synced reaches the connected peer once the manager is told.
#[test]
fn a_local_change_after_connecting_reaches_the_connected_peer() {
    let (mut laptop, mut phone) = linked_devices("laptop", "phone");
    let meeting_id = MeetingId::new("meeting-standup");
    laptop.record_meeting(&meeting_id, "Standup");
    laptop.discover(&phone);
    phone.discover(&laptop);
    run_until_quiet(&mut laptop, &mut phone);

    laptop
        .crdt()
        .set_meeting_title(&meeting_id, &title("Weekly standup"))
        .unwrap_or_else(|error| panic!("rename must succeed: {error}"));
    laptop
        .manager
        .notify_local_change(&meeting_id)
        .unwrap_or_else(|error| panic!("change notification must succeed: {error}"));
    run_until_quiet(&mut laptop, &mut phone);

    assert_eq!(
        phone.stored_title(&meeting_id).as_deref(),
        Some("Weekly standup")
    );
}

/// A meeting created locally after connecting reaches the peer once the manager is told.
#[test]
fn a_meeting_recorded_after_connecting_reaches_the_connected_peer() {
    let (mut laptop, mut phone) = linked_devices("laptop", "phone");
    laptop.discover(&phone);
    phone.discover(&laptop);
    run_until_quiet(&mut laptop, &mut phone);
    let meeting_id = MeetingId::new("meeting-late");

    laptop.record_meeting(&meeting_id, "Late call");
    laptop
        .manager
        .notify_local_change(&meeting_id)
        .unwrap_or_else(|error| panic!("change notification must succeed: {error}"));
    run_until_quiet(&mut laptop, &mut phone);

    assert_eq!(phone.meeting_ids(), vec![meeting_id.clone()]);
    assert_eq!(
        phone.stored_title(&meeting_id).as_deref(),
        Some("Late call")
    );
}

/// A peer that names more unknown meetings than the bound is refused, so its envelopes cannot
/// grow memory without limit, and no meeting is created from an empty handshake.
#[test]
fn unknown_meetings_beyond_the_pending_bound_are_refused() {
    let repository = device_repository("phone");
    let peer_device_id = DeviceId::new("laptop");
    let empty_handshake = AutoCommit::new()
        .sync()
        .generate_sync_message(&mut automerge::sync::State::new())
        .unwrap_or_else(|| panic!("a fresh state must produce a handshake"))
        .encode();
    let events = (0..=MAX_PENDING_REMOTE_MEETINGS)
        .map(|index| {
            let payload = SyncMessage::new(
                MeetingId::new(format!("meeting-{index}")),
                empty_handshake.clone(),
            )
            .to_bytes()
            .unwrap_or_else(|error| panic!("envelope must encode: {error}"));
            NetworkEvent::MessageReceived {
                peer_device_id: peer_device_id.clone(),
                payload,
            }
        })
        .collect();
    let mut manager = RepositorySyncManager::new(repository, ScriptedNetwork { events });

    for _ in 0..MAX_PENDING_REMOTE_MEETINGS {
        manager
            .process_next_network_event()
            .unwrap_or_else(|error| panic!("a handshake within the bound must succeed: {error}"));
    }
    let over_bound = manager.process_next_network_event();

    assert!(matches!(
        over_bound,
        Err(RepositorySyncManagerError::TooManyPendingMeetings)
    ));
    let created = manager
        .repository()
        .list_meeting_ids()
        .unwrap_or_else(|error| panic!("meetings must list: {error}"));
    assert!(created.is_empty());
}
