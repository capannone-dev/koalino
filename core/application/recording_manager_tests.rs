//! Specifies the recording manager's worker lifecycle: FIFO chunk processing, error isolation,
//! draining on stop, builder failures, and engine reuse. Engines are mocks; storage is in-memory.

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

use crate::{
    adapters::storage::sqlite_adapter::SqliteNoteRepository,
    application::{
        crypto::voice_encryptor::ephemeral_cipher,
        pipeline::{
            meeting_session::VadAudioChunk,
            pipeline_factory::{NoDiarizationEngine, NoVoiceMatcher, PipelineFactory},
        },
        recording_manager::{RecordingManager, RecordingManagerError},
    },
    domain::{
        errors::TranscriptionError,
        models::{
            AudioRecording, DeviceId, MeetingId, RecordingId, Transcript, TranscriptionContext,
        },
    },
    ports::ai::TranscriptionEngine,
};

/// Counts calls, optionally sleeps per call, and fails for one chosen recording ID.
struct ScriptedTranscriptionEngine {
    call_count: Arc<AtomicUsize>,
    /// Simulated inference time per chunk.
    delay_per_call: Duration,
    /// Recording ID whose transcription fails, if any.
    failing_recording_id: Option<&'static str>,
}

impl TranscriptionEngine for ScriptedTranscriptionEngine {
    /// Counts the call, waits the simulated inference time, and returns a transcript or failure.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        _transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        thread::sleep(self.delay_per_call);
        if self.failing_recording_id == Some(audio_recording.recording_id().as_str()) {
            return Err(TranscriptionError::Unavailable);
        }
        Ok(Transcript::new(
            audio_recording.meeting_id().clone(),
            "chunk words",
        ))
    }
}

type TestFactory = PipelineFactory<
    ScriptedTranscriptionEngine,
    NoDiarizationEngine,
    NoVoiceMatcher,
    SqliteNoteRepository,
>;

/// Builds a factory with an in-memory repository around the scripted engine.
fn test_factory(
    call_count: Arc<AtomicUsize>,
    delay_per_call: Duration,
    failing_recording_id: Option<&'static str>,
) -> Result<TestFactory, String> {
    let note_repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .map_err(|error| error.to_string())?;
    Ok(PipelineFactory::new(
        ScriptedTranscriptionEngine {
            call_count,
            delay_per_call,
            failing_recording_id,
        },
        note_repository,
    ))
}

/// Spawns a manager whose worker builds the test factory.
fn spawn_manager(
    call_count: &Arc<AtomicUsize>,
    delay_per_call: Duration,
    failing_recording_id: Option<&'static str>,
) -> RecordingManager {
    let worker_call_count = Arc::clone(call_count);
    RecordingManager::spawn(move || {
        test_factory(worker_call_count, delay_per_call, failing_recording_id)
    })
    .unwrap_or_else(|error| panic!("manager should start: {error}"))
}

/// Builds a two-second chunk numbered `sequence` for `meeting_id`.
fn vad_chunk(meeting_id: &str, sequence: u64) -> VadAudioChunk {
    VadAudioChunk::new(
        AudioRecording::new(
            RecordingId::new(format!("chunk-{sequence}")),
            MeetingId::new(meeting_id),
            DeviceId::new("device-test"),
        ),
        sequence * 2_000,
        sequence * 2_000 + 2_000,
        vec![0; 8],
    )
    .unwrap_or_else(|error| panic!("chunk bounds should be valid: {error}"))
}

/// Verifies chunks submitted during a recording are all processed and summarized on stop.
#[test]
fn stopping_processes_every_submitted_chunk() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let manager = spawn_manager(&call_count, Duration::ZERO, None);
    let meeting_id = MeetingId::new("meeting-three-chunks");

    manager
        .begin_recording(meeting_id.clone())
        .unwrap_or_else(|error| panic!("recording should begin: {error}"));
    for sequence in 0..3 {
        manager
            .submit_chunk(vad_chunk("meeting-three-chunks", sequence))
            .unwrap_or_else(|error| panic!("chunk should be queued: {error}"));
    }
    let summary = manager
        .end_recording()
        .unwrap_or_else(|error| panic!("recording should end: {error}"));

    assert_eq!(summary.meeting_id(), &meeting_id);
    assert_eq!(summary.processed_chunk_count(), 3);
    assert_eq!(summary.failed_chunk_count(), 0);
    assert_eq!(summary.segment_count(), 3);
    assert_eq!(call_count.load(Ordering::SeqCst), 3);
}

/// Verifies one failing chunk is counted and logged while later chunks keep processing.
#[test]
fn a_failing_chunk_does_not_stop_the_recording() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let manager = spawn_manager(&call_count, Duration::ZERO, Some("chunk-1"));

    manager
        .begin_recording(MeetingId::new("meeting-failure"))
        .unwrap_or_else(|error| panic!("recording should begin: {error}"));
    for sequence in 0..3 {
        manager
            .submit_chunk(vad_chunk("meeting-failure", sequence))
            .unwrap_or_else(|error| panic!("chunk should be queued: {error}"));
    }
    let summary = manager
        .end_recording()
        .unwrap_or_else(|error| panic!("recording should end: {error}"));

    assert_eq!(summary.processed_chunk_count(), 2);
    assert_eq!(summary.failed_chunk_count(), 1);
    assert_eq!(summary.segment_count(), 2);
}

/// Verifies stop waits for slow in-flight chunks instead of dropping them.
#[test]
fn stopping_waits_for_in_flight_chunks() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let manager = spawn_manager(&call_count, Duration::from_millis(40), None);

    manager
        .begin_recording(MeetingId::new("meeting-slow"))
        .unwrap_or_else(|error| panic!("recording should begin: {error}"));
    for sequence in 0..4 {
        manager
            .submit_chunk(vad_chunk("meeting-slow", sequence))
            .unwrap_or_else(|error| panic!("chunk should be queued: {error}"));
    }
    assert!(manager.queued_chunk_count() > 0);
    let summary = manager
        .end_recording()
        .unwrap_or_else(|error| panic!("recording should end: {error}"));

    assert_eq!(summary.processed_chunk_count(), 4);
    assert_eq!(call_count.load(Ordering::SeqCst), 4);
    assert_eq!(manager.queued_chunk_count(), 0);
}

/// Verifies a builder failure is reported by `spawn` instead of a dead worker.
#[test]
fn spawn_reports_a_pipeline_build_failure() {
    let spawn_result = RecordingManager::spawn(|| -> Result<TestFactory, String> {
        Err("no transcription engine is configured".to_owned())
    });

    match spawn_result {
        Err(RecordingManagerError::PipelineUnavailable { reason }) => {
            assert!(reason.contains("no transcription engine"));
        }
        Err(other) => panic!("unexpected error: {other}"),
        Ok(_) => panic!("spawn should fail when the pipeline cannot be built"),
    }
}

/// Verifies two consecutive recordings share one engine and keep separate meetings.
#[test]
fn consecutive_recordings_reuse_the_pipeline() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let manager = spawn_manager(&call_count, Duration::ZERO, None);

    for meeting in ["meeting-first", "meeting-second"] {
        manager
            .begin_recording(MeetingId::new(meeting))
            .unwrap_or_else(|error| panic!("recording should begin: {error}"));
        manager
            .submit_chunk(vad_chunk(meeting, 0))
            .unwrap_or_else(|error| panic!("chunk should be queued: {error}"));
        let summary = manager
            .end_recording()
            .unwrap_or_else(|error| panic!("recording should end: {error}"));
        assert_eq!(summary.processed_chunk_count(), 1);
    }

    assert_eq!(call_count.load(Ordering::SeqCst), 2);
}

/// Verifies lifecycle misuse is rejected without disturbing the worker.
#[test]
fn lifecycle_misuse_is_rejected() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let manager = spawn_manager(&call_count, Duration::ZERO, None);

    assert!(matches!(
        manager.end_recording(),
        Err(RecordingManagerError::NotRecording)
    ));
    assert!(matches!(
        manager.submit_chunk(vad_chunk("meeting-idle", 0)),
        Err(RecordingManagerError::NotRecording)
    ));
    manager
        .begin_recording(MeetingId::new("meeting-busy"))
        .unwrap_or_else(|error| panic!("recording should begin: {error}"));
    assert!(matches!(
        manager.begin_recording(MeetingId::new("meeting-other")),
        Err(RecordingManagerError::AlreadyRecording)
    ));
    assert!(manager.is_recording());
    manager
        .end_recording()
        .unwrap_or_else(|error| panic!("recording should end: {error}"));
    assert!(!manager.is_recording());
}

/// Verifies dropping the manager mid-recording still drains queued chunks into storage.
#[test]
fn dropping_the_manager_drains_the_active_recording() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let manager = spawn_manager(&call_count, Duration::from_millis(10), None);

    manager
        .begin_recording(MeetingId::new("meeting-dropped"))
        .unwrap_or_else(|error| panic!("recording should begin: {error}"));
    for sequence in 0..3 {
        manager
            .submit_chunk(vad_chunk("meeting-dropped", sequence))
            .unwrap_or_else(|error| panic!("chunk should be queued: {error}"));
    }
    drop(manager);

    assert_eq!(call_count.load(Ordering::SeqCst), 3);
}

/// Verifies chunk results reach the repository the factory owns (the session is real).
#[test]
fn processed_segments_are_persisted() {
    let manager = RecordingManager::spawn(|| -> Result<_, String> {
        let note_repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
            .map_err(|error| error.to_string())?;
        Ok(PipelineFactory::new(
            ScriptedTranscriptionEngine {
                call_count: Arc::new(AtomicUsize::new(0)),
                delay_per_call: Duration::ZERO,
                failing_recording_id: None,
            },
            note_repository,
        ))
    })
    .unwrap_or_else(|error| panic!("manager should start: {error}"));
    manager
        .begin_recording(MeetingId::new("meeting-persisted"))
        .unwrap_or_else(|error| panic!("recording should begin: {error}"));
    manager
        .submit_chunk(vad_chunk("meeting-persisted", 0))
        .unwrap_or_else(|error| panic!("chunk should be queued: {error}"));
    let summary = manager
        .end_recording()
        .unwrap_or_else(|error| panic!("recording should end: {error}"));

    let recorded_meeting_id = summary.meeting_id().clone();
    let stored_segments = manager
        .with_note_repository(move |note_repository| {
            note_repository.load_transcript_segments(&recorded_meeting_id)
        })
        .unwrap_or_else(|error| panic!("worker should answer: {error}"))
        .unwrap_or_else(|error| panic!("segments should load: {error}"));
    assert_eq!(stored_segments.len(), 1);
}

/// Remembers external-ID mappings in memory, like the SQLite repository does on disk.
#[derive(Default)]
struct InMemoryExternalIdentities {
    mappings:
        std::sync::Mutex<std::collections::HashMap<String, crate::domain::models::SpeakerIdentity>>,
}

impl crate::ports::external_bridge::ExternalIdentityRepository for InMemoryExternalIdentities {
    /// Returns the remembered identity for `external_id`.
    fn find_by_external_id(
        &self,
        external_id: &str,
    ) -> Result<Option<crate::domain::models::SpeakerIdentity>, crate::domain::errors::StorageError>
    {
        Ok(self
            .mappings
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(external_id)
            .cloned())
    }

    /// Remembers `identity` for `external_id`.
    fn save_external_mapping(
        &self,
        external_id: &str,
        identity: &crate::domain::models::SpeakerIdentity,
    ) -> Result<(), crate::domain::errors::StorageError> {
        self.mappings
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(external_id.to_owned(), identity.clone());
        Ok(())
    }
}

/// Builds a shared Teams identity mapper over in-memory mappings.
fn shared_identity_mapper() -> crate::application::recording_manager::SharedIdentityMapper {
    Arc::new(
        crate::application::teams_processor::TeamsIdentityMapper::new(
            InMemoryExternalIdentities::default(),
        ),
    )
}

/// Builds a 1 s external chunk from `external_id` at `timestamp_ms` for `meeting_id`.
fn external_chunk(
    meeting_id: &str,
    external_id: &str,
    timestamp_ms: u64,
) -> crate::application::recording_manager::ExternalChunkRequest {
    crate::application::recording_manager::ExternalChunkRequest {
        media_chunk: crate::domain::models::ExternalMediaChunk::new(
            crate::domain::models::ExternalParticipant::new(
                external_id.to_owned(),
                format!("Name of {external_id}"),
                None,
            ),
            vec![0; 32_000],
            timestamp_ms,
        ),
        audio_recording: AudioRecording::new(
            RecordingId::new(format!("external-{timestamp_ms}")),
            MeetingId::new(meeting_id),
            DeviceId::new("device-test"),
        ),
        duration_ms: 1_000,
    }
}

/// Verifies external chunks become Known segments, alongside an independent local recording.
#[test]
fn external_chunks_are_processed_into_known_segments() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let manager = spawn_manager(&call_count, Duration::ZERO, None);
    manager
        .begin_external_meeting(MeetingId::new("teams-meeting"), shared_identity_mapper())
        .unwrap_or_else(|error| panic!("external meeting should begin: {error}"));
    manager
        .begin_recording(MeetingId::new("local-meeting"))
        .unwrap_or_else(|error| panic!("a local recording may run alongside: {error}"));

    for (index, external_id) in ["teams-user-1", "teams-user-2", "teams-user-1"]
        .into_iter()
        .enumerate()
    {
        let produced_segments = manager
            .process_external_chunk(external_chunk(
                "teams-meeting",
                external_id,
                u64::try_from(index).unwrap_or(0) * 1_000,
            ))
            .unwrap_or_else(|error| panic!("external chunk should process: {error}"));
        assert_eq!(produced_segments, 1);
    }
    let summary = manager
        .end_external_meeting()
        .unwrap_or_else(|error| panic!("external meeting should end: {error}"));
    assert_eq!(summary.processed_chunk_count(), 3);
    assert_eq!(summary.segment_count(), 3);
    assert!(manager.is_recording(), "the local recording is unaffected");

    let stored_segments = manager
        .with_note_repository(|note_repository| {
            note_repository.load_transcript_segments(&MeetingId::new("teams-meeting"))
        })
        .unwrap_or_else(|error| panic!("worker should answer: {error}"))
        .unwrap_or_else(|error| panic!("segments should load: {error}"));
    let speaker_ids: Vec<_> = stored_segments
        .iter()
        .map(|segment| segment.speaker_identity_id())
        .collect();
    assert!(speaker_ids.iter().all(Option::is_some), "{speaker_ids:?}");
    assert_eq!(
        speaker_ids[0], speaker_ids[2],
        "the same participant maps to one identity"
    );
    assert_ne!(speaker_ids[0], speaker_ids[1]);
}

/// Verifies re-beginning the same meeting resumes it and a new meeting ends the previous one.
#[test]
fn beginning_an_external_meeting_resumes_or_replaces() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let manager = spawn_manager(&call_count, Duration::ZERO, None);
    let mapper = shared_identity_mapper();
    for _reconnect in 0..2 {
        manager
            .begin_external_meeting(MeetingId::new("teams-a"), Arc::clone(&mapper))
            .unwrap_or_else(|error| panic!("external meeting should begin: {error}"));
        manager
            .process_external_chunk(external_chunk("teams-a", "teams-user-1", 0))
            .unwrap_or_else(|error| panic!("external chunk should process: {error}"));
    }
    manager
        .begin_external_meeting(MeetingId::new("teams-b"), mapper)
        .unwrap_or_else(|error| panic!("a new meeting should begin: {error}"));
    let wrong_meeting =
        manager.process_external_chunk(external_chunk("teams-a", "teams-user-1", 0));
    assert!(matches!(
        wrong_meeting,
        Err(RecordingManagerError::Session(_))
    ));

    let summary = manager
        .end_external_meeting()
        .unwrap_or_else(|error| panic!("external meeting should end: {error}"));
    assert_eq!(summary.meeting_id(), &MeetingId::new("teams-b"));
    assert_eq!(summary.processed_chunk_count(), 0);
    assert_eq!(summary.failed_chunk_count(), 1);
}

/// Verifies external chunks and ends without a meeting are rejected.
#[test]
fn external_commands_require_an_external_meeting() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let manager = spawn_manager(&call_count, Duration::ZERO, None);
    assert!(matches!(
        manager.process_external_chunk(external_chunk("teams-a", "teams-user-1", 0)),
        Err(RecordingManagerError::NotRecording)
    ));
    assert!(matches!(
        manager.end_external_meeting(),
        Err(RecordingManagerError::NotRecording)
    ));
    assert_eq!(call_count.load(Ordering::SeqCst), 0);
}
