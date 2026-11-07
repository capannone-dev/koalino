//! Specifies how SQLite stores a meeting folded into another device's recording of it (#88):
//! folding, listing recordings, splitting them apart for good, and receiving a fold from a peer.

use std::{path::PathBuf, sync::Arc};

use uuid::Uuid;

use super::sqlite_adapter::SqliteNoteRepository;
use crate::{
    application::{
        crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
        meeting_document::AutomergeMeetingDocumentCodec,
    },
    domain::{
        errors::StorageError,
        models::{DeviceId, MeetingId},
    },
    ports::{
        meeting_document::MeetingCombination,
        storage::{MeetingRecording, NoteRepository},
    },
};

/// A device database with the meeting-document codec, removed on drop.
struct Device {
    directory: PathBuf,
    repository: SqliteNoteRepository,
}

impl Device {
    fn new(device_id: &str) -> Self {
        let directory = std::env::temp_dir().join(format!("combined-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap_or_else(|error| panic!("{error}"));
        let repository = SqliteNoteRepository::open(
            directory.join("notes.sqlite3"),
            DeviceId::new(device_id),
            Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([5; 32]))),
        )
        .and_then(|repository| {
            repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
        })
        .unwrap_or_else(|error| panic!("repository must open: {error}"));
        Self {
            directory,
            repository,
        }
    }

    fn meeting(&self, meeting_id: &str) -> MeetingId {
        let meeting_id = MeetingId::new(meeting_id);
        self.repository
            .create_meeting(&meeting_id, &automerge::AutoCommit::new().save())
            .unwrap_or_else(|error| panic!("meeting must be created: {error}"));
        meeting_id
    }

    fn recording(&self, meeting_id: &MeetingId) -> MeetingRecording {
        self.repository
            .meeting_recordings()
            .unwrap_or_else(|error| panic!("{error}"))
            .into_iter()
            .find(|recording| &recording.meeting_id == meeting_id)
            .unwrap_or_else(|| panic!("the meeting must be listed"))
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn into_laptop_meeting(laptop_meeting: &MeetingId) -> MeetingCombination {
    MeetingCombination {
        into: laptop_meeting.clone(),
        offset_ms: 2_500,
        combine_on: DeviceId::new("laptop"),
    }
}

#[test]
fn a_meeting_is_folded_into_the_other_recording_and_listed_as_such() {
    let device = Device::new("laptop");
    let laptop_meeting = device.meeting("meeting-laptop");
    let phone_meeting = device.meeting("meeting-phone");

    device
        .repository
        .combine_meeting(&phone_meeting, &into_laptop_meeting(&laptop_meeting))
        .unwrap_or_else(|error| panic!("{error}"));

    let folded = device.recording(&phone_meeting);
    assert_eq!(
        folded.combined_into,
        Some(into_laptop_meeting(&laptop_meeting))
    );
    assert_eq!(folded.acquisition_device_id, DeviceId::new("laptop"));
    assert_eq!(device.recording(&laptop_meeting).combined_into, None);
}

#[test]
fn splitting_lists_both_again_and_keeps_them_apart() {
    let device = Device::new("laptop");
    let laptop_meeting = device.meeting("meeting-laptop");
    let phone_meeting = device.meeting("meeting-phone");
    device
        .repository
        .combine_meeting(&phone_meeting, &into_laptop_meeting(&laptop_meeting))
        .unwrap_or_else(|error| panic!("{error}"));

    let split = device.repository.split_meeting(&phone_meeting);

    assert!(matches!(split, Ok(true)));
    let phone = device.recording(&phone_meeting);
    assert_eq!(phone.combined_into, None);
    assert_eq!(phone.kept_apart_from, vec![laptop_meeting]);
    assert!(matches!(
        device.repository.split_meeting(&phone_meeting),
        Ok(false)
    ));
}

#[test]
fn a_fold_made_on_one_device_reaches_its_peer_with_the_document() {
    let laptop = Device::new("laptop");
    let laptop_meeting = laptop.meeting("meeting-laptop");
    let phone_meeting = laptop.meeting("meeting-phone");
    laptop
        .repository
        .combine_meeting(&phone_meeting, &into_laptop_meeting(&laptop_meeting))
        .unwrap_or_else(|error| panic!("{error}"));
    let document = laptop
        .repository
        .load_crdt_document(&phone_meeting)
        .unwrap_or_else(|error| panic!("{error}"));

    let phone = Device::new("phone");
    phone
        .repository
        .create_meeting(&phone_meeting, &document)
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(
        phone.recording(&phone_meeting).combined_into,
        Some(into_laptop_meeting(&laptop_meeting))
    );
}

#[test]
fn folding_into_an_unknown_meeting_or_into_itself_is_refused() {
    let device = Device::new("laptop");
    let phone_meeting = device.meeting("meeting-phone");

    let unknown = device.repository.combine_meeting(
        &phone_meeting,
        &into_laptop_meeting(&MeetingId::new("meeting-missing")),
    );
    let itself = device
        .repository
        .combine_meeting(&phone_meeting, &into_laptop_meeting(&phone_meeting));

    assert!(matches!(unknown, Err(StorageError::Rejected { .. })));
    assert!(matches!(itself, Err(StorageError::Rejected { .. })));
    assert_eq!(device.recording(&phone_meeting).combined_into, None);
}

#[test]
fn deleting_a_combined_meeting_deletes_the_recording_folded_into_it() {
    let laptop = Device::new("laptop");
    let first = laptop.meeting("meeting-laptop");
    let folded = laptop.meeting("meeting-phone");
    let kept = laptop.meeting("meeting-other");
    laptop
        .repository
        .combine_meeting(
            &folded,
            &MeetingCombination {
                into: first.clone(),
                offset_ms: 2_000,
                combine_on: DeviceId::new("laptop"),
            },
        )
        .unwrap_or_else(|error| panic!("{error}"));

    crate::application::deletion::delete_meeting(&laptop.repository, None, &first)
        .unwrap_or_else(|error| panic!("{error}"));

    let left: Vec<MeetingId> = laptop
        .repository
        .list_meeting_ids()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(left, vec![kept]);
}

#[test]
fn splitting_the_listed_meeting_lists_its_folded_recordings_again() {
    use crate::application::combined_meetings::split_recordings;
    let laptop = Device::new("laptop");
    let first = laptop.meeting("meeting-laptop");
    let folded = laptop.meeting("meeting-phone");
    laptop
        .repository
        .combine_meeting(
            &folded,
            &MeetingCombination {
                into: first.clone(),
                offset_ms: 2_000,
                combine_on: DeviceId::new("laptop"),
            },
        )
        .unwrap_or_else(|error| panic!("{error}"));

    let split =
        split_recordings(&laptop.repository, &first).unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(split, vec![folded.clone()]);
    assert_eq!(laptop.recording(&folded).combined_into, None);
    assert_eq!(
        laptop.recording(&folded).kept_apart_from,
        vec![first.clone()]
    );
    assert!(
        split_recordings(&laptop.repository, &first)
            .unwrap_or_else(|error| panic!("{error}"))
            .is_empty(),
        "nothing left to split"
    );
}

#[test]
fn a_folded_recording_can_be_split_by_its_own_id() {
    use crate::application::combined_meetings::split_recordings;
    let laptop = Device::new("laptop");
    let first = laptop.meeting("meeting-laptop");
    let folded = laptop.meeting("meeting-phone");
    laptop
        .repository
        .combine_meeting(
            &folded,
            &MeetingCombination {
                into: first,
                offset_ms: 2_000,
                combine_on: DeviceId::new("laptop"),
            },
        )
        .unwrap_or_else(|error| panic!("{error}"));

    let split =
        split_recordings(&laptop.repository, &folded).unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(split, vec![folded]);
}
