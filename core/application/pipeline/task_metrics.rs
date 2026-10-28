//! Records one privacy-safe telemetry row per AI task the meeting pipeline completes (Issue #53).
//!
//! The Telemetry dashboard reads `TaskMetrics` rows, but recordings never went through
//! `ComputeRouter`, so nothing wrote them. `MeetingSession` now times each chunk's transcription
//! (and diarization, when it runs) and hands the duration to a `TaskMetricsRecorder`, which saves
//! the device, task type, model label, timing and an energy estimate: never audio, text or
//! embeddings. Saving is best effort: a telemetry store that cannot write is logged and the chunk
//! carries on, since a transcript matters more than its statistics.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use uuid::Uuid;

use crate::{
    domain::models::{DeviceId, TaskMetrics, TaskType},
    ports::telemetry::MetricsRepository,
};

/// What one kind of task is recorded as.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskProfile {
    task_type: TaskType,
    /// The model label shown on the dashboard, e.g. a whisper model file name or `whisper-1`.
    model_name: String,
    /// Rough average device draw while the task runs, in milliwatts.
    estimated_power_milliwatts: f32,
}

impl TaskProfile {
    /// Creates a profile.
    ///
    /// Inputs: The task category, a non-sensitive model label, and the device's estimated draw
    /// in milliwatts while the task runs (a heuristic, not a measurement).
    /// Outputs: The profile. Side effects: None.
    pub fn new(
        task_type: TaskType,
        model_name: impl Into<String>,
        estimated_power_milliwatts: f32,
    ) -> Self {
        Self {
            task_type,
            model_name: model_name.into(),
            estimated_power_milliwatts,
        }
    }

    /// Estimates the energy a task of `duration_ms` used.
    ///
    /// Inputs: The task's duration in milliseconds.
    /// Outputs: Milliwatt-hours: the profile's draw times the duration.
    /// Side effects: None.
    pub fn estimated_power_mwh(&self, duration_ms: u64) -> f32 {
        duration_ms as f32 * self.estimated_power_milliwatts / 3_600_000.0
    }
}

/// Saves a `TaskMetrics` row for each completed pipeline task.
pub struct TaskMetricsRecorder {
    metrics_repository: Box<dyn MetricsRepository>,
    /// The device doing the work; every row is stamped with it.
    device_id: DeviceId,
    transcription: TaskProfile,
    /// `None` records diarization under a generic label.
    diarization: Option<TaskProfile>,
}

impl TaskMetricsRecorder {
    /// Creates a recorder for transcriptions.
    ///
    /// Inputs: The telemetry store (its own connection; the pipeline's worker thread owns it),
    /// this device's ID, and how transcriptions are recorded (`Transcription` on device,
    /// `CloudTranscription` through a provider).
    /// Outputs: The recorder. Side effects: None.
    pub fn new(
        metrics_repository: Box<dyn MetricsRepository>,
        device_id: DeviceId,
        transcription: TaskProfile,
    ) -> Self {
        Self {
            metrics_repository,
            device_id,
            transcription,
            diarization: None,
        }
    }

    /// Sets how diarization is recorded.
    ///
    /// Inputs: The diarization profile. Outputs: The recorder. Side effects: None.
    pub fn with_diarization(mut self, diarization: TaskProfile) -> Self {
        self.diarization = Some(diarization);
        self
    }

    /// Records one completed transcription.
    ///
    /// Inputs: How long the engine took. Outputs: None.
    /// Side effects: Saves one row; a failed save is logged as a warning and otherwise ignored.
    pub fn record_transcription(&self, elapsed: Duration) {
        self.record(&self.transcription, elapsed);
    }

    /// Records one completed diarization.
    ///
    /// Inputs: How long the engine took. Outputs: None.
    /// Side effects: Saves one row (labelled `diarization` without a profile); a failed save is
    /// logged as a warning and otherwise ignored.
    pub fn record_diarization(&self, elapsed: Duration) {
        match &self.diarization {
            Some(profile) => self.record(profile, elapsed),
            None => self.record(
                &TaskProfile::new(TaskType::Diarization, "diarization", 0.0),
                elapsed,
            ),
        }
    }

    /// Saves one row for `profile`.
    ///
    /// Inputs: The task's profile and duration. Outputs: None.
    /// Side effects: Writes to the telemetry store; logs a failure without task data.
    fn record(&self, profile: &TaskProfile, elapsed: Duration) {
        let duration_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        let completed_epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|since_epoch| u64::try_from(since_epoch.as_millis()).unwrap_or(u64::MAX))
            .unwrap_or(0);
        // The pipeline runs each task as soon as its chunk arrives, so latency equals duration.
        let task_metrics = TaskMetrics::new(
            Uuid::new_v4(),
            self.device_id.clone(),
            profile.task_type,
            profile.model_name.clone(),
            duration_ms,
            duration_ms,
            profile.estimated_power_mwh(duration_ms),
            completed_epoch_ms,
        );
        if let Err(error) = self.metrics_repository.save_metrics(task_metrics) {
            tracing::warn!(
                task_type = profile.task_type.as_str(),
                error = %error,
                "Unable to save AI task telemetry; the chunk is unaffected"
            );
        }
    }
}
