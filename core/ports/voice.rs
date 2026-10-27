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

    /// Creates a speaker identity, or renames it when its UUID is already stored.
    ///
    /// Inputs: `identity` carries a stable UUID and a non-blank user-visible display name.
    /// Outputs: Success or a portable storage error; the default reports `Unavailable`.
    /// Side effects: Persists display metadata only (no biometric data); a rename keeps the
    /// identity's creation time and voice variants.
    fn save_speaker_identity(&self, _identity: &SpeakerIdentity) -> Result<(), StorageError> {
        Err(StorageError::Unavailable)
    }

    /// Loads one speaker identity by UUID.
    ///
    /// Inputs: `identity_id` identifies the person.
    /// Outputs: The identity, `None` when it is not stored, or a portable storage error. The
    /// default searches `list_speaker_identities`.
    /// Side effects: Reads identity metadata only.
    fn load_speaker_identity(
        &self,
        identity_id: Uuid,
    ) -> Result<Option<SpeakerIdentity>, StorageError> {
        Ok(self
            .list_speaker_identities()?
            .into_iter()
            .find(|identity| identity.identity_id() == identity_id))
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

/// Forwards immutable voice-storage calls through references for application-service composition.
impl<T> VoiceVectorDB for &T
where
    T: VoiceVectorDB + ?Sized,
{
    fn list_speaker_identities(&self) -> Result<Vec<SpeakerIdentity>, StorageError> {
        (*self).list_speaker_identities()
    }

    fn save_speaker_identity(&self, identity: &SpeakerIdentity) -> Result<(), StorageError> {
        (*self).save_speaker_identity(identity)
    }

    fn load_speaker_identity(
        &self,
        identity_id: Uuid,
    ) -> Result<Option<SpeakerIdentity>, StorageError> {
        (*self).load_speaker_identity(identity_id)
    }

    fn voice_variant_count(&self, identity_id: Uuid) -> Result<usize, StorageError> {
        (*self).voice_variant_count(identity_id)
    }

    fn store_embedding(&self, embedding: VoiceEmbedding) -> Result<(), StorageError> {
        (*self).store_embedding(embedding)
    }

    fn search_similar(
        &self,
        vector: &[f32],
        top_k: usize,
    ) -> Result<Vec<(VoiceEmbedding, f32)>, StorageError> {
        (*self).search_similar(vector, top_k)
    }

    fn delete_identity(&self, identity_id: Uuid) -> Result<(), StorageError> {
        (*self).delete_identity(identity_id)
    }
}
