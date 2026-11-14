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
            ClusterReviewError, NamedSpanSource, assign_cluster_to_named_identity,
            assign_clusters_to_named_identity, find_cluster_review_clip, group_unknown_voices,
            list_unknown_clusters, load_voice_library, load_voice_library_with_names,
        },
        unknown_speaker_clusterer::{ClusteringConfig, ClusteringError, UnknownSpeakerClusterer},
    },
    domain::models::{
        DeviceId, MeetingId, SpeakerIdentity, TranscriptSegment, UnassignedVoiceClip,
        VoiceEmbedding,
    },
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

    let summaries =
        list_unknown_clusters(&note_repository, &InMemoryVoiceVectorDB::new(), &meeting_id)
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

/// Verifies each unknown voice carries the label its transcript shows, numbered by first
/// appearance rather than by creation order (#118).
#[test]
fn listed_clusters_carry_the_transcripts_speaker_numbers() {
    let (note_repository, meeting_id, large_cluster_id, small_cluster_id) =
        repository_with_two_clusters();
    // The cluster created second speaks first.
    note_repository
        .save_transcript_segments(
            &meeting_id,
            &[
                TranscriptSegment::new(meeting_id.clone(), 0, 1_000, None, "first")
                    .with_unknown_cluster_id(Some(small_cluster_id)),
                TranscriptSegment::new(meeting_id.clone(), 1_000, 2_000, None, "second")
                    .with_unknown_cluster_id(Some(large_cluster_id)),
            ],
        )
        .unwrap_or_else(|error| panic!("segments should save: {error}"));

    let summaries =
        list_unknown_clusters(&note_repository, &InMemoryVoiceVectorDB::new(), &meeting_id)
            .unwrap_or_else(|error| panic!("clusters should list: {error}"));

    assert_eq!(summaries[0].cluster_id(), large_cluster_id);
    assert_eq!(summaries[0].speaker_label(), Some("Speaker 2"));
    assert_eq!(summaries[1].speaker_label(), Some("Speaker 1"));
}

/// Verifies a meeting without clusters (or an unknown meeting) lists nothing.
#[test]
fn lists_no_clusters_for_an_unknown_meeting() {
    let (note_repository, _, _, _) = repository_with_two_clusters();

    let summaries = list_unknown_clusters(
        &note_repository,
        &InMemoryVoiceVectorDB::new(),
        &MeetingId::new("meeting-other"),
    )
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
    assert_eq!(assignment.variants_stored(), 0);
    assert_eq!(assignment.segments_updated(), 1);
    assert_eq!(
        voice_vector_database
            .voice_variant_count(assignment.identity_id())
            .unwrap_or_default(),
        0
    );
    let remaining =
        list_unknown_clusters(&note_repository, &InMemoryVoiceVectorDB::new(), &meeting_id)
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
        list_unknown_clusters(&note_repository, &InMemoryVoiceVectorDB::new(), &meeting_id)
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
        list_unknown_clusters(
            repository_object,
            &InMemoryVoiceVectorDB::new(),
            &meeting_id
        )
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
    assert_eq!(variant_count, 0);
    assert_eq!(
        first_cluster.and_then(|cluster| cluster.resolved_identity_id()),
        Some(first_assignment.identity_id())
    );
}

/// Verifies the library lists clusters across meetings, with titles and clip availability, and
/// saved people with their variant counts, without biometric data.
#[test]
fn voice_library_spans_meetings_and_lists_saved_people() {
    let (note_repository, meeting_id, large_cluster_id, small_cluster_id) =
        repository_with_two_clusters();
    let other_meeting_id = MeetingId::new("meeting-other");
    note_repository
        .create_meeting(&other_meeting_id, &[])
        .unwrap_or_else(|error| panic!("meeting should be created: {error}"));
    let other_cluster_id =
        UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default())
            .find_or_create_cluster(&other_meeting_id, &[1.0, 0.0], String::from("o1"), 900)
            .unwrap_or_else(|error| panic!("chunk should be clustered: {error}"))
            .cluster_id();
    note_repository
        .save_unassigned_clip(
            UnassignedVoiceClip::new(Uuid::new_v4(), meeting_id.clone(), 0, 500, vec![1, 2])
                .with_cluster_id(Some(large_cluster_id)),
        )
        .unwrap_or_else(|error| panic!("clip should be saved: {error}"));
    let voice_database = InMemoryVoiceVectorDB::new();
    assign_cluster_to_named_identity(
        &note_repository,
        &voice_database,
        small_cluster_id,
        "Grace Hopper",
    )
    .unwrap_or_else(|error| panic!("cluster should be assigned: {error}"));

    let library = load_voice_library(&note_repository, &voice_database)
        .unwrap_or_else(|error| panic!("library should load: {error}"));

    let listed_clusters = library
        .unknown_clusters()
        .iter()
        .map(|cluster| (cluster.summary().cluster_id(), cluster.has_review_clip()))
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        listed_clusters,
        std::collections::HashSet::from([(large_cluster_id, true), (other_cluster_id, false)])
    );
    assert_eq!(library.saved_people().len(), 1);
    assert_eq!(library.saved_people()[0].display_name(), "Grace Hopper");
    assert_eq!(library.saved_people()[0].voice_variant_count(), 0);
}

/// Verifies a cluster's review clip is found by cluster alone, and that none is reported for a
/// cluster without one or for an unknown cluster.
#[test]
fn finds_a_review_clip_by_cluster() {
    let (note_repository, meeting_id, large_cluster_id, small_cluster_id) =
        repository_with_two_clusters();
    note_repository
        .save_unassigned_clip(
            UnassignedVoiceClip::new(Uuid::new_v4(), meeting_id, 0, 500, vec![1, 2])
                .with_cluster_id(Some(large_cluster_id)),
        )
        .unwrap_or_else(|error| panic!("clip should be saved: {error}"));

    let review_clip = |cluster_id| {
        find_cluster_review_clip(&note_repository, cluster_id)
            .unwrap_or_else(|error| panic!("lookup should work: {error}"))
    };

    assert_eq!(
        review_clip(large_cluster_id).map(|clip| clip.audio_bytes().to_vec()),
        Some(vec![1, 2])
    );
    assert!(review_clip(small_cluster_id).is_none());
    assert!(review_clip(Uuid::new_v4()).is_none());
}

/// Adds one single-chunk cluster with `embedding` to a new meeting called `meeting_name`.
///
/// Outputs: The cluster ID. Side effects: Creates the meeting and the cluster in SQLite.
fn add_meeting_with_voice(
    note_repository: &SqliteNoteRepository,
    meeting_name: &str,
    embedding: &[f32],
    duration_ms: u64,
) -> Uuid {
    let meeting_id = MeetingId::new(meeting_name);
    note_repository
        .create_meeting(&meeting_id, &[])
        .unwrap_or_else(|error| panic!("meeting should be created: {error}"));
    UnknownSpeakerClusterer::new(note_repository, ClusteringConfig::default())
        .find_or_create_cluster(
            &meeting_id,
            embedding,
            Uuid::new_v4().to_string(),
            duration_ms,
        )
        .unwrap_or_else(|error| panic!("chunk should be clustered: {error}"))
        .cluster_id()
}

/// Verifies close voices form one group and distant ones stay apart (#82).
#[test]
fn close_voices_form_one_group() {
    let labels = group_unknown_voices(
        &[
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.95, 0.31, 0.0],
            vec![0.0, 0.0, 2.0],
        ],
        0.6,
    );
    assert_eq!(labels, vec![0, 1, 0, 2]);
}

/// Verifies a voice that cannot be compared stays on its own instead of joining or failing.
#[test]
fn an_unusable_voice_stays_alone() {
    let labels = group_unknown_voices(
        &[
            vec![1.0, 0.0],
            vec![0.0, 0.0],
            vec![1.0, 0.0, 0.0],
            vec![1.0, 0.0],
        ],
        0.6,
    );
    assert_eq!(labels, vec![0, 1, 2, 0]);
}

/// Verifies the library puts one person's clusters from several meetings into one group, listed
/// together, and keeps another person apart (#82).
#[test]
fn the_library_groups_one_persons_voices_across_meetings() {
    let note_repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite should open: {error}"));
    let first = add_meeting_with_voice(&note_repository, "meeting-a", &[1.0, 0.0, 0.0], 30_000);
    let other = add_meeting_with_voice(&note_repository, "meeting-b", &[0.0, 1.0, 0.0], 5_000);
    let second = add_meeting_with_voice(&note_repository, "meeting-c", &[0.97, 0.0, 0.24], 12_000);

    let library = load_voice_library(&note_repository, &InMemoryVoiceVectorDB::new())
        .unwrap_or_else(|error| panic!("library should load: {error}"));

    let groups = library.unknown_voice_groups();
    assert_eq!(groups.len(), 2);
    let grouped: Vec<Vec<Uuid>> = groups
        .iter()
        .map(|group| {
            group
                .clusters()
                .iter()
                .map(|cluster| cluster.summary().cluster_id())
                .collect()
        })
        .collect();
    assert!(grouped.contains(&vec![first, second]), "{grouped:?}");
    assert!(grouped.contains(&vec![other]));
    let pair = groups
        .iter()
        .find(|group| group.clusters().len() == 2)
        .unwrap_or_else(|| panic!("a group of two should exist"));
    assert_eq!(pair.total_duration_ms(), 42_000);
    assert_eq!(pair.meeting_count(), 2);
    assert!(
        pair.clusters()
            .iter()
            .all(|cluster| cluster.voice_group_id() == pair.group_id())
    );
    let flat: Vec<Uuid> = library
        .unknown_clusters()
        .iter()
        .map(|cluster| cluster.summary().cluster_id())
        .collect();
    let first_position = flat.iter().position(|id| *id == first);
    let second_position = flat.iter().position(|id| *id == second);
    assert_eq!(
        first_position.map(|position| position + 1),
        second_position,
        "a group's clusters are listed next to each other"
    );
}

/// Verifies naming several clusters at once creates one person and resolves each cluster (#82).
#[test]
fn naming_a_group_names_each_cluster_once_for_one_person() {
    let note_repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite should open: {error}"));
    let first = add_meeting_with_voice(&note_repository, "meeting-a", &[1.0, 0.0], 3_000);
    let second = add_meeting_with_voice(&note_repository, "meeting-b", &[0.9, 0.1], 2_000);
    let voice_database = InMemoryVoiceVectorDB::new();

    let assignment = assign_clusters_to_named_identity(
        &note_repository,
        &voice_database,
        &[first, second],
        "  Ada Lovelace ",
    )
    .unwrap_or_else(|error| panic!("the group should be named: {error}"));

    assert_eq!(assignment.identity_name(), "Ada Lovelace");
    assert!(assignment.identity_created());
    assert_eq!(assignment.assigned_cluster_ids(), &[first, second]);
    let identities = voice_database
        .list_speaker_identities()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(identities.len(), 1, "one person, not one per cluster");
    for cluster_id in [first, second] {
        let cluster = note_repository
            .load_unknown_speaker_cluster(cluster_id)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            cluster.and_then(|cluster| cluster.resolved_identity_id()),
            Some(assignment.identity_id())
        );
    }
}

/// Verifies a cluster named meanwhile (for example on a paired device) is skipped, not fatal,
/// while a blank name or an empty selection writes nothing.
#[test]
fn naming_a_group_skips_clusters_already_named_and_rejects_bad_input() {
    let note_repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite should open: {error}"));
    let first = add_meeting_with_voice(&note_repository, "meeting-a", &[1.0, 0.0], 3_000);
    let second = add_meeting_with_voice(&note_repository, "meeting-b", &[0.9, 0.1], 2_000);
    let voice_database = InMemoryVoiceVectorDB::new();
    assign_cluster_to_named_identity(&note_repository, &voice_database, first, "Grace")
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(matches!(
        assign_clusters_to_named_identity(&note_repository, &voice_database, &[second], "  "),
        Err(ClusterReviewError::BlankIdentityName)
    ));
    assert!(matches!(
        assign_clusters_to_named_identity(&note_repository, &voice_database, &[], "Grace"),
        Err(ClusterReviewError::NoClustersSelected)
    ));
    let assignment = assign_clusters_to_named_identity(
        &note_repository,
        &voice_database,
        &[first, second, Uuid::new_v4()],
        "grace",
    )
    .unwrap_or_else(|error| panic!("the rest of the group should be named: {error}"));
    assert_eq!(assignment.assigned_cluster_ids(), &[second]);
    assert_eq!(assignment.skipped_cluster_count(), 2);
    assert_eq!(assignment.identity_name(), "Grace");
    assert!(!assignment.identity_created());

    assert!(matches!(
        assign_clusters_to_named_identity(&note_repository, &voice_database, &[first], "Grace"),
        Err(ClusterReviewError::Clustering(
            ClusteringError::ClusterAlreadyResolved { .. }
        ))
    ));
}

/// Names shown on a call window, per meeting, for the suggestion tests (#113).
struct FixedNames(Vec<crate::domain::speaker_names::NamedSpan>);

impl NamedSpanSource for FixedNames {
    fn named_spans(&self, _meeting_id: &MeetingId) -> Vec<crate::domain::speaker_names::NamedSpan> {
        self.0.clone()
    }
}

/// Verifies an unknown voice is offered the name the call window showed while it spoke, and a
/// voice without such evidence is offered none (#113).
#[test]
fn voices_are_offered_the_names_shown_while_they_spoke() {
    use crate::domain::speaker_names::NamedSpan;
    let (note_repository, meeting_id, large_cluster_id, small_cluster_id) =
        repository_with_two_clusters();
    note_repository
        .save_transcript_segments(
            &meeting_id,
            &[
                TranscriptSegment::new(meeting_id.clone(), 0, 4_000, None, "one")
                    .with_unknown_cluster_id(Some(large_cluster_id)),
                TranscriptSegment::new(meeting_id.clone(), 5_000, 6_000, None, "two")
                    .with_unknown_cluster_id(Some(small_cluster_id)),
            ],
        )
        .unwrap_or_else(|error| panic!("segments should save: {error}"));
    let names = FixedNames(vec![
        NamedSpan::new(0, 4_000, "Ada Lovelace"),
        NamedSpan::new(5_000, 6_000, "Grace Hopper"),
    ]);

    let library =
        load_voice_library_with_names(&note_repository, &InMemoryVoiceVectorDB::new(), &names)
            .unwrap_or_else(|error| panic!("library should load: {error}"));

    let suggestion_of = |cluster_id: Uuid| {
        library
            .unknown_clusters()
            .iter()
            .find(|cluster| cluster.summary().cluster_id() == cluster_id)
            .and_then(|cluster| cluster.suggested_name().map(str::to_owned))
    };
    assert_eq!(
        suggestion_of(large_cluster_id).as_deref(),
        Some("Ada Lovelace")
    );
    // One second under the name is too little evidence.
    assert_eq!(suggestion_of(small_cluster_id), None);
    let plain = load_voice_library(&note_repository, &InMemoryVoiceVectorDB::new())
        .unwrap_or_else(|error| panic!("library should load: {error}"));
    assert!(
        plain
            .unknown_clusters()
            .iter()
            .all(|cluster| cluster.suggested_name().is_none())
    );
}
