//! Defines telemetry collection and aggregation requirements before implementation.

use std::{cell::Cell, rc::Rc, thread, time::Duration};

use crate::{
    adapters::storage::sqlite_adapter::SqliteNoteRepository,
    application::compute_router::{ComputeDispatcher, ComputePreference},
    domain::{
        errors::TranscriptionError,
        models::{
            AudioRecording, DeviceId, DeviceManifest, MeetingId, RecordingId, TaskMetrics,
            TaskType, Transcript, TranscriptionModel,
        },
    },
    ports::{ai::TranscriptionEngine, telemetry::MetricsRepository},
};

/// Provides a measurable deterministic transcription response without loading a real model.
struct TimedTranscriptionEngine {
    invocation_count: Rc<Cell<u32>>,
}

impl TranscriptionEngine for TimedTranscriptionEngine {
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
    ) -> Result<Transcript, TranscriptionError> {
        self.invocation_count
            .set(self.invocation_count.get().saturating_add(1));
        thread::sleep(Duration::from_millis(2));
        Ok(Transcript::new(
            audio_recording.meeting_id().clone(),
            "metrics fixture",
        ))
    }
}

/// Builds recording metadata for the dispatcher telemetry test.
fn telemetry_recording() -> AudioRecording {
    AudioRecording::new(
        RecordingId::new("telemetry-recording"),
        MeetingId::new("telemetry-meeting"),
        DeviceId::new("capture-device"),
    )
}

/// Builds a single local Whisper-capable device manifest.
fn telemetry_manifest() -> DeviceManifest {
    DeviceManifest::new(
        DeviceId::new("metrics-device"),
        8,
        0,
        vec![TranscriptionModel::Whisper],
    )
}

/// Verifies dispatch records one device-owned metric after a successful transcription.
#[test]
fn transcription_dispatch_persists_duration_device_and_power_metrics() {
    let invocation_count = Rc::new(Cell::new(0));
    let telemetry_repository = SqliteNoteRepository::open_in_memory()
        .unwrap_or_else(|error| panic!("in-memory telemetry repository should open: {error}"));
    let mut dispatcher = ComputeDispatcher::new(
        telemetry_manifest(),
        TimedTranscriptionEngine {
            invocation_count: Rc::clone(&invocation_count),
        },
    );
    dispatcher.set_metrics_repository(telemetry_repository);

    dispatcher
        .dispatch_transcription(
            &telemetry_recording(),
            TranscriptionModel::Whisper,
            &ComputePreference::LocalOnly,
        )
        .unwrap_or_else(|error| panic!("transcription dispatch should succeed: {error}"));

    let metrics = dispatcher
        .metrics_for_device("metrics-device")
        .unwrap_or_else(|error| panic!("metrics should be readable: {error}"));
    assert_eq!(invocation_count.get(), 1);
    assert_eq!(metrics.len(), 1);
    assert_eq!(metrics[0].device_id().as_str(), "metrics-device");
    assert_eq!(metrics[0].task_type(), TaskType::Transcription);
    assert_eq!(metrics[0].model_name(), "whisper");
    assert!(metrics[0].duration_ms() >= 2);
    assert_eq!(metrics[0].latency_ms(), metrics[0].duration_ms());
    assert!(metrics[0].estimated_power_mwh() > 0.0);
}

/// Verifies the SQLite repository groups persisted task metrics into stable aggregates.
#[test]
fn aggregated_stats_sum_duration_and_average_latency_in_memory() {
    let telemetry_repository = SqliteNoteRepository::open_in_memory()
        .unwrap_or_else(|error| panic!("in-memory telemetry repository should open: {error}"));
    telemetry_repository
        .save_metrics(TaskMetrics::new(
            uuid::Uuid::new_v4(),
            DeviceId::new("device-a"),
            TaskType::Transcription,
            "whisper",
            120,
            40,
            0.5,
            1_000,
        ))
        .unwrap_or_else(|error| panic!("first metric should save: {error}"));
    telemetry_repository
        .save_metrics(TaskMetrics::new(
            uuid::Uuid::new_v4(),
            DeviceId::new("device-b"),
            TaskType::Diarization,
            "pyannote",
            180,
            80,
            0.8,
            2_000,
        ))
        .unwrap_or_else(|error| panic!("second metric should save: {error}"));

    let aggregated_stats = telemetry_repository
        .get_aggregated_stats()
        .unwrap_or_else(|error| panic!("aggregated stats should load: {error}"));

    assert_eq!(aggregated_stats.task_count(), 2);
    assert_eq!(aggregated_stats.total_duration_ms(), 300);
    assert_eq!(aggregated_stats.average_latency_ms(), 60);
}
