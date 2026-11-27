//! Implements a dependency-light in-memory cosine-similarity adapter for derived voice embeddings.

use std::cell::RefCell;

use uuid::Uuid;

use crate::{
    domain::{
        errors::StorageError,
        models::{SpeakerIdentity, VoiceEmbedding},
    },
    ports::voice::VoiceVectorDB,
};

/// Stores voice variants in process memory and calculates cosine similarity with a simple loop.
///
/// This adapter exists for tests and local development only; production composition uses
/// `SqliteVoiceVectorDB`. Unlike the SQLite store, it accepts variants for identities whose name
/// was never saved, so application tests can enroll bare UUIDs.
#[derive(Default)]
pub struct InMemoryVoiceVectorDB {
    stored_embeddings: RefCell<Vec<VoiceEmbedding>>,
    /// Saved identities in creation order.
    stored_identities: RefCell<Vec<SpeakerIdentity>>,
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
    /// Lists saved identities in creation order.
    ///
    /// Inputs: None.
    /// Outputs: Identity metadata without embeddings.
    /// Side effects: Reads the in-memory identity list.
    fn list_speaker_identities(&self) -> Result<Vec<SpeakerIdentity>, StorageError> {
        Ok(self.stored_identities.borrow().clone())
    }

    /// Adds the identity, or renames it in place when its UUID is already saved.
    ///
    /// Inputs: `identity` to store.
    /// Outputs: Success.
    /// Side effects: Mutates the in-memory identity list for this process only.
    fn save_speaker_identity(&self, identity: &SpeakerIdentity) -> Result<(), StorageError> {
        let mut stored_identities = self.stored_identities.borrow_mut();
        match stored_identities
            .iter_mut()
            .find(|stored| stored.identity_id() == identity.identity_id())
        {
            Some(stored_identity) => *stored_identity = identity.clone(),
            None => stored_identities.push(identity.clone()),
        }
        Ok(())
    }

    /// Counts retained variants owned by one identity without returning biometric dimensions.
    ///
    /// Inputs: `identity_id` identifies the in-memory identity.
    /// Outputs: The number of stored variants for that identity.
    /// Side effects: Reads the local in-memory biometric collection without logging it.
    fn voice_variant_count(&self, identity_id: Uuid) -> Result<usize, StorageError> {
        Ok(self
            .stored_embeddings
            .borrow()
            .iter()
            .filter(|embedding| embedding.identity_id() == identity_id)
            .count())
    }

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

    /// Removes an identity's name and all of its stored variants.
    ///
    /// Inputs: `identity_id` identifies every in-memory variant to delete.
    /// Outputs: Success, including when the identity has no stored variants.
    /// Side effects: Irreversibly removes matching in-memory biometric vectors.
    fn delete_identity(&self, identity_id: Uuid) -> Result<(), StorageError> {
        self.stored_embeddings
            .borrow_mut()
            .retain(|embedding| embedding.identity_id() != identity_id);
        self.stored_identities
            .borrow_mut()
            .retain(|identity| identity.identity_id() != identity_id);
        Ok(())
    }

    /// Moves `source`'s variants to `target` under new IDs, then deletes `source` (#188).
    ///
    /// Inputs: Two different saved identities. Outputs: The number of moved variants, or
    /// `Rejected`. Side effects: Mutates the in-memory collections.
    fn merge_identity(&self, source: Uuid, target: Uuid) -> Result<usize, StorageError> {
        let saved = |id: Uuid| {
            self.stored_identities
                .borrow()
                .iter()
                .any(|identity| identity.identity_id() == id)
        };
        if source == target || !saved(source) || !saved(target) {
            return Err(StorageError::Rejected {
                reason: "a merge needs two different saved people".to_owned(),
            });
        }
        let mut embeddings = self.stored_embeddings.borrow_mut();
        let moved: Vec<VoiceEmbedding> = embeddings
            .iter()
            .filter(|embedding| embedding.identity_id() == source)
            .map(|embedding| {
                VoiceEmbedding::new(target, Uuid::new_v4(), embedding.vector().to_vec())
            })
            .collect();
        let moved_count = moved.len();
        embeddings.retain(|embedding| embedding.identity_id() != source);
        embeddings.extend(moved);
        drop(embeddings);
        self.stored_identities
            .borrow_mut()
            .retain(|identity| identity.identity_id() != source);
        Ok(moved_count)
    }
}

/// Rejects vectors that cannot safely participate in cosine similarity.
///
/// Inputs: `vector` is a caller-provided derived embedding.
/// Outputs: Success for non-empty finite dimensions or a portable rejection.
/// Side effects: None.
pub(super) fn validate_vector(vector: &[f32]) -> Result<(), StorageError> {
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
pub(super) fn cosine_similarity(query_vector: &[f32], stored_vector: &[f32]) -> Option<f32> {
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
        domain::models::{SpeakerIdentity, VoiceEmbedding},
        ports::voice::VoiceVectorDB,
    };

    /// Verifies names are saved, renamed in place, loaded, listed, and removed with the identity.
    #[test]
    fn saves_renames_lists_and_deletes_identity_names() {
        let database = InMemoryVoiceVectorDB::new();
        let grace_id = Uuid::new_v4();
        let ada = SpeakerIdentity::new(Uuid::new_v4(), "Ada");
        for identity in [
            SpeakerIdentity::new(grace_id, "Grace"),
            ada.clone(),
            SpeakerIdentity::new(grace_id, "Grace Hopper"),
        ] {
            database
                .save_speaker_identity(&identity)
                .unwrap_or_else(|error| panic!("identity should save: {error}"));
        }

        assert_eq!(
            database.list_speaker_identities().ok(),
            Some(vec![
                SpeakerIdentity::new(grace_id, "Grace Hopper"),
                ada.clone()
            ])
        );
        assert_eq!(
            database.load_speaker_identity(ada.identity_id()).ok(),
            Some(Some(ada.clone()))
        );
        database
            .delete_identity(grace_id)
            .unwrap_or_else(|error| panic!("identity should delete: {error}"));
        assert_eq!(database.list_speaker_identities().ok(), Some(vec![ada]));
    }

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
