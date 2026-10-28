//! Specifies per-chunk AI telemetry (Issue #53): every transcribed chunk, and every diarized one,
//! leaves one privacy-safe `TaskMetrics` row, failures leave none, and a telemetry store that
//! cannot write never costs the transcript. Mock AI ports and in-memory SQLite only.

use std::{cell::RefCell, rc::Rc};

use crate::{
    adapters::storage::sqlite_adapter::SqliteNoteRepository,
    application::{
        crypto::voice_encryptor::ephemeral_cipher,
        pipeline::{
            meeting_session::VadAudioChunk,
            pipeline_factory::PipelineFactory,
            task_metrics::{TaskMetricsRecorder, TaskProfile},
        },
    },
    domain::{
        errors::{DiarizationError, StorageError, TranscriptionError},
        models::{
            AggregatedStats, AudioRecording, DeviceId, DeviceLoadStats, MeetingId, RecordingId,
            TaskMetrics, TaskType, Transcript, TranscriptionContext,
        },
    },
    ports::{
        ai::{DiarizationEngine, SpeakerSegment, TranscriptionEngine},
        telemetry::MetricsRepository,
    },
};

const MEETING_ID: &str = "meeting-metrics";

/// Collects saved metrics in memory; clones share the list, or fails every save.
#[derive(Clone, Default)]
struct RecordedMetrics {
    saved_metrics: Rc<RefCell<Vec<TaskMetrics>>>,
    fails: bool,
}

impl MetricsRepository for RecordedMetrics {
    fn save_metrics(&self, metrics: TaskMetrics) -> Result<(), StorageError> {
        if self.fails {
            return Err(StorageError::Unavailable);
        }
        self.saved_metrics.borrow_mut().push(metrics);
        Ok(())
    }

    fn get_metrics_for_device(&self, _device_id: &str) -> Result<Vec<TaskMetrics>, StorageError> {
        Ok(self.saved_metrics.borrow().clone())
    }

    fn get_all_metrics(&self) -> Result<Vec<TaskMetrics>, StorageError> {
        Ok(self.saved_metrics.borrow().clone())
    }

    fn get_aggregated_stats(&self) -> Result<AggregatedStats, StorageError> {
        Err(StorageError::Unavailable)
    }

    fn save_device_load(&self, _device_load_stats: DeviceLoadStats) -> Result<(), StorageError> {
        Ok(())
    }
}

/// Returns a fixed transcript, or fails every call.
struct ScriptedTranscriptionEngine {
    fails: bool,
}

impl TranscriptionEngine for ScriptedTranscriptionEngine {
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        _transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        if self.fails {
            return Err(TranscriptionError::Unavailable);
        }
        Ok(Transcript::new(
            audio_recording.meeting_id().clone(),
            "hello team",
        ))
    }
}

/// Reports one speaker over the whole chunk.
struct OneSpeakerDiarizer;

#[async_trait::async_trait]
impl DiarizationEngine for OneSpeakerDiarizer {
    async fn diarize(&self, _audio_bytes: &[u8]) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        Ok(vec![SpeakerSegment::new(0, 3_000, vec![1.0, 0.0])])
    }
}

/// The recorder every test uses: a local whisper model on `device-metrics`.
fn recorder(metrics: &RecordedMetrics) -> TaskMetricsRecorder {
    TaskMetricsRecorder::new(
        Box::new(metrics.clone()),
        DeviceId::new("device-metrics"),
        TaskProfile::new(TaskType::Transcription, "ggml-small", 5_000.0),
    )
}

/// Opens an isolated in-memory repository with a throwaway embedding key.
fn in_memory_repository() -> SqliteNoteRepository {
    SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .unwrap_or_else(|error| panic!("in-memory repository should open: {error}"))
}

/// A three-second chunk of the test meeting.
fn vad_chunk(sequence: u32) -> VadAudioChunk {
    let start_offset_ms = u64::from(sequence) * 3_000;
    VadAudioChunk::new(
        AudioRecording::new(
            RecordingId::new(format!("recording-{sequence}")),
            MeetingId::new(MEETING_ID),
            DeviceId::new("device-metrics"),
        ),
        start_offset_ms,
        start_offset_ms + 3_000,
        vec![0, 1, 2, 3],
    )
    .unwrap_or_else(|error| panic!("chunk bounds should be valid: {error}"))
}

/// Verifies each transcribed chunk saves one transcription metric for the device and model.
#[tokio::test]
async fn each_transcribed_chunk_records_one_transcription_metric() {
    let metrics = RecordedMetrics::default();
    let factory = PipelineFactory::new(
        ScriptedTranscriptionEngine { fails: false },
        in_memory_repository(),
    )
    .with_task_metrics(recorder(&metrics));
    let mut session = factory
        .create_session(&MeetingId::new(MEETING_ID))
        .unwrap_or_else(|error| panic!("session should start: {error}"));

    for sequence in 0..2 {
        session
            .process_vad_chunk(&vad_chunk(sequence))
            .await
            .unwrap_or_else(|error| panic!("chunk should process: {error}"));
    }

    let saved_metrics = metrics.saved_metrics.borrow();
    assert_eq!(saved_metrics.len(), 2);
    for metric in saved_metrics.iter() {
        assert_eq!(metric.task_type(), TaskType::Transcription);
        assert_eq!(metric.device_id(), &DeviceId::new("device-metrics"));
        assert_eq!(metric.model_name(), "ggml-small");
        assert_eq!(metric.latency_ms(), metric.duration_ms());
        assert!(metric.timestamp_epoch_ms() > 0);
    }
    assert_ne!(saved_metrics[0].task_id(), saved_metrics[1].task_id());
}

/// Verifies the power estimate is the profile's draw over the measured duration.
#[test]
fn power_estimate_is_the_profile_draw_over_the_duration() {
    let profile = TaskProfile::new(TaskType::CloudTranscription, "whisper-1", 3_600.0);

    assert!((profile.estimated_power_mwh(2_000) - 2.0).abs() < f32::EPSILON);
    assert_eq!(profile.estimated_power_mwh(0), 0.0);
}

/// Verifies a failed transcription records nothing: metrics describe completed work only.
#[tokio::test]
async fn failed_transcription_records_no_metric() {
    let metrics = RecordedMetrics::default();
    let factory = PipelineFactory::new(
        ScriptedTranscriptionEngine { fails: true },
        in_memory_repository(),
    )
    .with_task_metrics(recorder(&metrics));
    let mut session = factory
        .create_session(&MeetingId::new(MEETING_ID))
        .unwrap_or_else(|error| panic!("session should start: {error}"));

    let outcome = session.process_vad_chunk(&vad_chunk(0)).await;

    assert!(outcome.is_err());
    assert!(metrics.saved_metrics.borrow().is_empty());
}

/// Verifies a chunk that is diarized also records a diarization metric with its model.
#[tokio::test]
async fn diarized_chunk_also_records_a_diarization_metric() {
    let metrics = RecordedMetrics::default();
    let factory = PipelineFactory::new(
        ScriptedTranscriptionEngine { fails: false },
        in_memory_repository(),
    )
    .with_diarization_engine(Some(OneSpeakerDiarizer))
    .with_task_metrics(recorder(&metrics).with_diarization(TaskProfile::new(
        TaskType::Diarization,
        "pyannote-3.1",
        5_000.0,
    )));
    let mut session = factory
        .create_session(&MeetingId::new(MEETING_ID))
        .unwrap_or_else(|error| panic!("session should start: {error}"));

    session
        .process_vad_chunk(&vad_chunk(0))
        .await
        .unwrap_or_else(|error| panic!("chunk should process: {error}"));

    let recorded: Vec<(TaskType, String)> = metrics
        .saved_metrics
        .borrow()
        .iter()
        .map(|metric| (metric.task_type(), metric.model_name().to_owned()))
        .collect();
    assert_eq!(
        recorded,
        vec![
            (TaskType::Transcription, "ggml-small".to_owned()),
            (TaskType::Diarization, "pyannote-3.1".to_owned()),
        ]
    );
}

/// Verifies a telemetry store that cannot write leaves the transcript intact.
#[tokio::test]
async fn unwritable_metrics_store_does_not_fail_the_chunk() {
    let metrics = RecordedMetrics {
        fails: true,
        ..RecordedMetrics::default()
    };
    let factory = PipelineFactory::new(
        ScriptedTranscriptionEngine { fails: false },
        in_memory_repository(),
    )
    .with_task_metrics(recorder(&metrics));
    let mut session = factory
        .create_session(&MeetingId::new(MEETING_ID))
        .unwrap_or_else(|error| panic!("session should start: {error}"));

    let produced = session
        .process_vad_chunk(&vad_chunk(0))
        .await
        .unwrap_or_else(|error| panic!("telemetry must not fail the chunk: {error}"));

    assert_eq!(produced.len(), 1);
}

/// Verifies sessions without a recorder record nothing and still work (the default).
#[tokio::test]
async fn sessions_without_a_recorder_record_nothing() {
    let factory = PipelineFactory::new(
        ScriptedTranscriptionEngine { fails: false },
        in_memory_repository(),
    );
    let mut session = factory
        .create_session(&MeetingId::new(MEETING_ID))
        .unwrap_or_else(|error| panic!("session should start: {error}"));

    let produced = session
        .process_vad_chunk(&vad_chunk(0))
        .await
        .unwrap_or_else(|error| panic!("chunk should process: {error}"));

    assert_eq!(produced.len(), 1);
}
