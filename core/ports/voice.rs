//! Declares the adapter-neutral persistence and similarity-search contract for voice embeddings.

use uuid::Uuid;

use crate::domain::{
    errors::StorageError,
    models::{SpeakerIdentity, VoiceEmbedding, VoiceMatchResult},
};

/// Classifies a derived speaker embedding without exposing vector-search implementation details.
pub trait VoiceIdentityMatcher {
    /// Matches one derived embedding against the configured voice-signature library.
    ///
    /// Inputs: `embedding` contains model-derived biometric values and must never be logged.
    /// Outputs: A conservative strong, weak, or absent match classification, or a storage error.
    /// Side effects: Queries the adapter-owned voice-signature store.
    fn match_embedding(&self, embedding: &[f32]) -> Result<VoiceMatchResult, StorageError>;
}

/// Stores derived voice signatures and returns adapter-calculated similarity candidates.
pub trait VoiceVectorDB {
    /// Lists display-safe speaker identities known to this voice storage implementation.
    ///
    /// Inputs: None.
    /// Outputs: Identities without embedding vectors, or a portable storage error.
    /// Side effects: Reads identity metadata only; implementations must never expose biometric vectors.
    fn list_speaker_identities(&self) -> Result<Vec<SpeakerIdentity>, StorageError> {
        Ok(Vec::new())
    }

    /// Counts stored acoustic variants for one speaker without exposing vectors.
    ///
    /// Inputs: `identity_id` identifies the speaker whose variants should be counted.
    /// Outputs: The number of stored variants, or a portable storage error.
    /// Side effects: Reads biometric-store metadata only; implementations must not expose vectors.
    fn voice_variant_count(&self, _identity_id: Uuid) -> Result<usize, StorageError> {
        Ok(0)
    }

    /// Stores one derived signature variant for its existing speaker identity.
    ///
    /// Inputs: `embedding` contains a model-derived vector and UUID metadata, never raw audio.
    /// Outputs: Success or a portable storage failure.
    /// Side effects: An implementation persists sensitive biometric data according to its security policy.
    fn store_embedding(&self, embedding: VoiceEmbedding) -> Result<(), StorageError>;

    /// Searches stored variants and returns the highest-scoring candidates from adapter-owned math.
    ///
    /// Inputs: `vector` is a derived query embedding; `top_k` bounds returned candidates.
    /// Outputs: Candidate embeddings paired with descending adapter-calculated similarity scores, or a storage failure.
    /// Side effects: Reads sensitive biometric data but must not log vectors.
    fn search_similar(
        &self,
        vector: &[f32],
        top_k: usize,
    ) -> Result<Vec<(VoiceEmbedding, f32)>, StorageError>;

    /// Removes every signature variant associated with a speaker identity.
    ///
    /// Inputs: `identity_id` identifies the person whose biometric variants must be removed.
    /// Outputs: Success or a portable storage failure.
    /// Side effects: Deletes sensitive biometric data from the adapter-owned store.
    fn delete_identity(&self, identity_id: Uuid) -> Result<(), StorageError>;
}
