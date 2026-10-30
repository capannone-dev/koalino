//! Specifies how the SQLite repository exports and imports unknown-speaker cluster shares
//! (Issue #50): a share carries one unresolved cluster with its chunks and clips, the receiver
//! stores it sealed with its own key and can review it, imports are idempotent, and nothing is
//! imported for a missing meeting or a cluster already resolved. A resolution travels in the
//! meeting document: it resolves the cluster on the receiving device, deletes its clips there,
//! and keeps later shares of the cluster from bringing audio back.

use std::{path::PathBuf, sync::Arc};

use chrono::{TimeZone, Utc};
use rusqlite::{Connection, params};
use uuid::Uuid;

use super::sqlite_adapter::SqliteNoteRepository;
use crate::{
    application::{
        crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
        meeting_document::AutomergeMeetingDocumentCodec,
    },
    domain::{
        errors::StorageError,
        models::{
            ClusterDigest, ClusterShare, ClusterShareImport, DeviceId, MeetingId,
            UnassignedVoiceClip, UnknownSpeakerChunk, UnknownSpeakerCluster,
        },
    },
    ports::{meeting_document::MeetingDocumentCodec, storage::NoteRepository},
};

/// The meeting both devices know.
fn meeting_id() -> MeetingId {
    MeetingId::new("meeting-shared")
}

/// The cluster under test, and a second one in the same meeting.
const CLUSTER: Uuid = Uuid::from_u128(0xC1);
const OTHER_CLUSTER: Uuid = Uuid::from_u128(0xC2);

/// A database file in a fresh scratch directory, removed on drop.
struct Device {
    directory: PathBuf,
    repository: SqliteNoteRepository,
}

impl Device {
    /// Opens a repository sealed with `key_byte`, holding `meeting_id()`.
    fn new(key_byte: u8) -> Self {
        Self::open(key_byte, None)
    }

    /// Opens a repository that keeps meeting documents, holding `meeting_id()` created from
    /// `document` (a new, empty document when `None`).
    fn with_documents(key_byte: u8, document: Option<&[u8]>) -> Self {
        Self::open(
            key_byte,
            Some(document.map_or_else(|| automerge::AutoCommit::new().save(), <[u8]>::to_vec)),
        )
    }

    /// Opens a repository sealed with `key_byte`; with a document, it uses the document codec and
    /// creates `meeting_id()` from it.
    fn open(key_byte: u8, document: Option<Vec<u8>>) -> Self {
        let directory = std::env::temp_dir().join(format!("cluster-share-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory)
            .unwrap_or_else(|error| panic!("scratch directory must be created: {error}"));
        let repository = SqliteNoteRepository::open(
            directory.join("notes.sqlite3"),
            DeviceId::new("device"),
            Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes(
                [key_byte; 32],
            ))),
        )
        .and_then(|repository| match &document {
            Some(_) => repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec)),
            None => Ok(repository),
        })
        .unwrap_or_else(|error| panic!("repository must open: {error}"));
        repository
            .create_meeting(&meeting_id(), document.as_deref().unwrap_or(&[1]))
            .unwrap_or_else(|error| panic!("meeting must be created: {error}"));
        Self {
            directory,
            repository,
        }
    }

    /// Opens a second connection to inspect stored bytes.
    fn raw_connection(&self) -> Connection {
        Connection::open(self.directory.join("notes.sqlite3"))
            .unwrap_or_else(|error| panic!("raw connection must open: {error}"))
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// A cluster value with the given members.
fn cluster(cluster_id: Uuid, chunk_ids: &[&str]) -> UnknownSpeakerCluster {
    UnknownSpeakerCluster::new(
        cluster_id,
        meeting_id(),
        vec![0.5, 0.25, -0.25],
        chunk_ids
            .iter()
            .map(|chunk_id| (*chunk_id).to_owned())
            .collect(),
        1_000 * u64::try_from(chunk_ids.len()).unwrap_or_default(),
        Utc.timestamp_millis_opt(1_700_000_000_000)
            .single()
            .unwrap_or_else(|| panic!("valid timestamp")),
        None,
    )
}

/// A member chunk.
fn chunk(chunk_id: &str) -> UnknownSpeakerChunk {
    UnknownSpeakerChunk::new(chunk_id, vec![0.5, 0.25, -0.25], 1_000)
}

/// A review clip of `cluster_id` whose audio is `audio_byte` repeated.
fn clip(clip_number: u128, cluster_id: Uuid, audio_byte: u8) -> UnassignedVoiceClip {
    UnassignedVoiceClip::with_review_context(
        Uuid::from_u128(0x100 + clip_number),
        meeting_id(),
        0,
        900,
        vec![audio_byte; 48],
        vec![0.5, 0.25, -0.25],
        None,
    )
    .with_cluster_id(Some(cluster_id))
}

/// Records `CLUSTER` with chunks a and b and clips 1 and 2, and `OTHER_CLUSTER` with one chunk
/// and clip 3, on `device`.
fn record_clusters(device: &Device) {
    let repository = &device.repository;
    repository
        .save_unknown_speaker_cluster_chunk(&cluster(CLUSTER, &["chunk-a"]), &chunk("chunk-a"))
        .and_then(|()| {
            repository.save_unknown_speaker_cluster_chunk(
                &cluster(CLUSTER, &["chunk-a", "chunk-b"]),
                &chunk("chunk-b"),
            )
        })
        .and_then(|()| {
            repository.save_unknown_speaker_cluster_chunk(
                &cluster(OTHER_CLUSTER, &["chunk-c"]),
                &chunk("chunk-c"),
            )
        })
        .and_then(|()| repository.save_unassigned_clip(clip(1, CLUSTER, 0xAB)))
        .and_then(|()| repository.save_unassigned_clip(clip(2, CLUSTER, 0xCD)))
        .and_then(|()| repository.save_unassigned_clip(clip(3, OTHER_CLUSTER, 0xEF)))
        .unwrap_or_else(|error| panic!("clusters must be recorded: {error}"));
}

/// Loads `CLUSTER`'s share from `device`.
fn load_share(device: &Device) -> ClusterShare {
    device
        .repository
        .load_cluster_share(CLUSTER)
        .unwrap_or_else(|error| panic!("share must load: {error}"))
        .unwrap_or_else(|| panic!("an unresolved cluster must have a share"))
}

/// Imports `share` into `device`.
fn import(device: &Device, share: &ClusterShare) -> ClusterShareImport {
    device
        .repository
        .import_cluster_share(share)
        .unwrap_or_else(|error| panic!("share must import: {error}"))
}

/// Lists the review clips of `CLUSTER` on `device`.
fn cluster_clips(device: &Device) -> Vec<UnassignedVoiceClip> {
    device
        .repository
        .get_unassigned_clips(&meeting_id())
        .unwrap_or_else(|error| panic!("clips must load: {error}"))
        .into_iter()
        .filter(|clip| clip.cluster_id() == Some(CLUSTER))
        .collect()
}

/// The share holds the cluster, both chunks and only that cluster's clips; the digests count
/// what each unresolved cluster holds.
#[test]
fn a_share_carries_one_cluster_with_its_chunks_and_clips() {
    let sender = Device::new(1);
    record_clusters(&sender);

    let share = load_share(&sender);
    let digests = sender
        .repository
        .unresolved_cluster_digests()
        .unwrap_or_else(|error| panic!("digests must list: {error}"));

    assert_eq!(share.cluster().cluster_id(), CLUSTER);
    assert_eq!(share.chunks(), &[chunk("chunk-a"), chunk("chunk-b")]);
    assert_eq!(
        share.clips(),
        &[clip(1, CLUSTER, 0xAB), clip(2, CLUSTER, 0xCD)]
    );
    assert_eq!(
        digests,
        vec![
            ClusterDigest::new(CLUSTER, 2, 2),
            ClusterDigest::new(OTHER_CLUSTER, 1, 1),
        ]
    );
}

/// The receiver can review the imported cluster and play its clips, and stores the audio and
/// embeddings sealed with its own key.
#[test]
fn an_imported_share_is_reviewable_and_sealed_with_the_receivers_key() {
    let sender = Device::new(1);
    let receiver = Device::new(2);
    record_clusters(&sender);

    let outcome = import(&receiver, &load_share(&sender));

    assert_eq!(outcome, ClusterShareImport::Imported);
    let clusters = receiver
        .repository
        .load_unknown_speaker_clusters(&meeting_id())
        .unwrap_or_else(|error| panic!("clusters must load: {error}"));
    assert_eq!(clusters, vec![cluster(CLUSTER, &["chunk-a", "chunk-b"])]);
    assert_eq!(
        receiver
            .repository
            .load_unknown_speaker_chunks(CLUSTER)
            .unwrap_or_else(|error| panic!("chunks must load: {error}")),
        vec![chunk("chunk-a"), chunk("chunk-b")]
    );
    assert_eq!(
        cluster_clips(&receiver),
        vec![clip(1, CLUSTER, 0xAB), clip(2, CLUSTER, 0xCD)]
    );
    let stored_audio: Vec<(i64, Vec<u8>)> = receiver
        .raw_connection()
        .prepare("SELECT audio_format, audio_bytes FROM unassigned_voice_clips ORDER BY rowid")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .and_then(Iterator::collect)
        })
        .unwrap_or_else(|error| panic!("stored clips must read: {error}"));
    assert!(
        stored_audio
            .iter()
            .all(|(format, bytes)| *format == 1 && bytes != &vec![0xAB; 48])
    );
}

/// Importing the same share twice adds nothing, and a later share with more clips adds only the
/// new ones.
#[test]
fn imports_are_idempotent_and_add_only_what_is_new() {
    let sender = Device::new(1);
    let receiver = Device::new(2);
    record_clusters(&sender);
    let full_share = load_share(&sender);
    let first_clip_only = ClusterShare::new(
        full_share.cluster().clone(),
        full_share.chunks().to_vec(),
        full_share.clips()[..1].to_vec(),
    );

    import(&receiver, &first_clip_only);
    import(&receiver, &first_clip_only);
    assert_eq!(cluster_clips(&receiver).len(), 1);
    import(&receiver, &full_share);
    import(&receiver, &full_share);

    assert_eq!(cluster_clips(&receiver).len(), 2);
    assert_eq!(
        receiver
            .repository
            .unresolved_cluster_digests()
            .unwrap_or_else(|error| panic!("digests must list: {error}")),
        vec![ClusterDigest::new(CLUSTER, 2, 2)]
    );
}

/// A share for a meeting the receiver does not have yet is reported and stores nothing, so it
/// can be retried once the meeting syncs.
#[test]
fn a_share_for_a_missing_meeting_is_deferred() {
    let sender = Device::new(1);
    record_clusters(&sender);
    let share = load_share(&sender);
    let receiver = Device::new(2);
    receiver
        .raw_connection()
        .execute(
            "DELETE FROM meetings WHERE meeting_id = ?1",
            params![meeting_id().as_str()],
        )
        .unwrap_or_else(|error| panic!("meeting must be removed: {error}"));

    assert_eq!(
        import(&receiver, &share),
        ClusterShareImport::UnknownMeeting
    );
    assert!(
        receiver
            .repository
            .unresolved_cluster_digests()
            .unwrap_or_else(|error| panic!("digests must list: {error}"))
            .is_empty()
    );
}

/// Once the cluster is resolved on the receiver, a late share brings no audio back, and a
/// resolved cluster is neither shared nor listed.
#[test]
fn a_resolved_cluster_takes_no_clips_and_is_not_shared() {
    let sender = Device::new(1);
    let receiver = Device::new(2);
    record_clusters(&sender);
    let share = load_share(&sender);
    import(&receiver, &share);
    receiver
        .repository
        .resolve_unknown_speaker_cluster(CLUSTER, Uuid::from_u128(0x1D))
        .unwrap_or_else(|error| panic!("cluster must resolve: {error}"));

    assert_eq!(
        import(&receiver, &share),
        ClusterShareImport::AlreadyResolved
    );
    assert!(cluster_clips(&receiver).is_empty());
    assert_eq!(
        receiver
            .repository
            .load_cluster_share(CLUSTER)
            .unwrap_or_else(|error| panic!("share lookup must answer: {error}")),
        None
    );
    assert!(
        receiver
            .repository
            .unresolved_cluster_digests()
            .unwrap_or_else(|error| panic!("digests must list: {error}"))
            .is_empty()
    );
}

/// A share naming a cluster the receiver keeps under another meeting is refused.
#[test]
fn a_share_moving_a_cluster_to_another_meeting_is_refused() {
    let sender = Device::new(1);
    let receiver = Device::new(2);
    record_clusters(&sender);
    let share = load_share(&sender);
    let other_meeting = MeetingId::new("meeting-other");
    receiver
        .repository
        .create_meeting(&other_meeting, &[1])
        .and_then(|()| {
            receiver.repository.save_unknown_speaker_cluster_chunk(
                &UnknownSpeakerCluster::new(
                    CLUSTER,
                    other_meeting.clone(),
                    vec![1.0],
                    vec![String::from("chunk-z")],
                    1_000,
                    Utc::now(),
                    None,
                ),
                &UnknownSpeakerChunk::new("chunk-z", vec![1.0], 1_000),
            )
        })
        .unwrap_or_else(|error| panic!("conflicting cluster must be recorded: {error}"));

    assert!(matches!(
        receiver.repository.import_cluster_share(&share),
        Err(StorageError::Rejected { .. })
    ));
}

/// The meeting document `device` holds.
fn document(device: &Device) -> Vec<u8> {
    device
        .repository
        .load_crdt_document(&meeting_id())
        .unwrap_or_else(|error| panic!("document must load: {error}"))
}

/// The identity the owner names the cluster as.
const IDENTITY: Uuid = Uuid::from_u128(0x1D);

/// Resolving a cluster records the resolution in its meeting's document, so it syncs.
#[test]
fn resolving_a_cluster_records_it_in_the_meeting_document() {
    let device = Device::with_documents(1, None);
    record_clusters(&device);

    device
        .repository
        .resolve_unknown_speaker_cluster(CLUSTER, IDENTITY)
        .unwrap_or_else(|error| panic!("cluster must resolve: {error}"));
    let projection = AutomergeMeetingDocumentCodec
        .project(&meeting_id(), &document(&device))
        .unwrap_or_else(|error| panic!("document must project: {error}"));

    assert_eq!(projection.cluster_resolutions, vec![(CLUSTER, IDENTITY)]);
    assert!(cluster_clips(&device).is_empty());
}

/// A resolution arriving in the document from a paired device resolves the copied cluster and
/// deletes its copied clips here; the other cluster is untouched.
#[test]
fn a_resolution_from_a_paired_device_resolves_the_cluster_here() {
    let recorder = Device::with_documents(1, None);
    record_clusters(&recorder);
    let receiver = Device::with_documents(2, Some(&document(&recorder)));
    import(&receiver, &load_share(&recorder));
    let other_share = recorder
        .repository
        .load_cluster_share(OTHER_CLUSTER)
        .ok()
        .flatten()
        .unwrap_or_else(|| panic!("the other cluster must have a share"));
    import(&receiver, &other_share);
    assert_eq!(cluster_clips(&receiver).len(), 2);

    recorder
        .repository
        .resolve_unknown_speaker_cluster(CLUSTER, IDENTITY)
        .and_then(|()| {
            receiver
                .repository
                .save_crdt_document(&meeting_id(), &document(&recorder))
        })
        .unwrap_or_else(|error| panic!("resolution must sync: {error}"));

    assert!(cluster_clips(&receiver).is_empty());
    assert_eq!(
        receiver
            .repository
            .load_unknown_speaker_cluster(CLUSTER)
            .unwrap_or_else(|error| panic!("cluster must load: {error}"))
            .and_then(|cluster| cluster.resolved_identity_id()),
        Some(IDENTITY)
    );
    assert_eq!(
        receiver
            .repository
            .unresolved_cluster_digests()
            .unwrap_or_else(|error| panic!("digests must list: {error}"))
            .iter()
            .map(ClusterDigest::cluster_id)
            .collect::<Vec<_>>(),
        vec![OTHER_CLUSTER]
    );
}

/// A share that arrives after the resolution reached this device stores nothing.
#[test]
fn a_share_arriving_after_its_resolution_brings_nothing_back() {
    let recorder = Device::with_documents(1, None);
    record_clusters(&recorder);
    let share = load_share(&recorder);
    recorder
        .repository
        .resolve_unknown_speaker_cluster(CLUSTER, IDENTITY)
        .unwrap_or_else(|error| panic!("cluster must resolve: {error}"));
    let receiver = Device::with_documents(2, Some(&document(&recorder)));

    assert_eq!(
        import(&receiver, &share),
        ClusterShareImport::AlreadyResolved
    );
    assert!(cluster_clips(&receiver).is_empty());
    assert_eq!(
        receiver
            .repository
            .load_unknown_speaker_cluster(CLUSTER)
            .unwrap_or_else(|error| panic!("cluster lookup must answer: {error}")),
        None
    );
}
