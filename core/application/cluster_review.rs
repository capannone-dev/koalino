//! Lets a user review a meeting's unknown-speaker clusters and assign them to named identities.
//!
//! This is the single use case behind the CLI, HTTP, and FFI cluster commands, so those driving
//! adapters only parse input and format output. Listing delegates to
//! `UnknownSpeakerClusterer::get_clusters_for_meeting` and strips biometric vectors; assignment
//! validates the typed name, resolves it to an identity, and delegates to
//! `UnknownSpeakerClusterer::assign_identity_to_cluster`.
//!
//! Name resolution: a name matching an identity stored in `VoiceVectorDB` (trimmed,
//! case-insensitive) reuses that identity and keeps its stored spelling; any other name becomes a
//! new identity with a fresh UUID. The clusterer persists the identity, so names survive restarts
//! wherever the composition root uses a persistent voice store (`SqliteVoiceVectorDB`).

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{
    application::unknown_speaker_clusterer::{
        ClusterAssignment, ClusteringConfig, ClusteringError, UnknownSpeakerClusterer,
    },
    domain::models::{MeetingId, SpeakerIdentity, UnknownSpeakerCluster},
    ports::{storage::NoteRepository, voice::VoiceVectorDB},
};

/// Reports why a cluster could not be listed or assigned.
#[derive(Debug, thiserror::Error)]
pub enum ClusterReviewError {
    /// The typed identity name is empty or whitespace only.
    #[error("identity name must not be blank")]
    BlankIdentityName,
    /// The clusterer rejected the request (unknown or resolved cluster, storage failure).
    #[error(transparent)]
    Clustering(#[from] ClusteringError),
}

/// Describes one unresolved cluster without its biometric representative embedding.
#[derive(Clone, Debug, PartialEq)]
pub struct UnknownClusterSummary {
    cluster_id: Uuid,
    meeting_id: MeetingId,
    total_duration_ms: u64,
    chunk_count: usize,
    created_at: DateTime<Utc>,
}

impl UnknownClusterSummary {
    /// Copies the display-safe fields of `cluster`, dropping its embedding.
    fn from_cluster(cluster: &UnknownSpeakerCluster) -> Self {
        Self {
            cluster_id: cluster.cluster_id(),
            meeting_id: cluster.meeting_id().clone(),
            total_duration_ms: cluster.total_duration_ms(),
            chunk_count: cluster.chunk_ids().len(),
            created_at: cluster.created_at(),
        }
    }

    /// Returns the cluster UUID.
    pub fn cluster_id(&self) -> Uuid {
        self.cluster_id
    }

    /// Borrows the owning meeting identifier.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns the summed speech duration of every member chunk in milliseconds.
    pub fn total_duration_ms(&self) -> u64 {
        self.total_duration_ms
    }

    /// Returns how many voice chunks the cluster contains.
    pub fn chunk_count(&self) -> usize {
        self.chunk_count
    }

    /// Returns when the cluster's first chunk was recorded.
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
}

/// Summarizes a cluster assignment to a named, persisted identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedClusterAssignment {
    assignment: ClusterAssignment,
}

impl NamedClusterAssignment {
    /// Returns the stored display name of the identity the cluster was assigned to.
    pub fn identity_name(&self) -> &str {
        self.assignment.identity().display_name()
    }

    /// Returns the resolved cluster.
    pub fn cluster_id(&self) -> Uuid {
        self.assignment.cluster_id()
    }

    /// Returns the identity the cluster was assigned to.
    pub fn identity_id(&self) -> Uuid {
        self.assignment.identity_id()
    }

    /// Reports whether the identity had no voice variants before this assignment.
    pub fn identity_created(&self) -> bool {
        self.assignment.identity_created()
    }

    /// Returns how many member embeddings were stored as voice variants.
    pub fn variants_stored(&self) -> usize {
        self.assignment.variants_stored()
    }

    /// Returns how many transcript segments were re-attributed to the identity.
    pub fn segments_updated(&self) -> usize {
        self.assignment.segments_updated()
    }
}

/// Lists a meeting's unresolved unknown-speaker clusters for review.
///
/// Inputs: `note_repository` holds clusters (it may be a `dyn` port); `meeting_id` identifies the
/// meeting, and an unknown meeting simply has no clusters.
/// Outputs: Display-safe summaries in creation order, or a storage error.
/// Side effects: Reads cluster rows; the biometric representative vectors are discarded.
pub fn list_unknown_clusters<RepositoryPort>(
    note_repository: &RepositoryPort,
    meeting_id: &MeetingId,
) -> Result<Vec<UnknownClusterSummary>, ClusterReviewError>
where
    RepositoryPort: NoteRepository + ?Sized,
{
    Ok(
        UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default())
            .get_clusters_for_meeting(meeting_id)?
            .iter()
            .map(UnknownClusterSummary::from_cluster)
            .collect(),
    )
}

/// Assigns every chunk of a cluster to the identity named `identity_name`.
///
/// Inputs: `note_repository` and `voice_vector_database` are the persistence ports (either may be
/// a `dyn` port); `cluster_id` must name an unresolved cluster; `identity_name` must contain a
/// non-whitespace character.
/// Outputs: The assignment summary, or `BlankIdentityName`, or the clusterer's not-found,
/// already-resolved, or storage error.
/// Side effects: Reads identities, then (through the clusterer) saves the identity, stores one
/// voice variant per member embedding, relinks the meeting's transcript segments, resolves the
/// cluster, and deletes its review clips. Nothing is written when the name is blank.
pub fn assign_cluster_to_named_identity<RepositoryPort, VectorDatabase>(
    note_repository: &RepositoryPort,
    voice_vector_database: &VectorDatabase,
    cluster_id: Uuid,
    identity_name: &str,
) -> Result<NamedClusterAssignment, ClusterReviewError>
where
    RepositoryPort: NoteRepository + ?Sized,
    VectorDatabase: VoiceVectorDB + ?Sized,
{
    let trimmed_name = identity_name.trim();
    if trimmed_name.is_empty() {
        return Err(ClusterReviewError::BlankIdentityName);
    }
    let known_identity = voice_vector_database
        .list_speaker_identities()
        .map_err(ClusteringError::from)?
        .into_iter()
        .find(|identity| {
            identity.display_name().trim().to_lowercase() == trimmed_name.to_lowercase()
        });
    let identity =
        known_identity.unwrap_or_else(|| SpeakerIdentity::new(Uuid::new_v4(), trimmed_name));
    let assignment = UnknownSpeakerClusterer::new(&note_repository, ClusteringConfig::default())
        .assign_identity_to_cluster(&voice_vector_database, cluster_id, &identity)?;
    Ok(NamedClusterAssignment { assignment })
}
