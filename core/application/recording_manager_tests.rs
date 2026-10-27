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
