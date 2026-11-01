//! Specifies the SQLite voice store: persisted speaker names, voice variants, and migration safety.
//!
//! File-backed tests use unique paths under the OS temp directory and delete them afterwards; no
//! AI model is loaded and embeddings are short hand-written vectors.

use std::{path::PathBuf, sync::Arc};

use uuid::Uuid;

use crate::{
    adapters::storage::{
        sqlite_adapter::SqliteNoteRepository, sqlite_voice_vector_db::SqliteVoiceVectorDB,
    },
    application::{
        crypto::voice_encryptor::ephemeral_cipher,
        unknown_speaker_clusterer::{ClusteringConfig, UnknownSpeakerClusterer},
    },
    domain::models::{DeviceId, MeetingId, SpeakerIdentity, TranscriptSegment, VoiceEmbedding},
    ports::{crypto::EmbeddingCipher, storage::NoteRepository, voice::VoiceVectorDB},
};

/// Deletes a temporary SQLite file (and its journal files) when dropped.
struct TemporaryDatabase {
    path: PathBuf,
    /// One key for every open of this file, as a real installation has.
    cipher: Arc<dyn EmbeddingCipher>,
}

impl TemporaryDatabase {
    /// Reserves a unique database path; the file is created by the first adapter that opens it.
    fn new() -> Self {
        Self {
            path: std::env::temp_dir().join(format!("note-taker-voice-{}.sqlite3", Uuid::new_v4())),
            cipher: ephemeral_cipher(),
        }
    }

    /// Opens the voice store on this file.
    fn open_voice_store(&self) -> SqliteVoiceVectorDB {
        SqliteVoiceVectorDB::open(&self.path, Arc::clone(&self.cipher))
            .unwrap_or_else(|error| panic!("voice store should open: {error}"))
    }

    /// Opens the note repository on this file.
    fn open_note_repository(&self) -> SqliteNoteRepository {
        SqliteNoteRepository::open(
            &self.path,
            DeviceId::new("voice-store-test"),
            Arc::clone(&self.cipher),
        )
        .unwrap_or_else(|error| panic!("note repository should open: {error}"))
    }
}

impl Drop for TemporaryDatabase {
    fn drop(&mut self) {
        for suffix in ["", "-journal", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", self.path.display()));
        }
    }
}

/// Opens an isolated in-memory voice store.
fn in_memory_store() -> SqliteVoiceVectorDB {
    SqliteVoiceVectorDB::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory voice store should open: {error}"))
}

/// Saves `identity`, failing the test on error.
fn save(store: &SqliteVoiceVectorDB, identity: &SpeakerIdentity) {
    store
        .save_speaker_identity(identity)
        .unwrap_or_else(|error| panic!("identity should save: {error}"));
}

/// Stores one variant for `identity_id`, failing the test on error.
fn store_variant(store: &SqliteVoiceVectorDB, identity_id: Uuid, vector: Vec<f32>) -> Uuid {
    let variant_id = Uuid::new_v4();
    store
        .store_embedding(VoiceEmbedding::new(identity_id, variant_id, vector))
        .unwrap_or_else(|error| panic!("variant should store: {error}"));
    variant_id
}

/// Lists display names in storage order, failing the test on error.
fn listed_names(store: &SqliteVoiceVectorDB) -> Vec<String> {
    store
        .list_speaker_identities()
        .unwrap_or_else(|error| panic!("identities should list: {error}"))
        .iter()
        .map(|identity| String::from(identity.display_name()))
        .collect()
}

/// Save identity → retrieve by ID returns the same name.
#[test]
fn saved_identity_is_returned_by_id() {
    let store = in_memory_store();
    let identity = SpeakerIdentity::new(Uuid::new_v4(), "Grace Hopper");

    save(&store, &identity);

    assert_eq!(
        store
            .load_speaker_identity(identity.identity_id())
            .unwrap_or_else(|error| panic!("identity should load: {error}")),
        Some(identity)
    );
    assert_eq!(
        store
            .load_speaker_identity(Uuid::new_v4())
            .unwrap_or_else(|error| panic!("missing identity should load: {error}")),
        None
    );
}

/// List identities → returns every saved name in creation order.
#[test]
fn lists_every_saved_identity_in_creation_order() {
    let store = in_memory_store();
    for name in ["Ada Lovelace", "Grace Hopper", "Alan Turing"] {
        save(&store, &SpeakerIdentity::new(Uuid::new_v4(), name));
    }

    assert_eq!(
        listed_names(&store),
        vec!["Ada Lovelace", "Grace Hopper", "Alan Turing"]
    );
}

/// Saving an existing ID renames it in place instead of duplicating it.
#[test]
fn saving_an_existing_identity_renames_it() {
    let store = in_memory_store();
    let identity_id = Uuid::new_v4();
    save(&store, &SpeakerIdentity::new(identity_id, "Grace"));

    save(&store, &SpeakerIdentity::new(identity_id, "Grace Hopper"));

    assert_eq!(listed_names(&store), vec!["Grace Hopper"]);
}

/// Variants persist per identity and take part in similarity search.
#[test]
fn stores_counts_and_searches_variants_of_saved_identities() {
    let store = in_memory_store();
    let grace = SpeakerIdentity::new(Uuid::new_v4(), "Grace");
    let ada = SpeakerIdentity::new(Uuid::new_v4(), "Ada");
    save(&store, &grace);
    save(&store, &ada);
    store_variant(&store, grace.identity_id(), vec![1.0, 0.0]);
    store_variant(&store, grace.identity_id(), vec![0.9, 0.1]);
    let ada_variant_id = store_variant(&store, ada.identity_id(), vec![0.0, 1.0]);

    assert_eq!(store.voice_variant_count(grace.identity_id()).ok(), Some(2));
    assert_eq!(store.voice_variant_count(ada.identity_id()).ok(), Some(1));
    let results = store
        .search_similar(&[0.0, 1.0], 2)
        .unwrap_or_else(|error| panic!("search should succeed: {error}"));
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0.variant_id(), ada_variant_id);
    assert_eq!(results[0].0.identity_id(), ada.identity_id());
    assert_eq!(results[0].0.vector(), &[0.0, 1.0]);
    assert!((results[0].1 - 1.0).abs() < 1e-6);
    assert!(results[0].1 >= results[1].1);
}

/// A variant for an identity that was never saved is rejected (foreign-key integrity).
#[test]
fn rejects_variants_for_unknown_identities() {
    let store = in_memory_store();

    let outcome = store.store_embedding(VoiceEmbedding::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        vec![1.0, 0.0],
    ));

    assert!(outcome.is_err());
    assert!(
        store
            .search_similar(&[1.0, 0.0], 5)
            .unwrap_or_default()
            .is_empty()
    );
}

/// Empty and non-finite embeddings are rejected before storage.
#[test]
fn rejects_invalid_embedding_vectors() {
    let store = in_memory_store();
    let identity = SpeakerIdentity::new(Uuid::new_v4(), "Grace");
    save(&store, &identity);

    for invalid_vector in [Vec::new(), vec![f32::NAN, 1.0]] {
        assert!(
            store
                .store_embedding(VoiceEmbedding::new(
                    identity.identity_id(),
                    Uuid::new_v4(),
                    invalid_vector,
                ))
                .is_err()
        );
    }
}

/// Delete identity → its name and variants are gone.
#[test]
fn deleting_an_identity_removes_its_name_and_variants() {
    let store = in_memory_store();
    let grace = SpeakerIdentity::new(Uuid::new_v4(), "Grace");
    let ada = SpeakerIdentity::new(Uuid::new_v4(), "Ada");
    save(&store, &grace);
    save(&store, &ada);
    store_variant(&store, grace.identity_id(), vec![1.0, 0.0]);

    store
        .delete_identity(grace.identity_id())
        .unwrap_or_else(|error| panic!("identity should delete: {error}"));

    assert_eq!(listed_names(&store), vec!["Ada"]);
    assert_eq!(store.voice_variant_count(grace.identity_id()).ok(), Some(0));
    assert!(
        store
            .search_similar(&[1.0, 0.0], 5)
            .unwrap_or_default()
            .is_empty()
    );
    assert!(
        store.delete_identity(Uuid::new_v4()).is_ok(),
        "deleting an unknown identity is a no-op"
    );
}

/// Names and variants survive closing and reopening the database file (an app restart).
#[test]
fn identities_and_variants_survive_reopening_the_file() {
    let database = TemporaryDatabase::new();
    let identity = SpeakerIdentity::new(Uuid::new_v4(), "Grace Hopper");
    {
        let store = database.open_voice_store();
        save(&store, &identity);
        store_variant(&store, identity.identity_id(), vec![1.0, 0.0]);
    }

    let reopened = database.open_voice_store();

    assert_eq!(listed_names(&reopened), vec!["Grace Hopper"]);
    assert_eq!(
        reopened.voice_variant_count(identity.identity_id()).ok(),
        Some(1)
    );
}

/// A database created before Issue #30 gains the voice tables without losing any existing data,
/// and reopening it again is harmless.
#[test]
fn pre_existing_database_migrates_without_data_loss() {
    let database = TemporaryDatabase::new();
    let meeting_id = MeetingId::new("legacy-meeting");
    let cluster_id = {
        let note_repository = database.open_note_repository();
        note_repository
            .create_meeting(&meeting_id, &[1, 2, 3])
            .unwrap_or_else(|error| panic!("legacy meeting should save: {error}"));
        let cluster_id =
            UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default())
                .find_or_create_cluster(&meeting_id, &[1.0, 0.0], String::from("c1"), 1_000)
                .unwrap_or_else(|error| panic!("legacy cluster should save: {error}"))
                .cluster_id();
        note_repository
            .save_transcript_segments(
                &meeting_id,
                &[
                    TranscriptSegment::new(meeting_id.clone(), 0, 1_000, None, "legacy text")
                        .with_unknown_cluster_id(Some(cluster_id)),
                ],
            )
            .unwrap_or_else(|error| panic!("legacy segments should save: {error}"));
        cluster_id
    };
    let identity = SpeakerIdentity::new(Uuid::new_v4(), "Grace");

    {
        let store = database.open_voice_store();
        save(&store, &identity);
        store_variant(&store, identity.identity_id(), vec![1.0, 0.0]);
    }
    let store = database.open_voice_store();
    let note_repository = database.open_note_repository();

    assert_eq!(listed_names(&store), vec!["Grace"]);
    assert_eq!(
        note_repository
            .load_crdt_document(&meeting_id)
            .unwrap_or_default(),
        vec![1, 2, 3]
    );
    let segments = note_repository
        .load_transcript_segments(&meeting_id)
        .unwrap_or_else(|error| panic!("legacy segments should load: {error}"));
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].text(), "legacy text");
    assert_eq!(segments[0].unknown_cluster_id(), Some(cluster_id));
    assert!(
        note_repository
            .load_unknown_speaker_cluster(cluster_id)
            .unwrap_or_default()
            .is_some()
    );
}

/// The note repository and the voice store can share one database file concurrently, as the CLI,
/// FFI, and daemon compose them.
#[test]
fn note_repository_and_voice_store_share_one_file() {
    let database = TemporaryDatabase::new();
    let note_repository = database.open_note_repository();
    let store = database.open_voice_store();
    let identity = SpeakerIdentity::new(Uuid::new_v4(), "Grace");

    note_repository
        .create_meeting(&MeetingId::new("shared-meeting"), &[])
        .unwrap_or_else(|error| panic!("meeting should save: {error}"));
    save(&store, &identity);

    assert_eq!(
        note_repository.list_meeting_ids().unwrap_or_default().len(),
        1
    );
    assert_eq!(listed_names(&store), vec!["Grace"]);
}

/// A person linked to an external account can still be deleted, and the link goes with them.
#[test]
fn deleting_an_identity_removes_its_external_mappings() {
    let store = in_memory_store();
    use crate::ports::external_bridge::ExternalIdentityRepository;

    let identity_id = Uuid::new_v4();
    let identity = SpeakerIdentity::new(identity_id, "Ada");
    save(&store, &identity);
    store
        .save_external_mapping("teams:ada", &identity)
        .unwrap_or_else(|error| panic!("mapping must save: {error}"));

    store
        .delete_identity(identity_id)
        .unwrap_or_else(|error| panic!("deletion must succeed: {error}"));

    assert!(listed_names(&store).is_empty());
    assert!(
        store
            .find_by_external_id("teams:ada")
            .unwrap_or_else(|error| panic!("lookup must succeed: {error}"))
            .is_none()
    );
}
