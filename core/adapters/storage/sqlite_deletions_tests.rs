//! Specifies deleting meetings and unknown voices: everything they hold goes in one step, and the
//! deletion records keep paired devices from bringing them back.

use std::{path::PathBuf, sync::Arc};

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use super::sqlite_adapter::SqliteNoteRepository;
use crate::{
    application::crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
    domain::{
        errors::StorageError,
        models::{
            ClusterShareImport, DeletionRecords, DeviceId, MeetingId, Transcript,
            UnassignedVoiceClip, UnknownSpeakerChunk, UnknownSpeakerCluster,
        },
    },
    ports::storage::NoteRepository,
};

const CLUSTER: Uuid = Uuid::from_u128(0xC1);
const OTHER_CLUSTER: Uuid = Uuid::from_u128(0xC2);

fn doomed_meeting() -> MeetingId {
    MeetingId::new("meeting-doomed")
}

fn kept_meeting() -> MeetingId {
    MeetingId::new("meeting-kept")
}

/// A repository in a fresh scratch directory, removed on drop.
struct Device {
    directory: PathBuf,
    repository: SqliteNoteRepository,
}

impl Device {
    /// Opens a repository holding both meetings, each with a transcript and a cluster.
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("deletions-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory)
            .unwrap_or_else(|error| panic!("scratch directory must be created: {error}"));
        let repository = SqliteNoteRepository::open(
            directory.join("notes.sqlite3"),
            DeviceId::new("device"),
            Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([9; 32]))),
        )
        .unwrap_or_else(|error| panic!("repository must open: {error}"));
        let device = Self {
            directory,
            repository,
        };
        device.record(&doomed_meeting(), CLUSTER, 1);
        device.record(&kept_meeting(), OTHER_CLUSTER, 2);
        device
    }

    /// Stores `meeting_id` with a transcript, one cluster, one chunk, and one review clip.
    fn record(&self, meeting_id: &MeetingId, cluster_id: Uuid, clip_number: u128) {
        let repository = &self.repository;
        repository
            .create_meeting(meeting_id, &[1])
            .and_then(|()| repository.save_transcript(&Transcript::new(meeting_id.clone(), "text")))
            .and_then(|()| {
                repository
                    .save_unknown_speaker_cluster_chunk(&cluster(meeting_id, cluster_id), &chunk())
            })
            .and_then(|()| {
                repository.save_unassigned_clip(clip(meeting_id, cluster_id, clip_number))
            })
            .unwrap_or_else(|error| panic!("meeting must be recorded: {error}"));
    }

    /// Whether `meeting_id` is still listed.
    fn lists(&self, meeting_id: &MeetingId) -> bool {
        self.repository
            .list_meeting_ids()
            .unwrap_or_else(|error| panic!("meetings must list: {error}"))
            .contains(meeting_id)
    }

    /// Counts the rows of `table` matching `condition`, through a second connection.
    fn count(&self, table: &str, condition: &str) -> i64 {
        rusqlite::Connection::open(self.directory.join("notes.sqlite3"))
            .and_then(|connection| {
                connection.query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE {condition}"),
                    [],
                    |row| row.get(0),
                )
            })
            .unwrap_or_else(|error| panic!("rows must count: {error}"))
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn cluster(meeting_id: &MeetingId, cluster_id: Uuid) -> UnknownSpeakerCluster {
    UnknownSpeakerCluster::new(
        cluster_id,
        meeting_id.clone(),
        vec![0.5, 0.25, -0.25],
        vec!["chunk".to_owned()],
        1_000,
        Utc.timestamp_millis_opt(1_700_000_000_000)
            .single()
            .unwrap_or_else(|| panic!("valid timestamp")),
        None,
    )
}

fn chunk() -> UnknownSpeakerChunk {
    UnknownSpeakerChunk::new("chunk", vec![0.5, 0.25, -0.25], 1_000)
}

fn clip(meeting_id: &MeetingId, cluster_id: Uuid, clip_number: u128) -> UnassignedVoiceClip {
    UnassignedVoiceClip::with_review_context(
        Uuid::from_u128(0x100 + clip_number),
        meeting_id.clone(),
        0,
        900,
        vec![0xAB; 48],
        vec![0.5, 0.25, -0.25],
        None,
    )
    .with_cluster_id(Some(cluster_id))
}

fn checked<T>(result: Result<T, StorageError>) -> T {
    result.unwrap_or_else(|error| panic!("storage must succeed: {error}"))
}

/// A deleted meeting leaves nothing behind, and the other meeting is untouched.
#[test]
fn deleting_a_meeting_removes_everything_it_holds() {
    let device = Device::new();

    assert!(checked(device.repository.delete_meeting(&doomed_meeting())));

    assert!(!device.lists(&doomed_meeting()));
    let doomed = "meeting_id = 'meeting-doomed'";
    for table in [
        "meetings",
        "unassigned_voice_clips",
        "unknown_speaker_clusters",
    ] {
        assert_eq!(device.count(table, doomed), 0, "{table}");
    }
    assert_eq!(
        device.count("cluster_chunks", &format!("cluster_id = '{CLUSTER}'")),
        0
    );
    assert!(device.lists(&kept_meeting()));
    assert_eq!(
        device.count("unknown_speaker_clusters", "meeting_id = 'meeting-kept'"),
        1
    );
    assert_eq!(
        device.count("unassigned_voice_clips", "meeting_id = 'meeting-kept'"),
        1
    );
}

/// Deleting a meeting this device never had reports it and records nothing.
#[test]
fn deleting_an_unknown_meeting_changes_nothing() {
    let device = Device::new();

    assert!(!checked(
        device.repository.delete_meeting(&MeetingId::new("missing"))
    ));

    assert_eq!(
        checked(device.repository.deletion_records()),
        DeletionRecords::default()
    );
}

/// A new write takes a later revision than the deleted newest meeting had, so a watcher that saw
/// that revision still notices the write.
#[test]
fn a_write_after_deleting_the_newest_meeting_is_still_noticed() {
    let device = Device::new();
    let seen = checked(device.repository.document_changes_since(0)).latest_revision;

    assert!(checked(device.repository.delete_meeting(&kept_meeting())));
    checked(
        device
            .repository
            .create_meeting(&MeetingId::new("meeting-new"), &[1]),
    );

    let changes = checked(device.repository.document_changes_since(seen));
    assert_eq!(changes.meeting_ids, vec![MeetingId::new("meeting-new")]);
}

/// A meeting's deletion stays on record, for peers and for a later check.
#[test]
fn a_deleted_meeting_stays_on_record() {
    let device = Device::new();

    checked(device.repository.delete_meeting(&doomed_meeting()));

    assert!(checked(
        device.repository.is_meeting_deleted(&doomed_meeting())
    ));
    assert!(!checked(
        device.repository.is_meeting_deleted(&kept_meeting())
    ));
    assert_eq!(
        checked(device.repository.deletion_records()).meeting_ids,
        vec![doomed_meeting()]
    );
}

/// Deleting an unknown voice removes its data, leaves its meeting, and blocks a later share.
#[test]
fn a_deleted_unknown_voice_cannot_be_shared_back() {
    let device = Device::new();
    let share = checked(device.repository.load_cluster_share(CLUSTER))
        .unwrap_or_else(|| panic!("the cluster must have a share"));

    assert!(checked(device.repository.delete_unknown_voice(CLUSTER)));

    assert!(checked(device.repository.load_unknown_speaker_cluster(CLUSTER)).is_none());
    assert_eq!(
        device.count(
            "unassigned_voice_clips",
            &format!("cluster_id = '{CLUSTER}'")
        ),
        0
    );
    assert!(device.lists(&doomed_meeting()));
    assert_eq!(
        checked(device.repository.import_cluster_share(&share)),
        ClusterShareImport::Deleted
    );
    assert!(checked(device.repository.load_unknown_speaker_cluster(CLUSTER)).is_none());
    assert!(!checked(device.repository.delete_unknown_voice(CLUSTER)));
}

/// A share of any voice in a deleted meeting is refused, rather than waiting for the meeting.
#[test]
fn a_voice_share_of_a_deleted_meeting_is_refused() {
    let device = Device::new();
    let share = checked(device.repository.load_cluster_share(CLUSTER))
        .unwrap_or_else(|| panic!("the cluster must have a share"));

    checked(device.repository.delete_meeting(&doomed_meeting()));

    assert_eq!(
        checked(device.repository.import_cluster_share(&share)),
        ClusterShareImport::Deleted
    );
}

/// Records from a paired device delete the same things, and applying them twice changes nothing.
#[test]
fn records_from_a_peer_delete_here_too() {
    let device = Device::new();
    let records = DeletionRecords {
        meeting_ids: vec![doomed_meeting()],
        cluster_ids: vec![OTHER_CLUSTER],
    };

    assert_eq!(
        checked(device.repository.apply_deletion_records(&records)),
        vec![doomed_meeting()]
    );

    assert!(!device.lists(&doomed_meeting()));
    assert!(
        checked(
            device
                .repository
                .load_unknown_speaker_cluster(OTHER_CLUSTER)
        )
        .is_none()
    );
    assert!(device.lists(&kept_meeting()));
    assert_eq!(checked(device.repository.deletion_records()), records);
    assert!(checked(device.repository.apply_deletion_records(&records)).is_empty());
}
