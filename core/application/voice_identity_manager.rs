//! Coordinates voice-match policy through a vector-search port without implementing similarity math.

use uuid::Uuid;

use crate::{
    domain::{
        errors::StorageError,
        models::{VoiceEmbedding, VoiceMatchResult},
    },
    ports::voice::VoiceVectorDB,
};

/// Defines conservative confidence cutoffs for application-level voice attribution.
const STRONG_MATCH_THRESHOLD: f32 = 0.80;
/// Defines the minimum plausible candidate score that warrants user confirmation.
const WEAK_MATCH_THRESHOLD: f32 = 0.50;

/// Classifies adapter-provided voice candidates and persists user-confirmed acoustic variants.
pub struct VoiceIdentityManager<VectorDatabase> {
    vector_database: VectorDatabase,
}

impl<VectorDatabase> VoiceIdentityManager<VectorDatabase>
where
    VectorDatabase: VoiceVectorDB,
{
    /// Creates a voice-identity use case with one injected vector storage and search capability.
    ///
    /// Inputs: `vector_database` implements storage and similarity search behind the hexagonal port.
    /// Outputs: A manager that has no knowledge of vector-math implementation details.
    /// Side effects: Takes ownership of the injected port without querying or writing it.
    pub fn new(vector_database: VectorDatabase) -> Self {
        Self { vector_database }
    }

    /// Classifies the best adapter-provided similarity candidate for a derived query embedding.
    ///
    /// Inputs: `raw_embedding` contains derived numeric features, never raw audio samples.
    /// Outputs: A strong match above 0.80, weak match from 0.50 through 0.80, or no match below 0.50.
    /// Side effects: Queries the vector port and emits privacy-safe structured match metadata only.
    pub fn match_embedding(&self, raw_embedding: &[f32]) -> Result<VoiceMatchResult, StorageError> {
        let search_results = self.vector_database.search_similar(raw_embedding, 1)?;
        let Some((closest_embedding, confidence_score)) = search_results.into_iter().next() else {
            tracing::info!("No voice match found");
            return Ok(VoiceMatchResult::NoMatch);
        };
        let identity_id = closest_embedding.identity_id();

        if confidence_score > STRONG_MATCH_THRESHOLD {
            tracing::info!(score = confidence_score, identity = %identity_id, "Strong voice match detected");
            Ok(VoiceMatchResult::StrongMatch(identity_id))
        } else if confidence_score >= WEAK_MATCH_THRESHOLD {
            tracing::info!(score = confidence_score, identity = %identity_id, "Weak voice match detected, prompting user");
            Ok(VoiceMatchResult::WeakMatch(identity_id, confidence_score))
        } else {
            tracing::info!(
                score = confidence_score,
                "Voice candidate is below the confidence threshold"
            );
            Ok(VoiceMatchResult::NoMatch)
        }
    }

    /// Stores a new acoustic signature variant after a user confirms a weak identity match.
    ///
    /// Inputs: `identity_id` is the user-confirmed person; `new_embedding` is a derived vector
    /// from the confirmed audio environment and must never contain raw audio.
    /// Outputs: The stored variant's UUID or a portable storage failure.
    /// Side effects: Writes one sensitive biometric vector through the injected storage port.
    pub fn confirm_weak_match(
        &self,
        identity_id: Uuid,
        new_embedding: &[f32],
    ) -> Result<Uuid, StorageError> {
        let variant_id = Uuid::new_v4();
        self.vector_database.store_embedding(VoiceEmbedding::new(
            identity_id,
            variant_id,
            new_embedding.to_vec(),
        ))?;
        tracing::info!(identity = %identity_id, variant = %variant_id, "Confirmed voice variant stored");
        Ok(variant_id)
    }
}
