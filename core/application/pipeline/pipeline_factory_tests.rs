//! Specifies the pipeline composition root: dependency availability, graceful degradation, and
//! meeting creation, using mock AI ports and an in-memory SQLite repository only.

use std::cell::Cell;

use uuid::Uuid;

use crate::{
    adapters::storage::sqlite_adapter::SqliteNoteRepository,
    application::{
        crypto::voice_encryptor::ephemeral_cipher,
        meeting_crdt::MeetingCrdtService,
        pipeline::{
            meeting_session::{SpeakerAttribution, VadAudioChunk},
            pipeline_factory::{PipelineAvailability, PipelineFactory},
        },
        unknown_speaker_clusterer::ClusteringConfig,
    },
    domain::{
        errors::{DiarizationError, StorageError, TranscriptionError},
        models::{
            AudioRecording, DeviceId, MeetingId, RecordingId, Transcript, TranscriptionContext,
            VoiceMatchResult,
        },
    },
    ports::{
        ai::{DiarizationEngine, SpeakerSegment, TranscriptionEngine},
        storage::NoteRepository,
        voice::VoiceIdentityMatcher,
    },
};

/// Returns the same transcript text for every call and counts the calls.
#[derive(Default)]
struct CountingTranscriptionEngine {
    call_count: Cell<usize>,
}

impl TranscriptionEngine for CountingTranscriptionEngine {
    /// Counts the call and returns a fixed transcript for the recording's meeting.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        _transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        self.call_count.set(self.call_count.get() + 1);
        Ok(Transcript::new(
            audio_recording.meeting_id().clone(),
            "hello team",
        ))
    }
}

/// Reports one speaker covering the first three seconds of every chunk.
struct SingleSpeakerDiarizationEngine;

#[async_trait::async_trait]
impl DiarizationEngine for SingleSpeakerDiarizationEngine {
    /// Returns one segment whose embedding the mock matcher treats as a strong match.
    async fn diarize(&self, _audio_bytes: &[u8]) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        Ok(vec![SpeakerSegment::new(0, 3_000, vec![1.0, 0.0])])
    }
}

/// Classifies every embedding as a strong match for one fixed identity.
struct StrongMatchVoiceMatcher {
    known_identity_id: Uuid,
}

impl VoiceIdentityMatcher for StrongMatchVoiceMatcher {
    /// Returns a strong match for the configured identity.
    fn match_embedding(&self, _embedding: &[f32]) -> Result<VoiceMatchResult, StorageError> {
        Ok(VoiceMatchResult::StrongMatch(self.known_identity_id))
    }
}

/// Opens an isolated in-memory repository with a throwaway embedding key.
fn in_memory_repository() -> SqliteNoteRepository {
    SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .unwrap_or_else(|error| panic!("in-memory repository should open: {error}"))
}

/// Builds a three-second VAD chunk at `start_offset_ms` for `meeting_id`.
fn vad_chunk(meeting_id: &str, sequence: u32, start_offset_ms: u64) -> VadAudioChunk {
    VadAudioChunk::new(
        AudioRecording::new(
            RecordingId::new(format!("recording-{sequence}")),
            MeetingId::new(meeting_id),
            DeviceId::new("device-test"),
        ),
        start_offset_ms,
        start_offset_ms + 3_000,
        vec![0, 1, 2, 3],
    )
    .unwrap_or_else(|error| panic!("chunk bounds should be valid: {error}"))
}

/// Verifies a fully composed factory yields sessions that attribute strong matches as Known.
#[tokio::test]
async fn factory_with_all_dependencies_produces_known_segments() {
    let known_identity_id = Uuid::new_v4();
    let factory = PipelineFactory::new(
        CountingTranscriptionEngine::default(),
        in_memory_repository(),
    )
    .with_diarization_engine(Some(SingleSpeakerDiarizationEngine))
    .with_voice_matcher(Some(StrongMatchVoiceMatcher { known_identity_id }));

    assert_eq!(
        factory.availability(),
        PipelineAvailability {
            diarization: true,
            voice_matching: true,
        }
    );
    let mut session = factory
        .create_session(&MeetingId::new("meeting-known"))
        .unwrap_or_else(|error| panic!("session should start: {error}"));
    let produced = session
        .process_vad_chunk(&vad_chunk("meeting-known", 1, 0))
        .await
        .unwrap_or_else(|error| panic!("chunk should process: {error}"));

    assert_eq!(produced.len(), 1);
    assert_eq!(
        produced[0].attribution(),
        &SpeakerAttribution::Known {
            identity_id: known_identity_id
        }
    );
}

/// Verifies a missing diarization engine degrades segments to Unattributed instead of failing.
#[tokio::test]
async fn factory_without_diarization_produces_unattributed_segments() {
    let factory = PipelineFactory::new(
        CountingTranscriptionEngine::default(),
        in_memory_repository(),
    )
    .with_voice_matcher(Some(StrongMatchVoiceMatcher {
        known_identity_id: Uuid::new_v4(),
    }));

    assert!(!factory.availability().diarization);
    let mut session = factory
        .create_session(&MeetingId::new("meeting-unattributed"))
        .unwrap_or_else(|error| panic!("session should start: {error}"));
    let produced = session
        .process_vad_chunk(&vad_chunk("meeting-unattributed", 1, 0))
        .await
        .unwrap_or_else(|error| panic!("chunk should process: {error}"));

    assert_eq!(produced.len(), 1);
    assert_eq!(produced[0].attribution(), &SpeakerAttribution::Unattributed);
    assert_eq!(produced[0].segment().speaker_identity_id(), None);
}

/// Verifies a missing voice matcher treats every diarized voice as unknown.
#[tokio::test]
async fn factory_without_voice_matcher_produces_unknown_segments() {
    let factory = PipelineFactory::new(
        CountingTranscriptionEngine::default(),
        in_memory_repository(),
    )
    .with_diarization_engine(Some(SingleSpeakerDiarizationEngine));

    assert_eq!(
        factory.availability(),
        PipelineAvailability {
            diarization: true,
            voice_matching: false,
        }
    );
    let mut session = factory
        .create_session(&MeetingId::new("meeting-unknown"))
        .unwrap_or_else(|error| panic!("session should start: {error}"));
    let produced = session
        .process_vad_chunk(&vad_chunk("meeting-unknown", 1, 0))
        .await
        .unwrap_or_else(|error| panic!("chunk should process: {error}"));

    assert_eq!(produced.len(), 1);
    assert!(matches!(
        produced[0].attribution(),
        SpeakerAttribution::Unknown { .. }
    ));
}

/// Verifies a session for an unknown meeting creates it, and segments persist under it.
#[tokio::test]
async fn create_session_creates_a_missing_meeting() {
    let factory = PipelineFactory::new(
        CountingTranscriptionEngine::default(),
        in_memory_repository(),
    );
    let meeting_id = MeetingId::new("meeting-new");
    assert!(
        factory
            .note_repository()
            .load_meeting_metadata(&meeting_id)
            .unwrap_or_else(|error| panic!("metadata read should succeed: {error}"))
            .is_none()
    );

    let mut session = factory
        .create_session(&meeting_id)
        .unwrap_or_else(|error| panic!("session should start: {error}"));
    session
        .process_vad_chunk(&vad_chunk("meeting-new", 1, 0))
        .await
        .unwrap_or_else(|error| panic!("chunk should process: {error}"));

    assert!(
        factory
            .note_repository()
            .load_meeting_metadata(&meeting_id)
            .unwrap_or_else(|error| panic!("metadata read should succeed: {error}"))
            .is_some()
    );
    let stored_segments = factory
        .note_repository()
        .load_transcript_segments(&meeting_id)
        .unwrap_or_else(|error| panic!("segments should load: {error}"));
    assert_eq!(stored_segments.len(), 1);
}

/// Verifies an existing meeting is reused: its CRDT note survives and new segments append.
#[tokio::test]
async fn create_session_reuses_an_existing_meeting_and_its_timeline() {
    let factory = PipelineFactory::new(
        CountingTranscriptionEngine::default(),
        in_memory_repository(),
    );
    let meeting_id = MeetingId::new("meeting-existing");
    let crdt_service = MeetingCrdtService::new(factory.note_repository());
    crdt_service
        .create_meeting(&meeting_id)
        .unwrap_or_else(|error| panic!("meeting should be created: {error}"));
    crdt_service
        .replace_note_text(&meeting_id, "agenda")
        .unwrap_or_else(|error| panic!("note should be written: {error}"));

    for (recording_index, start_offset_ms) in [0_u64, 10_000].into_iter().enumerate() {
        let mut session = factory
            .create_session(&meeting_id)
            .unwrap_or_else(|error| panic!("session should start: {error}"));
        session
            .process_vad_chunk(&vad_chunk(
                "meeting-existing",
                u32::try_from(recording_index).unwrap_or(u32::MAX),
                start_offset_ms,
            ))
            .await
            .unwrap_or_else(|error| panic!("chunk should process: {error}"));
    }

    let stored_segments = factory
        .note_repository()
        .load_transcript_segments(&meeting_id)
        .unwrap_or_else(|error| panic!("segments should load: {error}"));
    assert_eq!(stored_segments.len(), 2);
    assert_eq!(
        crdt_service
            .load_note_text(&meeting_id)
            .unwrap_or_else(|error| panic!("note should load: {error}")),
        Some("agenda".to_owned())
    );
}

/// Verifies sessions borrow the factory's engine instead of rebuilding it per recording.
#[tokio::test]
async fn sessions_reuse_the_factory_engines_across_recordings() {
    let factory = PipelineFactory::new(
        CountingTranscriptionEngine::default(),
        in_memory_repository(),
    );

    for meeting in ["meeting-a", "meeting-b"] {
        let mut session = factory
            .create_session(&MeetingId::new(meeting))
            .unwrap_or_else(|error| panic!("session should start: {error}"));
        session
            .process_vad_chunk(&vad_chunk(meeting, 1, 0))
            .await
            .unwrap_or_else(|error| panic!("chunk should process: {error}"));
    }

    assert_eq!(factory.transcription_engine().call_count.get(), 2);
}

/// Reports two speakers whose embeddings have cosine similarity 0.8.
struct TwoSimilarSpeakersDiarizationEngine;

#[async_trait::async_trait]
impl DiarizationEngine for TwoSimilarSpeakersDiarizationEngine {
    /// Returns two consecutive segments with embeddings [1, 0] and [0.8, 0.6].
    async fn diarize(&self, _audio_bytes: &[u8]) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        Ok(vec![
            SpeakerSegment::new(0, 1_500, vec![1.0, 0.0]),
            SpeakerSegment::new(1_500, 3_000, vec![0.8, 0.6]),
        ])
    }
}

/// Returns the cluster IDs of the two unknown voices in one processed chunk.
async fn cluster_ids_with(clustering_config: Option<ClusteringConfig>) -> Vec<Option<Uuid>> {
    let factory = PipelineFactory::new(
        CountingTranscriptionEngine::default(),
        in_memory_repository(),
    )
    .with_diarization_engine(Some(TwoSimilarSpeakersDiarizationEngine));
    let factory = match clustering_config {
        Some(clustering_config) => factory.with_clustering_config(clustering_config),
        None => factory,
    };
    let mut session = factory
        .create_session(&MeetingId::new("meeting-clusters"))
        .unwrap_or_else(|error| panic!("session should start: {error}"));
    session
        .process_vad_chunk(&vad_chunk("meeting-clusters", 1, 0))
        .await
        .unwrap_or_else(|error| panic!("chunk should process: {error}"))
        .iter()
        .map(|segment| match segment.attribution() {
            SpeakerAttribution::Unknown { cluster_id, .. } => cluster_id.to_owned(),
            other => panic!("expected an unknown voice, got {other:?}"),
        })
        .collect()
}

/// Verifies sessions use the factory's clustering policy: 0.8 similarity joins one cluster under
/// the default 0.40 threshold but founds two clusters under a 0.90 threshold.
#[tokio::test]
async fn sessions_use_the_factory_clustering_policy() {
    let default_clusters = cluster_ids_with(None).await;
    assert_eq!(default_clusters.len(), 2);
    assert!(default_clusters[0].is_some());
    assert_eq!(default_clusters[0], default_clusters[1]);

    let strict_config = ClusteringConfig::new(0.9)
        .unwrap_or_else(|error| panic!("threshold should be valid: {error}"));
    let strict_clusters = cluster_ids_with(Some(strict_config)).await;
    assert_eq!(strict_clusters.len(), 2);
    assert_ne!(strict_clusters[0], strict_clusters[1]);
}
