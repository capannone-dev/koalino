//! Declares provider-neutral external meeting media, identity, and attachment workflow contracts.

use std::sync::mpsc::Receiver;

use crate::domain::{
    errors::{ExternalBridgeError, StorageError},
    models::{ExternalMediaChunk, ExternalParticipant, SlideCapture, SpeakerIdentity},
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
