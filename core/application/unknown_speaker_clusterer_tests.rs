//! Specifies unknown-speaker clustering and cluster identity assignment.
//!
//! Tests use in-memory SQLite and the in-memory vector database only; no AI model is loaded and
//! embeddings are short hand-written vectors whose cosine similarities are known in advance.

use chrono::Utc;
use uuid::Uuid;

use crate::{
    adapters::storage::{
        in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
    },
    application::unknown_speaker_clusterer::{
        ClusteringConfig, ClusteringError, DEFAULT_CLUSTER_SIMILARITY_THRESHOLD,
        UnknownSpeakerClusterer, cosine_similarity,
    },
    domain::{
        errors::StorageError,
        models::{
            MeetingId, Transcript, TranscriptSegment, UnassignedVoiceClip, UnknownSpeakerCluster,
        },
    },
    ports::{storage::NoteRepository, voice::VoiceVectorDB},
};

/// Creates an in-memory repository containing the given meetings.
fn repository_with_meetings(meeting_ids: &[&MeetingId]) -> SqliteNoteRepository {
    let note_repository = SqliteNoteRepository::open_in_memory()
        .unwrap_or_else(|error| panic!("in-memory SQLite should open: {error}"));
    for meeting_id in meeting_ids {
        note_repository
            .create_meeting(meeting_id, &[])
            .unwrap_or_else(|error| panic!("meeting should be created: {error}"));
    }
    note_repository
}

/// Adds one chunk through the clusterer, failing the test on error.
fn add_chunk(
    clusterer: &UnknownSpeakerClusterer<'_, SqliteNoteRepository>,
    meeting_id: &MeetingId,
    embedding: &[f32],
    chunk_id: &str,
    duration_ms: u64,
) -> (Uuid, bool) {
    let membership = clusterer
        .find_or_create_cluster(meeting_id, embedding, chunk_id.to_owned(), duration_ms)
        .unwrap_or_else(|error| panic!("chunk {chunk_id} should be clustered: {error}"));
    (membership.cluster_id(), membership.is_new_cluster())
}

/// Loads a meeting's clusters, failing the test on error.
fn clusters_of(
    clusterer: &UnknownSpeakerClusterer<'_, SqliteNoteRepository>,
    meeting_id: &MeetingId,
) -> Vec<UnknownSpeakerCluster> {
    clusterer
        .get_clusters_for_meeting(meeting_id)
        .unwrap_or_else(|error| panic!("clusters should load: {error}"))
}

/// Verifies the default threshold matches the documented 0.70 policy.
#[test]
fn default_threshold_is_seventy_percent() {
    assert!((DEFAULT_CLUSTER_SIMILARITY_THRESHOLD - 0.70).abs() < f32::EPSILON);
    assert!(
        (ClusteringConfig::default().similarity_threshold() - 0.70).abs() < f32::EPSILON,
        "default config should use the documented threshold"
    );
}

/// Verifies thresholds outside (0, 1] or non-finite are rejected.
#[test]
fn invalid_thresholds_are_rejected() {
    for invalid_threshold in [0.0, -0.2, 1.01, f32::NAN] {
        assert!(matches!(
            ClusteringConfig::new(invalid_threshold),
            Err(ClusteringError::InvalidThreshold { .. })
        ));
    }
    assert!(ClusteringConfig::new(1.0).is_ok());
}

/// Verifies cosine similarity on known vectors and its rejection of incomparable inputs.
#[test]
fn cosine_similarity_handles_known_and_degenerate_vectors() {
    let identical = cosine_similarity(&[1.0, 2.0], &[2.0, 4.0]).unwrap_or(f32::NAN);
    assert!((identical - 1.0).abs() < 1e-6);
    let orthogonal = cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]).unwrap_or(f32::NAN);
    assert!(orthogonal.abs() < 1e-6);
    assert_eq!(cosine_similarity(&[1.0, 0.0], &[1.0]), None);
    assert_eq!(cosine_similarity(&[0.0, 0.0], &[1.0, 0.0]), None);
    assert_eq!(cosine_similarity(&[], &[]), None);
}

/// Verifies the first unknown chunk of a meeting creates a new single-member cluster.
#[test]
fn first_unknown_chunk_creates_new_cluster() {
    let meeting_id = MeetingId::new("meeting-a");
    let note_repository = repository_with_meetings(&[&meeting_id]);
    let clusterer = UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default());

    let (cluster_id, is_new_cluster) = add_chunk(&clusterer, &meeting_id, &[1.0, 0.0], "c1", 1_500);

    assert!(is_new_cluster);
    let clusters = clusters_of(&clusterer, &meeting_id);
    assert_eq!(clusters.len(), 1);
    assert_eq!(clusters[0].cluster_id(), cluster_id);
    assert_eq!(clusters[0].meeting_id(), &meeting_id);
    assert_eq!(clusters[0].chunk_ids(), ["c1".to_owned()]);
    assert_eq!(clusters[0].total_duration_ms(), 1_500);
    assert_eq!(clusters[0].representative_embedding(), [1.0, 0.0]);
    assert!(!clusters[0].is_resolved());
}

/// Verifies a chunk whose cosine similarity exceeds 0.70 joins the existing cluster.
#[test]
fn similar_unknown_chunk_joins_existing_cluster() {
    let meeting_id = MeetingId::new("meeting-a");
    let note_repository = repository_with_meetings(&[&meeting_id]);
    let clusterer = UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default());

    let (first_cluster_id, _) = add_chunk(&clusterer, &meeting_id, &[1.0, 0.0], "c1", 1_000);
    // cos([1,0],[0.9,0.3]) ≈ 0.949 > 0.70
    let (second_cluster_id, is_new_cluster) =
        add_chunk(&clusterer, &meeting_id, &[0.9, 0.3], "c2", 2_000);

    assert!(!is_new_cluster);
    assert_eq!(second_cluster_id, first_cluster_id);
    let clusters = clusters_of(&clusterer, &meeting_id);
    assert_eq!(clusters.len(), 1);
    assert_eq!(clusters[0].chunk_ids(), ["c1".to_owned(), "c2".to_owned()]);
    assert_eq!(clusters[0].total_duration_ms(), 3_000);
}

/// Verifies a chunk whose cosine similarity is below 0.70 starts another cluster.
#[test]
fn different_unknown_chunk_creates_another_cluster() {
    let meeting_id = MeetingId::new("meeting-a");
    let note_repository = repository_with_meetings(&[&meeting_id]);
    let clusterer = UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default());

    let (first_cluster_id, _) = add_chunk(&clusterer, &meeting_id, &[1.0, 0.0], "c1", 1_000);
    add_chunk(&clusterer, &meeting_id, &[0.9, 0.3], "c2", 1_000);
    // cos([1,0],[0.5,0.8]) ≈ 0.53 < 0.70
    let (third_cluster_id, is_new_cluster) =
        add_chunk(&clusterer, &meeting_id, &[0.5, 0.8], "c3", 1_000);

    assert!(is_new_cluster);
    assert_ne!(third_cluster_id, first_cluster_id);
    let clusters = clusters_of(&clusterer, &meeting_id);
    assert_eq!(clusters.len(), 2);
    assert_eq!(clusters[1].chunk_ids(), ["c3".to_owned()]);
}

/// Verifies a similarity exactly at the threshold does not join (the rule is strictly greater).
#[test]
fn similarity_equal_to_threshold_creates_new_cluster() {
    let meeting_id = MeetingId::new("meeting-a");
    let note_repository = repository_with_meetings(&[&meeting_id]);
    let exact_threshold = cosine_similarity(&[1.0, 0.0], &[0.6, 0.8]).unwrap_or(f32::NAN);
    let config = ClusteringConfig::new(exact_threshold)
        .unwrap_or_else(|error| panic!("threshold should be valid: {error}"));
    let clusterer = UnknownSpeakerClusterer::new(&note_repository, config);

    add_chunk(&clusterer, &meeting_id, &[1.0, 0.0], "c1", 1_000);
    let (_, is_new_cluster) = add_chunk(&clusterer, &meeting_id, &[0.6, 0.8], "c2", 1_000);

    assert!(is_new_cluster);
}

/// Verifies a configured threshold changes the clustering decision for the same vectors.
#[test]
fn configured_threshold_controls_grouping() {
    let meeting_id = MeetingId::new("meeting-a");
    let note_repository = repository_with_meetings(&[&meeting_id]);
    let lenient_config = ClusteringConfig::new(0.5)
        .unwrap_or_else(|error| panic!("threshold should be valid: {error}"));
    let clusterer = UnknownSpeakerClusterer::new(&note_repository, lenient_config);

    add_chunk(&clusterer, &meeting_id, &[1.0, 0.0], "c1", 1_000);
    // ≈ 0.53: below the default 0.70 but above the lenient 0.50
    let (_, is_new_cluster) = add_chunk(&clusterer, &meeting_id, &[0.5, 0.8], "c2", 1_000);

    assert!(!is_new_cluster);
}

/// Verifies a new chunk joins the most similar of several qualifying clusters.
#[test]
fn chunk_joins_most_similar_qualifying_cluster() {
    let meeting_id = MeetingId::new("meeting-a");
    let note_repository = repository_with_meetings(&[&meeting_id]);
    let lenient_config = ClusteringConfig::new(0.5)
        .unwrap_or_else(|error| panic!("threshold should be valid: {error}"));
    let clusterer = UnknownSpeakerClusterer::new(&note_repository, lenient_config);

    add_chunk(&clusterer, &meeting_id, &[1.0, 0.0, 0.0], "c1", 1_000);
    let (second_cluster_id, _) = add_chunk(&clusterer, &meeting_id, &[0.0, 1.0, 0.0], "c2", 1_000);
    // ≈ 0.62 to the first cluster, ≈ 0.78 to the second
    let (joined_cluster_id, is_new_cluster) =
        add_chunk(&clusterer, &meeting_id, &[0.8, 1.0, 0.0], "c3", 1_000);

    assert!(!is_new_cluster);
    assert_eq!(joined_cluster_id, second_cluster_id);
}

/// Verifies the representative embedding is the running mean of member embeddings.
#[test]
fn representative_embedding_is_running_average() {
    let meeting_id = MeetingId::new("meeting-a");
    let note_repository = repository_with_meetings(&[&meeting_id]);
    let clusterer = UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default());

    add_chunk(&clusterer, &meeting_id, &[1.0, 0.0], "c1", 1_000);
    add_chunk(&clusterer, &meeting_id, &[0.8, 0.4], "c2", 1_000);
    add_chunk(&clusterer, &meeting_id, &[0.9, 0.2], "c3", 1_000);

    let clusters = clusters_of(&clusterer, &meeting_id);
    assert_eq!(clusters.len(), 1);
    let representative = clusters[0].representative_embedding();
    assert_eq!(representative.len(), 2);
    assert!((representative[0] - 0.9).abs() < 1e-6);
    assert!((representative[1] - 0.2).abs() < 1e-6);
}

/// Verifies identical voices in different meetings never share a cluster.
#[test]
fn clusters_are_isolated_per_meeting() {
    let meeting_a = MeetingId::new("meeting-a");
    let meeting_b = MeetingId::new("meeting-b");
    let note_repository = repository_with_meetings(&[&meeting_a, &meeting_b]);
    let clusterer = UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default());

    let (cluster_in_a, _) = add_chunk(&clusterer, &meeting_a, &[1.0, 0.0], "a1", 1_000);
    let (cluster_in_b, is_new_cluster) =
        add_chunk(&clusterer, &meeting_b, &[1.0, 0.0], "b1", 1_000);

    assert!(is_new_cluster);
    assert_ne!(cluster_in_a, cluster_in_b);
    assert_eq!(clusters_of(&clusterer, &meeting_a).len(), 1);
    let clusters_in_b = clusters_of(&clusterer, &meeting_b);
    assert_eq!(clusters_in_b.len(), 1);
    assert_eq!(clusters_in_b[0].chunk_ids(), ["b1".to_owned()]);
}

/// Verifies empty, non-finite, and zero-norm embeddings are rejected before storage.
#[test]
fn invalid_embeddings_are_rejected() {
    let meeting_id = MeetingId::new("meeting-a");
    let note_repository = repository_with_meetings(&[&meeting_id]);
    let clusterer = UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default());

    for invalid_embedding in [vec![], vec![f32::NAN, 1.0], vec![0.0, 0.0]] {
        assert!(matches!(
            clusterer.find_or_create_cluster(&meeting_id, &invalid_embedding, "bad".to_owned(), 1),
            Err(ClusteringError::InvalidEmbedding { .. })
        ));
    }
    assert!(clusters_of(&clusterer, &meeting_id).is_empty());
}

/// Verifies a chunk whose dimensionality differs from every cluster starts its own cluster.
#[test]
fn mismatched_dimensions_create_new_cluster() {
    let meeting_id = MeetingId::new("meeting-a");
    let note_repository = repository_with_meetings(&[&meeting_id]);
    let clusterer = UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default());

    add_chunk(&clusterer, &meeting_id, &[1.0, 0.0], "c1", 1_000);
    let (_, is_new_cluster) = add_chunk(&clusterer, &meeting_id, &[1.0, 0.0, 0.0], "c2", 1_000);

    assert!(is_new_cluster);
}

/// Returns one cluster without members, simulating a partially written row from storage.
struct EmptyClusterRepository {
    meeting_id: MeetingId,
}

impl NoteRepository for EmptyClusterRepository {
    fn create_meeting(
        &self,
        _meeting_id: &MeetingId,
        _document: &[u8],
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
        _document: &[u8],
    ) -> Result<(), StorageError> {
        Ok(())
    }
    fn load_crdt_document(&self, _meeting_id: &MeetingId) -> Result<Vec<u8>, StorageError> {
        Ok(Vec::new())
    }
    fn save_unassigned_clip(&self, _clip: UnassignedVoiceClip) -> Result<(), StorageError> {
        Ok(())
    }
    fn get_unassigned_clips(
        &self,
        _meeting_id: &MeetingId,
    ) -> Result<Vec<UnassignedVoiceClip>, StorageError> {
        Ok(Vec::new())
    }
    /// Returns a single cluster that has no member chunks.
    fn load_unknown_speaker_clusters(
        &self,
        _meeting_id: &MeetingId,
    ) -> Result<Vec<UnknownSpeakerCluster>, StorageError> {
        Ok(vec![UnknownSpeakerCluster::new(
            Uuid::new_v4(),
            self.meeting_id.clone(),
            vec![1.0, 0.0],
            Vec::new(),
            0,
            Utc::now(),
            None,
        )])
    }
}

/// Verifies a cluster without member chunks is never returned to callers.
#[test]
fn empty_cluster_is_not_returned() {
    let meeting_id = MeetingId::new("meeting-a");
    let note_repository = EmptyClusterRepository {
        meeting_id: meeting_id.clone(),
    };
    let clusterer = UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default());

    let clusters = clusterer
        .get_clusters_for_meeting(&meeting_id)
        .unwrap_or_else(|error| panic!("clusters should load: {error}"));

    assert!(clusters.is_empty());
}

/// Verifies assignment enrolls every member embedding, relinks segments, and resolves the cluster.
#[test]
fn assigning_identity_updates_chunks_segments_and_resolves_cluster() {
    let meeting_id = MeetingId::new("meeting-a");
    let note_repository = repository_with_meetings(&[&meeting_id]);
    let clusterer = UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default());
    let (target_cluster_id, _) = add_chunk(&clusterer, &meeting_id, &[1.0, 0.0], "c1", 1_000);
    add_chunk(&clusterer, &meeting_id, &[0.9, 0.3], "c2", 1_000);
    let (other_cluster_id, _) = add_chunk(&clusterer, &meeting_id, &[0.0, 1.0], "c3", 1_000);
    note_repository
        .save_transcript_segments(
            &meeting_id,
            &[
                TranscriptSegment::new(meeting_id.clone(), 0, 1_000, None, "first")
                    .with_unknown_cluster_id(Some(target_cluster_id)),
                TranscriptSegment::new(meeting_id.clone(), 1_000, 2_000, None, "other")
                    .with_unknown_cluster_id(Some(other_cluster_id)),
                TranscriptSegment::new(meeting_id.clone(), 2_000, 3_000, None, "second")
                    .with_unknown_cluster_id(Some(target_cluster_id)),
            ],
        )
        .unwrap_or_else(|error| panic!("segments should save: {error}"));
    note_repository
        .save_unassigned_clip(
            UnassignedVoiceClip::new(Uuid::new_v4(), meeting_id.clone(), 0, 1_000, vec![1, 2])
                .with_cluster_id(Some(target_cluster_id)),
        )
        .unwrap_or_else(|error| panic!("clip should save: {error}"));
    let voice_vector_database = InMemoryVoiceVectorDB::new();
    let new_identity_id = Uuid::new_v4();

    let assignment = clusterer
        .assign_identity_to_cluster(&voice_vector_database, target_cluster_id, new_identity_id)
        .unwrap_or_else(|error| panic!("assignment should succeed: {error}"));

    assert!(assignment.identity_created());
    assert_eq!(assignment.variants_stored(), 2);
    assert_eq!(assignment.segments_updated(), 2);
    assert_eq!(
        voice_vector_database
            .voice_variant_count(new_identity_id)
            .unwrap_or_default(),
        2
    );
    let segment_speakers: Vec<Option<Uuid>> = note_repository
        .load_transcript_segments(&meeting_id)
        .unwrap_or_else(|error| panic!("segments should load: {error}"))
        .iter()
        .map(TranscriptSegment::speaker_identity_id)
        .collect();
    assert_eq!(
        segment_speakers,
        vec![Some(new_identity_id), None, Some(new_identity_id)]
    );
    let remaining_clusters = clusters_of(&clusterer, &meeting_id);
    assert_eq!(remaining_clusters.len(), 1);
    assert_eq!(remaining_clusters[0].cluster_id(), other_cluster_id);
    let resolved_cluster = note_repository
        .load_unknown_speaker_cluster(target_cluster_id)
        .unwrap_or_else(|error| panic!("cluster should load: {error}"));
    assert_eq!(
        resolved_cluster.and_then(|cluster| cluster.resolved_identity_id()),
        Some(new_identity_id)
    );
    assert!(
        note_repository
            .get_unassigned_clips(&meeting_id)
            .unwrap_or_else(|error| panic!("clips should load: {error}"))
            .is_empty(),
        "review audio for the resolved cluster must be deleted"
    );
}

/// Verifies assigning to an identity that already has variants reports it as existing.
#[test]
fn assigning_existing_identity_adds_variants_without_creating_it() {
    let meeting_id = MeetingId::new("meeting-a");
    let note_repository = repository_with_meetings(&[&meeting_id]);
    let clusterer = UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default());
    let (cluster_id, _) = add_chunk(&clusterer, &meeting_id, &[1.0, 0.0], "c1", 1_000);
    let voice_vector_database = InMemoryVoiceVectorDB::new();
    let existing_identity_id = Uuid::new_v4();
    voice_vector_database
        .store_embedding(crate::domain::models::VoiceEmbedding::new(
            existing_identity_id,
            Uuid::new_v4(),
            vec![0.0, 1.0],
        ))
        .unwrap_or_else(|error| panic!("enrollment should succeed: {error}"));

    let assignment = clusterer
        .assign_identity_to_cluster(&voice_vector_database, cluster_id, existing_identity_id)
        .unwrap_or_else(|error| panic!("assignment should succeed: {error}"));

    assert!(!assignment.identity_created());
    assert_eq!(
        voice_vector_database
            .voice_variant_count(existing_identity_id)
            .unwrap_or_default(),
        2
    );
}

/// Verifies unknown and already-resolved clusters are rejected without enrolling variants.
#[test]
fn assigning_missing_or_resolved_cluster_is_rejected() {
    let meeting_id = MeetingId::new("meeting-a");
    let note_repository = repository_with_meetings(&[&meeting_id]);
    let clusterer = UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default());
    let voice_vector_database = InMemoryVoiceVectorDB::new();
    let identity_id = Uuid::new_v4();

    assert!(matches!(
        clusterer.assign_identity_to_cluster(&voice_vector_database, Uuid::new_v4(), identity_id),
        Err(ClusteringError::ClusterNotFound { .. })
    ));

    let (cluster_id, _) = add_chunk(&clusterer, &meeting_id, &[1.0, 0.0], "c1", 1_000);
    clusterer
        .assign_identity_to_cluster(&voice_vector_database, cluster_id, identity_id)
        .unwrap_or_else(|error| panic!("first assignment should succeed: {error}"));
    assert!(matches!(
        clusterer.assign_identity_to_cluster(&voice_vector_database, cluster_id, identity_id),
        Err(ClusteringError::ClusterAlreadyResolved { .. })
    ));
    assert_eq!(
        voice_vector_database
            .voice_variant_count(identity_id)
            .unwrap_or_default(),
        1
    );
}

/// Verifies a resolved cluster no longer attracts new chunks of the same voice.
#[test]
fn resolved_cluster_is_not_reused_for_new_chunks() {
    let meeting_id = MeetingId::new("meeting-a");
    let note_repository = repository_with_meetings(&[&meeting_id]);
    let clusterer = UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default());
    let (resolved_cluster_id, _) = add_chunk(&clusterer, &meeting_id, &[1.0, 0.0], "c1", 1_000);
    clusterer
        .assign_identity_to_cluster(
            &InMemoryVoiceVectorDB::new(),
            resolved_cluster_id,
            Uuid::new_v4(),
        )
        .unwrap_or_else(|error| panic!("assignment should succeed: {error}"));

    let (new_cluster_id, is_new_cluster) =
        add_chunk(&clusterer, &meeting_id, &[1.0, 0.0], "c2", 1_000);

    assert!(is_new_cluster);
    assert_ne!(new_cluster_id, resolved_cluster_id);
}
