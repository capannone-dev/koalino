//! Times a short public clip with Automatic's Whisper model on this device (#109).
//!
//! Automatic picks a model from the hardware, which on an unknown device is an estimate. When
//! Automatic is on and no recording has measured its model here yet, the after-meeting worker
//! runs this check: the 15 s clip in `core/assets` (Italian speech from the European Parliament,
//! VoxPopuli, CC0) is transcribed `CHECK_RUNS` times, and each run is saved to telemetry like a
//! recording's chunk, so the #68 rule counts the model as measured and proposes a smaller one when
//! it falls behind. A model that falls behind has the next smaller model on the device timed too,
//! until one keeps up. Results go to `<database>.speed-check.json` with a key for the hardware and
//! the models, so new hardware or a downloaded model checks again; Settings words them.
//!
//! The worker runs it only when the device is free (no recording; on a phone, while charging), and
//! the check stops between runs when that changes, keeping no result. Nothing is logged but the
//! model's name and its factor.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::{
    adapters::audio::chunk_audio_store::ChunkAudioStore,
    domain::{
        models::{
            AudioRecording, DeviceId, MeetingId, RecordingId, TaskMetrics, TaskType,
            TranscriptionContext,
        },
        whisper_proposal::{TranscriptionMeasurement, is_measured},
        whisper_speed_check::{
            CHECK_RUNS, SpeedCheckRecord, next_model_to_check, real_time_factor, with_result,
        },
    },
    pipeline_composition::{LOCAL_TRANSCRIPTION_POWER_MILLIWATTS, local_model_label},
    ports::{ai::TranscriptionEngine, telemetry::MetricsRepository},
};

/// The clip: 15 s of 16 kHz mono 16-bit speech.
const CLIP_WAV: &[u8] = include_bytes!("../assets/speed_check_clip.wav");
/// The clip's length, which each telemetry row records as its audio.
pub const CLIP_MS: u64 = 15_000;

/// What to check, read from the settings at each round.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckTarget {
    /// Automatic's model, such as `ggml-small.bin`.
    pub automatic_file: String,
    /// Whether efficient mode is part of Automatic's choice, so the check runs as recordings do.
    pub efficient: bool,
    /// The hardware and the models on the device; a change checks again.
    pub hardware_key: String,
    /// The models on the device, by file name.
    pub models: BTreeMap<String, PathBuf>,
}

/// Reads the target; `None` when Automatic is off, transcription is not local, or no model is here.
pub type TargetSource = Box<dyn Fn() -> Option<CheckTarget> + Send>;
/// Loads a model (path, efficient mode) reading audio from the given store.
pub type CheckEngineFactory = Box<
    dyn Fn(&Path, bool, Arc<ChunkAudioStore>) -> anyhow::Result<Box<dyn TranscriptionEngine>>
        + Send,
>;
/// Opens the telemetry store.
pub type MetricsFactory = Box<dyn Fn() -> anyhow::Result<Box<dyn MetricsRepository>> + Send>;

/// The record's file beside the database: `<database>.speed-check.json`.
///
/// Inputs: The database path. Outputs: The path. Side effects: None.
pub fn record_file_for(database_path: &Path) -> PathBuf {
    let mut record_file = database_path.as_os_str().to_owned();
    record_file.push(".speed-check.json");
    PathBuf::from(record_file)
}

/// Reads the record; an unreadable or missing file is an empty record.
///
/// Side effects: Reads the file.
pub fn load_record(record_file: &Path) -> SpeedCheckRecord {
    std::fs::read(record_file)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Distinctive words of the clip's opening (VoxPopuli's transcript: "Sono inoltre d'accordo con il
/// Commissario Füle e con i colleghi che mi hanno preceduto, che esistono ragioni morali e di
/// giustizia…"), to tell a transcript of it from garbled output.
const CLIP_WORDS: [&str; 8] = [
    "inoltre",
    "accordo",
    "commissario",
    "colleghi",
    "preceduto",
    "ragioni",
    "morali",
    "giustizia",
];
/// How many of them a transcript must contain: a model may miss or misspell a few.
const CLIP_WORDS_NEEDED: usize = 4;

/// Reports whether a text is a transcript of the clip, not garbled output (#183).
///
/// Inputs: The transcript. Outputs: `true` when it holds at least `CLIP_WORDS_NEEDED` of the clip's
/// distinctive words, ignoring case and punctuation. Side effects: None.
pub fn clip_is_understood(text: &str) -> bool {
    let lowered = text.to_lowercase();
    CLIP_WORDS
        .iter()
        .filter(|word| lowered.contains(*word))
        .count()
        >= CLIP_WORDS_NEEDED
}

/// The clip's samples.
///
/// Outputs: The 16-bit samples; empty if the embedded file were not a mono 16-bit WAV.
/// Side effects: None.
pub fn clip_samples() -> Vec<i16> {
    crate::domain::pcm_wav::pcm16_mono_samples(CLIP_WAV)
        .map(|(_, bytes)| {
            let (pairs, _odd_byte) = bytes.as_chunks::<2>();
            pairs.iter().map(|pair| i16::from_le_bytes(*pair)).collect()
        })
        .unwrap_or_default()
}

/// Whether a model needs timing on this device, so charging should start the worker for it.
///
/// Inputs: The database, its telemetry, and this device's ID.
/// Outputs: true when Automatic's model, or a smaller one after it fell behind, is due.
/// Side effects: Reads the settings, the record and telemetry; may start whisper.cpp's backends
/// to find a GPU.
pub fn is_due(database_path: &Path, metrics: &dyn MetricsRepository, device_id: &str) -> bool {
    let Some(target) = crate::ffi_transcription::speed_check_target(database_path) else {
        return false;
    };
    let measurements = transcription_measurements(metrics, device_id);
    next_model_to_check(
        &target.automatic_file,
        &load_record(&record_file_for(database_path)),
        &target.hardware_key,
        &|file| is_measured(&measurements, file),
        &|file| target.models.contains_key(file),
    )
    .is_some()
}

/// A device's transcription measurements, for the #68 rule.
///
/// Side effects: Reads telemetry; a failure reads as none.
fn transcription_measurements(
    metrics: &dyn MetricsRepository,
    device_id: &str,
) -> Vec<TranscriptionMeasurement> {
    metrics
        .get_metrics_for_device(device_id)
        .unwrap_or_default()
        .iter()
        .filter(|metric| metric.task_type() == TaskType::Transcription)
        .map(|metric| TranscriptionMeasurement {
            model_label: metric.model_name().to_owned(),
            audio_ms: metric.audio_duration_ms().unwrap_or(0),
            processing_ms: metric.duration_ms(),
            power_mwh: metric.estimated_power_mwh(),
        })
        .collect()
}

/// Runs the check for the after-meeting worker.
pub struct SpeedCheck {
    target: TargetSource,
    make_engine: CheckEngineFactory,
    open_metrics: MetricsFactory,
    record_file: PathBuf,
    device_id: DeviceId,
    /// Monotonic time; replaced in tests.
    stopwatch: Box<dyn Fn() -> Duration + Send>,
}

impl SpeedCheck {
    /// Creates the check.
    ///
    /// Inputs: Where the target comes from, how to load a model, how to open telemetry, the record
    /// file, and this device. Outputs: The check. Side effects: None.
    pub fn new(
        target: TargetSource,
        make_engine: CheckEngineFactory,
        open_metrics: MetricsFactory,
        record_file: PathBuf,
        device_id: DeviceId,
    ) -> Self {
        let origin = Instant::now();
        Self {
            target,
            make_engine,
            open_metrics,
            record_file,
            device_id,
            stopwatch: Box::new(move || origin.elapsed()),
        }
    }

    /// Replaces the clock, so tests decide how long a transcription takes.
    #[must_use]
    pub fn with_stopwatch(mut self, stopwatch: Box<dyn Fn() -> Duration + Send>) -> Self {
        self.stopwatch = stopwatch;
        self
    }

    /// Times every model that needs it, one after the other.
    ///
    /// Inputs: `may_continue`, asked before each run; false stops the check.
    /// Outputs: How many models were timed to the end.
    /// Side effects: Loads models and runs inference; saves one telemetry row per run and the
    /// record after each model; logs the model and its factor only.
    pub fn run(&self, may_continue: &dyn Fn() -> bool) -> usize {
        let Some(target) = (self.target)() else {
            return 0;
        };
        let metrics = match (self.open_metrics)() {
            Ok(metrics) => metrics,
            Err(error) => {
                tracing::warn!(error = %format!("{error:#}"), "the speed check cannot open telemetry");
                return 0;
            }
        };
        let measurements = transcription_measurements(metrics.as_ref(), self.device_id.as_str());
        let mut checked = 0;
        // Each model is timed at most once, so the ladder bounds the rounds.
        for _ in 0..target.models.len() {
            let record = load_record(&self.record_file);
            let Some(file_name) = next_model_to_check(
                &target.automatic_file,
                &record,
                &target.hardware_key,
                &|file| is_measured(&measurements, file),
                &|file| target.models.contains_key(file),
            ) else {
                break;
            };
            let Some(model_path) = target.models.get(&file_name) else {
                break;
            };
            let Some(factor) =
                self.time_model(model_path, target.efficient, metrics.as_ref(), may_continue)
            else {
                break;
            };
            tracing::info!(model = %file_name, real_time_factor = factor, "Speed check finished");
            let record = with_result(record, &target.hardware_key, &file_name, factor);
            if let Err(error) = save_record(&self.record_file, &record) {
                tracing::warn!(error = %error, "the speed check result could not be saved");
                break;
            }
            checked += 1;
        }
        checked
    }

    /// Transcribes the clip `CHECK_RUNS` times with one model.
    ///
    /// Outputs: The factor, or `None` when the model cannot load, a run fails, or `may_continue`
    /// turned false. Side effects: Loads the model; saves one telemetry row per finished run.
    fn time_model(
        &self,
        model_path: &Path,
        efficient: bool,
        metrics: &dyn MetricsRepository,
        may_continue: &dyn Fn() -> bool,
    ) -> Option<f32> {
        let samples = clip_samples();
        if samples.is_empty() {
            return None;
        }
        let audio = Arc::new(ChunkAudioStore::new());
        let engine = match (self.make_engine)(model_path, efficient, Arc::clone(&audio)) {
            Ok(engine) => engine,
            Err(error) => {
                tracing::warn!(error = %format!("{error:#}"), "the speed check cannot load the model");
                return None;
            }
        };
        let label = local_model_label(model_path);
        let meeting = MeetingId::new("speed-check");
        let mut total = Duration::ZERO;
        for run in 0..CHECK_RUNS {
            if !may_continue() {
                audio.clear();
                return None;
            }
            let recording_id = RecordingId::new(format!("speed-check-{run}"));
            audio.insert(&recording_id, samples.clone());
            let recording =
                AudioRecording::new(recording_id, meeting.clone(), self.device_id.clone());
            let started = (self.stopwatch)();
            if let Err(error) = engine.transcribe(&recording, &TranscriptionContext::default()) {
                tracing::warn!(error = %error, "the speed check could not transcribe the clip");
                audio.clear();
                return None;
            }
            let elapsed = (self.stopwatch)().saturating_sub(started);
            total += elapsed;
            self.save_run(metrics, &label, elapsed);
        }
        let audio_ms = CLIP_MS * CHECK_RUNS as u64;
        let processing_ms = u64::try_from(total.as_millis()).unwrap_or(u64::MAX);
        Some(real_time_factor(audio_ms, processing_ms))
    }

    /// Saves one run as a transcription row, as recordings do.
    ///
    /// Side effects: Writes telemetry; a failure is logged and otherwise ignored.
    fn save_run(&self, metrics: &dyn MetricsRepository, label: &str, elapsed: Duration) {
        let duration_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
        let completed_epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|since| u64::try_from(since.as_millis()).unwrap_or(u64::MAX))
            .unwrap_or(0);
        let row = TaskMetrics::new(
            uuid::Uuid::new_v4(),
            self.device_id.clone(),
            TaskType::Transcription,
            label,
            duration_ms,
            duration_ms,
            duration_ms as f32 * LOCAL_TRANSCRIPTION_POWER_MILLIWATTS / 3_600_000.0,
            completed_epoch_ms,
        )
        .with_audio_duration_ms(Some(CLIP_MS));
        if let Err(error) = metrics.save_metrics(row) {
            tracing::warn!(error = %error, "a speed check run could not be saved to telemetry");
        }
    }
}

/// Writes the record atomically enough for one writer: a temporary file, then a rename.
///
/// Side effects: Writes the file.
fn save_record(record_file: &Path, record: &SpeedCheckRecord) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(record).map_err(std::io::Error::other)?;
    let mut temporary = record_file.as_os_str().to_owned();
    temporary.push(".tmp");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, record_file)
}
