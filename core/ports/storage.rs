//! Declares persistence ports so use cases never depend directly on SQLite or CRDT libraries.

use crate::domain::{errors::StorageError, models::Transcript};

/// Persists meeting text in an offline-capable repository.
pub trait NoteRepository {
    /// Stores a transcript transactionally according to the adapter's synchronization strategy.
    ///
    /// Inputs: `transcript` must contain normalized content and its owning meeting identifier.
    /// Outputs: Success or a portable storage error.
    /// Side effects: An implementation writes encrypted local state and may enqueue CRDT sync.
    fn save_transcript(&self, transcript: &Transcript) -> Result<(), StorageError>;
}

/// Stores encrypted voice-vector metadata without accepting raw audio as input.
pub trait VoiceVectorDatabase {
    /// Persists an already-derived voice vector in an adapter-owned encrypted store.
    ///
    /// Inputs: `identity_reference` and `embedding` must not contain raw audio data.
    /// Outputs: Success or a portable storage error.
    /// Side effects: An implementation encrypts and writes biometric data at rest.
    fn save_embedding(
        &self,
        identity_reference: &str,
        embedding: &[f32],
    ) -> Result<(), StorageError>;
}
