//! Specifies the cluster-review use case shared by the CLI, HTTP, and FFI driving adapters.
//!
//! Tests use in-memory SQLite plus the in-memory or file-backed SQLite voice store; no AI model is
//! loaded and embeddings are short hand-written vectors.

use uuid::Uuid;

use crate::{
    adapters::storage::{
        in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
        sqlite_voice_vector_db::SqliteVoiceVectorDB,
    },
    application::{
        cluster_review::{
            ClusterReviewError, assign_cluster_to_named_identity, list_unknown_clusters,
        },
        unknown_speaker_clusterer::{ClusteringConfig, ClusteringError, UnknownSpeakerClusterer},
    },
    domain::models::{DeviceId, MeetingId, SpeakerIdentity, TranscriptSegment, VoiceEmbedding},
    ports::{storage::NoteRepository, voice::VoiceVectorDB},
};

/// Creates an in-memory repository with one meeting and two unknown-speaker clusters.
///
/// Outputs: The repository, the meeting, the two-chunk cluster ID, and the one-chunk cluster ID.
fn repository_with_two_clusters() -> (SqliteNoteRepository, MeetingId, Uuid, Uuid) {
    let meeting_id = MeetingId::new("meeting-review");
    let note_repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite should open: {error}"));
    note_repository
        .create_meeting(&meeting_id, &[])
        .unwrap_or_else(|error| panic!("meeting should be created: {error}"));
    let clusterer = UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default());
    let add_chunk = |embedding: &[f32], chunk_id: &str, duration_ms: u64| {
        clusterer
            .find_or_create_cluster(&meeting_id, embedding, chunk_id.to_owned(), duration_ms)
            .unwrap_or_else(|error| panic!("chunk {chunk_id} should be clustered: {error}"))
            .cluster_id()
    };
    let large_cluster_id = add_chunk(&[1.0, 0.0], "c1", 1_500);
    add_chunk(&[0.9, 0.3], "c2", 2_000);
    let small_cluster_id = add_chunk(&[0.0, 1.0], "c3", 700);
    (
        note_repository,
        meeting_id,
        large_cluster_id,
        small_cluster_id,
    )
}

/// Verifies listing returns display-safe summaries in creation order.
#[test]
fn lists_unresolved_cluster_summaries_for_a_meeting() {
    let (note_repository, meeting_id, large_cluster_id, small_cluster_id) =
        repository_with_two_clusters();

    let summaries = list_unknown_clusters(&note_repository, &meeting_id)
        .unwrap_or_else(|error| panic!("clusters should list: {error}"));

    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].cluster_id(), large_cluster_id);
    assert_eq!(summaries[0].meeting_id(), &meeting_id);
    assert_eq!(summaries[0].total_duration_ms(), 3_500);
    assert_eq!(summaries[0].chunk_count(), 2);
    assert_eq!(summaries[1].cluster_id(), small_cluster_id);
    assert_eq!(summaries[1].total_duration_ms(), 700);
    assert_eq!(summaries[1].chunk_count(), 1);
}

/// Verifies a meeting without clusters (or an unknown meeting) lists nothing.
#[test]
fn lists_no_clusters_for_an_unknown_meeting() {
    let (note_repository, _, _, _) = repository_with_two_clusters();

    let summaries = list_unknown_clusters(&note_repository, &MeetingId::new("meeting-other"))
        .unwrap_or_else(|error| panic!("clusters should list: {error}"));

    assert!(summaries.is_empty());
}

/// Verifies a new name creates a fresh identity, relinks segments, and hides the cluster.
#[test]
fn assigning_a_new_name_creates_an_identity_and_resolves_the_cluster() {
    let (note_repository, meeting_id, large_cluster_id, small_cluster_id) =
        repository_with_two_clusters();
    note_repository
        .save_transcript_segments(
            &meeting_id,
            &[
                TranscriptSegment::new(meeting_id.clone(), 0, 1_500, None, "hello")
                    .with_unknown_cluster_id(Some(large_cluster_id)),
            ],
        )
        .unwrap_or_else(|error| panic!("segments should save: {error}"));
    let voice_vector_database = InMemoryVoiceVectorDB::new();

    let assignment = assign_cluster_to_named_identity(
        &note_repository,
        &voice_vector_database,
        large_cluster_id,
        "  Grace Hopper ",
    )
    .unwrap_or_else(|error| panic!("assignment should succeed: {error}"));

    assert_eq!(assignment.cluster_id(), large_cluster_id);
    assert_eq!(assignment.identity_name(), "Grace Hopper");
    assert!(assignment.identity_created());
    assert_eq!(assignment.variants_stored(), 2);
    assert_eq!(assignment.segments_updated(), 1);
    assert_eq!(
        voice_vector_database
            .voice_variant_count(assignment.identity_id())
            .unwrap_or_default(),
        2
    );
    let remaining = list_unknown_clusters(&note_repository, &meeting_id)
        .unwrap_or_else(|error| panic!("clusters should list: {error}"));
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].cluster_id(), small_cluster_id);
}

/// Verifies a name matching a known identity (ignoring case) reuses that identity.
#[test]
fn assigning_a_known_name_reuses_the_existing_identity() {
    let (note_repository, _, large_cluster_id, _) = repository_with_two_clusters();
    let existing_identity_id = Uuid::new_v4();
    let voice_vector_database = InMemoryVoiceVectorDB::new();
    voice_vector_database
        .save_speaker_identity(&SpeakerIdentity::new(existing_identity_id, "Ada Lovelace"))
        .unwrap_or_else(|error| panic!("existing identity should save: {error}"));
    voice_vector_database
        .store_embedding(VoiceEmbedding::new(
            existing_identity_id,
            Uuid::new_v4(),
            vec![1.0, 0.0],
        ))
        .unwrap_or_else(|error| panic!("existing variant should store: {error}"));

    let assignment = assign_cluster_to_named_identity(
        &note_repository,
        &voice_vector_database,
        large_cluster_id,
        "ada lovelace",
    )
    .unwrap_or_else(|error| panic!("assignment should succeed: {error}"));

    assert_eq!(assignment.identity_id(), existing_identity_id);
    assert_eq!(assignment.identity_name(), "Ada Lovelace");
    assert!(!assignment.identity_created());
    assert_eq!(
        voice_vector_database
            .list_speaker_identities()
            .map(|identities| identities.len())
            .ok(),
        Some(1),
        "reusing a name must not create a duplicate identity"
    );
}

/// Verifies blank names are rejected before anything is written.
#[test]
fn blank_identity_names_are_rejected() {
    let (note_repository, meeting_id, large_cluster_id, _) = repository_with_two_clusters();
    let voice_vector_database = InMemoryVoiceVectorDB::new();

    let outcome = assign_cluster_to_named_identity(
        &note_repository,
        &voice_vector_database,
        large_cluster_id,
        "   ",
    );

    assert!(matches!(
        outcome,
        Err(ClusterReviewError::BlankIdentityName)
    ));
    assert_eq!(
        list_unknown_clusters(&note_repository, &meeting_id)
            .unwrap_or_else(|error| panic!("clusters should list: {error}"))
            .len(),
        2
    );
}

/// Verifies unknown and already-resolved clusters surface the clusterer's errors.
#[test]
fn unknown_and_resolved_clusters_are_reported() {
    let (note_repository, _, large_cluster_id, _) = repository_with_two_clusters();
    let voice_vector_database = InMemoryVoiceVectorDB::new();
    let missing_cluster_id = Uuid::new_v4();

    let missing = assign_cluster_to_named_identity(
        &note_repository,
        &voice_vector_database,
        missing_cluster_id,
        "Grace",
    );
    assert!(matches!(
        missing,
        Err(ClusterReviewError::Clustering(ClusteringError::ClusterNotFound { cluster_id }))
            if cluster_id == missing_cluster_id
    ));

    assign_cluster_to_named_identity(
        &note_repository,
        &voice_vector_database,
        large_cluster_id,
        "Grace",
    )
    .unwrap_or_else(|error| panic!("first assignment should succeed: {error}"));
    let repeated = assign_cluster_to_named_identity(
        &note_repository,
        &voice_vector_database,
        large_cluster_id,
        "Grace",
    );
    assert!(matches!(
        repeated,
        Err(ClusterReviewError::Clustering(
            ClusteringError::ClusterAlreadyResolved { .. }
        ))
    ));
}

/// Verifies the use case accepts unsized port objects, as the HTTP adapter holds them.
#[test]
fn accepts_trait_object_ports() {
    let (note_repository, meeting_id, large_cluster_id, _) = repository_with_two_clusters();
    let voice_vector_database = InMemoryVoiceVectorDB::new();
    let repository_object: &dyn NoteRepository = &note_repository;
    let voice_object: &dyn VoiceVectorDB = &voice_vector_database;

    assert_eq!(
        list_unknown_clusters(repository_object, &meeting_id)
            .unwrap_or_else(|error| panic!("clusters should list: {error}"))
            .len(),
        2
    );
    assign_cluster_to_named_identity(repository_object, voice_object, large_cluster_id, "Grace")
        .unwrap_or_else(|error| panic!("assignment should succeed: {error}"));
}

/// Assign cluster with name → restart app → the name is still there, and assigning another
/// cluster with the same name in any letter case reuses that identity without a duplicate.
#[test]
fn assigned_names_survive_a_restart_and_are_reused_case_insensitively() {
    let database_path = std::env::temp_dir().join(format!(
        "note-taker-cluster-review-{}.sqlite3",
        Uuid::new_v4()
    ));
    let meeting_id = MeetingId::new("meeting-restart");
    let database_cipher = crate::application::crypto::voice_encryptor::ephemeral_cipher();
    let open_stores = || {
        (
            SqliteNoteRepository::open(
                &database_path,
                DeviceId::new("cluster-review-test"),
                std::sync::Arc::clone(&database_cipher),
            )
            .unwrap_or_else(|error| panic!("note repository should open: {error}")),
            SqliteVoiceVectorDB::open(&database_path, std::sync::Arc::clone(&database_cipher))
                .unwrap_or_else(|error| panic!("voice store should open: {error}")),
        )
    };
    let (first_cluster_id, second_cluster_id, first_assignment) = {
        let (note_repository, voice_store) = open_stores();
        note_repository
            .create_meeting(&meeting_id, &[])
            .unwrap_or_else(|error| panic!("meeting should be created: {error}"));
        let clusterer = UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default());
        let first_cluster_id = clusterer
            .find_or_create_cluster(&meeting_id, &[1.0, 0.0], String::from("c1"), 1_000)
            .unwrap_or_else(|error| panic!("chunk should cluster: {error}"))
            .cluster_id();
        let second_cluster_id = clusterer
            .find_or_create_cluster(&meeting_id, &[0.0, 1.0], String::from("c2"), 1_000)
            .unwrap_or_else(|error| panic!("chunk should cluster: {error}"))
            .cluster_id();
        let first_assignment = assign_cluster_to_named_identity(
            &note_repository,
            &voice_store,
            first_cluster_id,
            "Grace Hopper",
        )
        .unwrap_or_else(|error| panic!("first assignment should succeed: {error}"));
        (first_cluster_id, second_cluster_id, first_assignment)
    };

    let (note_repository, voice_store) = open_stores();
    let names_after_restart: Vec<String> = voice_store
        .list_speaker_identities()
        .unwrap_or_default()
        .iter()
        .map(|identity| String::from(identity.display_name()))
        .collect();
    let second_assignment = assign_cluster_to_named_identity(
        &note_repository,
        &voice_store,
        second_cluster_id,
        "grace HOPPER",
    );
    let identity_count = voice_store
        .list_speaker_identities()
        .map(|identities| identities.len())
        .unwrap_or_default();
    let variant_count = voice_store
        .voice_variant_count(first_assignment.identity_id())
        .unwrap_or_default();
    let first_cluster = note_repository
        .load_unknown_speaker_cluster(first_cluster_id)
        .unwrap_or_default();
    drop((note_repository, voice_store));
    for suffix in ["", "-journal", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", database_path.display()));
    }

    assert!(first_assignment.identity_created());
    assert_eq!(names_after_restart, vec!["Grace Hopper"]);
    let second_assignment = second_assignment
        .unwrap_or_else(|error| panic!("second assignment should succeed: {error}"));
    assert_eq!(
        second_assignment.identity_id(),
        first_assignment.identity_id()
    );
    assert_eq!(second_assignment.identity_name(), "Grace Hopper");
    assert!(!second_assignment.identity_created());
    assert_eq!(identity_count, 1, "no duplicate identity may be created");
    assert_eq!(variant_count, 2);
    assert_eq!(
        first_cluster.and_then(|cluster| cluster.resolved_identity_id()),
        Some(first_assignment.identity_id())
    );
}
