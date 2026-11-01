//! Groups one meeting's unknown-voice chunks by cosine similarity and resolves clusters to identities.
//!
//! When the pipeline produces an "Unknown" or "Unknown (pending review)" speaker segment, its
//! derived embedding is compared with the representative embedding of every unresolved cluster of
//! the same meeting. The best cluster whose cosine similarity is strictly above the configured
//! threshold (default 0.40) absorbs the chunk and its representative becomes the running mean of
//! its members; otherwise a new cluster is created. Assigning an identity to a cluster enrolls
//! every member embedding as a voice variant, relinks the cluster's transcript segments, and
//! resolves the cluster (deleting its temporary review audio).
//!
//! Clustering is application policy over persisted data, so the cosine math lives here rather
//! than in the vector-search adapter. Persistence goes only through `NoteRepository`; identity
//! enrollment goes only through `VoiceVectorDB`. Embeddings are never logged.

use crate::application::pipeline::meeting_speaker_pass::{
    MIN_CLEAN_VOICE_CONSISTENCY, speaker_voices,
};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    domain::{
        errors::StorageError,
        models::{
            MeetingId, SpeakerIdentity, UnknownSpeakerChunk, UnknownSpeakerCluster, VoiceEmbedding,
        },
    },
    ports::{storage::NoteRepository, voice::VoiceVectorDB},
};

/// Default minimum cosine similarity (exclusive) for a chunk to join an existing cluster.
///
/// Higher values split one person across more clusters (more review work, fewer merges of
/// different people); lower values merge similar-sounding people into one cluster. Tuned on ECAPA embeddings of
/// LibriSpeech (#44): different speakers stay below about 0.33 (99th percentile), while the same
/// speaker's 4 s segments stay above about 0.49 (5th percentile).
pub const DEFAULT_CLUSTER_SIMILARITY_THRESHOLD: f32 = 0.40;

/// Configures unknown-speaker clustering.
/// Least clean speech a member needs for its voice to be saved when its cluster is named.
pub const MIN_SAVED_VOICE_MS: u64 = 8_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClusteringConfig {
    /// Exclusive cosine-similarity cutoff in `(0, 1]`.
    similarity_threshold: f32,
}

impl Default for ClusteringConfig {
    /// Uses `DEFAULT_CLUSTER_SIMILARITY_THRESHOLD`.
    fn default() -> Self {
        Self {
            similarity_threshold: DEFAULT_CLUSTER_SIMILARITY_THRESHOLD,
        }
    }
}

impl ClusteringConfig {
    /// Creates a configuration with a custom similarity threshold.
    ///
    /// Inputs: `similarity_threshold` must be finite and in `(0, 1]`; a chunk joins a cluster only
    /// when its cosine similarity is strictly greater than this value.
    /// Outputs: The configuration, or `ClusteringError::InvalidThreshold`.
    /// Side effects: None.
    pub fn new(similarity_threshold: f32) -> Result<Self, ClusteringError> {
        if !similarity_threshold.is_finite()
            || similarity_threshold <= 0.0
            || similarity_threshold > 1.0
        {
            return Err(ClusteringError::InvalidThreshold {
                threshold: similarity_threshold,
            });
        }
        Ok(Self {
            similarity_threshold,
        })
    }

    /// Returns the exclusive cosine-similarity cutoff.
    pub fn similarity_threshold(&self) -> f32 {
        self.similarity_threshold
    }
}

/// Reports why clustering or cluster assignment could not complete.
#[derive(Debug, thiserror::Error)]
pub enum ClusteringError {
    /// The configured threshold is not a finite value in `(0, 1]`.
    #[error("cluster similarity threshold {threshold} must be finite and in (0, 1]")]
    InvalidThreshold {
        /// The rejected threshold.
        threshold: f32,
    },
    /// The chunk embedding cannot be compared by cosine similarity.
    #[error("unknown-speaker embedding is invalid: {reason}")]
    InvalidEmbedding {
        /// A privacy-safe description that never contains vector values.
        reason: &'static str,
    },
    /// No cluster with this identifier exists.
    #[error("unknown-speaker cluster {cluster_id} was not found")]
    ClusterNotFound {
        /// The requested cluster.
        cluster_id: Uuid,
    },
    /// The cluster was already assigned to an identity.
    #[error("unknown-speaker cluster {cluster_id} is already resolved")]
    ClusterAlreadyResolved {
        /// The requested cluster.
        cluster_id: Uuid,
    },
    /// Reading or writing clusters, segments, or voice variants failed.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// Reports which cluster absorbed a chunk.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClusterMembership {
    cluster_id: Uuid,
    /// `true` when the chunk founded a new cluster rather than joining an existing one.
    is_new_cluster: bool,
}

impl ClusterMembership {
    /// Returns the cluster that now contains the chunk.
    pub fn cluster_id(&self) -> Uuid {
        self.cluster_id
    }

    /// Reports whether the chunk created the cluster (and so should carry its review clip).
    pub fn is_new_cluster(&self) -> bool {
        self.is_new_cluster
    }
}

/// Summarizes a completed cluster-to-identity assignment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClusterAssignment {
    cluster_id: Uuid,
    /// The identity as read back from the voice store after it was saved.
    identity: SpeakerIdentity,
    /// `true` when the voice store had no record of the identity before this assignment.
    identity_created: bool,
    variants_stored: usize,
    segments_updated: usize,
}

impl ClusterAssignment {
    /// Returns the resolved cluster.
    pub fn cluster_id(&self) -> Uuid {
        self.cluster_id
    }

    /// Returns the UUID of the identity the cluster was assigned to.
    pub fn identity_id(&self) -> Uuid {
        self.identity.identity_id()
    }

    /// Borrows the persisted identity (UUID and stored display name) the cluster was assigned to.
    pub fn identity(&self) -> &SpeakerIdentity {
        &self.identity
    }

    /// Reports whether this assignment created the identity's voice-store record.
    pub fn identity_created(&self) -> bool {
        self.identity_created
    }

    /// Returns how many member embeddings were stored as voice variants.
    pub fn variants_stored(&self) -> usize {
        self.variants_stored
    }

    /// Returns how many transcript segments were re-attributed to the identity.
    pub fn segments_updated(&self) -> usize {
        self.segments_updated
    }
}

/// Computes the cosine similarity of two derived embeddings.
///
/// Inputs: Two vectors of equal, non-zero length.
/// Outputs: The similarity in `[-1, 1]`, or `None` when lengths differ, a vector is empty, or
/// either vector has zero norm.
/// Side effects: None.
pub fn cosine_similarity(left_vector: &[f32], right_vector: &[f32]) -> Option<f32> {
    if left_vector.is_empty() || left_vector.len() != right_vector.len() {
        return None;
    }
    let (dot_product, left_norm_squared, right_norm_squared) = left_vector
        .iter()
        .zip(right_vector)
        .fold((0.0_f32, 0.0_f32, 0.0_f32), |(dot, left, right), (a, b)| {
            (dot + a * b, left + a * a, right + b * b)
        });
    let norm_product = left_norm_squared.sqrt() * right_norm_squared.sqrt();
    if norm_product == 0.0 || !norm_product.is_finite() {
        return None;
    }
    Some(dot_product / norm_product)
}

/// Clusters unknown voices of a meeting and resolves clusters through injected ports.
pub struct UnknownSpeakerClusterer<'repository, RepositoryPort> {
    /// Borrowed so `MeetingSession` can cluster through the repository it already owns.
    note_repository: &'repository RepositoryPort,
    config: ClusteringConfig,
}

impl<'repository, RepositoryPort> UnknownSpeakerClusterer<'repository, RepositoryPort>
where
    RepositoryPort: NoteRepository,
{
    /// Creates a clusterer over a borrowed repository.
    ///
    /// Inputs: `note_repository` persists clusters and segments; `config` sets the threshold.
    /// Outputs: A ready clusterer.
    /// Side effects: None.
    pub fn new(note_repository: &'repository RepositoryPort, config: ClusteringConfig) -> Self {
        Self {
            note_repository,
            config,
        }
    }

    /// Adds one unknown-voice chunk to the most similar cluster of its meeting, or a new one.
    ///
    /// Inputs: `meeting_id` owns the chunk and must exist; `embedding` is the diarization-derived
    /// vector (non-empty, finite, non-zero); `chunk_id` is unique within the meeting;
    /// `duration_ms` is the speaker segment length.
    /// Outputs: The absorbing cluster and whether it was just created, or an invalid-embedding or
    /// storage error (in which case nothing was written).
    /// Side effects: Reads the meeting's clusters and atomically writes the updated cluster and
    /// the new member row, including biometric vectors. Logs only identifiers and counts.
    pub fn find_or_create_cluster(
        &self,
        meeting_id: &MeetingId,
        embedding: &[f32],
        chunk_id: String,
        duration_ms: u64,
    ) -> Result<ClusterMembership, ClusteringError> {
        validate_embedding(embedding)?;
        let best_match = self
            .get_clusters_for_meeting(meeting_id)?
            .into_iter()
            .filter_map(|cluster| {
                cosine_similarity(embedding, cluster.representative_embedding())
                    .filter(|similarity| *similarity > self.config.similarity_threshold)
                    .map(|similarity| (cluster, similarity))
            })
            .max_by(|(_, left), (_, right)| left.total_cmp(right));

        let (updated_cluster, is_new_cluster) = match best_match {
            Some((existing_cluster, _)) => (
                absorb_chunk(existing_cluster, embedding, &chunk_id, duration_ms),
                false,
            ),
            None => (
                UnknownSpeakerCluster::new(
                    Uuid::new_v4(),
                    meeting_id.clone(),
                    embedding.to_vec(),
                    vec![chunk_id.clone()],
                    duration_ms,
                    Utc::now(),
                    None,
                ),
                true,
            ),
        };
        self.note_repository.save_unknown_speaker_cluster_chunk(
            &updated_cluster,
            &UnknownSpeakerChunk::new(chunk_id, embedding.to_vec(), duration_ms),
        )?;
        tracing::info!(
            meeting_id = %meeting_id.as_str(),
            cluster_id = %updated_cluster.cluster_id(),
            cluster_size = updated_cluster.chunk_ids().len(),
            is_new_cluster,
            "Unknown speaker chunk clustered"
        );
        Ok(ClusterMembership {
            cluster_id: updated_cluster.cluster_id(),
            is_new_cluster,
        })
    }

    /// Lists a meeting's unresolved clusters that contain at least one chunk.
    ///
    /// Inputs: `meeting_id` identifies the meeting.
    /// Outputs: Clusters in creation order, or a storage error.
    /// Side effects: Reads cluster rows, including biometric representative vectors.
    pub fn get_clusters_for_meeting(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Vec<UnknownSpeakerCluster>, ClusteringError> {
        Ok(self
            .note_repository
            .load_unknown_speaker_clusters(meeting_id)?
            .into_iter()
            .filter(|cluster| !cluster.is_resolved() && !cluster.chunk_ids().is_empty())
            .collect())
    }

    /// Assigns a user-chosen identity to every chunk of a cluster and persists that identity.
    ///
    /// Inputs: `voice_vector_database` stores identities and voice variants; `cluster_id` must
    /// exist and be unresolved; `identity` is an existing identity or a fresh UUID with the
    /// user's chosen name for a new person.
    /// Outputs: A summary carrying the identity as read back from the voice store, plus created
    /// variants and updated segments; or not-found, already-resolved, or storage errors.
    /// Side effects: In order, saves the identity (creating it, or renaming it to
    /// `identity.display_name()`), stores one averaged voice when the members are clean, rewrites the
    /// meeting's transcript segments, then marks the cluster resolved and deletes its review
    /// clips. A failure part-way leaves earlier steps applied; retrying after a segment failure
    /// re-stores variants, which only adds duplicate acoustic variants.
    pub fn assign_identity_to_cluster<VectorDatabase>(
        &self,
        voice_vector_database: &VectorDatabase,
        cluster_id: Uuid,
        identity: &SpeakerIdentity,
    ) -> Result<ClusterAssignment, ClusteringError>
    where
        VectorDatabase: VoiceVectorDB,
    {
        let cluster = self
            .note_repository
            .load_unknown_speaker_cluster(cluster_id)?
            .ok_or(ClusteringError::ClusterNotFound { cluster_id })?;
        if cluster.is_resolved() {
            return Err(ClusteringError::ClusterAlreadyResolved { cluster_id });
        }

        let identity_id = identity.identity_id();
        let identity_created = voice_vector_database
            .load_speaker_identity(identity_id)?
            .is_none();
        voice_vector_database.save_speaker_identity(identity)?;
        let persisted_identity = voice_vector_database
            .load_speaker_identity(identity_id)?
            .ok_or_else(|| StorageError::Rejected {
                reason: format!("speaker identity {identity_id} was not persisted"),
            })?;
        let member_chunks = self
            .note_repository
            .load_unknown_speaker_chunks(cluster_id)?;
        // Saving every member would keep any mixed turn in the library for good, and sync it (#74).
        let saved_voice = clean_voice_of(&member_chunks);
        if let Some(saved_voice) = &saved_voice {
            voice_vector_database.store_embedding(VoiceEmbedding::new(
                identity_id,
                Uuid::new_v4(),
                saved_voice.clone(),
            ))?;
        }
        let variants_stored = usize::from(saved_voice.is_some());

        let meeting_id = cluster.meeting_id();
        let mut segments_updated = 0_usize;
        let relinked_segments: Vec<_> = self
            .note_repository
            .load_transcript_segments(meeting_id)?
            .into_iter()
            .map(|segment| {
                if segment.unknown_cluster_id() == Some(cluster_id) {
                    segments_updated += 1;
                    segment.with_speaker_identity_id(Some(identity_id))
                } else {
                    segment
                }
            })
            .collect();
        if segments_updated > 0 {
            self.note_repository
                .save_transcript_segments(meeting_id, &relinked_segments)?;
        }

        self.note_repository
            .resolve_unknown_speaker_cluster(cluster_id, identity_id)?;
        tracing::info!(
            meeting_id = %meeting_id.as_str(),
            cluster_id = %cluster_id,
            identity_id = %identity_id,
            identity_created,
            variants_stored,
            segments_updated,
            "Unknown speaker cluster assigned to identity"
        );
        Ok(ClusterAssignment {
            cluster_id,
            identity: persisted_identity,
            identity_created,
            variants_stored,
            segments_updated,
        })
    }
}

/// Averages a cluster's members into one voice, if they are fit to save.
///
/// Inputs: The members. A member's duration is its clean speech; the final pass records zero for a
/// group that isn't clean.
/// Outputs: The voice, or `None` with too little speech or inconsistent members. Side effects: None.
fn clean_voice_of(member_chunks: &[UnknownSpeakerChunk]) -> Option<Vec<f32>> {
    let clean_speech_ms: u64 = member_chunks
        .iter()
        .map(UnknownSpeakerChunk::duration_ms)
        .sum();
    if clean_speech_ms < MIN_SAVED_VOICE_MS {
        return None;
    }
    let embeddings: Vec<Vec<f32>> = member_chunks
        .iter()
        .map(|member_chunk| member_chunk.embedding().to_vec())
        .collect();
    let voices = speaker_voices(&embeddings, &vec![0; embeddings.len()]);
    let voice = voices.into_iter().next()?;
    (voice.consistency >= MIN_CLEAN_VOICE_CONSISTENCY).then_some(voice.centroid)
}

/// Rejects embeddings that cannot take part in cosine comparison.
///
/// Inputs: A derived embedding.
/// Outputs: `Ok` for non-empty, finite, non-zero vectors; otherwise `InvalidEmbedding`.
/// Side effects: None.
fn validate_embedding(embedding: &[f32]) -> Result<(), ClusteringError> {
    if embedding.is_empty() {
        return Err(ClusteringError::InvalidEmbedding {
            reason: "embedding is empty",
        });
    }
    if embedding.iter().any(|value| !value.is_finite()) {
        return Err(ClusteringError::InvalidEmbedding {
            reason: "embedding contains a non-finite value",
        });
    }
    if embedding.iter().all(|value| *value == 0.0) {
        return Err(ClusteringError::InvalidEmbedding {
            reason: "embedding has zero norm",
        });
    }
    Ok(())
}

/// Returns `cluster` with one more member and its representative updated as a running mean.
///
/// Inputs: An existing cluster whose representative has the same length as `embedding`.
/// Outputs: The updated cluster: `rep' = (rep · n + embedding) / (n + 1)` with `n` members.
/// Side effects: None.
fn absorb_chunk(
    cluster: UnknownSpeakerCluster,
    embedding: &[f32],
    chunk_id: &str,
    duration_ms: u64,
) -> UnknownSpeakerCluster {
    // Lossy only beyond 2^24 members, far above any meeting's chunk count.
    let previous_member_count = cluster.chunk_ids().len() as f32;
    let averaged_embedding = cluster
        .representative_embedding()
        .iter()
        .zip(embedding)
        .map(|(representative_value, new_value)| {
            (representative_value * previous_member_count + new_value)
                / (previous_member_count + 1.0)
        })
        .collect();
    let mut chunk_ids = cluster.chunk_ids().to_vec();
    chunk_ids.push(chunk_id.to_owned());
    UnknownSpeakerCluster::new(
        cluster.cluster_id(),
        cluster.meeting_id().clone(),
        averaged_embedding,
        chunk_ids,
        cluster.total_duration_ms().saturating_add(duration_ms),
        cluster.created_at(),
        None,
    )
}
