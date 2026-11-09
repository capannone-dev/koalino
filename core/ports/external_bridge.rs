//! Declares provider-neutral external meeting media, identity, and attachment workflow contracts.

use std::sync::mpsc::Receiver;

use crate::domain::{
    errors::{ExternalBridgeError, StorageError},
    models::{
        ExternalMediaChunk, ExternalParticipant, SlideCapture, SpeakerIdentity, VoiceEmbedding,
    },
};

/// Subscribes to provider-isolated external media without exposing provider SDK types.
pub trait MeetingBridge {
    /// Subscribes to externally isolated speaker-audio chunks.
    ///
    /// Inputs: None; bridge authentication and provider setup remain adapter responsibilities.
    /// Outputs: A standard receiver of portable chunks or a bridge subscription error.
    /// Side effects: May establish an adapter-managed external meeting subscription.
    fn subscribe_media_chunks(&self) -> Result<Receiver<ExternalMediaChunk>, ExternalBridgeError>;

    /// Subscribes to externally captured screen-share frames.
    ///
    /// Inputs: None; bridge authentication and provider setup remain adapter responsibilities.
    /// Outputs: A standard receiver of portable captures or a bridge subscription error.
    /// Side effects: May establish an adapter-managed external meeting subscription.
    fn subscribe_slide_captures(&self) -> Result<Receiver<SlideCapture>, ExternalBridgeError>;
}

/// Resolves a stable external participant ID to a local speaker identity.
pub trait IdentityMapper {
    /// Finds an existing local identity or creates one named from a new participant's display name.
    ///
    /// Inputs: `external` contains stable ID and UI name; implementations must key mappings by ID.
    /// Outputs: The mapped local identity or a portable persistence failure.
    /// Side effects: May create and persist an identity plus durable external-ID mapping.
    fn map_external_to_local(
        &self,
        external: ExternalParticipant,
    ) -> Result<SpeakerIdentity, StorageError>;

    /// Offers a participant's voice for the voice gallery (#103).
    ///
    /// The session offers a voice only from a track that carried one voice for long enough, and
    /// only when it does not sound like someone else in the gallery. Voice prints are biometric,
    /// so implementations keep it only when the owner opted in.
    /// Inputs: The participant's mapped `identity` and the voice's mean `embedding` (never logged).
    /// Outputs: `true` when the voice was kept, `false` when keeping is off (the default), or a
    /// storage failure.
    /// Side effects: May store one voice variant for the identity.
    fn keep_participant_voice(
        &self,
        identity: &SpeakerIdentity,
        embedding: Vec<f32>,
    ) -> Result<bool, StorageError> {
        let _ = (identity, embedding);
        Ok(false)
    }
}

/// Persists stable external-ID to local-identity associations without provider SDK knowledge.
pub trait ExternalIdentityRepository {
    /// Looks up the local identity associated with one stable external identifier.
    ///
    /// Inputs: `external_id` is a provider-stable, non-secret mapping key.
    /// Outputs: An existing local identity, `None` when unseen, or a storage failure.
    /// Side effects: Reads local mapping metadata only.
    fn find_by_external_id(
        &self,
        external_id: &str,
    ) -> Result<Option<SpeakerIdentity>, StorageError>;

    /// Stores a stable external-ID association for a newly created local identity.
    ///
    /// Inputs: `external_id` is stable; `identity` is the newly created local person record.
    /// Outputs: Success or a portable persistence failure.
    /// Side effects: Writes local mapping metadata; implementations must not store audio here.
    fn save_external_mapping(
        &self,
        external_id: &str,
        identity: &SpeakerIdentity,
    ) -> Result<(), StorageError>;

    /// Stores a participant's voice variant in the gallery (#103).
    ///
    /// Inputs: The embedding, whose identity must already be saved (the mapping saves it).
    /// Outputs: Success, or a storage failure; repositories without a gallery reject it.
    /// Side effects: Writes one encrypted voice variant; never logs the vector.
    fn store_participant_voice(&self, embedding: VoiceEmbedding) -> Result<(), StorageError> {
        let _ = embedding;
        Err(StorageError::Rejected {
            reason: String::from("this repository keeps no voices"),
        })
    }
}

impl<RepositoryPort> ExternalIdentityRepository for std::sync::Mutex<RepositoryPort>
where
    RepositoryPort: ExternalIdentityRepository,
{
    /// Locks the inner repository and looks up `external_id` (recovers from poisoning).
    ///
    /// Inputs: A provider-stable external ID.
    /// Outputs: The inner repository's answer.
    /// Side effects: Holds the lock for the duration of the read.
    fn find_by_external_id(
        &self,
        external_id: &str,
    ) -> Result<Option<SpeakerIdentity>, StorageError> {
        self.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .find_by_external_id(external_id)
    }

    /// Locks the inner repository and saves the mapping (recovers from poisoning).
    ///
    /// Inputs: A stable external ID and its identity.
    /// Outputs: The inner repository's answer.
    /// Side effects: Holds the lock for the duration of the write.
    fn save_external_mapping(
        &self,
        external_id: &str,
        identity: &SpeakerIdentity,
    ) -> Result<(), StorageError> {
        self.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .save_external_mapping(external_id, identity)
    }

    /// Locks the inner repository and stores the voice (recovers from poisoning).
    ///
    /// Inputs: The voice variant. Outputs: The inner repository's answer.
    /// Side effects: Holds the lock for the duration of the write.
    fn store_participant_voice(&self, embedding: VoiceEmbedding) -> Result<(), StorageError> {
        self.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .store_participant_voice(embedding)
    }
}

/// Derives a voice signature from an isolated external-speaker audio stream.
pub trait VoiceEmbeddingExtractor {
    /// Extracts one derived vector from bridge-provided isolated audio.
    ///
    /// Inputs: `media_chunk` contains raw audio and must never be logged.
    /// Outputs: A derived biometric vector or a portable bridge/AI boundary error.
    /// Side effects: May run an adapter-managed local or remote model.
    fn extract_embedding(
        &self,
        media_chunk: &ExternalMediaChunk,
    ) -> Result<Vec<f32>, ExternalBridgeError>;
}

/// Receives external slide captures for the attachment workflow.
pub trait AttachmentWorkflow {
    /// Routes a capture into the configured attachment manager implementation.
    ///
    /// Inputs: `slide_capture` contains opaque image bytes and bridge metadata.
    /// Outputs: Success or a portable persistence/workflow failure.
    /// Side effects: May retain a user-authorized slide attachment; must not log image bytes.
    fn handle_slide_capture(&self, slide_capture: SlideCapture) -> Result<(), StorageError>;
}
