//! Verifies the voice-identity manager's threshold policy and variant-creation contract.

use std::{cell::RefCell, rc::Rc};

use uuid::Uuid;

use crate::{
    application::voice_identity_manager::VoiceIdentityManager,
    domain::{
        errors::StorageError,
        models::{VoiceEmbedding, VoiceMatchResult},
    },
    ports::voice::VoiceVectorDB,
};

/// Supplies deterministic similarity-search responses and records stored variants for manager tests.
struct ScriptedVoiceVectorDatabase {
    search_results: Vec<(VoiceEmbedding, f32)>,
    stored_embeddings: Rc<RefCell<Vec<VoiceEmbedding>>>,
}

impl ScriptedVoiceVectorDatabase {
    /// Creates a test port with one fixed response to every similarity query.
    ///
    /// Inputs: `search_results` contains adapter-produced candidates and scores; `stored_embeddings`
    /// receives values written through the port.
    /// Outputs: A deterministic in-memory port double.
    /// Side effects: None until its trait methods are called.
    fn new(
        search_results: Vec<(VoiceEmbedding, f32)>,
        stored_embeddings: Rc<RefCell<Vec<VoiceEmbedding>>>,
    ) -> Self {
        Self {
            search_results,
            stored_embeddings,
        }
    }
}

impl VoiceVectorDB for ScriptedVoiceVectorDatabase {
    fn store_embedding(&self, embedding: VoiceEmbedding) -> Result<(), StorageError> {
        self.stored_embeddings.borrow_mut().push(embedding);
        Ok(())
    }

    fn search_similar(
        &self,
        _vector: &[f32],
        _top_k: usize,
    ) -> Result<Vec<(VoiceEmbedding, f32)>, StorageError> {
        Ok(self.search_results.clone())
    }

    fn delete_identity(&self, _identity_id: Uuid) -> Result<(), StorageError> {
        Ok(())
    }
}

/// Creates one non-sensitive embedding fixture for matching-policy tests.
fn embedding_for(identity_id: Uuid, variant_id: Uuid) -> VoiceEmbedding {
    VoiceEmbedding::new(identity_id, variant_id, vec![1.0, 0.0])
}

/// Builds a manager wired to a scripted adapter response and returns its write observation handle.
fn manager_with_result(
    search_results: Vec<(VoiceEmbedding, f32)>,
) -> (
    VoiceIdentityManager<ScriptedVoiceVectorDatabase>,
    Rc<RefCell<Vec<VoiceEmbedding>>>,
) {
    let stored_embeddings = Rc::new(RefCell::new(Vec::new()));
    let database = ScriptedVoiceVectorDatabase::new(search_results, Rc::clone(&stored_embeddings));
    (VoiceIdentityManager::new(database), stored_embeddings)
}

/// Verifies an exact similarity score is classified as a high-confidence identity match.
#[test]
fn exact_match_returns_strong_match() {
    let identity_id = Uuid::new_v4();
    let (manager, _) = manager_with_result(vec![(embedding_for(identity_id, Uuid::new_v4()), 1.0)]);

    let match_result = manager
        .match_embedding(&[1.0, 0.0])
        .unwrap_or_else(|error| panic!("matching should succeed: {error}"));

    assert_eq!(match_result, VoiceMatchResult::StrongMatch(identity_id));
}

/// Verifies a score in the ambiguous range prompts the caller to seek confirmation.
#[test]
fn slightly_altered_vector_returns_weak_match() {
    let identity_id = Uuid::new_v4();
    let (manager, _) =
        manager_with_result(vec![(embedding_for(identity_id, Uuid::new_v4()), 0.60)]);

    let match_result = manager
        .match_embedding(&[0.60, 0.80])
        .unwrap_or_else(|error| panic!("matching should succeed: {error}"));

    assert_eq!(match_result, VoiceMatchResult::WeakMatch(identity_id, 0.60));
}

/// Verifies equality at the strong threshold remains weak and requires user confirmation.
#[test]
fn strong_threshold_boundary_returns_weak_match() {
    let identity_id = Uuid::new_v4();
    let (manager, _) =
        manager_with_result(vec![(embedding_for(identity_id, Uuid::new_v4()), 0.80)]);

    let match_result = manager
        .match_embedding(&[0.80, 0.60])
        .unwrap_or_else(|error| panic!("matching should succeed: {error}"));

    assert_eq!(match_result, VoiceMatchResult::WeakMatch(identity_id, 0.80));
}

/// Verifies equality at the weak threshold is retained as a candidate for manual confirmation.
#[test]
fn weak_threshold_boundary_returns_weak_match() {
    let identity_id = Uuid::new_v4();
    let (manager, _) =
        manager_with_result(vec![(embedding_for(identity_id, Uuid::new_v4()), 0.50)]);

    let match_result = manager
        .match_embedding(&[0.50, 0.866_025_4])
        .unwrap_or_else(|error| panic!("matching should succeed: {error}"));

    assert_eq!(match_result, VoiceMatchResult::WeakMatch(identity_id, 0.50));
}

/// Verifies low-confidence and absent candidates do not attach a voice to an identity.
#[test]
fn random_or_absent_vector_returns_no_match() {
    let identity_id = Uuid::new_v4();
    let (low_score_manager, _) =
        manager_with_result(vec![(embedding_for(identity_id, Uuid::new_v4()), 0.49)]);
    let (empty_manager, _) = manager_with_result(Vec::new());

    let low_score_result = low_score_manager
        .match_embedding(&[0.0, 1.0])
        .unwrap_or_else(|error| panic!("matching should succeed: {error}"));
    let empty_result = empty_manager
        .match_embedding(&[0.0, 1.0])
        .unwrap_or_else(|error| panic!("matching should succeed: {error}"));

    assert_eq!(low_score_result, VoiceMatchResult::NoMatch);
    assert_eq!(empty_result, VoiceMatchResult::NoMatch);
}

/// Verifies confirmed weak matches add a new, distinct signature variant to the existing identity.
#[test]
fn confirm_weak_match_adds_second_variant_for_existing_identity() {
    let identity_id = Uuid::new_v4();
    let existing_variant_id = Uuid::new_v4();
    let existing_embedding = embedding_for(identity_id, existing_variant_id);
    let (manager, stored_embeddings) = manager_with_result(Vec::new());
    stored_embeddings
        .borrow_mut()
        .push(existing_embedding.clone());

    let created_variant_id = manager
        .confirm_weak_match(identity_id, &[0.60, 0.80])
        .unwrap_or_else(|error| panic!("confirmation should store a new variant: {error}"));

    let persisted_embeddings = stored_embeddings.borrow();
    assert_eq!(persisted_embeddings.len(), 2);
    let stored_embedding = persisted_embeddings
        .iter()
        .find(|embedding| embedding.variant_id() == created_variant_id)
        .unwrap_or_else(|| panic!("the confirmed variant should be stored"));
    assert_eq!(stored_embedding.identity_id(), identity_id);
    assert_ne!(
        stored_embedding.variant_id(),
        existing_embedding.variant_id()
    );
    assert_eq!(stored_embedding.vector(), &[0.60, 0.80]);
}
