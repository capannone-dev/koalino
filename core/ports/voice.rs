//! Declares the adapter-neutral persistence and similarity-search contract for voice embeddings.

use uuid::Uuid;

use crate::domain::{errors::StorageError, models::VoiceEmbedding};

/// Stores derived voice signatures and returns adapter-calculated similarity candidates.
pub trait VoiceVectorDB {
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
