//! Specifies the on-device speed check runner (#109): it times Automatic's model on the clip, saves
//! each run to telemetry, checks a smaller model when that one falls behind, stops when the device
//! is needed, and leaves models already measured alone.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};

use crate::{
    adapters::audio::chunk_audio_store::ChunkAudioStore,
    domain::{
        errors::{StorageError, TranscriptionError},
        models::{
            AggregatedStats, AudioRecording, DeviceId, DeviceLoadStats, TaskMetrics, TaskType,
            Transcript, TranscriptionContext,
        },
        whisper_speed_check::{CHECK_RUNS, SpeedCheckRecord},
    },
    ports::{ai::TranscriptionEngine, telemetry::MetricsRepository},
    speed_check::{CLIP_MS, CheckTarget, SpeedCheck},
};

/// Telemetry in memory, shared by every repository the runner opens.
#[derive(Clone, Default)]
struct Metrics(Arc<Mutex<Vec<TaskMetrics>>>);

impl Metrics {
    fn rows(&self) -> Vec<TaskMetrics> {
        self.0.lock().map(|rows| rows.clone()).unwrap_or_default()
    }
}

impl MetricsRepository for Metrics {
    fn save_metrics(&self, metrics: TaskMetrics) -> Result<(), StorageError> {
        self.0
            .lock()
            .map_err(|_| StorageError::Unavailable)?
            .push(metrics);
        Ok(())
    }

    fn get_metrics_for_device(&self, _device_id: &str) -> Result<Vec<TaskMetrics>, StorageError> {
        Ok(self.rows())
    }

    fn get_all_metrics(&self) -> Result<Vec<TaskMetrics>, StorageError> {
        Ok(self.rows())
    }

    fn get_aggregated_stats(&self) -> Result<AggregatedStats, StorageError> {
        Err(StorageError::Unavailable)
    }

    fn save_device_load(&self, _stats: DeviceLoadStats) -> Result<(), StorageError> {
        Ok(())
    }
}

/// Moves the fake clock on by the model's factor times the clip on every transcription.
struct Engine {
    factor: f32,
    clock_ms: Arc<AtomicU64>,
    audio: Arc<ChunkAudioStore>,
    transcriptions: Arc<AtomicUsize>,
}

impl TranscriptionEngine for Engine {
    fn transcribe(
        &self,
        recording: &AudioRecording,
        _context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        assert!(
            self.audio.discard(recording.recording_id()),
            "the clip is in the store"
        );
        self.transcriptions.fetch_add(1, Ordering::SeqCst);
        self.clock_ms
            .fetch_add((CLIP_MS as f32 * self.factor) as u64, Ordering::SeqCst);
        Ok(Transcript::new(recording.meeting_id().clone(), "testo"))
    }
}

/// A runner over a temporary record file, with models `small` and `base` on the device.
struct Setup {
    folder: PathBuf,
    metrics: Metrics,
    loaded: Arc<Mutex<Vec<PathBuf>>>,
    transcriptions: Arc<AtomicUsize>,
    automatic: bool,
}

impl Setup {
    fn new(name: &str) -> Self {
        let folder =
            std::env::temp_dir().join(format!("speed-check-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&folder).ok();
        Self {
            folder,
            metrics: Metrics::default(),
            loaded: Arc::default(),
            transcriptions: Arc::default(),
            automatic: true,
        }
    }

    fn record_file(&self) -> PathBuf {
        self.folder.join("db.speed-check.json")
    }

    fn record(&self) -> Option<SpeedCheckRecord> {
        std::fs::read(self.record_file())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
    }

    /// The runner, with `small` running at `small_factor` and `base` at `base_factor`.
    fn check(&self, small_factor: f32, base_factor: f32) -> SpeedCheck {
        let automatic = self.automatic;
        let target = Box::new(move || {
            automatic.then(|| CheckTarget {
                automatic_file: "ggml-small.bin".to_owned(),
                efficient: false,
                hardware_key: "test hardware".to_owned(),
                models: BTreeMap::from([
                    (
                        "ggml-small.bin".to_owned(),
                        PathBuf::from("/m/ggml-small.bin"),
                    ),
                    (
                        "ggml-base.bin".to_owned(),
                        PathBuf::from("/m/ggml-base.bin"),
                    ),
                ]),
            })
        });
        let clock_ms = Arc::new(AtomicU64::new(0));
        let engine_clock = Arc::clone(&clock_ms);
        let loaded = Arc::clone(&self.loaded);
        let transcriptions = Arc::clone(&self.transcriptions);
        let make_engine = Box::new(
            move |model: &Path, _efficient: bool, audio: Arc<ChunkAudioStore>| {
                if let Ok(mut loaded) = loaded.lock() {
                    loaded.push(model.to_path_buf());
                }
                let factor = if model.ends_with("ggml-small.bin") {
                    small_factor
                } else {
                    base_factor
                };
                Ok(Box::new(Engine {
                    factor,
                    clock_ms: Arc::clone(&engine_clock),
                    audio,
                    transcriptions: Arc::clone(&transcriptions),
                }) as Box<dyn TranscriptionEngine>)
            },
        );
        let metrics = self.metrics.clone();
        SpeedCheck::new(
            target,
            make_engine,
            Box::new(move || Ok(Box::new(metrics.clone()) as Box<dyn MetricsRepository>)),
            self.record_file(),
            DeviceId::new("phone"),
        )
        .with_stopwatch(Box::new(move || {
            Duration::from_millis(clock_ms.load(Ordering::SeqCst))
        }))
    }
}

impl Drop for Setup {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.folder);
    }
}

#[test]
fn automatics_model_is_timed_once_and_saved_to_telemetry() {
    let setup = Setup::new("once");
    assert_eq!(setup.check(0.3, 0.1).run(&|| true), 1);

    let rows = setup.metrics.rows();
    assert_eq!(rows.len(), CHECK_RUNS);
    assert!(rows.iter().all(|row| row.model_name() == "ggml-small"
        && row.task_type() == TaskType::Transcription
        && row.audio_duration_ms() == Some(CLIP_MS)));
    let record = setup.record().unwrap_or_default();
    assert_eq!(record.hardware_key, "test hardware");
    assert_eq!(record.models.len(), 1);
    assert!((record.models[0].real_time_factor - 0.3).abs() < 0.01);

    assert_eq!(setup.check(0.3, 0.1).run(&|| true), 0, "not timed again");
    assert_eq!(setup.metrics.rows().len(), CHECK_RUNS);
}

#[test]
fn a_model_too_slow_has_the_smaller_one_timed_in_the_same_round() {
    let setup = Setup::new("slow");
    assert_eq!(setup.check(1.4, 0.4).run(&|| true), 2);

    assert_eq!(
        setup
            .loaded
            .lock()
            .map(|loaded| loaded.clone())
            .unwrap_or_default(),
        vec![
            PathBuf::from("/m/ggml-small.bin"),
            PathBuf::from("/m/ggml-base.bin")
        ]
    );
    let record = setup.record().unwrap_or_default();
    let files: Vec<&str> = record
        .models
        .iter()
        .map(|model| model.file_name.as_str())
        .collect();
    assert_eq!(files, vec!["ggml-small.bin", "ggml-base.bin"]);
}

#[test]
fn the_check_stops_when_the_device_is_needed_and_keeps_no_result() {
    let setup = Setup::new("stop");
    let transcriptions = Arc::clone(&setup.transcriptions);
    let checked = setup
        .check(0.3, 0.1)
        .run(&move || transcriptions.load(Ordering::SeqCst) < 3);
    assert_eq!(checked, 0);
    assert_eq!(setup.transcriptions.load(Ordering::SeqCst), 3);
    assert_eq!(setup.record(), None, "a partial check is not a result");
}

#[test]
fn nothing_is_timed_without_automatic_or_once_recordings_measured_it() {
    let mut setup = Setup::new("skip");
    setup.automatic = false;
    assert_eq!(setup.check(0.3, 0.1).run(&|| true), 0);
    assert!(setup.loaded.lock().map(|l| l.is_empty()).unwrap_or(false));

    setup.automatic = true;
    for _ in 0..10 {
        let _ = setup.metrics.save_metrics(
            TaskMetrics::new(
                uuid::Uuid::new_v4(),
                DeviceId::new("phone"),
                TaskType::Transcription,
                "ggml-small",
                5_000,
                5_000,
                1.0,
                0,
            )
            .with_audio_duration_ms(Some(15_000)),
        );
    }
    assert_eq!(setup.check(0.3, 0.1).run(&|| true), 0);
    assert!(setup.loaded.lock().map(|l| l.is_empty()).unwrap_or(false));
}

#[test]
fn the_clip_is_fifteen_seconds_of_sixteen_kilohertz_speech() {
    assert_eq!(crate::speed_check::clip_samples().len(), 15 * 16_000);
}

#[test]
fn a_transcript_of_the_clip_is_recognised_and_garbage_is_not() {
    use crate::speed_check::clip_is_understood;
    // whisper.cpp small's wording, and the reference (VoxPopuli's gold transcript).
    assert!(clip_is_understood(
        "Sono inoltre d'accordo con il commissario Füle e con i colleghi che mi hanno preceduto, \
         che esistono ragioni morali e di giustizia"
    ));
    assert!(
        clip_is_understood(
            "sono inoltre daccordo col Commissario e coi colleghi che hanno preceduto: ragioni morali"
        ),
        "a few words missed or misspelled still pass"
    );
    assert!(
        !clip_is_understood("@@@@@@@@@@@@@@@@@@@@@@@@"),
        "garbled output (#183)"
    );
    assert!(!clip_is_understood(""), "nothing");
    assert!(
        !clip_is_understood(
            "I also agree with the Commissioner and with colleagues who spoke before me"
        ),
        "another language is not this clip"
    );
}
