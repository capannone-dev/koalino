//! Specifies end-to-end meeting pipeline orchestration with deterministic port mocks only.
//!
//! No test loads an AI model: transcription, diarization, identity matching, and identity mapping
//! are scripted mocks, and the integration test uses only in-memory SQLite and vector storage.

use std::{
    cell::{Cell, RefCell},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use uuid::Uuid;

use crate::{
    adapters::storage::{
        in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
    },
    application::{
        pipeline::{
            alignment::{
                ChunkBounds, SpeakerRegion, align_chunk_regions, align_timed_words,
                align_transcript,
            },
            meeting_session::{
                MeetingSession, MeetingSessionError, SpeakerAttribution, VadAudioChunk,
            },
        },
        unknown_speaker_clusterer::{ClusteringConfig, UnknownSpeakerClusterer},
        voice_identity_manager::VoiceIdentityManager,
    },
    domain::{
        errors::{DiarizationError, StorageError, TranscriptionError},
        models::{
            AudioRecording, DeviceId, ExternalMediaChunk, ExternalParticipant, MeetingId,
            RecordingId, SpeakerIdentity, TimedWord, Transcript, TranscriptSegment,
            TranscriptionContext, UnassignedVoiceClip, VoiceEmbedding, VoiceMatchResult,
        },
        transcription_language::TranscriptionLanguage,
    },
    ports::{
        ai::{DiarizationEngine, SpeakerSegment, TranscriptionEngine},
        external_bridge::IdentityMapper,
        storage::NoteRepository,
        voice::{VoiceIdentityMatcher, VoiceVectorDB},
    },
};

const MEETING_ID: &str = "meeting-pipeline";

/// Returns scripted transcripts in call order, or a scripted failure for every call.
struct MockTranscriptionEngine {
    scripted_texts: RefCell<Vec<&'static str>>,
    fails: bool,
    /// The previous text each `transcribe` call received, in call order.
    received_previous_texts: RefCell<Vec<Option<String>>>,
    received_languages: RefCell<Vec<Option<TranscriptionLanguage>>>,
}

impl MockTranscriptionEngine {
    /// Creates an engine that returns each scripted text once, in order.
    fn returning(scripted_texts: &[&'static str]) -> Self {
        Self {
            scripted_texts: RefCell::new(scripted_texts.iter().rev().copied().collect()),
            fails: false,
            received_previous_texts: RefCell::new(Vec::new()),
            received_languages: RefCell::new(Vec::new()),
        }
    }

    /// Creates an engine whose every call fails as unavailable.
    fn failing() -> Self {
        Self {
            scripted_texts: RefCell::new(Vec::new()),
            fails: true,
            received_previous_texts: RefCell::new(Vec::new()),
            received_languages: RefCell::new(Vec::new()),
        }
    }

    /// Pops the next scripted transcript for the recording's meeting.
    fn next_transcript(&self, meeting_id: &MeetingId) -> Result<Transcript, TranscriptionError> {
        if self.fails {
            return Err(TranscriptionError::Unavailable);
        }
        let next_text = self
            .scripted_texts
            .borrow_mut()
            .pop()
            .ok_or(TranscriptionError::Unavailable)?;
        Ok(Transcript::new(meeting_id.clone(), next_text))
    }
}

impl TranscriptionEngine for MockTranscriptionEngine {
    /// Returns the next scripted transcript for a local recording.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        self.received_previous_texts
            .borrow_mut()
            .push(transcription_context.previous_text().map(str::to_owned));
        self.received_languages
            .borrow_mut()
            .push(transcription_context.language());
        self.next_transcript(audio_recording.meeting_id())
    }

    /// Returns the next scripted transcript for an external chunk.
    fn transcribe_external_media(
        &self,
        _media_chunk: &ExternalMediaChunk,
        audio_recording: &AudioRecording,
    ) -> Result<Transcript, TranscriptionError> {
        self.next_transcript(audio_recording.meeting_id())
    }
}

/// Returns scripted diarization output per call and counts invocations.
struct MockDiarizationEngine {
    scripted_results: Mutex<Vec<Result<Vec<SpeakerSegment>, DiarizationError>>>,
    call_count: Arc<AtomicUsize>,
}

impl MockDiarizationEngine {
    /// Creates an engine returning each scripted result once, in order.
    fn scripted(
        scripted_results: Vec<Result<Vec<SpeakerSegment>, DiarizationError>>,
        call_count: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            scripted_results: Mutex::new(scripted_results.into_iter().rev().collect()),
            call_count,
        }
    }
}

#[async_trait::async_trait]
impl DiarizationEngine for MockDiarizationEngine {
    /// Records the call and pops the next scripted result.
    async fn diarize(&self, _audio_bytes: &[u8]) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        self.scripted_results
            .lock()
            .map_err(|_| DiarizationError::Unavailable)?
            .pop()
            .unwrap_or(Err(DiarizationError::Unavailable))
    }

    /// Reports no voice windows, so the final pass leaves the live result alone.
    async fn voice_windows(
        &self,
        _audio_bytes: &[u8],
    ) -> Result<Vec<crate::ports::ai::SpeakerSegment>, crate::domain::errors::DiarizationError>
    {
        Ok(Vec::new())
    }
}

/// Classifies embeddings by their first component so each test controls the outcome per segment.
struct MockVoiceIdentityMatcher {
    known_identity_id: Uuid,
    weak_candidate_id: Uuid,
}

impl VoiceIdentityMatcher for MockVoiceIdentityMatcher {
    /// Maps 1.0 → strong, 0.5 → weak, -1.0 → storage failure, anything else → no match.
    fn match_embedding(&self, embedding: &[f32]) -> Result<VoiceMatchResult, StorageError> {
        match embedding.first().copied() {
            Some(value) if value >= 1.0 => {
                Ok(VoiceMatchResult::StrongMatch(self.known_identity_id))
            }
            Some(value) if (value - 0.5).abs() < f32::EPSILON => {
                Ok(VoiceMatchResult::WeakMatch(self.weak_candidate_id, 0.6))
            }
            Some(value) if value < 0.0 => Err(StorageError::Unavailable),
            _ => Ok(VoiceMatchResult::NoMatch),
        }
    }
}

/// Captures persisted segments and review clips for assertions.
#[derive(Default)]
struct MockNoteRepository {
    saved_segments: RefCell<Vec<TranscriptSegment>>,
    segment_save_count: Cell<usize>,
    saved_clips: RefCell<Vec<UnassignedVoiceClip>>,
    /// Each recorded transcription language with the offset it was first used at.
    recorded_languages: RefCell<Vec<(TranscriptionLanguage, u64)>>,
}

impl NoteRepository for MockNoteRepository {
    /// Captures the language for assertions.
    fn record_transcription_language(
        &self,
        _meeting_id: &MeetingId,
        language: TranscriptionLanguage,
        first_used_at_ms: u64,
    ) -> Result<(), StorageError> {
        self.recorded_languages
            .borrow_mut()
            .push((language, first_used_at_ms));
        Ok(())
    }

    /// Accepts meeting creation without state because sessions require an existing meeting.
    fn create_meeting(
        &self,
        _meeting_id: &MeetingId,
        _document: &[u8],
    ) -> Result<(), StorageError> {
        Ok(())
    }
    /// Accepts flat transcripts without state; the pipeline persists segments instead.
    fn save_transcript(&self, _transcript: &Transcript) -> Result<(), StorageError> {
        Ok(())
    }
    /// Returns no flat transcript.
    fn load_transcript(&self, _meeting_id: &MeetingId) -> Result<Option<Transcript>, StorageError> {
        Ok(None)
    }
    /// Accepts CRDT documents without state.
    fn save_crdt_document(
        &self,
        _meeting_id: &MeetingId,
        _document: &[u8],
    ) -> Result<(), StorageError> {
        Ok(())
    }
    /// Returns an empty CRDT document.
    fn load_crdt_document(&self, _meeting_id: &MeetingId) -> Result<Vec<u8>, StorageError> {
        Ok(Vec::new())
    }
    /// Captures a manual-review clip.
    fn save_unassigned_clip(&self, clip: UnassignedVoiceClip) -> Result<(), StorageError> {
        self.saved_clips.borrow_mut().push(clip);
        Ok(())
    }
    /// Returns captured clips.
    fn get_unassigned_clips(
        &self,
        _meeting_id: &MeetingId,
    ) -> Result<Vec<UnassignedVoiceClip>, StorageError> {
        Ok(self.saved_clips.borrow().clone())
    }
    /// Replaces the captured segment timeline, mirroring the SQLite contract.
    fn save_transcript_segments(
        &self,
        _meeting_id: &MeetingId,
        segments: &[TranscriptSegment],
    ) -> Result<(), StorageError> {
        self.segment_save_count
            .set(self.segment_save_count.get() + 1);
        *self.saved_segments.borrow_mut() = segments.to_vec();
        Ok(())
    }
    /// Returns the captured segment timeline.
    fn load_transcript_segments(
        &self,
        _meeting_id: &MeetingId,
    ) -> Result<Vec<TranscriptSegment>, StorageError> {
        Ok(self.saved_segments.borrow().clone())
    }
}

/// Maps every external participant to one fixed local identity, or fails on demand.
struct MockIdentityMapper {
    mapped_identity: SpeakerIdentity,
    fails: bool,
}

impl IdentityMapper for MockIdentityMapper {
    /// Returns the fixed mapped identity or a storage failure.
    fn map_external_to_local(
        &self,
        _external: ExternalParticipant,
    ) -> Result<SpeakerIdentity, StorageError> {
        if self.fails {
            return Err(StorageError::Unavailable);
        }
        Ok(self.mapped_identity.clone())
    }
}

type MockSession = MeetingSession<
    MockTranscriptionEngine,
    MockDiarizationEngine,
    MockVoiceIdentityMatcher,
    MockNoteRepository,
>;

/// Bundles a mock session with identifiers and counters needed by assertions.
struct SessionFixture {
    session: MockSession,
    known_identity_id: Uuid,
    weak_candidate_id: Uuid,
    diarization_call_count: Arc<AtomicUsize>,
}

/// Builds a session with scripted transcription and diarization outputs.
fn session_fixture(
    transcription_engine: MockTranscriptionEngine,
    diarization_results: Vec<Result<Vec<SpeakerSegment>, DiarizationError>>,
) -> SessionFixture {
    let known_identity_id = Uuid::new_v4();
    let weak_candidate_id = Uuid::new_v4();
    let diarization_call_count = Arc::new(AtomicUsize::new(0));
    let session = MeetingSession::start(
        MeetingId::new(MEETING_ID),
        transcription_engine,
        Some(MockDiarizationEngine::scripted(
            diarization_results,
            Arc::clone(&diarization_call_count),
        )),
        MockVoiceIdentityMatcher {
            known_identity_id,
            weak_candidate_id,
        },
        MockNoteRepository::default(),
    )
    .unwrap_or_else(|error| panic!("session should start: {error}"));
    SessionFixture {
        session,
        known_identity_id,
        weak_candidate_id,
        diarization_call_count,
    }
}

/// Builds a meeting-relative VAD chunk for the pipeline meeting.
fn vad_chunk(sequence: u32, start_offset_ms: u64, end_offset_ms: u64) -> VadAudioChunk {
    vad_chunk_for(MEETING_ID, sequence, start_offset_ms, end_offset_ms)
}

/// Builds a meeting-relative VAD chunk for an arbitrary meeting.
fn vad_chunk_for(
    meeting_id: &str,
    sequence: u32,
    start_offset_ms: u64,
    end_offset_ms: u64,
) -> VadAudioChunk {
    VadAudioChunk::new(
        AudioRecording::new(
            RecordingId::new(format!("recording-{sequence}")),
            MeetingId::new(meeting_id),
            DeviceId::new("device-test"),
        ),
        start_offset_ms,
        end_offset_ms,
        vec![1, 2, 3, 4],
    )
    .unwrap_or_else(|error| panic!("chunk bounds should be valid: {error}"))
}

/// Verifies one chunk's transcript and diarization merge into a persisted, meeting-timed segment.
#[tokio::test]
async fn audio_chunk_with_transcript_and_diarization_persists_merged_attributed_segment() {
    let mut fixture = session_fixture(
        MockTranscriptionEngine::returning(&["hello team welcome"]),
        vec![Ok(vec![SpeakerSegment::new(0, 3_000, vec![1.0, 0.0])])],
    );

    let produced_segments = fixture
        .session
        .process_vad_chunk(&vad_chunk(1, 10_000, 13_000))
        .await
        .unwrap_or_else(|error| panic!("chunk processing should succeed: {error}"));

    assert_eq!(produced_segments.len(), 1);
    let persisted_segments = fixture
        .session
        .note_repository()
        .saved_segments
        .borrow()
        .clone();
    assert_eq!(
        persisted_segments,
        vec![TranscriptSegment::new(
            MeetingId::new(MEETING_ID),
            10_000,
            13_000,
            Some(fixture.known_identity_id),
            "hello team welcome",
        )]
    );
}

/// Verifies a strong voice match attributes the segment to the known identity without a clip.
#[tokio::test]
async fn strong_voice_match_attributes_segment_to_known_identity() {
    let mut fixture = session_fixture(
        MockTranscriptionEngine::returning(&["status update"]),
        vec![Ok(vec![SpeakerSegment::new(0, 2_000, vec![1.0])])],
    );

    let produced_segments = fixture
        .session
        .process_vad_chunk(&vad_chunk(1, 0, 2_000))
        .await
        .unwrap_or_else(|error| panic!("chunk processing should succeed: {error}"));

    assert_eq!(
        produced_segments[0].attribution(),
        &SpeakerAttribution::Known {
            identity_id: fixture.known_identity_id
        }
    );
    assert_eq!(
        produced_segments[0].segment().speaker_identity_id(),
        Some(fixture.known_identity_id)
    );
    assert!(
        fixture
            .session
            .note_repository()
            .saved_clips
            .borrow()
            .is_empty()
    );
}

/// Verifies a weak match is labelled pending review, persisted unattributed, and clipped for review.
#[tokio::test]
async fn weak_voice_match_marks_pending_review_and_saves_clip() {
    let mut fixture = session_fixture(
        MockTranscriptionEngine::returning(&["maybe this is alice"]),
        vec![Ok(vec![SpeakerSegment::new(200, 1_800, vec![0.5])])],
    );

    let produced_segments = fixture
        .session
        .process_vad_chunk(&vad_chunk(1, 5_000, 7_000))
        .await
        .unwrap_or_else(|error| panic!("chunk processing should succeed: {error}"));

    let saved_clips = fixture
        .session
        .note_repository()
        .saved_clips
        .borrow()
        .clone();
    assert_eq!(saved_clips.len(), 1);
    assert_eq!(
        saved_clips[0]
            .weak_match_candidate()
            .map(|candidate| candidate.identity_id()),
        Some(fixture.weak_candidate_id)
    );
    assert_eq!(
        produced_segments[0].attribution(),
        &SpeakerAttribution::PendingReview {
            clip_id: Some(saved_clips[0].clip_id()),
            candidate_identity_id: fixture.weak_candidate_id,
            cluster_id: None,
        }
    );
    assert_eq!(
        produced_segments[0].attribution().label(),
        "Unknown (pending review)"
    );
    let persisted_segments = fixture
        .session
        .note_repository()
        .saved_segments
        .borrow()
        .clone();
    assert_eq!(persisted_segments.len(), 1);
    assert_eq!(persisted_segments[0].speaker_identity_id(), None);
    assert_eq!(persisted_segments[0].text(), "maybe this is alice");
}

/// Verifies an unmatched voice is labelled unknown and clipped for manual assignment.
#[tokio::test]
async fn no_voice_match_marks_unknown_and_saves_clip() {
    let mut fixture = session_fixture(
        MockTranscriptionEngine::returning(&["who said this"]),
        vec![Ok(vec![SpeakerSegment::new(0, 1_000, vec![0.1])])],
    );

    let produced_segments = fixture
        .session
        .process_vad_chunk(&vad_chunk(1, 0, 1_000))
        .await
        .unwrap_or_else(|error| panic!("chunk processing should succeed: {error}"));

    let saved_clips = fixture
        .session
        .note_repository()
        .saved_clips
        .borrow()
        .clone();
    assert_eq!(saved_clips.len(), 1);
    assert!(saved_clips[0].weak_match_candidate().is_none());
    assert_eq!(
        produced_segments[0].attribution(),
        &SpeakerAttribution::Unknown {
            clip_id: Some(saved_clips[0].clip_id()),
            cluster_id: None,
        }
    );
    assert_eq!(produced_segments[0].attribution().label(), "Unknown");
    assert_eq!(
        fixture.session.note_repository().saved_segments.borrow()[0].speaker_identity_id(),
        None
    );
}

/// Verifies guaranteed external identities bypass diarization and are persisted as known speakers.
#[tokio::test]
async fn teams_external_chunk_uses_mapped_identity_without_diarization() {
    let mut fixture = session_fixture(MockTranscriptionEngine::returning(&["from teams"]), vec![]);
    let mapped_identity = SpeakerIdentity::new(Uuid::new_v4(), "Dana External");
    let identity_mapper = MockIdentityMapper {
        mapped_identity: mapped_identity.clone(),
        fails: false,
    };
    let media_chunk = ExternalMediaChunk::new(
        ExternalParticipant::new("teams-user-1", "Dana External", None),
        vec![7, 7, 7],
        42_000,
    );
    let audio_recording = AudioRecording::new(
        RecordingId::new("external-teams-user-1-42000"),
        MeetingId::new(MEETING_ID),
        DeviceId::new("device-test"),
    );

    let produced_segments = fixture
        .session
        .process_external_chunk(&identity_mapper, &media_chunk, &audio_recording, 2_500)
        .unwrap_or_else(|error| panic!("external processing should succeed: {error}"));

    assert_eq!(fixture.diarization_call_count.load(Ordering::SeqCst), 0);
    assert_eq!(
        produced_segments[0].attribution(),
        &SpeakerAttribution::Known {
            identity_id: mapped_identity.identity_id()
        }
    );
    assert_eq!(
        fixture
            .session
            .note_repository()
            .saved_segments
            .borrow()
            .clone(),
        vec![TranscriptSegment::new(
            MeetingId::new(MEETING_ID),
            42_000,
            44_500,
            Some(mapped_identity.identity_id()),
            "from teams",
        )]
    );
}

/// Verifies a failed external identity mapping still persists the transcript as unknown.
#[tokio::test]
async fn teams_mapping_failure_persists_unknown_segment() {
    let mut fixture = session_fixture(MockTranscriptionEngine::returning(&["unmapped"]), vec![]);
    let identity_mapper = MockIdentityMapper {
        mapped_identity: SpeakerIdentity::new(Uuid::new_v4(), "Unused"),
        fails: true,
    };
    let media_chunk = ExternalMediaChunk::new(
        ExternalParticipant::new("teams-user-2", "Erin", None),
        vec![1],
        0,
    );
    let audio_recording = AudioRecording::new(
        RecordingId::new("external-teams-user-2-0"),
        MeetingId::new(MEETING_ID),
        DeviceId::new("device-test"),
    );

    let produced_segments = fixture
        .session
        .process_external_chunk(&identity_mapper, &media_chunk, &audio_recording, 1_000)
        .unwrap_or_else(|error| panic!("external processing should degrade: {error}"));

    assert_eq!(
        produced_segments[0].attribution(),
        &SpeakerAttribution::Unknown {
            clip_id: None,
            cluster_id: None,
        }
    );
    assert_eq!(
        fixture.session.note_repository().saved_segments.borrow()[0].text(),
        "unmapped"
    );
}

/// Verifies a transcription failure surfaces an error and persists no partial segment.
#[tokio::test]
async fn transcription_failure_persists_no_segment() {
    let mut fixture = session_fixture(
        MockTranscriptionEngine::failing(),
        vec![Ok(vec![SpeakerSegment::new(0, 1_000, vec![1.0])])],
    );

    let processing_result = fixture
        .session
        .process_vad_chunk(&vad_chunk(1, 0, 1_000))
        .await;

    assert!(matches!(
        processing_result,
        Err(MeetingSessionError::Transcription(
            TranscriptionError::Unavailable
        ))
    ));
    let repository = fixture.session.note_repository();
    assert_eq!(repository.segment_save_count.get(), 0);
    assert!(repository.saved_clips.borrow().is_empty());
    assert_eq!(fixture.diarization_call_count.load(Ordering::SeqCst), 0);
}

/// Verifies a diarization failure keeps the transcript as one unattributed whole-chunk segment.
#[tokio::test]
async fn diarization_failure_persists_unattributed_transcript() {
    let mut fixture = session_fixture(
        MockTranscriptionEngine::returning(&["still captured"]),
        vec![Err(DiarizationError::Unavailable)],
    );

    let produced_segments = fixture
        .session
        .process_vad_chunk(&vad_chunk(1, 3_000, 4_500))
        .await
        .unwrap_or_else(|error| panic!("diarization failure should degrade: {error}"));

    assert_eq!(produced_segments.len(), 1);
    assert_eq!(
        produced_segments[0].attribution(),
        &SpeakerAttribution::Unattributed
    );
    assert_eq!(produced_segments[0].attribution().label(), "Unattributed");
    assert_eq!(
        fixture
            .session
            .note_repository()
            .saved_segments
            .borrow()
            .clone(),
        vec![TranscriptSegment::new(
            MeetingId::new(MEETING_ID),
            3_000,
            4_500,
            None,
            "still captured",
        )]
    );
}

/// Verifies a session without a diarization engine persists unattributed segments.
#[tokio::test]
async fn missing_diarization_engine_persists_unattributed_transcript() {
    let mut session: MockSession = MeetingSession::start(
        MeetingId::new(MEETING_ID),
        MockTranscriptionEngine::returning(&["solo"]),
        None,
        MockVoiceIdentityMatcher {
            known_identity_id: Uuid::new_v4(),
            weak_candidate_id: Uuid::new_v4(),
        },
        MockNoteRepository::default(),
    )
    .unwrap_or_else(|error| panic!("session should start: {error}"));

    let produced_segments = session
        .process_vad_chunk(&vad_chunk(1, 0, 900))
        .await
        .unwrap_or_else(|error| panic!("processing should succeed: {error}"));

    assert_eq!(
        produced_segments[0].attribution(),
        &SpeakerAttribution::Unattributed
    );
}

/// Verifies a voice-matching failure marks only that region unknown and processing continues.
#[tokio::test]
async fn voice_matching_failure_marks_unknown_and_continues() {
    let mut fixture = session_fixture(
        MockTranscriptionEngine::returning(&["one two three four"]),
        vec![Ok(vec![
            SpeakerSegment::new(0, 1_000, vec![-1.0]),
            SpeakerSegment::new(1_000, 2_000, vec![1.0]),
        ])],
    );

    let produced_segments = fixture
        .session
        .process_vad_chunk(&vad_chunk(1, 0, 2_000))
        .await
        .unwrap_or_else(|error| panic!("matching failure should degrade: {error}"));

    assert_eq!(produced_segments.len(), 2);
    assert!(matches!(
        produced_segments[0].attribution(),
        SpeakerAttribution::Unknown { .. }
    ));
    assert_eq!(
        produced_segments[1].attribution(),
        &SpeakerAttribution::Known {
            identity_id: fixture.known_identity_id
        }
    );
    assert_eq!(produced_segments[0].segment().text(), "one two");
    assert_eq!(produced_segments[1].segment().text(), "three four");
}

/// Verifies segments from successive chunks accumulate instead of replacing earlier ones.
#[tokio::test]
async fn successive_chunks_accumulate_persisted_segments() {
    let mut fixture = session_fixture(
        MockTranscriptionEngine::returning(&["first", "second"]),
        vec![
            Ok(vec![SpeakerSegment::new(0, 1_000, vec![1.0])]),
            Ok(vec![SpeakerSegment::new(0, 1_000, vec![1.0])]),
        ],
    );

    for (sequence, start_offset_ms) in [(1, 0), (2, 1_000)] {
        fixture
            .session
            .process_vad_chunk(&vad_chunk(
                sequence,
                start_offset_ms,
                start_offset_ms + 1_000,
            ))
            .await
            .unwrap_or_else(|error| panic!("chunk {sequence} should succeed: {error}"));
    }

    let persisted_texts: Vec<String> = fixture
        .session
        .note_repository()
        .saved_segments
        .borrow()
        .iter()
        .map(|segment| segment.text().to_owned())
        .collect();
    assert_eq!(persisted_texts, vec!["first", "second"]);
}

/// Verifies a chunk belonging to another meeting is rejected before any AI work.
#[tokio::test]
async fn chunk_for_another_meeting_is_rejected() {
    let mut fixture = session_fixture(MockTranscriptionEngine::returning(&["x"]), vec![]);

    let processing_result = fixture
        .session
        .process_vad_chunk(&vad_chunk_for("other-meeting", 1, 0, 1_000))
        .await;

    assert!(matches!(
        processing_result,
        Err(MeetingSessionError::MeetingMismatch { .. })
    ));
    assert_eq!(
        fixture.session.note_repository().segment_save_count.get(),
        0
    );
}

/// Verifies inverted VAD boundaries are rejected at construction.
#[test]
fn inverted_chunk_bounds_are_rejected() {
    assert!(ChunkBounds::new(2_000, 1_000).is_err());
    assert!(
        VadAudioChunk::new(
            AudioRecording::new(
                RecordingId::new("r"),
                MeetingId::new(MEETING_ID),
                DeviceId::new("d"),
            ),
            5,
            4,
            Vec::new(),
        )
        .is_err()
    );
}

/// Verifies two speakers split the text by duration and tile the VAD chunk in meeting time.
#[test]
fn alignment_splits_words_by_region_duration_and_tiles_chunk() {
    let alice = SpeakerAttribution::Known {
        identity_id: Uuid::new_v4(),
    };
    let bob = SpeakerAttribution::Known {
        identity_id: Uuid::new_v4(),
    };
    let chunk_bounds = ChunkBounds::new(60_000, 64_000)
        .unwrap_or_else(|error| panic!("bounds should be valid: {error}"));

    // Diarization leaves a 200 ms leading gap, a 400 ms internal gap, and overshoots the chunk.
    let aligned_regions = align_chunk_regions(
        chunk_bounds,
        vec![
            SpeakerRegion::new(200, 1_000, alice.clone()),
            SpeakerRegion::new(1_400, 9_000, bob.clone()),
        ],
        "one two three four five six seven eight",
    );

    assert_eq!(aligned_regions.len(), 2);
    assert_eq!(aligned_regions[0].start_time_ms(), 60_000);
    assert_eq!(aligned_regions[0].end_time_ms(), 61_200);
    assert_eq!(aligned_regions[0].attribution(), &alice);
    assert_eq!(aligned_regions[0].text(), "one two");
    assert_eq!(aligned_regions[1].start_time_ms(), 61_200);
    assert_eq!(aligned_regions[1].end_time_ms(), 64_000);
    assert_eq!(aligned_regions[1].attribution(), &bob);
    assert_eq!(aligned_regions[1].text(), "three four five six seven eight");
}

/// Verifies a word goes to the speaker at its midpoint, even when one speaker talks faster.
#[test]
fn timed_words_go_to_the_region_at_their_midpoint() {
    let alice = SpeakerAttribution::Known {
        identity_id: Uuid::new_v4(),
    };
    let bob = SpeakerAttribution::Known {
        identity_id: Uuid::new_v4(),
    };
    let chunk_bounds = ChunkBounds::new(10_000, 14_000)
        .unwrap_or_else(|error| panic!("bounds should be valid: {error}"));
    let words = [
        TimedWord::new("one", 0, 200),
        TimedWord::new("two", 200, 400),
        TimedWord::new("three", 400, 600),
        TimedWord::new("four", 2_900, 3_100),
    ];

    let aligned_regions = align_timed_words(
        chunk_bounds,
        vec![
            SpeakerRegion::new(0, 2_000, alice.clone()),
            SpeakerRegion::new(2_000, 4_000, bob.clone()),
        ],
        &words,
    );

    assert_eq!(aligned_regions.len(), 2);
    assert_eq!(aligned_regions[0].attribution(), &alice);
    assert_eq!(aligned_regions[0].text(), "one two three");
    assert_eq!(aligned_regions[0].start_time_ms(), 10_000);
    assert_eq!(aligned_regions[1].attribution(), &bob);
    assert_eq!(aligned_regions[1].text(), "four");
    assert_eq!(aligned_regions[1].end_time_ms(), 14_000);
}

/// Verifies a timing glitch neither loses a word nor reorders the text.
#[test]
fn timed_words_out_of_order_or_past_the_chunk_keep_every_word_in_order() {
    let alice = SpeakerAttribution::Known {
        identity_id: Uuid::new_v4(),
    };
    let bob = SpeakerAttribution::Known {
        identity_id: Uuid::new_v4(),
    };
    let chunk_bounds = ChunkBounds::new(0, 2_000)
        .unwrap_or_else(|error| panic!("bounds should be valid: {error}"));
    let words = [
        TimedWord::new("late", 1_500, 1_600),
        TimedWord::new("early", 100, 200),
        TimedWord::new("beyond", 9_000, 9_100),
    ];

    let aligned_regions = align_timed_words(
        chunk_bounds,
        vec![
            SpeakerRegion::new(0, 1_000, alice),
            SpeakerRegion::new(1_000, 2_000, bob.clone()),
        ],
        &words,
    );

    assert_eq!(aligned_regions.len(), 1);
    assert_eq!(aligned_regions[0].attribution(), &bob);
    assert_eq!(aligned_regions[0].text(), "late early beyond");
}

/// Verifies engines without timing still split by time, and engines with timing do not.
#[test]
fn align_transcript_uses_timing_only_when_it_exists() {
    let alice = SpeakerAttribution::Known {
        identity_id: Uuid::new_v4(),
    };
    let bob = SpeakerAttribution::Known {
        identity_id: Uuid::new_v4(),
    };
    let chunk_bounds = ChunkBounds::new(0, 2_000)
        .unwrap_or_else(|error| panic!("bounds should be valid: {error}"));
    let regions = vec![
        SpeakerRegion::new(0, 1_000, alice),
        SpeakerRegion::new(1_000, 2_000, bob),
    ];
    let meeting_id = MeetingId::new("meeting");

    let plain = Transcript::new(meeting_id.clone(), "a b c d");
    let split_by_time = align_transcript(chunk_bounds, regions.clone(), &plain);
    assert_eq!(split_by_time[0].text(), "a b");

    let timed = plain.clone().with_timed_words(vec![
        TimedWord::new("a", 0, 100),
        TimedWord::new("b", 100, 200),
        TimedWord::new("c", 200, 300),
        TimedWord::new("d", 1_500, 1_600),
    ]);
    let by_word = align_transcript(chunk_bounds, regions, &timed);
    assert_eq!(by_word[0].text(), "a b c");
    assert_eq!(by_word[1].text(), "d");
}

/// Verifies adjacent regions of one speaker are merged so a sentence is not split.
#[test]
fn alignment_merges_adjacent_regions_of_the_same_speaker() {
    let alice = SpeakerAttribution::Known {
        identity_id: Uuid::new_v4(),
    };
    let chunk_bounds = ChunkBounds::new(0, 3_000)
        .unwrap_or_else(|error| panic!("bounds should be valid: {error}"));

    let aligned_regions = align_chunk_regions(
        chunk_bounds,
        vec![
            SpeakerRegion::new(1_500, 3_000, alice.clone()),
            SpeakerRegion::new(0, 1_500, alice.clone()),
        ],
        "a single long sentence",
    );

    assert_eq!(aligned_regions.len(), 1);
    assert_eq!(aligned_regions[0].start_time_ms(), 0);
    assert_eq!(aligned_regions[0].end_time_ms(), 3_000);
    assert_eq!(aligned_regions[0].text(), "a single long sentence");
}

/// Verifies empty diarization output falls back to one unattributed whole-chunk region.
#[test]
fn alignment_without_regions_produces_unattributed_chunk() {
    let chunk_bounds = ChunkBounds::new(100, 600)
        .unwrap_or_else(|error| panic!("bounds should be valid: {error}"));

    let aligned_regions = align_chunk_regions(chunk_bounds, Vec::new(), "orphan words");

    assert_eq!(aligned_regions.len(), 1);
    assert_eq!(
        aligned_regions[0].attribution(),
        &SpeakerAttribution::Unattributed
    );
    assert_eq!(aligned_regions[0].start_time_ms(), 100);
    assert_eq!(aligned_regions[0].end_time_ms(), 600);
}

/// Returns each scripted segment list in order and never loads a model.
struct ScriptedIntegrationDiarizer {
    scripted_results: Mutex<Vec<Vec<SpeakerSegment>>>,
}

#[async_trait::async_trait]
impl DiarizationEngine for ScriptedIntegrationDiarizer {
    /// Pops the next scripted speaker segments.
    async fn diarize(&self, _audio_bytes: &[u8]) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        Ok(self
            .scripted_results
            .lock()
            .map_err(|_| DiarizationError::Unavailable)?
            .pop()
            .unwrap_or_default())
    }

    /// Reports no voice windows, so the final pass leaves the live result alone.
    async fn voice_windows(
        &self,
        _audio_bytes: &[u8],
    ) -> Result<Vec<crate::ports::ai::SpeakerSegment>, crate::domain::errors::DiarizationError>
    {
        Ok(Vec::new())
    }
}

/// Simulates a full meeting through real in-memory SQLite and vector-search adapters.
#[tokio::test]
async fn full_meeting_persists_three_attributed_segments_through_sqlite() {
    let meeting_id = MeetingId::new("meeting-integration");
    let note_repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite should open: {error}"));
    note_repository
        .create_meeting(&meeting_id, &[])
        .unwrap_or_else(|error| panic!("meeting should be created: {error}"));

    let alice_id = Uuid::new_v4();
    let bob_id = Uuid::new_v4();
    let voice_vector_database = InMemoryVoiceVectorDB::new();
    for (identity_id, enrolled_vector) in [(alice_id, vec![1.0, 0.0]), (bob_id, vec![0.0, 1.0])] {
        voice_vector_database
            .store_embedding(VoiceEmbedding::new(
                identity_id,
                Uuid::new_v4(),
                enrolled_vector,
            ))
            .unwrap_or_else(|error| panic!("embedding should be enrolled: {error}"));
    }

    let mut session = MeetingSession::start(
        meeting_id.clone(),
        MockTranscriptionEngine::returning(&[
            "good morning everyone",
            "thanks alice",
            "lets begin",
        ]),
        Some(ScriptedIntegrationDiarizer {
            scripted_results: Mutex::new(
                vec![
                    vec![SpeakerSegment::new(0, 2_500, vec![0.99, 0.05])],
                    vec![SpeakerSegment::new(100, 1_900, vec![0.02, 0.98])],
                    vec![SpeakerSegment::new(0, 3_000, vec![0.97, 0.1])],
                ]
                .into_iter()
                .rev()
                .collect(),
            ),
        }),
        VoiceIdentityManager::new(voice_vector_database),
        note_repository,
    )
    .unwrap_or_else(|error| panic!("session should start: {error}"));

    for (sequence, (start_offset_ms, end_offset_ms)) in [(0, 2_500), (2_500, 4_500), (4_500, 7_500)]
        .into_iter()
        .enumerate()
    {
        let sequence = u32::try_from(sequence).unwrap_or(u32::MAX);
        session
            .process_vad_chunk(&vad_chunk_for(
                "meeting-integration",
                sequence,
                start_offset_ms,
                end_offset_ms,
            ))
            .await
            .unwrap_or_else(|error| panic!("chunk {sequence} should succeed: {error}"));
    }

    let persisted_segments = session
        .note_repository()
        .load_transcript_segments(&meeting_id)
        .unwrap_or_else(|error| panic!("segments should load: {error}"));
    assert_eq!(
        persisted_segments,
        vec![
            TranscriptSegment::new(
                meeting_id.clone(),
                0,
                2_500,
                Some(alice_id),
                "good morning everyone",
            ),
            TranscriptSegment::new(
                meeting_id.clone(),
                2_500,
                4_500,
                Some(bob_id),
                "thanks alice"
            ),
            TranscriptSegment::new(meeting_id, 4_500, 7_500, Some(alice_id), "lets begin"),
        ]
    );
}

/// Builds an in-memory SQLite repository containing the pipeline meeting.
fn sqlite_repository_with_meeting(meeting_id: &MeetingId) -> SqliteNoteRepository {
    let note_repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .unwrap_or_else(|error| panic!("in-memory SQLite should open: {error}"));
    note_repository
        .create_meeting(meeting_id, &[])
        .unwrap_or_else(|error| panic!("meeting should be created: {error}"));
    note_repository
}

/// Builds a diarizer that returns one full-chunk speaker segment per scripted embedding.
fn one_speaker_per_chunk_diarizer(
    chunk_duration_ms: u64,
    embeddings: Vec<Vec<f32>>,
) -> ScriptedIntegrationDiarizer {
    ScriptedIntegrationDiarizer {
        scripted_results: Mutex::new(
            embeddings
                .into_iter()
                .rev()
                .map(|embedding| vec![SpeakerSegment::new(0, chunk_duration_ms, embedding)])
                .collect(),
        ),
    }
}

/// Verifies two similar unknown voices share one cluster, one review clip, and the segment link.
#[tokio::test]
async fn similar_unknown_voices_share_one_cluster_and_one_review_clip() {
    let meeting_id = MeetingId::new(MEETING_ID);
    let mut session = MeetingSession::start(
        meeting_id.clone(),
        MockTranscriptionEngine::returning(&["first words", "second words"]),
        Some(one_speaker_per_chunk_diarizer(
            1_000,
            vec![vec![0.1, 0.9], vec![0.12, 0.88]],
        )),
        MockVoiceIdentityMatcher {
            known_identity_id: Uuid::new_v4(),
            weak_candidate_id: Uuid::new_v4(),
        },
        sqlite_repository_with_meeting(&meeting_id),
    )
    .unwrap_or_else(|error| panic!("session should start: {error}"));

    let mut produced_segments = Vec::new();
    for (sequence, start_offset_ms) in [(1, 0), (2, 1_000)] {
        produced_segments.extend(
            session
                .process_vad_chunk(&vad_chunk(
                    sequence,
                    start_offset_ms,
                    start_offset_ms + 1_000,
                ))
                .await
                .unwrap_or_else(|error| panic!("chunk {sequence} should succeed: {error}")),
        );
    }

    let clusters =
        UnknownSpeakerClusterer::new(session.note_repository(), ClusteringConfig::default())
            .get_clusters_for_meeting(&meeting_id)
            .unwrap_or_else(|error| panic!("clusters should load: {error}"));
    assert_eq!(clusters.len(), 1);
    let cluster_id = clusters[0].cluster_id();
    let saved_clips = session
        .note_repository()
        .get_unassigned_clips(&meeting_id)
        .unwrap_or_else(|error| panic!("clips should load: {error}"));
    assert_eq!(
        saved_clips.len(),
        1,
        "one review clip per cluster, not per chunk"
    );
    assert_eq!(saved_clips[0].cluster_id(), Some(cluster_id));
    assert_eq!(
        produced_segments
            .iter()
            .map(|produced| produced.attribution().clone())
            .collect::<Vec<_>>(),
        vec![
            SpeakerAttribution::Unknown {
                clip_id: Some(saved_clips[0].clip_id()),
                cluster_id: Some(cluster_id),
            },
            SpeakerAttribution::Unknown {
                clip_id: None,
                cluster_id: Some(cluster_id),
            },
        ]
    );
    assert!(
        session
            .persisted_segments()
            .iter()
            .all(|segment| segment.unknown_cluster_id() == Some(cluster_id))
    );
}

/// Verifies weak matches are clustered too and keep their pending-review candidate.
#[tokio::test]
async fn weak_voice_match_is_clustered_as_pending_review() {
    let meeting_id = MeetingId::new(MEETING_ID);
    let weak_candidate_id = Uuid::new_v4();
    let mut session = MeetingSession::start(
        meeting_id.clone(),
        MockTranscriptionEngine::returning(&["maybe alice"]),
        Some(one_speaker_per_chunk_diarizer(1_000, vec![vec![0.5, 0.5]])),
        MockVoiceIdentityMatcher {
            known_identity_id: Uuid::new_v4(),
            weak_candidate_id,
        },
        sqlite_repository_with_meeting(&meeting_id),
    )
    .unwrap_or_else(|error| panic!("session should start: {error}"));

    let produced_segments = session
        .process_vad_chunk(&vad_chunk(1, 0, 1_000))
        .await
        .unwrap_or_else(|error| panic!("chunk should succeed: {error}"));

    let saved_clips = session
        .note_repository()
        .get_unassigned_clips(&meeting_id)
        .unwrap_or_else(|error| panic!("clips should load: {error}"));
    assert_eq!(saved_clips.len(), 1);
    assert!(saved_clips[0].cluster_id().is_some());
    assert_eq!(
        produced_segments[0].attribution(),
        &SpeakerAttribution::PendingReview {
            clip_id: Some(saved_clips[0].clip_id()),
            candidate_identity_id: weak_candidate_id,
            cluster_id: saved_clips[0].cluster_id(),
        }
    );
}

/// Verifies a clustering storage failure degrades to the per-chunk review clip without a cluster.
#[tokio::test]
async fn clustering_failure_falls_back_to_unclustered_review_clip() {
    let mut fixture = session_fixture(
        MockTranscriptionEngine::returning(&["first", "second"]),
        vec![
            Ok(vec![SpeakerSegment::new(0, 1_000, vec![0.1, 0.9])]),
            Ok(vec![SpeakerSegment::new(0, 1_000, vec![0.1, 0.9])]),
        ],
    );

    for (sequence, start_offset_ms) in [(1, 0), (2, 1_000)] {
        fixture
            .session
            .process_vad_chunk(&vad_chunk(
                sequence,
                start_offset_ms,
                start_offset_ms + 1_000,
            ))
            .await
            .unwrap_or_else(|error| panic!("chunk {sequence} should degrade: {error}"));
    }

    let saved_clips = fixture
        .session
        .note_repository()
        .saved_clips
        .borrow()
        .clone();
    assert_eq!(saved_clips.len(), 2);
    assert!(saved_clips.iter().all(|clip| clip.cluster_id().is_none()));
}

/// Simulates five unknown chunks (three of one voice, two of another) and assigns one cluster.
#[tokio::test]
async fn five_unknown_chunks_form_two_clusters_and_assignment_attributes_three_segments() {
    let meeting_id = MeetingId::new("meeting-clustering");
    let voice_vector_database = InMemoryVoiceVectorDB::new();
    let voice_a = [
        vec![0.9, 0.1, 0.0],
        vec![0.85, 0.15, 0.05],
        vec![0.95, 0.05, 0.0],
    ];
    let voice_b = [vec![0.0, 0.2, 0.9], vec![0.05, 0.1, 0.95]];
    let mut session = MeetingSession::start(
        meeting_id.clone(),
        MockTranscriptionEngine::returning(&[
            "a one", "b one", "a two", "b two", "a three", "a again",
        ]),
        Some(one_speaker_per_chunk_diarizer(
            3_000,
            vec![
                voice_a[0].clone(),
                voice_b[0].clone(),
                voice_a[1].clone(),
                voice_b[1].clone(),
                voice_a[2].clone(),
                vec![0.92, 0.08, 0.0],
            ],
        )),
        VoiceIdentityManager::new(&voice_vector_database),
        sqlite_repository_with_meeting(&meeting_id),
    )
    .unwrap_or_else(|error| panic!("session should start: {error}"));
    for sequence in 0..5_u32 {
        let start_offset_ms = u64::from(sequence) * 1_000;
        session
            .process_vad_chunk(&vad_chunk_for(
                "meeting-clustering",
                sequence,
                start_offset_ms,
                start_offset_ms + 1_000,
            ))
            .await
            .unwrap_or_else(|error| panic!("chunk {sequence} should succeed: {error}"));
    }

    let clusters =
        UnknownSpeakerClusterer::new(session.note_repository(), ClusteringConfig::default())
            .get_clusters_for_meeting(&meeting_id)
            .unwrap_or_else(|error| panic!("clusters should load: {error}"));
    assert_eq!(clusters.len(), 2);
    assert_eq!(clusters[0].chunk_ids().len(), 3);
    assert_eq!(clusters[1].chunk_ids().len(), 2);
    assert_eq!(clusters[0].total_duration_ms(), 9_000);
    let alice_id = Uuid::new_v4();

    let assignment = session
        .assign_identity_to_cluster(
            &voice_vector_database,
            clusters[0].cluster_id(),
            &SpeakerIdentity::new(alice_id, "Alice"),
        )
        .unwrap_or_else(|error| panic!("assignment should succeed: {error}"));

    assert_eq!(assignment.segments_updated(), 3);
    assert_eq!(assignment.variants_stored(), 1);
    let attributed_texts = |segments: &[TranscriptSegment]| -> Vec<String> {
        segments
            .iter()
            .filter(|segment| segment.speaker_identity_id() == Some(alice_id))
            .map(|segment| segment.text().to_owned())
            .collect()
    };
    let stored_segments = session
        .note_repository()
        .load_transcript_segments(&meeting_id)
        .unwrap_or_else(|error| panic!("segments should load: {error}"));
    assert_eq!(
        attributed_texts(&stored_segments),
        vec!["a one", "a two", "a three"]
    );
    assert_eq!(
        attributed_texts(session.persisted_segments()),
        vec!["a one", "a two", "a three"]
    );

    // A later chunk of the same voice now matches the enrolled identity, and rewriting the
    // timeline keeps the earlier reassignment instead of restoring stale unknown segments.
    session
        .process_vad_chunk(&vad_chunk_for("meeting-clustering", 5, 5_000, 6_000))
        .await
        .unwrap_or_else(|error| panic!("follow-up chunk should succeed: {error}"));
    let final_segments = session
        .note_repository()
        .load_transcript_segments(&meeting_id)
        .unwrap_or_else(|error| panic!("segments should load: {error}"));
    assert_eq!(
        attributed_texts(&final_segments),
        vec!["a one", "a two", "a three", "a again"]
    );
    assert_eq!(
        UnknownSpeakerClusterer::new(session.note_repository(), ClusteringConfig::default())
            .get_clusters_for_meeting(&meeting_id)
            .unwrap_or_else(|error| panic!("clusters should load: {error}"))
            .len(),
        1
    );
}

/// Verifies chunks are transcribed without the previous chunk's text as a prompt: with it,
/// Whisper repeated or completed the previous chunk's last words (Issue #51).
#[tokio::test]
async fn chunks_are_transcribed_without_the_previous_chunks_text() {
    let mut session: MockSession = MeetingSession::start(
        MeetingId::new(MEETING_ID),
        MockTranscriptionEngine::returning(&["first chunk", "second chunk"]),
        None,
        MockVoiceIdentityMatcher {
            known_identity_id: Uuid::new_v4(),
            weak_candidate_id: Uuid::new_v4(),
        },
        MockNoteRepository::default(),
    )
    .unwrap_or_else(|error| panic!("session should start: {error}"));

    for (sequence, start_ms) in [(1, 0), (2, 3_000)] {
        session
            .process_vad_chunk(&vad_chunk(sequence, start_ms, start_ms + 3_000))
            .await
            .unwrap_or_else(|error| panic!("chunk should process: {error}"));
    }

    assert_eq!(
        *session
            .transcription_engine()
            .received_previous_texts
            .borrow(),
        vec![None, None]
    );
}

/// Each chunk is transcribed in the language it was stamped with, so a change during a recording
/// reaches only the chunks cut after it.
#[tokio::test]
async fn a_chunks_language_reaches_the_engine() {
    let mut session: MockSession = MeetingSession::start(
        MeetingId::new(MEETING_ID),
        MockTranscriptionEngine::returning(&["uno", "two"]),
        None,
        MockVoiceIdentityMatcher {
            known_identity_id: Uuid::new_v4(),
            weak_candidate_id: Uuid::new_v4(),
        },
        MockNoteRepository::default(),
    )
    .unwrap_or_else(|error| panic!("session should start: {error}"));
    let italian = TranscriptionLanguage::parse("it").ok();

    session
        .process_vad_chunk(&vad_chunk(1, 0, 3_000).with_language(italian))
        .await
        .unwrap_or_else(|error| panic!("chunk should process: {error}"));
    session
        .process_vad_chunk(&vad_chunk(2, 3_000, 6_000))
        .await
        .unwrap_or_else(|error| panic!("chunk should process: {error}"));

    assert_eq!(
        *session.transcription_engine().received_languages.borrow(),
        vec![italian, None]
    );
}

/// The meeting records each language its chunks were transcribed in once, at the start of the
/// first chunk that used it (shown in the meeting's metadata); a chunk without one records nothing.
#[tokio::test]
async fn each_chunk_language_is_recorded_once_for_the_meeting() {
    let mut session: MockSession = MeetingSession::start(
        MeetingId::new(MEETING_ID),
        MockTranscriptionEngine::returning(&["uno", "due", "three", "four"]),
        None,
        MockVoiceIdentityMatcher {
            known_identity_id: Uuid::new_v4(),
            weak_candidate_id: Uuid::new_v4(),
        },
        MockNoteRepository::default(),
    )
    .unwrap_or_else(|error| panic!("session should start: {error}"));
    let italian = TranscriptionLanguage::parse("it").ok();
    let english = TranscriptionLanguage::parse("en").ok();

    for (number, start, language) in [
        (1, 0, italian),
        (2, 3_000, italian),
        (3, 6_000, english),
        (4, 9_000, None),
    ] {
        session
            .process_vad_chunk(&vad_chunk(number, start, start + 3_000).with_language(language))
            .await
            .unwrap_or_else(|error| panic!("chunk should process: {error}"));
    }

    let recorded: Vec<(&str, u64)> = session
        .note_repository()
        .recorded_languages
        .borrow()
        .iter()
        .map(|(language, offset)| (language.as_str(), *offset))
        .collect();
    assert_eq!(recorded, vec![("it", 0), ("en", 6_000)]);
}

/// Gives every chunk one live voice, and scripted voice windows for the final pass.
struct FinalPassDiarizer {
    live_voice: Vec<f32>,
    chunk_voices: Mutex<Vec<Vec<f32>>>,
}

#[async_trait::async_trait]
impl DiarizationEngine for FinalPassDiarizer {
    /// Reports the same live voice for the whole of each 10 s chunk, as a merged result would.
    async fn diarize(&self, _audio_bytes: &[u8]) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        Ok(vec![SpeakerSegment::new(
            0,
            10_000,
            self.live_voice.clone(),
        )])
    }

    /// Reports ten windows of the chunk's scripted voice.
    async fn voice_windows(
        &self,
        _audio_bytes: &[u8],
    ) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        let voice = self
            .chunk_voices
            .lock()
            .map_err(|_| DiarizationError::Unavailable)?
            .remove(0);
        Ok((0..10_u64)
            .map(|second| {
                SpeakerSegment::new(second * 1_000, second * 1_000 + 2_000, voice.clone())
            })
            .collect())
    }
}

/// Runs a recording of 10 s chunks, one per voice, whose live pass merges everyone.
async fn run_final_pass(
    voice_vector_database: &InMemoryVoiceVectorDB,
    chunk_voices: Vec<Vec<f32>>,
) -> (Vec<TranscriptSegment>, Option<usize>, usize) {
    let meeting_id = MeetingId::new("meeting-final");
    let mut session = MeetingSession::start(
        meeting_id.clone(),
        MockTranscriptionEngine::returning(&["chunk 0", "chunk 1"]),
        Some(FinalPassDiarizer {
            live_voice: vec![0.0, 0.0, 1.0],
            chunk_voices: Mutex::new(chunk_voices.clone()),
        }),
        VoiceIdentityManager::new(voice_vector_database),
        sqlite_repository_with_meeting(&meeting_id),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    for chunk in 0..chunk_voices.len() {
        let start_ms = chunk as u64 * 10_000;
        session
            .process_vad_chunk(&vad_chunk_for(
                "meeting-final",
                chunk as u32,
                start_ms,
                start_ms + 10_000,
            ))
            .await
            .unwrap_or_else(|error| panic!("{error}"));
    }
    let finished = session
        .finish_meeting()
        .unwrap_or_else(|error| panic!("{error}"));
    let clusters =
        UnknownSpeakerClusterer::new(session.note_repository(), ClusteringConfig::default())
            .get_clusters_for_meeting(&meeting_id)
            .unwrap_or_else(|error| panic!("{error}"));
    (
        session.persisted_segments().to_vec(),
        finished,
        clusters.len(),
    )
}

#[tokio::test]
async fn the_final_pass_separates_people_the_live_pass_merged() {
    let (segments, finished, cluster_count) = run_final_pass(
        &InMemoryVoiceVectorDB::new(),
        vec![vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0]],
    )
    .await;
    assert_eq!(finished, Some(2));
    assert_eq!(cluster_count, 2, "each unnamed speaker has one cluster");
    let clusters: Vec<_> = segments
        .iter()
        .map(TranscriptSegment::unknown_cluster_id)
        .collect();
    assert!(clusters[0].is_some() && clusters[1].is_some());
    assert_ne!(clusters[0], clusters[1]);
    assert_eq!(segments[0].text(), "chunk 0");
}

/// Gives each chunk a long voiced turn with words and a brief turn of another voice without any.
struct WordlessTurnDiarizer;

#[async_trait::async_trait]
impl DiarizationEngine for WordlessTurnDiarizer {
    /// Reports a 9.9 s turn and a 0.1 s turn, too short to receive any of the chunk's words.
    async fn diarize(&self, _audio_bytes: &[u8]) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        Ok(vec![
            SpeakerSegment::new(0, 9_900, vec![1.0, 0.0, 0.0]),
            SpeakerSegment::new(9_900, 10_000, vec![0.0, 0.0, 1.0]),
        ])
    }

    /// Reports ten windows of the first voice.
    async fn voice_windows(
        &self,
        _audio_bytes: &[u8],
    ) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        Ok((0..10_u64)
            .map(|second| {
                SpeakerSegment::new(second * 1_000, second * 1_000 + 2_000, vec![1.0, 0.0, 0.0])
            })
            .collect())
    }
}

#[tokio::test]
async fn the_final_pass_removes_live_voices_that_received_no_words() {
    let meeting_id = MeetingId::new("meeting-final");
    let mut session = MeetingSession::start(
        meeting_id.clone(),
        MockTranscriptionEngine::returning(&["one two three four five"]),
        Some(WordlessTurnDiarizer),
        VoiceIdentityManager::new(InMemoryVoiceVectorDB::new()),
        sqlite_repository_with_meeting(&meeting_id),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    session
        .process_vad_chunk(&vad_chunk_for("meeting-final", 0, 0, 10_000))
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    let clusterer =
        UnknownSpeakerClusterer::new(session.note_repository(), ClusteringConfig::default());
    let live_clusters = clusterer
        .get_clusters_for_meeting(&meeting_id)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(live_clusters.len(), 2, "the live pass saw two voices");

    session
        .finish_meeting()
        .unwrap_or_else(|error| panic!("{error}"));

    let clusters =
        UnknownSpeakerClusterer::new(session.note_repository(), ClusteringConfig::default())
            .get_clusters_for_meeting(&meeting_id)
            .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        clusters.len(),
        1,
        "only the speaker the final pass found is left to name"
    );
    assert!(
        session
            .note_repository()
            .get_unassigned_clips(&meeting_id)
            .unwrap_or_else(|error| panic!("{error}"))
            .iter()
            .all(|clip| clip.cluster_id() == Some(clusters[0].cluster_id())),
        "the wordless voice's review clip is gone too"
    );
}

#[tokio::test]
async fn the_final_pass_names_a_whole_speaker_below_the_per_turn_limit() {
    let voice_vector_database = InMemoryVoiceVectorDB::new();
    let identity_id = Uuid::new_v4();
    voice_vector_database
        .save_speaker_identity(&SpeakerIdentity::new(identity_id, "Ada"))
        .unwrap_or_else(|error| panic!("{error}"));
    voice_vector_database
        .store_embedding(VoiceEmbedding::new(
            identity_id,
            Uuid::new_v4(),
            vec![0.7, 0.714, 0.0],
        ))
        .unwrap_or_else(|error| panic!("{error}"));
    // Similarity 0.7: unnamed for a single turn, named for a whole speaker.
    let (segments, _, cluster_count) = run_final_pass(
        &voice_vector_database,
        vec![vec![1.0, 0.0, 0.0], vec![1.0, 0.0, 0.0]],
    )
    .await;
    assert!(
        segments
            .iter()
            .all(|segment| segment.speaker_identity_id() == Some(identity_id))
    );
    assert_eq!(cluster_count, 0, "the live cluster is no longer needed");
}

#[tokio::test]
async fn a_voice_below_the_name_limit_stays_unknown() {
    let voice_vector_database = InMemoryVoiceVectorDB::new();
    let identity_id = Uuid::new_v4();
    voice_vector_database
        .save_speaker_identity(&SpeakerIdentity::new(identity_id, "Ada"))
        .unwrap_or_else(|error| panic!("{error}"));
    voice_vector_database
        .store_embedding(VoiceEmbedding::new(
            identity_id,
            Uuid::new_v4(),
            vec![0.5, 0.866, 0.0],
        ))
        .unwrap_or_else(|error| panic!("{error}"));
    let (segments, _, _) = run_final_pass(&voice_vector_database, vec![vec![1.0, 0.0, 0.0]]).await;
    assert!(
        segments
            .iter()
            .all(|segment| segment.speaker_identity_id().is_none())
    );
}

#[tokio::test]
async fn without_voice_windows_the_live_result_stands() {
    let meeting_id = MeetingId::new("meeting-final");
    let mut session = MeetingSession::start(
        meeting_id.clone(),
        MockTranscriptionEngine::returning(&["hello"]),
        Some(one_speaker_per_chunk_diarizer(1_000, vec![vec![1.0, 0.0]])),
        VoiceIdentityManager::new(InMemoryVoiceVectorDB::new()),
        sqlite_repository_with_meeting(&meeting_id),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    session
        .process_vad_chunk(&vad_chunk_for("meeting-final", 0, 0, 1_000))
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    let before = session.persisted_segments().to_vec();
    assert_eq!(
        session
            .finish_meeting()
            .unwrap_or_else(|error| panic!("{error}")),
        None
    );
    assert_eq!(session.persisted_segments(), before);
}
