//! Coordinates provider-neutral external meeting streams with local identity and attachment ports.

use uuid::Uuid;

use crate::{
    domain::{
        errors::{ExternalBridgeError, StorageError, TranscriptionError},
        models::{AudioRecording, DeviceId, MeetingId, RecordingId, VoiceEmbedding},
    },
    ports::{
        ai::TranscriptionEngine,
        external_bridge::{
            AttachmentWorkflow, ExternalIdentityRepository, IdentityMapper, MeetingBridge,
            VoiceEmbeddingExtractor,
        },
        voice::VoiceVectorDB,
    },
};

/// Summarizes provider-neutral external meeting work without exposing media or transcript content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TeamsProcessingResult {
    auto_enrolled_variant_ids: Vec<Uuid>,
    routed_slide_count: usize,
}

impl TeamsProcessingResult {
    /// Borrows IDs of automatic voice variants stored during this processing pass.
    ///
    /// Inputs: None.
    /// Outputs: Variant UUIDs only; never embeddings or audio.
    /// Side effects: None.
    pub fn auto_enrolled_variant_ids(&self) -> &[Uuid] {
        &self.auto_enrolled_variant_ids
    }

    /// Returns the number of slide frames routed to the attachment workflow.
    ///
    /// Inputs: None.
    /// Outputs: A count of successfully routed captures.
    /// Side effects: None.
    pub fn routed_slide_count(&self) -> usize {
        self.routed_slide_count
    }
}

/// Describes an adapter-neutral failure while processing external meeting data.
#[derive(Debug, thiserror::Error)]
pub enum TeamsProcessingError {
    /// The external bridge could not provide one of its portable subscriptions.
    #[error(transparent)]
    Bridge(#[from] ExternalBridgeError),
    /// An identity mapper, attachment workflow, or vector store could not persist state.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// The configured transcription engine rejected an external media chunk.
    #[error(transparent)]
    Transcription(#[from] TranscriptionError),
}

/// Implements stable external-ID mapping and auto-names new local identities from bridge metadata.
pub struct TeamsIdentityMapper<RepositoryPort> {
    external_identity_repository: RepositoryPort,
}

impl<RepositoryPort> TeamsIdentityMapper<RepositoryPort>
where
    RepositoryPort: ExternalIdentityRepository,
{
    /// Creates a mapping service backed by an injected local external-ID repository.
    ///
    /// Inputs: `external_identity_repository` resolves and persists portable mapping metadata.
    /// Outputs: A mapper ready to reuse or auto-create local identities.
    /// Side effects: Takes ownership of the port without querying it.
    pub fn new(external_identity_repository: RepositoryPort) -> Self {
        Self {
            external_identity_repository,
        }
    }
}

impl<RepositoryPort> IdentityMapper for TeamsIdentityMapper<RepositoryPort>
where
    RepositoryPort: ExternalIdentityRepository,
{
    /// Reuses a durable mapping or auto-creates an identity named from a new external participant.
    ///
    /// Inputs: `external` carries the stable mapping key and UI display name.
    /// Outputs: An existing identity or a new UUID identity whose initial name is the display name.
    /// Side effects: Persists a new external-ID mapping only for unseen external IDs.
    fn map_external_to_local(
        &self,
        external: crate::domain::models::ExternalParticipant,
    ) -> Result<crate::domain::models::SpeakerIdentity, StorageError> {
        if let Some(existing_identity) = self
            .external_identity_repository
            .find_by_external_id(external.external_id())?
        {
            return Ok(existing_identity);
        }
        let created_identity =
            crate::domain::models::SpeakerIdentity::new(Uuid::new_v4(), external.display_name());
        self.external_identity_repository
            .save_external_mapping(external.external_id(), &created_identity)?;
        tracing::info!(
            teams_id = %external.external_id(),
            name = %created_identity.display_name(),
            identity_id = %created_identity.identity_id(),
            "Auto-created local identity from external meeting participant"
        );
        Ok(created_identity)
    }
}

/// Auto-enrolls guaranteed external identities without provider-SDK knowledge.
pub struct TeamsProcessor<
    BridgePort,
    MapperPort,
    EmbeddingPort,
    VectorDatabasePort,
    TranscriptionPort,
    AttachmentPort,
> {
    meeting_bridge: BridgePort,
    identity_mapper: MapperPort,
    voice_embedding_extractor: EmbeddingPort,
    voice_vector_database: VectorDatabasePort,
    transcription_engine: TranscriptionPort,
    attachment_workflow: AttachmentPort,
    meeting_id: MeetingId,
    acquisition_device_id: DeviceId,
}

/// Groups injected external meeting capabilities before constructing one processor.
pub struct TeamsProcessorPorts<
    BridgePort,
    MapperPort,
    EmbeddingPort,
    VectorDatabasePort,
    TranscriptionPort,
    AttachmentPort,
> {
    meeting_bridge: BridgePort,
    identity_mapper: MapperPort,
    voice_embedding_extractor: EmbeddingPort,
    voice_vector_database: VectorDatabasePort,
    transcription_engine: TranscriptionPort,
    attachment_workflow: AttachmentPort,
}

impl<BridgePort, MapperPort, EmbeddingPort, VectorDatabasePort, TranscriptionPort, AttachmentPort>
    TeamsProcessorPorts<
        BridgePort,
        MapperPort,
        EmbeddingPort,
        VectorDatabasePort,
        TranscriptionPort,
        AttachmentPort,
    >
{
    /// Groups adapter-neutral dependencies without opening connections or processing data.
    ///
    /// Inputs: Each value implements one required bridge, identity, AI, vector, or attachment port.
    /// Outputs: A dependency bundle accepted by `TeamsProcessor::new`.
    /// Side effects: Takes ownership of the injected ports.
    pub fn new(
        meeting_bridge: BridgePort,
        identity_mapper: MapperPort,
        voice_embedding_extractor: EmbeddingPort,
        voice_vector_database: VectorDatabasePort,
        transcription_engine: TranscriptionPort,
        attachment_workflow: AttachmentPort,
    ) -> Self {
        Self {
            meeting_bridge,
            identity_mapper,
            voice_embedding_extractor,
            voice_vector_database,
            transcription_engine,
            attachment_workflow,
        }
    }
}

impl<BridgePort, MapperPort, EmbeddingPort, VectorDatabasePort, TranscriptionPort, AttachmentPort>
    TeamsProcessor<
        BridgePort,
        MapperPort,
        EmbeddingPort,
        VectorDatabasePort,
        TranscriptionPort,
        AttachmentPort,
    >
where
    BridgePort: MeetingBridge,
    MapperPort: IdentityMapper,
    EmbeddingPort: VoiceEmbeddingExtractor,
    VectorDatabasePort: VoiceVectorDB,
    TranscriptionPort: TranscriptionEngine,
    AttachmentPort: AttachmentWorkflow,
{
    /// Subscribes to current bridge streams, auto-enrolls identities, and routes slide captures.
    ///
    /// Inputs: None; meeting and device context were supplied at construction.
    /// Outputs: Safe processing counts and generated variant IDs, or a portable port error.
    /// Side effects: Maps/creates identities, stores biometric variants, invokes transcription, and
    /// routes screen frames through the attachment workflow. It never logs raw media or embeddings.
    pub fn process_available(&self) -> Result<TeamsProcessingResult, TeamsProcessingError> {
        let media_chunk_receiver = self.meeting_bridge.subscribe_media_chunks()?;
        let slide_capture_receiver = self.meeting_bridge.subscribe_slide_captures()?;
        let mut auto_enrolled_variant_ids = Vec::new();
        let mut routed_slide_count = 0;

        for media_chunk in media_chunk_receiver.try_iter() {
            let external_participant = media_chunk.external_participant();
            let local_identity = self
                .identity_mapper
                .map_external_to_local(external_participant.clone())?;
            let derived_embedding = self
                .voice_embedding_extractor
                .extract_embedding(&media_chunk)?;
            let variant_id = Uuid::new_v4();
            self.voice_vector_database
                .store_embedding(VoiceEmbedding::new(
                    local_identity.identity_id(),
                    variant_id,
                    derived_embedding,
                ))?;
            let recording_id = RecordingId::new(format!(
                "external-{}-{}",
                external_participant.external_id(),
                media_chunk.timestamp_ms()
            ));
            let audio_recording = AudioRecording::new(
                recording_id,
                self.meeting_id.clone(),
                self.acquisition_device_id.clone(),
            );
            self.transcription_engine
                .transcribe_external_media(&media_chunk, &audio_recording)?;
            tracing::info!(
                teams_id = %external_participant.external_id(),
                name = %local_identity.display_name(),
                identity_id = %local_identity.identity_id(),
                variant_id = %variant_id,
                "Auto-enrolled voice identity from external meeting bridge"
            );
            auto_enrolled_variant_ids.push(variant_id);
        }

        for slide_capture in slide_capture_receiver.try_iter() {
            self.attachment_workflow
                .handle_slide_capture(slide_capture)?;
            routed_slide_count += 1;
        }

        Ok(TeamsProcessingResult {
            auto_enrolled_variant_ids,
            routed_slide_count,
        })
    }
}

impl<BridgePort, MapperPort, EmbeddingPort, VectorDatabasePort, TranscriptionPort, AttachmentPort>
    TeamsProcessor<
        BridgePort,
        MapperPort,
        EmbeddingPort,
        VectorDatabasePort,
        TranscriptionPort,
        AttachmentPort,
    >
{
    /// Creates a processor from injected adapter-neutral bridge, identity, AI, and storage ports.
    ///
    /// Inputs: Ports own external integration details; `meeting_id` and `acquisition_device_id`
    /// identify locally attributed work.
    /// Outputs: A processor ready for deterministic stream-draining calls.
    /// Side effects: Takes ownership of ports but opens no bridge connection until processing.
    pub fn new(
        ports: TeamsProcessorPorts<
            BridgePort,
            MapperPort,
            EmbeddingPort,
            VectorDatabasePort,
            TranscriptionPort,
            AttachmentPort,
        >,
        meeting_id: MeetingId,
        acquisition_device_id: DeviceId,
    ) -> Self {
        Self {
            meeting_bridge: ports.meeting_bridge,
            identity_mapper: ports.identity_mapper,
            voice_embedding_extractor: ports.voice_embedding_extractor,
            voice_vector_database: ports.voice_vector_database,
            transcription_engine: ports.transcription_engine,
            attachment_workflow: ports.attachment_workflow,
            meeting_id,
            acquisition_device_id,
        }
    }
}
