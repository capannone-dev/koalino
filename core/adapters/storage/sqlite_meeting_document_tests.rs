//! Specifies that the SQLite index follows the meeting document (Issue #43): local writes land in
//! the document, a received document rewrites the index, a meeting created from a peer's document
//! keeps that peer's origin, and legacy rows are migrated into the document once.

use std::{path::PathBuf, sync::Arc};

use automerge::AutoCommit;
use uuid::Uuid;

use super::sqlite_adapter::SqliteNoteRepository;
use crate::{
    application::{
        crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
        meeting_document::AutomergeMeetingDocumentCodec,
    },
    domain::models::{DeviceId, MeetingId, Transcript, TranscriptSegment},
    ports::{crypto::EmbeddingCipher, storage::NoteRepository},
};

/// The meeting all fixtures use.
fn meeting() -> MeetingId {
    MeetingId::new("meeting-rome")
}

/// A fixed-key cipher, so reopened repositories can read each other's rows.
fn cipher() -> Arc<dyn EmbeddingCipher> {
    Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([5; 32])))
}

/// Opens a document-backed repository for `device` at `path`.
fn device_repository(path: &PathBuf, device: &str) -> SqliteNoteRepository {
    SqliteNoteRepository::open(path, DeviceId::new(device), cipher())
        .and_then(|repository| {
            repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec))
        })
        .unwrap_or_else(|error| panic!("repository must open: {error}"))
}

/// A database path in a fresh temporary directory (left for the OS to clean).
fn scratch_database(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("meeting-document-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&directory)
        .unwrap_or_else(|error| panic!("scratch directory must be created: {error}"));
    directory.join(name)
}

/// Builds two segments of the fixture meeting.
fn segments() -> Vec<TranscriptSegment> {
    vec![
        TranscriptSegment::new(meeting(), 0, 1_000, Some(Uuid::from_u128(3)), "Ciao"),
        TranscriptSegment::new(meeting(), 1_000, 2_000, None, "Hello")
            .with_unknown_cluster_id(Some(Uuid::from_u128(4))),
    ]
}

/// Records a meeting with a transcript and segments on `repository`.
fn record_meeting(repository: &SqliteNoteRepository) {
    repository
        .create_meeting(&meeting(), &AutoCommit::new().save())
        .and_then(|()| repository.save_transcript(&Transcript::new(meeting(), "Ciao Hello")))
        .and_then(|()| repository.save_transcript_segments(&meeting(), &segments()))
        .unwrap_or_else(|error| panic!("meeting must be recorded: {error}"));
}

/// A meeting created on another device from the laptop's document carries the transcript,
/// segments, and the laptop's origin.
#[test]
fn a_meeting_created_from_a_peer_document_carries_its_contents_and_origin() {
    let laptop = device_repository(&scratch_database("laptop.sqlite3"), "device-laptop");
    let phone = device_repository(&scratch_database("phone.sqlite3"), "device-phone");
    record_meeting(&laptop);
    let laptop_document = laptop
        .load_crdt_document(&meeting())
        .unwrap_or_else(|error| panic!("document must load: {error}"));
    let laptop_created_at = laptop
        .load_meeting_metadata(&meeting())
        .ok()
        .flatten()
        .map(|metadata| metadata.created_at());

    phone
        .create_meeting(&meeting(), &laptop_document)
        .unwrap_or_else(|error| panic!("peer meeting must be created: {error}"));

    assert_eq!(
        phone
            .load_transcript(&meeting())
            .ok()
            .flatten()
            .map(|t| t.content().to_owned()),
        Some(String::from("Ciao Hello"))
    );
    assert_eq!(
        phone.load_transcript_segments(&meeting()).ok(),
        Some(segments())
    );
    assert_eq!(
        phone
            .load_meeting_metadata(&meeting())
            .ok()
            .flatten()
            .map(|metadata| metadata.created_at()),
        laptop_created_at
    );
    assert_eq!(
        phone.acquisition_device_id(&meeting()).ok().flatten(),
        Some(DeviceId::new("device-laptop"))
    );
}

/// Saving a received document rewrites the index, e.g. a speaker relinked on another device.
#[test]
fn saving_a_received_document_updates_the_index() {
    let laptop = device_repository(&scratch_database("laptop.sqlite3"), "device-laptop");
    let phone = device_repository(&scratch_database("phone.sqlite3"), "device-phone");
    record_meeting(&laptop);
    phone
        .create_meeting(
            &meeting(),
            &laptop
                .load_crdt_document(&meeting())
                .unwrap_or_else(|error| panic!("document must load: {error}")),
        )
        .unwrap_or_else(|error| panic!("peer meeting must be created: {error}"));
    let relinked: Vec<TranscriptSegment> = segments()
        .into_iter()
        .map(|segment| segment.with_speaker_identity_id(Some(Uuid::from_u128(9))))
        .collect();
    laptop
        .save_transcript_segments(&meeting(), &relinked)
        .unwrap_or_else(|error| panic!("relink must persist: {error}"));

    phone
        .save_crdt_document(
            &meeting(),
            &laptop
                .load_crdt_document(&meeting())
                .unwrap_or_else(|error| panic!("document must load: {error}")),
        )
        .unwrap_or_else(|error| panic!("received document must persist: {error}"));

    assert_eq!(
        phone.load_transcript_segments(&meeting()).ok(),
        Some(relinked)
    );
}

/// Rows written before the document held them are copied into it when the codec is attached,
/// so a later document save does not wipe them.
#[test]
fn legacy_rows_are_migrated_into_the_document() {
    let path = scratch_database("legacy.sqlite3");
    let legacy = SqliteNoteRepository::open(&path, DeviceId::new("device-laptop"), cipher())
        .unwrap_or_else(|error| panic!("legacy repository must open: {error}"));
    record_meeting(&legacy);
    drop(legacy);

    let upgraded = device_repository(&path, "device-laptop");
    let migrated_document = upgraded
        .load_crdt_document(&meeting())
        .unwrap_or_else(|error| panic!("document must load: {error}"));
    upgraded
        .save_crdt_document(&meeting(), &migrated_document)
        .unwrap_or_else(|error| panic!("document must save: {error}"));
    let other_device = device_repository(&scratch_database("phone.sqlite3"), "device-phone");
    other_device
        .create_meeting(&meeting(), &migrated_document)
        .unwrap_or_else(|error| panic!("peer meeting must be created: {error}"));

    assert_eq!(
        upgraded.load_transcript_segments(&meeting()).ok(),
        Some(segments())
    );
    assert_eq!(
        other_device.load_transcript_segments(&meeting()).ok(),
        Some(segments())
    );
    assert_eq!(
        other_device
            .load_transcript(&meeting())
            .ok()
            .flatten()
            .map(|t| t.content().to_owned()),
        Some(String::from("Ciao Hello"))
    );
}

/// A corrupt document from a peer is rejected before any row is written, so the meeting does not
/// exist afterwards.
#[test]
fn a_corrupt_peer_document_creates_no_meeting() {
    let repository = device_repository(&scratch_database("corrupt.sqlite3"), "phone");

    let creation = repository.create_meeting(&meeting(), &[1]);

    assert!(creation.is_err());
    assert!(repository.load_crdt_document(&meeting()).is_err());
}

/// Every document write advances the meeting's revision, so the P2P task can find writes made by
/// any connection or process (the recording pipeline, the CLI, the app) and send them at once.
#[test]
fn document_writes_from_any_connection_are_listed_by_revision() {
    let database_path = scratch_database("revisions.sqlite3");
    let p2p_view = device_repository(&database_path, "device-laptop");
    let start = p2p_view
        .document_changes_since(0)
        .unwrap_or_else(|error| panic!("changes must list: {error}"));
    assert!(start.meeting_ids.is_empty());

    // The recording pipeline writes through its own connection to the same file.
    let pipeline = device_repository(&database_path, "device-laptop");
    record_meeting(&pipeline);
    let after_recording = p2p_view
        .document_changes_since(start.latest_revision)
        .unwrap_or_else(|error| panic!("changes must list: {error}"));
    let unchanged = p2p_view
        .document_changes_since(after_recording.latest_revision)
        .unwrap_or_else(|error| panic!("changes must list: {error}"));
    let other_meeting = MeetingId::new("meeting-milan");
    pipeline
        .create_meeting(&other_meeting, &AutoCommit::new().save())
        .unwrap_or_else(|error| panic!("meeting must be created: {error}"));
    let after_second_meeting = p2p_view
        .document_changes_since(after_recording.latest_revision)
        .unwrap_or_else(|error| panic!("changes must list: {error}"));

    assert_eq!(after_recording.meeting_ids, vec![meeting()]);
    assert!(after_recording.latest_revision > start.latest_revision);
    assert!(unchanged.meeting_ids.is_empty());
    assert_eq!(unchanged.latest_revision, after_recording.latest_revision);
    assert_eq!(after_second_meeting.meeting_ids, vec![other_meeting]);
}
