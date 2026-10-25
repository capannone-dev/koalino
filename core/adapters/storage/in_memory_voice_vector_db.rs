//! Implements a dependency-light in-memory cosine-similarity adapter for derived voice embeddings.

use std::cell::RefCell;

use uuid::Uuid;

use crate::{
    domain::{errors::StorageError, models::VoiceEmbedding},
    ports::voice::VoiceVectorDB,
};

/// Stores voice variants in process memory and calculates cosine similarity with a simple loop.
///
/// This adapter exists for tests and local development only. A production adapter must provide
/// encrypted-at-rest persistence while preserving the same port contract.
#[derive(Default)]
pub struct InMemoryVoiceVectorDB {
    stored_embeddings: RefCell<Vec<VoiceEmbedding>>,
}

impl InMemoryVoiceVectorDB {
    /// Creates an empty in-memory voice-vector database.
    ///
    /// Inputs: None.
    /// Outputs: An adapter with no stored biometric signatures.
    /// Side effects: Allocates no external resources.
    pub fn new() -> Self {
        Self::default()
    }
}

impl VoiceVectorDB for InMemoryVoiceVectorDB {
    /// Validates and retains one derived embedding only for the lifetime of this process.
    ///
    /// Inputs: `embedding` must contain a non-empty, finite derived vector.
    /// Outputs: Success or rejection for an invalid biometric vector.
    /// Side effects: Stores sensitive biometric data in memory; it is never logged.
    fn store_embedding(&self, embedding: VoiceEmbedding) -> Result<(), StorageError> {
        validate_vector(embedding.vector())?;
        self.stored_embeddings.borrow_mut().push(embedding);
        Ok(())
    }

    /// Returns up to `top_k` variants in descending cosine-similarity order.
    ///
    /// Inputs: `vector` must be a non-empty finite query; `top_k` limits returned candidates.
    /// Outputs: Derived embedding candidates and cosine scores, or invalid-query rejection.
    /// Side effects: Reads in-memory biometric values without logging them.
    fn search_similar(
        &self,
        vector: &[f32],
        top_k: usize,
    ) -> Result<Vec<(VoiceEmbedding, f32)>, StorageError> {
        validate_vector(vector)?;
        if top_k == 0 {
            return Ok(Vec::new());
        }

        let mut scored_embeddings: Vec<(VoiceEmbedding, f32)> = self
            .stored_embeddings
            .borrow()
            .iter()
            .filter_map(|stored_embedding| {
                cosine_similarity(vector, stored_embedding.vector())
                    .map(|score| (stored_embedding.clone(), score))
            })
            .collect();
        scored_embeddings.sort_by(|left_candidate, right_candidate| {
            right_candidate.1.total_cmp(&left_candidate.1)
        });
        scored_embeddings.truncate(top_k);
        Ok(scored_embeddings)
    }

    /// Removes all stored variants belonging to an identity.
    ///
    /// Inputs: `identity_id` identifies every in-memory variant to delete.
    /// Outputs: Success, including when the identity has no stored variants.
    /// Side effects: Irreversibly removes matching in-memory biometric vectors.
    fn delete_identity(&self, identity_id: Uuid) -> Result<(), StorageError> {
        self.stored_embeddings
            .borrow_mut()
            .retain(|embedding| embedding.identity_id() != identity_id);
        Ok(())
    }
}

/// Rejects vectors that cannot safely participate in cosine similarity.
///
/// Inputs: `vector` is a caller-provided derived embedding.
/// Outputs: Success for non-empty finite dimensions or a portable rejection.
/// Side effects: None.
fn validate_vector(vector: &[f32]) -> Result<(), StorageError> {
    if vector.is_empty() {
        return Err(StorageError::Rejected {
            reason: String::from("voice embedding vector cannot be empty"),
        });
    }
    if vector.iter().any(|dimension| !dimension.is_finite()) {
        return Err(StorageError::Rejected {
            reason: String::from("voice embedding vector must contain only finite values"),
        });
    }
    Ok(())
}

/// Calculates cosine similarity for equally dimensioned, non-zero vectors.
///
/// Inputs: `query_vector` and `stored_vector` contain finite derived embedding dimensions.
/// Outputs: A score clamped to -1 through 1, or `None` for incompatible or zero-magnitude vectors.
/// Side effects: None.
fn cosine_similarity(query_vector: &[f32], stored_vector: &[f32]) -> Option<f32> {
    if query_vector.len() != stored_vector.len() {
        return None;
    }

    let (dot_product, query_squared_magnitude, stored_squared_magnitude) =
        query_vector.iter().zip(stored_vector).fold(
            (0.0_f32, 0.0_f32, 0.0_f32),
            |accumulator, (query, stored)| {
                (
                    accumulator.0 + query * stored,
                    accumulator.1 + query * query,
                    accumulator.2 + stored * stored,
                )
            },
        );
    let magnitude_product = query_squared_magnitude.sqrt() * stored_squared_magnitude.sqrt();
    if magnitude_product == 0.0 {
        return None;
    }
    Some((dot_product / magnitude_product).clamp(-1.0, 1.0))
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::{
        adapters::storage::in_memory_voice_vector_db::InMemoryVoiceVectorDB,
        domain::models::VoiceEmbedding, ports::voice::VoiceVectorDB,
    };

    /// Verifies the adapter calculates and ranks cosine candidates without a vector-DB dependency.
    #[test]
    fn returns_cosine_ranked_embeddings() {
        let database = InMemoryVoiceVectorDB::new();
        let identity_id = Uuid::new_v4();
        let closest_variant_id = Uuid::new_v4();
        database
            .store_embedding(VoiceEmbedding::new(
                identity_id,
                Uuid::new_v4(),
                vec![0.0, 1.0],
            ))
            .unwrap_or_else(|error| panic!("first embedding should store: {error}"));
        database
            .store_embedding(VoiceEmbedding::new(
                identity_id,
                closest_variant_id,
                vec![1.0, 0.0],
            ))
            .unwrap_or_else(|error| panic!("second embedding should store: {error}"));

        let results = database
            .search_similar(&[1.0, 0.0], 1)
            .unwrap_or_else(|error| panic!("search should succeed: {error}"));

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.variant_id(), closest_variant_id);
        assert_eq!(results[0].1, 1.0);
    }
}
