//! Specifies mock-only Teams identity enrollment and attachment-routing behavior before implementation.

use std::{
    cell::RefCell,
    rc::Rc,
    sync::mpsc::{self, Receiver},
};

use uuid::Uuid;

use crate::{
    application::teams_processor::{TeamsIdentityMapper, TeamsProcessor, TeamsProcessorPorts},
    domain::{
        errors::{ExternalBridgeError, StorageError, TranscriptionError},
        models::{
            AudioRecording, DeviceId, ExternalMediaChunk, ExternalParticipant, MeetingId,
            SlideCapture, SpeakerIdentity, Transcript, VoiceEmbedding,
        },
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

/// Supplies deterministic bridge events without calling an external meeting service.
struct MockMeetingBridge {
    media_chunks: Vec<ExternalMediaChunk>,
    slide_captures: Vec<SlideCapture>,
}

impl MeetingBridge for MockMeetingBridge {
    /// Returns a fixture audio stream as if delivered by an authenticated external meeting bridge.
    fn subscribe_media_chunks(&self) -> Result<Receiver<ExternalMediaChunk>, ExternalBridgeError> {
        let (media_sender, media_receiver) = mpsc::channel();
        for media_chunk in &self.media_chunks {
            media_sender
                .send(media_chunk.clone())
                .unwrap_or_else(|error| panic!("fixture receiver should accept media: {error}"));
        }
        Ok(media_receiver)
    }

    /// Returns a fixture slide stream without reading a network connection.
    fn subscribe_slide_captures(&self) -> Result<Receiver<SlideCapture>, ExternalBridgeError> {
        let (slide_sender, slide_receiver) = mpsc::channel();
        for slide_capture in &self.slide_captures {
            slide_sender
                .send(slide_capture.clone())
                .unwrap_or_else(|error| panic!("fixture receiver should accept slides: {error}"));
        }
        Ok(slide_receiver)
    }
}

/// Records external values supplied for mapping and returns a scripted local identity.
struct MockIdentityMapper {
    mapped_participants: Rc<RefCell<Vec<ExternalParticipant>>>,
    mapped_identity: SpeakerIdentity,
}

/// Emulates durable external-ID mappings without touching a database.
struct MockExternalIdentityRepository {
    existing_identity: Option<SpeakerIdentity>,
    saved_mappings: Rc<RefCell<Vec<(String, SpeakerIdentity)>>>,
}

impl ExternalIdentityRepository for MockExternalIdentityRepository {
    /// Returns the configured existing identity for each stable external ID lookup.
    fn find_by_external_id(
        &self,
        _external_id: &str,
    ) -> Result<Option<SpeakerIdentity>, StorageError> {
        Ok(self.existing_identity.clone())
    }

    /// Records the mapping creation request for assertions.
    fn save_external_mapping(
        &self,
        external_id: &str,
        identity: &SpeakerIdentity,
    ) -> Result<(), StorageError> {
        self.saved_mappings
            .borrow_mut()
            .push((String::from(external_id), identity.clone()));
        Ok(())
    }
}

impl IdentityMapper for MockIdentityMapper {
    /// Records the stable external identifier and display name that enrollment delegates to mapping.
    fn map_external_to_local(
        &self,
        external: ExternalParticipant,
    ) -> Result<SpeakerIdentity, StorageError> {
        self.mapped_participants.borrow_mut().push(external);
        Ok(self.mapped_identity.clone())
    }
}

/// Returns one deterministic embedding without loading a voice model.
struct MockVoiceEmbeddingExtractor {
    embedding: Vec<f32>,
}

impl VoiceEmbeddingExtractor for MockVoiceEmbeddingExtractor {
    /// Returns configured derived features without retaining or logging raw audio.
    fn extract_embedding(
        &self,
        _media_chunk: &ExternalMediaChunk,
    ) -> Result<Vec<f32>, ExternalBridgeError> {
        Ok(self.embedding.clone())
    }
}

/// Records automatic vector enrollment requests without using a vector database.
///
/// Like `SqliteVoiceVectorDB`, it rejects variants whose identity was never saved.
struct MockVoiceVectorDatabase {
    stored_embeddings: Rc<RefCell<Vec<VoiceEmbedding>>>,
    saved_identities: RefCell<Vec<SpeakerIdentity>>,
}

impl VoiceVectorDB for MockVoiceVectorDatabase {
    /// Lists identities saved by the processor.
    fn list_speaker_identities(&self) -> Result<Vec<SpeakerIdentity>, StorageError> {
        Ok(self.saved_identities.borrow().clone())
    }

    /// Records a saved identity for the foreign-key check in `store_embedding`.
    fn save_speaker_identity(&self, identity: &SpeakerIdentity) -> Result<(), StorageError> {
        self.saved_identities.borrow_mut().push(identity.clone());
        Ok(())
    }

    /// Records the enrollment variant for assertions, rejecting unsaved identities.
    fn store_embedding(&self, embedding: VoiceEmbedding) -> Result<(), StorageError> {
        if !self
            .saved_identities
            .borrow()
            .iter()
            .any(|identity| identity.identity_id() == embedding.identity_id())
        {
            return Err(StorageError::Rejected {
                reason: format!(
                    "speaker identity {} does not exist",
                    embedding.identity_id()
                ),
            });
        }
        self.stored_embeddings.borrow_mut().push(embedding);
        Ok(())
    }

    /// Returns no matches because these tests exercise direct external identity mapping.
    fn search_similar(
        &self,
        _vector: &[f32],
        _top_k: usize,
    ) -> Result<Vec<(VoiceEmbedding, f32)>, StorageError> {
        Ok(Vec::new())
    }

    /// Does nothing because enrollment tests never delete an identity.
    fn delete_identity(&self, _identity_id: Uuid) -> Result<(), StorageError> {
        Ok(())
    }
}

/// Records external-audio forwarding without starting transcription infrastructure.
struct MockTranscriptionEngine {
    forwarded_audio: Rc<RefCell<Vec<Vec<u8>>>>,
    meeting_id: MeetingId,
}

impl TranscriptionEngine for MockTranscriptionEngine {
    /// Returns a minimal transcript for legacy recording calls unused by this specification.
    fn transcribe(
        &self,
        _audio_recording: &AudioRecording,
        _transcription_context: &crate::domain::models::TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        Ok(Transcript::new(self.meeting_id.clone(), "unused"))
    }

    /// Records external audio passed from the processor and returns a meeting-owned transcript.
    fn transcribe_external_media(
        &self,
        media_chunk: &ExternalMediaChunk,
        _audio_recording: &AudioRecording,
    ) -> Result<Transcript, TranscriptionError> {
        self.forwarded_audio
            .borrow_mut()
            .push(media_chunk.audio_bytes().to_vec());
        Ok(Transcript::new(
            self.meeting_id.clone(),
            "external transcript",
        ))
    }
}

/// Records slide routing without writing image bytes to a filesystem or attachment provider.
struct MockAttachmentWorkflow {
    routed_slides: Rc<RefCell<Vec<SlideCapture>>>,
}

impl AttachmentWorkflow for MockAttachmentWorkflow {
    /// Records one slide capture for routing assertions.
    fn handle_slide_capture(&self, slide_capture: SlideCapture) -> Result<(), StorageError> {
        self.routed_slides.borrow_mut().push(slide_capture);
        Ok(())
    }
}

/// Creates one isolated-media fixture with stable external and UI identity metadata.
fn media_chunk_for(external_id: &str, display_name: &str) -> ExternalMediaChunk {
    ExternalMediaChunk::new(
        ExternalParticipant::new(
            external_id,
            display_name,
            Some(String::from("person@example.test")),
        ),
        vec![1, 2, 3],
        1_250,
    )
}

/// Proves an unseen stable external ID creates a UUID local identity named from the display label.
#[test]
fn identity_mapper_auto_creates_and_names_an_unseen_external_participant() {
    let saved_mappings = Rc::new(RefCell::new(Vec::new()));
    let mapper = TeamsIdentityMapper::new(MockExternalIdentityRepository {
        existing_identity: None,
        saved_mappings: Rc::clone(&saved_mappings),
    });
    let external_participant = ExternalParticipant::new("teams-new-identity", "Mario Rossi", None);

    let mapped_identity = mapper
        .map_external_to_local(external_participant)
        .unwrap_or_else(|error| panic!("new identity mapping should succeed: {error}"));

    assert_eq!(mapped_identity.display_name(), "Mario Rossi");
    let saved = saved_mappings.borrow();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].0, "teams-new-identity");
    assert_eq!(saved[0].1, mapped_identity);
}

/// Proves an existing stable external ID returns its local name without creating a replacement.
#[test]
fn identity_mapper_returns_existing_identity_without_renaming_or_remapping() {
    let existing_identity = SpeakerIdentity::new(Uuid::new_v4(), "Existing Local Name");
    let saved_mappings = Rc::new(RefCell::new(Vec::new()));
    let mapper = TeamsIdentityMapper::new(MockExternalIdentityRepository {
        existing_identity: Some(existing_identity.clone()),
        saved_mappings: Rc::clone(&saved_mappings),
    });

    let mapped_identity = mapper
        .map_external_to_local(ExternalParticipant::new(
            "teams-known-identity",
            "Changed External Name",
            None,
        ))
        .unwrap_or_else(|error| panic!("existing identity mapping should succeed: {error}"));

    assert_eq!(mapped_identity, existing_identity);
    assert_eq!(mapped_identity.display_name(), "Existing Local Name");
    assert!(saved_mappings.borrow().is_empty());
}

/// Builds the processor and observation handles required by enrollment tests.
type TestTeamsProcessor = TeamsProcessor<
    MockMeetingBridge,
    MockIdentityMapper,
    MockVoiceEmbeddingExtractor,
    MockVoiceVectorDatabase,
    MockTranscriptionEngine,
    MockAttachmentWorkflow,
>;

/// Groups observation handles returned by the processor fixture.
type ProcessorFixture = (
    TestTeamsProcessor,
    Rc<RefCell<Vec<ExternalParticipant>>>,
    Rc<RefCell<Vec<VoiceEmbedding>>>,
    Rc<RefCell<Vec<Vec<u8>>>>,
    Rc<RefCell<Vec<SlideCapture>>>,
);

fn processor_for(
    media_chunks: Vec<ExternalMediaChunk>,
    slide_captures: Vec<SlideCapture>,
    mapped_identity: SpeakerIdentity,
) -> ProcessorFixture {
    let mapped_participants = Rc::new(RefCell::new(Vec::new()));
    let stored_embeddings = Rc::new(RefCell::new(Vec::new()));
    let forwarded_audio = Rc::new(RefCell::new(Vec::new()));
    let routed_slides = Rc::new(RefCell::new(Vec::new()));
    let meeting_id = MeetingId::new("teams-meeting-1");
    let ports = TeamsProcessorPorts::new(
        MockMeetingBridge {
            media_chunks,
            slide_captures,
        },
        MockIdentityMapper {
            mapped_participants: Rc::clone(&mapped_participants),
            mapped_identity,
        },
        MockVoiceEmbeddingExtractor {
            embedding: vec![0.6, 0.8],
        },
        MockVoiceVectorDatabase {
            stored_embeddings: Rc::clone(&stored_embeddings),
            saved_identities: RefCell::new(Vec::new()),
        },
        MockTranscriptionEngine {
            forwarded_audio: Rc::clone(&forwarded_audio),
            meeting_id: meeting_id.clone(),
        },
        MockAttachmentWorkflow {
            routed_slides: Rc::clone(&routed_slides),
        },
    );
    let processor = TeamsProcessor::new(ports, meeting_id, DeviceId::new("teams-bridge-device"));
    (
        processor,
        mapped_participants,
        stored_embeddings,
        forwarded_audio,
        routed_slides,
    )
}

/// Proves a new stable Teams ID delegates creation with the supplied display name and enrolls a variant.
#[test]
fn new_external_participant_uses_display_name_and_auto_enrolls_embedding() {
    let new_identity = SpeakerIdentity::new(Uuid::new_v4(), "Mario Rossi");
    let (processor, mapped_participants, stored_embeddings, forwarded_audio, _) = processor_for(
        vec![media_chunk_for("teams-user-100", "Mario Rossi")],
        Vec::new(),
        new_identity.clone(),
    );

    let processing_result = processor
        .process_available()
        .unwrap_or_else(|error| panic!("new participant should process: {error}"));

    let mapped = mapped_participants.borrow();
    assert_eq!(mapped.len(), 1);
    assert_eq!(mapped[0].external_id(), "teams-user-100");
    assert_eq!(mapped[0].display_name(), "Mario Rossi");
    assert_eq!(new_identity.display_name(), "Mario Rossi");
    let enrolled = stored_embeddings.borrow();
    assert_eq!(enrolled.len(), 1);
    assert_eq!(enrolled[0].identity_id(), new_identity.identity_id());
    assert_eq!(enrolled[0].vector(), &[0.6, 0.8]);
    assert_eq!(forwarded_audio.borrow().as_slice(), &[vec![1, 2, 3]]);
    assert_eq!(processing_result.auto_enrolled_variant_ids().len(), 1);
}

/// Proves known external IDs retain their mapped local identity and receive a distinct new variant.
#[test]
fn existing_external_participant_adds_variant_without_renaming_identity() {
    let existing_identity = SpeakerIdentity::new(Uuid::new_v4(), "Existing Local Name");
    let (processor, mapped_participants, stored_embeddings, _, _) = processor_for(
        vec![media_chunk_for(
            "teams-user-known",
            "Updated Teams Display Name",
        )],
        Vec::new(),
        existing_identity.clone(),
    );

    processor
        .process_available()
        .unwrap_or_else(|error| panic!("existing participant should process: {error}"));

    assert_eq!(
        mapped_participants.borrow()[0].external_id(),
        "teams-user-known"
    );
    assert_eq!(existing_identity.display_name(), "Existing Local Name");
    assert_eq!(
        stored_embeddings.borrow()[0].identity_id(),
        existing_identity.identity_id()
    );
}

/// Proves bridge-provided screen-share frames route to the attachment workflow without external APIs.
#[test]
fn slide_capture_routes_to_attachment_workflow() {
    let slide_capture =
        SlideCapture::new(Uuid::new_v4(), vec![9, 8, 7], 7_500, "teams-screen-share");
    let (processor, _, _, _, routed_slides) = processor_for(
        Vec::new(),
        vec![slide_capture.clone()],
        SpeakerIdentity::new(Uuid::new_v4(), "Unused"),
    );

    let processing_result = processor
        .process_available()
        .unwrap_or_else(|error| panic!("slide capture should route: {error}"));

    assert_eq!(routed_slides.borrow().as_slice(), &[slide_capture]);
    assert_eq!(processing_result.routed_slide_count(), 1);
}
