//! Composes the live meeting pipeline from environment configuration for the FFI, CLI, and daemon.
//!
//! This is the only place that names concrete AI adapters. It reads the pipeline variables,
//! selects the transcription engine (whisper.cpp in `local-ai` builds; the app keeps no API keys,
//! so there is no cloud transcription), and spawns a `RecordingController` whose worker owns its own SQLite connections
//! (sharing the already-unlocked embedding cipher) and the encrypted voice library as matcher.
//! Diarization runs when requested, in a `diarization` build, with both ONNX models configured;
//! otherwise it logs why not and segments are `Unattributed`. Model loading happens on the worker
//! thread, never on the caller's.
//!
//! | Variable | Values | Default |
//! | --- | --- | --- |
//! | `NOTE_TAKER_WHISPER_MODEL` | path to a GGML/GGUF model (`local-ai` builds) | unset |
//! | `NOTE_TAKER_WHISPER_LANGUAGE` | `auto` or a 2–3 letter Whisper language code (`it`, `en`); the apps' choice overrides it per chunk | `auto` |
//! | `NOTE_TAKER_ENABLE_DIARIZATION` | boolean | `false` |
//! | `NOTE_TAKER_DIARIZATION_SEGMENTATION_MODEL`, `NOTE_TAKER_DIARIZATION_EMBEDDING_MODEL` | paths to the exported ONNX models (`diarization` builds) | unset |
//! | `NOTE_TAKER_VAD_ENABLED` | boolean; `false` cuts near every 15 s | `true` |

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, anyhow, bail};

use crate::transcription_settings::settings_file_for;
use crate::{
    adapters::{
        audio::{
            chunk_audio_store::ChunkAudioStore, retained_audio_store::RetainedAudioStore,
            retained_speech_writer::RetainedSpeechWriter, vad_chunker::VadConfig,
        },
        storage::sqlite_voice_vector_db::SqliteVoiceVectorDB,
    },
    application::{
        compute_router::ComputePreference,
        pipeline::{
            pipeline_factory::PipelineFactory,
            task_metrics::{TaskMetricsRecorder, TaskProfile},
        },
        recording_manager::RecordingManager,
        transcription_routing::{RemoteTranscriptionGateway, RoutedTranscriptionEngine},
        voice_identity_manager::VoiceIdentityManager,
    },
    domain::{
        errors::TranscriptionError,
        models::{
            AudioRecording, DeviceId, ExternalMediaChunk, TaskType, Transcript,
            TranscriptionContext, TranscriptionModel,
        },
        transcription_language::TranscriptionLanguage,
    },
    ports::{
        ai::TranscriptionEngine,
        crypto::EmbeddingCipher,
        remote_transcription::ChunkSamples,
        speech_transfer::{SpeechLink, SpeechSender},
        storage::NoteRepository,
    },
    recording_controller::{ChunkingMode, RecordingController, TranscriptPass},
    refinement_worker::{
        EngineFactory, RefinementControl, RefinementWorker, RepositoryFactory, RetainedSpeechInbox,
        retained_audio_directory,
    },
    storage_composition::open_note_repository,
};

#[cfg(test)]
#[path = "pipeline_composition_tests.rs"]
mod tests;

/// Estimated draw of on-device whisper.cpp, which runs on the CPU: the same 5 W the compute
/// router assumes for CPU work.
pub const LOCAL_TRANSCRIPTION_POWER_MILLIWATTS: f32 = 5_000.0;

/// How the NPU engine's model appears in telemetry (#75).
#[cfg(feature = "npu-whisper")]
pub const NPU_MODEL_LABEL: &str = "whisper-small-npu";

/// The assumed power of NPU transcription, in milliwatts (#75). It is an unmeasured estimate:
/// the NPU finishes a 30 s window in about 1.5 s, against whisper.cpp's several seconds on the CPU.
#[cfg(feature = "npu-whisper")]
pub const NPU_TRANSCRIPTION_POWER_MILLIWATTS: f32 = 1_500.0;
/// Path of the local whisper.cpp model.
pub const WHISPER_MODEL_VARIABLE: &str = "NOTE_TAKER_WHISPER_MODEL";
/// Local Whisper transcription language: `auto` or a Whisper language code such as `it` or `en`.
pub const WHISPER_LANGUAGE_VARIABLE: &str = "NOTE_TAKER_WHISPER_LANGUAGE";
/// Requests speaker diarization.
pub const ENABLE_DIARIZATION_VARIABLE: &str = "NOTE_TAKER_ENABLE_DIARIZATION";
/// Path of the exported segmentation model.
pub const DIARIZATION_SEGMENTATION_MODEL_VARIABLE: &str =
    "NOTE_TAKER_DIARIZATION_SEGMENTATION_MODEL";
/// Path of the exported speaker-embedding model.
pub const DIARIZATION_EMBEDDING_MODEL_VARIABLE: &str = "NOTE_TAKER_DIARIZATION_EMBEDDING_MODEL";
/// Enables speech-pause chunking.
pub const VAD_ENABLED_VARIABLE: &str = "NOTE_TAKER_VAD_ENABLED";
/// Turns the after-meeting transcript pass on or off (#87); on by default except on Android.
pub const TRANSCRIPT_PASS_VARIABLE: &str = "NOTE_TAKER_TRANSCRIPT_PASS";
/// The model file for the after-meeting pass; unset uses the live model.
pub const TRANSCRIPT_PASS_MODEL_VARIABLE: &str = "NOTE_TAKER_TRANSCRIPT_PASS_MODEL";

/// The diarization engine this build composes.
#[cfg(feature = "diarization")]
pub type ComposedDiarizationEngine =
    crate::adapters::ai::diarization::windowed_engine::WindowedDiarizationEngine<
        crate::adapters::ai::diarization::onnx_models::OnnxSegmentationModel,
        crate::adapters::ai::diarization::onnx_models::OnnxSpeakerEmbeddingModel,
    >;
/// The diarization engine this build composes: none without the `diarization` feature.
#[cfg(not(feature = "diarization"))]
pub type ComposedDiarizationEngine =
    crate::application::pipeline::pipeline_factory::NoDiarizationEngine;

/// Pipeline settings read from the environment.
#[derive(Clone)]
pub struct PipelineConfig {
    whisper_model_path: Option<PathBuf>,
    /// Always passed to whisper.cpp, whose own default (`en`) would force English output; also
    /// the language of recordings without a chosen one.
    whisper_language: TranscriptionLanguage,
    diarization_requested: bool,
    diarization_segmentation_model_path: Option<PathBuf>,
    diarization_embedding_model_path: Option<PathBuf>,
    vad_enabled: bool,
    /// True when local transcription should use efficient mode on a phone GPU.
    efficient_gpu: bool,
    /// The file recording an unfinished GPU attempt; `None` skips the crash check.
    gpu_attempt_marker: Option<PathBuf>,
    /// Whether a recording keeps its speech sealed for the after-meeting pass (#87).
    transcript_pass: bool,
    /// The model for that pass; `None` uses `whisper_model_path`.
    transcript_pass_model_path: Option<PathBuf>,
    /// The downloaded NPU folder and the multilingual model supplying its vocabulary, when the
    /// NPU engine should be tried first (#75).
    npu_whisper: Option<(PathBuf, PathBuf)>,
}

impl std::fmt::Debug for PipelineConfig {
    /// Prints the configuration.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PipelineConfig")
            .field("whisper_model_path", &self.whisper_model_path)
            .field("whisper_language", &self.whisper_language)
            .field("diarization_requested", &self.diarization_requested)
            .field(
                "diarization_segmentation_model_path",
                &self.diarization_segmentation_model_path,
            )
            .field(
                "diarization_embedding_model_path",
                &self.diarization_embedding_model_path,
            )
            .field("vad_enabled", &self.vad_enabled)
            .field("transcript_pass", &self.transcript_pass)
            .field(
                "transcript_pass_model_path",
                &self.transcript_pass_model_path,
            )
            .field("npu_whisper", &self.npu_whisper)
            .finish()
    }
}

impl PipelineConfig {
    /// Reads the configuration from the process environment.
    ///
    /// Inputs: None.
    /// Outputs: The configuration, or an error naming an invalid variable.
    /// Side effects: Reads environment variables; never logs their values.
    pub fn from_environment() -> Result<Self> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Reads the configuration through `lookup`, so tests never touch the environment.
    ///
    /// Inputs: A function returning a variable's value, or `None` when unset. Empty values
    /// count as unset.
    /// Outputs: The configuration, or an error naming an invalid variable.
    /// Side effects: None beyond `lookup`.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let read = |name: &str| lookup(name).filter(|value| !value.trim().is_empty());
        Ok(Self {
            whisper_model_path: read(WHISPER_MODEL_VARIABLE).map(PathBuf::from),
            whisper_language: parse_whisper_language(read(WHISPER_LANGUAGE_VARIABLE))?,
            diarization_requested: parse_flag(
                ENABLE_DIARIZATION_VARIABLE,
                read(ENABLE_DIARIZATION_VARIABLE),
                false,
            )?,
            diarization_segmentation_model_path: read(DIARIZATION_SEGMENTATION_MODEL_VARIABLE)
                .map(PathBuf::from),
            diarization_embedding_model_path: read(DIARIZATION_EMBEDDING_MODEL_VARIABLE)
                .map(PathBuf::from),
            vad_enabled: parse_flag(VAD_ENABLED_VARIABLE, read(VAD_ENABLED_VARIABLE), true)?,
            efficient_gpu: false,
            gpu_attempt_marker: None,
            // On everywhere: phones run it from the charging job (#87 step 2), which processes
            // and deletes the kept speech. Settings can turn it off.
            transcript_pass: parse_flag(
                TRANSCRIPT_PASS_VARIABLE,
                read(TRANSCRIPT_PASS_VARIABLE),
                true,
            )?,
            transcript_pass_model_path: read(TRANSCRIPT_PASS_MODEL_VARIABLE).map(PathBuf::from),
            npu_whisper: None,
        })
    }

    /// Returns the local Whisper language: `auto` (detect per chunk) or a lowercase language code.
    pub fn whisper_language(&self) -> &str {
        self.whisper_language.as_str()
    }

    /// Returns the configured language, which recordings without a chosen one are made in.
    pub fn transcription_language(&self) -> TranscriptionLanguage {
        self.whisper_language
    }

    /// Lists the transcription models this configuration can run, for the device manifest
    /// (REQUIREMENTS 3.4): local Whisper when a model is set in a local-ai build.
    ///
    /// Inputs: None. Outputs: The models, local first. Side effects: None; no model is loaded,
    /// so a model file that fails to load is still advertised.
    pub fn advertised_transcription_models(&self) -> Vec<TranscriptionModel> {
        let local_available = cfg!(feature = "local-ai") && self.whisper_model_path.is_some();
        if local_available {
            vec![TranscriptionModel::Whisper]
        } else {
            Vec::new()
        }
    }

    /// Replaces values with the ones the owner saved in Settings.
    ///
    /// Inputs: The model path, where `None` keeps the current value.
    /// Outputs: The configuration. Side effects: None.
    pub fn with_saved_transcription(mut self, whisper_model_path: Option<PathBuf>) -> Self {
        if whisper_model_path.is_some() {
            self.whisper_model_path = whisper_model_path;
        }
        self
    }

    /// Applies the owner's GPU choices.
    ///
    /// Inputs: Whether to use efficient mode, and the file recording an unfinished GPU attempt.
    /// Outputs: The configuration. Side effects: None.
    #[must_use]
    pub fn with_gpu_choices(mut self, efficient_gpu: bool, gpu_attempt_marker: PathBuf) -> Self {
        self.efficient_gpu = efficient_gpu;
        self.gpu_attempt_marker = Some(gpu_attempt_marker);
        self
    }

    /// Chooses the NPU engine (#75).
    ///
    /// Inputs: The NPU folder and the vocabulary model, or `None` for whisper.cpp only.
    /// Outputs: The configuration. Side effects: None.
    #[must_use]
    pub fn with_npu_whisper(mut self, npu_whisper: Option<(PathBuf, PathBuf)>) -> Self {
        self.npu_whisper = npu_whisper;
        self
    }

    /// Returns the NPU folder when the NPU engine should be tried. Side effects: None.
    pub fn npu_directory(&self) -> Option<&Path> {
        self.npu_whisper
            .as_ref()
            .map(|(folder, _)| folder.as_path())
    }

    /// Returns the model supplying the NPU engine's vocabulary. Side effects: None.
    pub fn npu_vocabulary_model(&self) -> Option<&Path> {
        self.npu_whisper.as_ref().map(|(_, model)| model.as_path())
    }

    /// Applies the owner's choices for the after-meeting transcript pass (#87).
    ///
    /// Inputs: Whether it runs and its model's path, where `None` keeps the current value.
    /// Outputs: The configuration. Side effects: None.
    #[must_use]
    pub fn with_transcript_pass(
        mut self,
        enabled: Option<bool>,
        model_path: Option<PathBuf>,
    ) -> Self {
        if let Some(enabled) = enabled {
            self.transcript_pass = enabled;
        }
        if model_path.is_some() {
            self.transcript_pass_model_path = model_path;
        }
        self
    }

    /// Reports whether recordings keep their speech for the after-meeting pass.
    pub fn transcript_pass(&self) -> bool {
        self.transcript_pass
    }

    /// Returns the model the after-meeting pass loads: its own, else the live one.
    pub fn transcript_pass_model_path(&self) -> Option<&Path> {
        self.transcript_pass_model_path
            .as_deref()
            .or(self.whisper_model_path.as_deref())
    }

    /// Returns the local model's path, if any.
    pub fn local_model_path(&self) -> Option<&Path> {
        self.whisper_model_path.as_deref()
    }

    /// Reports whether diarization was requested.
    pub fn diarization_requested(&self) -> bool {
        self.diarization_requested
    }

    /// Reports whether speech-pause chunking is enabled.
    pub fn vad_enabled(&self) -> bool {
        self.vad_enabled
    }

    /// Returns the chunking mode implied by `vad_enabled`.
    pub fn chunking_mode(&self) -> ChunkingMode {
        if self.vad_enabled {
            ChunkingMode::Vad(VadConfig::english())
        } else {
            ChunkingMode::TimeBased
        }
    }
}

/// Parses `NOTE_TAKER_WHISPER_LANGUAGE`.
///
/// Inputs: The variable's value (`None` when unset or blank).
/// Outputs: `auto` when unset, else the value trimmed and lowercased, which must be `auto` or a
/// code Whisper knows; anything else is an error naming the variable.
/// Side effects: None.
fn parse_whisper_language(value: Option<String>) -> Result<TranscriptionLanguage> {
    let Some(value) = value else {
        return Ok(TranscriptionLanguage::Auto);
    };
    TranscriptionLanguage::parse(&value).map_err(|_| {
        anyhow!(
            "{WHISPER_LANGUAGE_VARIABLE} must be auto or a Whisper language code such as it or en"
        )
    })
}

/// Parses a boolean variable.
///
/// Inputs: The variable name (for the error), its value, and the default for `None`.
/// Outputs: The flag, or an error for values other than true/false, 1/0, yes/no, on/off.
/// Side effects: None.
pub(crate) fn parse_flag(name: &str, value: Option<String>, default: bool) -> Result<bool> {
    match value
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        None => Ok(default),
        Some("1" | "true" | "yes" | "on") => Ok(true),
        Some("0" | "false" | "no" | "off") => Ok(false),
        Some(_) => bail!("{name} must be true or false"),
    }
}

/// The transcription engine chosen for this device.
pub enum SelectedTranscriptionEngine {
    /// Local whisper.cpp inference.
    #[cfg(feature = "local-ai")]
    Local(crate::adapters::ai::local_whisper_cpp::LocalWhisperCppTranscriptionEngine),
    /// Whisper Small on a Snapdragon NPU (#75).
    #[cfg(feature = "npu-whisper")]
    Npu(Box<crate::adapters::ai::npu_whisper::qnn_engine::QnnWhisperEngine>),
    /// A build without local transcription has no engine to select.
    #[cfg(not(feature = "local-ai"))]
    Unavailable(std::convert::Infallible),
}

impl TranscriptionEngine for SelectedTranscriptionEngine {
    /// Delegates to the selected engine.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        match self {
            #[cfg(feature = "local-ai")]
            Self::Local(engine) => engine.transcribe(audio_recording, transcription_context),
            #[cfg(feature = "npu-whisper")]
            Self::Npu(engine) => engine.transcribe(audio_recording, transcription_context),
            #[cfg(not(feature = "local-ai"))]
            Self::Unavailable(never) => {
                let _ = (audio_recording, transcription_context);
                match *never {}
            }
        }
    }

    /// Delegates to the selected engine.
    fn transcribe_external_media(
        &self,
        media_chunk: &ExternalMediaChunk,
        audio_recording: &AudioRecording,
    ) -> Result<Transcript, TranscriptionError> {
        match self {
            #[cfg(feature = "local-ai")]
            Self::Local(engine) => engine.transcribe_external_media(media_chunk, audio_recording),
            #[cfg(feature = "npu-whisper")]
            Self::Npu(engine) => engine.transcribe_external_media(media_chunk, audio_recording),
            #[cfg(not(feature = "local-ai"))]
            Self::Unavailable(never) => {
                let _ = (media_chunk, audio_recording);
                match *never {}
            }
        }
    }
}

/// Builds the transcription engine selected by `config`.
///
/// Inputs: The configuration and the audio store the engine reads chunks from.
/// Outputs: The engine, or an error naming the variables that would configure one.
/// Side effects: Loads the local model (slow; run it off the UI thread).
pub fn select_transcription_engine(
    config: &PipelineConfig,
    chunk_audio_store: &Arc<ChunkAudioStore>,
) -> Result<SelectedTranscriptionEngine> {
    if config.whisper_model_path.is_none() {
        bail!(
            "no transcription engine is configured: set {WHISPER_MODEL_VARIABLE} (local-ai \
             builds) to a Whisper model for on-device transcription"
        );
    }
    #[cfg(feature = "npu-whisper")]
    if let Some(engine) = npu_engine(config, chunk_audio_store) {
        return Ok(engine);
    }
    local_engine(config, chunk_audio_store)
}

/// Opens the NPU engine when the owner chose it (#75).
///
/// Inputs: The configuration and the audio store. Outputs: The engine; `None` when the NPU was
/// not chosen or cannot open (another phone, a damaged download), so whisper.cpp is used.
/// Side effects: Loads ONNX Runtime, the QNN libraries and the model onto the NPU; logs why it
/// could not.
#[cfg(feature = "npu-whisper")]
fn npu_engine(
    config: &PipelineConfig,
    chunk_audio_store: &Arc<ChunkAudioStore>,
) -> Option<SelectedTranscriptionEngine> {
    use crate::adapters::ai::npu_whisper::{
        install::NpuInstall,
        qnn_engine::{NpuWhisperFiles, QnnWhisperEngine},
    };
    let (directory, vocabulary_model) = config.npu_whisper.as_ref()?;
    let install = NpuInstall::new(directory);
    let files = NpuWhisperFiles {
        model_dir: install.model_dir(),
        qnn_dir: install.qnn_dir(),
        vocabulary_model: vocabulary_model.clone(),
    };
    match QnnWhisperEngine::open(
        &files,
        Some(config.whisper_language.as_str().to_owned()),
        Arc::clone(chunk_audio_store) as _,
    ) {
        Ok(engine) => {
            tracing::info!("NPU transcription model loaded");
            Some(SelectedTranscriptionEngine::Npu(Box::new(engine)))
        }
        Err(error) => {
            tracing::warn!(%error, "the NPU could not be used; transcribing with whisper.cpp");
            None
        }
    }
}

/// Describes how the selected engine's transcriptions appear in telemetry (Issue #53).
///
/// Inputs: The selected engine and the configuration it was built from.
/// Outputs: `Transcription` with the model file's name.
/// Side effects: None.
pub fn transcription_task_profile(
    engine: &SelectedTranscriptionEngine,
    config: &PipelineConfig,
) -> TaskProfile {
    match engine {
        #[cfg(not(feature = "local-ai"))]
        SelectedTranscriptionEngine::Unavailable(never) => {
            let _ = config;
            match *never {}
        }
        #[cfg(feature = "local-ai")]
        SelectedTranscriptionEngine::Local(_) => TaskProfile::new(
            TaskType::Transcription,
            config
                .whisper_model_path
                .as_deref()
                .map_or_else(|| "whisper.cpp".to_owned(), local_model_label),
            LOCAL_TRANSCRIPTION_POWER_MILLIWATTS,
        ),
        #[cfg(feature = "npu-whisper")]
        SelectedTranscriptionEngine::Npu(_) => TaskProfile::new(
            TaskType::Transcription,
            NPU_MODEL_LABEL.to_owned(),
            NPU_TRANSCRIPTION_POWER_MILLIWATTS,
        ),
    }
}

/// Says whether local transcription may try a GPU.
///
/// Outputs: True unless `NOTE_TAKER_WHISPER_GPU` is `off`, the way to rule a GPU driver in or out.
/// Side effects: Reads the environment.
#[cfg(feature = "local-ai")]
fn whisper_gpu_allowed() -> bool {
    std::env::var("NOTE_TAKER_WHISPER_GPU").map_or(true, |value| value != "off")
}

/// Returns the GPU memory this device should advertise to paired devices.
///
/// Outputs: A discrete GPU's memory in megabytes, or zero. Side effects: Starts whisper.cpp's
/// backends on first use, which may load the Vulkan library.
#[cfg(feature = "local-ai")]
pub fn local_gpu_megabytes() -> u32 {
    use crate::adapters::ai::{compute_resources, local_whisper_cpp};
    if !whisper_gpu_allowed() {
        return 0;
    }
    compute_resources::advertised_gpu_megabytes(compute_resources::choose_gpu(
        &local_whisper_cpp::available_gpus(),
    ))
}

/// Reports no GPU for a build without local transcription.
///
/// Outputs: Zero. Side effects: None.
#[cfg(not(feature = "local-ai"))]
pub fn local_gpu_megabytes() -> u32 {
    0
}

/// Reports the GPU state for Settings.
///
/// Outputs: The name of a GPU efficient mode can use, and whether the latest engine uses a GPU.
/// Side effects: Looks for OpenCL on a phone, which only reads what the driver reports.
#[cfg(feature = "local-ai")]
pub fn efficient_gpu_status() -> (Option<String>, bool) {
    use crate::adapters::ai::local_whisper_cpp;
    let name = whisper_gpu_allowed()
        .then(local_whisper_cpp::efficient_gpu)
        .flatten();
    (name, local_whisper_cpp::engine_uses_gpu())
}

/// Reports no GPU for a build without local transcription.
///
/// Outputs: No GPU, not in use. Side effects: None.
#[cfg(not(feature = "local-ai"))]
pub fn efficient_gpu_status() -> (Option<String>, bool) {
    (None, false)
}

/// Labels a local model by its file name without folder or extension (`ggml-small`).
///
/// Inputs: The model path, with `/` or `\` separators whatever the host.
/// Outputs: The label; `whisper.cpp` when the path has no file name. The folder is dropped so
/// telemetry, which peers can see, never carries a path that may name the user.
/// Side effects: None.
pub fn local_model_label(model_path: &Path) -> String {
    let path_text = model_path.to_string_lossy();
    let file_name = path_text.rsplit(['/', '\\']).next().unwrap_or_default();
    let stem = file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _extension)| stem);
    if stem.is_empty() {
        "whisper.cpp".to_owned()
    } else {
        stem.to_owned()
    }
}

/// Builds the local whisper.cpp engine.
///
/// Inputs: The configuration (requires the model path) and the chunk audio store.
/// Outputs: The engine, or an error naming the missing model path or the model load failure.
/// Side effects: Loads the model into memory.
#[cfg(feature = "local-ai")]
fn local_engine(
    config: &PipelineConfig,
    chunk_audio_store: &Arc<ChunkAudioStore>,
) -> Result<SelectedTranscriptionEngine> {
    use crate::adapters::ai::local_whisper_cpp::{
        LocalWhisperCppConfig, LocalWhisperCppTranscriptionEngine,
    };

    let model_path = config.whisper_model_path.clone().ok_or_else(|| {
        anyhow!("local transcription requires {WHISPER_MODEL_VARIABLE} to name a model file")
    })?;
    let thread_count = crate::adapters::ai::compute_resources::preferred_inference_threads();
    // An explicit language is required: whisper.cpp defaults to English, so Italian meetings
    // would otherwise come out as English text. `auto` makes it detect the language per chunk.
    // A phone stays on its CPU unless the owner chose efficient mode.
    let use_gpu = whisper_gpu_allowed() && (!cfg!(target_os = "android") || config.efficient_gpu);
    let mut local_config = LocalWhisperCppConfig::new(
        model_path,
        thread_count,
        use_gpu,
        Some(config.whisper_language.as_str().to_owned()),
    )
    .context("invalid local transcription configuration")?
    .with_efficient_gpu(config.efficient_gpu);
    if let Some(marker_file) = &config.gpu_attempt_marker {
        local_config = local_config.with_gpu_attempt_marker(marker_file.clone());
    }
    let engine =
        LocalWhisperCppTranscriptionEngine::new(local_config, Arc::clone(chunk_audio_store) as _)
            .context("unable to load the local whisper model")?;
    tracing::info!("Local transcription model loaded");
    Ok(SelectedTranscriptionEngine::Local(engine))
}

/// Reports that this build cannot transcribe locally.
///
/// Inputs: Ignored.
/// Outputs: Always an error explaining the missing `local-ai` feature.
/// Side effects: None.
#[cfg(not(feature = "local-ai"))]
fn local_engine(
    _config: &PipelineConfig,
    _chunk_audio_store: &Arc<ChunkAudioStore>,
) -> Result<SelectedTranscriptionEngine> {
    bail!(
        "local transcription is unavailable: this build was compiled without the local-ai feature"
    )
}

/// Selects the diarization engine.
///
/// Inputs: The configuration.
/// Outputs: The engine; `None` when diarization was not requested or cannot run here (no
/// `diarization` feature, or a model path missing), so meetings stay unattributed; or an error
/// when the named models cannot load.
/// Side effects: Loads the models into memory; logs why a requested engine is absent.
pub fn select_diarization_engine(
    config: &PipelineConfig,
) -> Result<Option<ComposedDiarizationEngine>> {
    if !config.diarization_requested {
        return Ok(None);
    }
    load_diarization_engine(config)
}

/// Loads the ONNX models named by the configuration.
///
/// Inputs: The configuration. Outputs: The engine, `None` when a path is missing, or a load error.
/// Side effects: Loads the models into memory.
#[cfg(feature = "diarization")]
fn load_diarization_engine(config: &PipelineConfig) -> Result<Option<ComposedDiarizationEngine>> {
    use crate::adapters::ai::diarization::{
        onnx_models::{OnnxSegmentationModel, OnnxSpeakerEmbeddingModel},
        windowed_engine::DiarizationTuning,
    };

    let (Some(segmentation_path), Some(embedding_path)) = (
        config.diarization_segmentation_model_path.as_deref(),
        config.diarization_embedding_model_path.as_deref(),
    ) else {
        tracing::warn!(
            "Diarization was requested, but {DIARIZATION_SEGMENTATION_MODEL_VARIABLE} and \
             {DIARIZATION_EMBEDDING_MODEL_VARIABLE} must both be set; segments will be unattributed"
        );
        return Ok(None);
    };
    let segmentation_model = OnnxSegmentationModel::load(segmentation_path)
        .context("unable to load the diarization segmentation model")?;
    let embedding_model = OnnxSpeakerEmbeddingModel::load(embedding_path)
        .context("unable to load the diarization embedding model")?;
    tracing::info!("Diarization models loaded");
    Ok(Some(ComposedDiarizationEngine::new(
        segmentation_model,
        embedding_model,
        DiarizationTuning::default(),
    )))
}

/// Reports that this build cannot diarize.
///
/// Inputs: Ignored. Outputs: Always `None`. Side effects: Logs a warning.
#[cfg(not(feature = "diarization"))]
fn load_diarization_engine(_config: &PipelineConfig) -> Result<Option<ComposedDiarizationEngine>> {
    tracing::warn!(
        "Diarization was requested, but this build was compiled without the diarization \
         feature; segments will be unattributed"
    );
    Ok(None)
}

/// Starts the pipeline worker and returns an idle recording controller.
///
/// Inputs: The SQLite path (the same database the caller already opened), the device ID, the
/// verified embedding cipher from `open_encrypted_stores`, the pipeline configuration, and the
/// P2P task to route transcription through (`None` keeps it on this device), and the P2P link that
/// carries retained speech between devices recording one meeting (#88; `None` without P2P).
/// Outputs: The controller, or an error explaining which dependency is missing.
/// Side effects: Spawns the worker thread, which selects (and may load) the transcription and
/// diarization engines and opens its own note-repository, telemetry and voice-store connections; every
/// transcribed chunk then saves one telemetry row. Blocks until ready.
pub fn spawn_recording_controller(
    database_path: &Path,
    device_id: DeviceId,
    embedding_cipher: Arc<dyn EmbeddingCipher>,
    config: PipelineConfig,
    remote_gateway: Option<Arc<dyn RemoteTranscriptionGateway>>,
    speech_link: Option<Arc<dyn SpeechLink>>,
) -> Result<RecordingController> {
    let chunk_audio_store = Arc::new(ChunkAudioStore::new());
    let engine_audio_store = Arc::clone(&chunk_audio_store);
    let pass_cipher = Arc::clone(&embedding_cipher);
    let pass_config = config.clone();
    let chunking_mode = config.chunking_mode();
    let default_language = config.transcription_language();
    let worker_database_path = database_path.to_path_buf();
    let worker_device_id = device_id.clone();
    // Read as each recording starts, so a glossary saved anywhere applies to the next one.
    let glossary_source =
        crate::transcription_settings::glossary_source(settings_file_for(database_path));
    let recording_manager = RecordingManager::spawn(move || -> Result<_> {
        let selected_engine = match select_transcription_engine(&config, &engine_audio_store) {
            Ok(engine) => Some(engine),
            // A device without an engine can still record when the owner chose another device.
            Err(error)
                if remote_gateway
                    .as_ref()
                    .is_some_and(|gateway| gateway.route() != ComputePreference::LocalOnly) =>
            {
                tracing::warn!(error = %format!("{error:#}"), "no local transcription engine; using paired devices only");
                None
            }
            Err(error) => return Err(error),
        };
        let task_profile = selected_engine.as_ref().map_or_else(
            || {
                TaskProfile::new(
                    TaskType::Transcription,
                    "paired-device".to_owned(),
                    LOCAL_TRANSCRIPTION_POWER_MILLIWATTS,
                )
            },
            |engine| transcription_task_profile(engine, &config),
        );
        let transcription_engine = match (remote_gateway, selected_engine) {
            (Some(gateway), selected_engine) => RoutedTranscriptionEngine::routed(
                selected_engine,
                gateway,
                Arc::clone(&engine_audio_store) as Arc<dyn ChunkSamples>,
            ),
            (None, Some(selected_engine)) => RoutedTranscriptionEngine::local(selected_engine),
            (None, None) => bail!("no transcription engine is available"),
        };
        let note_repository = open_note_repository(
            &worker_database_path,
            worker_device_id.clone(),
            Arc::clone(&embedding_cipher),
        )
        .context("unable to open the pipeline note repository")?;
        // Its own connection, so telemetry writes never wait on the transcript's.
        let metrics_repository = open_note_repository(
            &worker_database_path,
            worker_device_id.clone(),
            Arc::clone(&embedding_cipher),
        )
        .context("unable to open the pipeline telemetry store")?;
        let task_metrics =
            TaskMetricsRecorder::new(Box::new(metrics_repository), worker_device_id, task_profile);
        let voice_store = SqliteVoiceVectorDB::open(&worker_database_path, embedding_cipher)
            .context("unable to open the pipeline voice store")?;
        let diarization_engine = select_diarization_engine(&config)?;
        Ok(PipelineFactory::new(transcription_engine, note_repository)
            .with_diarization_engine(diarization_engine)
            .with_voice_matcher(Some(VoiceIdentityManager::new(voice_store)))
            .with_task_metrics(task_metrics)
            .with_default_language(Some(default_language))
            .with_glossary_source(Some(glossary_source)))
    })?;
    // Recordings without a chosen language carry the configured one, so it is recorded too.
    let controller = RecordingController::new(
        recording_manager,
        chunk_audio_store,
        chunking_mode,
        device_id.clone(),
    )
    .with_default_language(Some(default_language));
    let controller = match speaker_names_setup(database_path, device_id.clone(), &pass_cipher) {
        Ok(setup) => controller.with_speaker_names(setup),
        Err(error) => {
            tracing::warn!(error = %format!("{error:#}"), "Call window names are off: their store could not be opened");
            controller
        }
    };
    Ok(
        match start_transcript_pass(
            database_path,
            device_id,
            pass_cipher,
            &pass_config,
            speech_link,
        ) {
            Some(transcript_pass) => controller.with_transcript_pass(transcript_pass),
            None => controller,
        },
    )
}

/// Composes reading speaking names from call windows during recordings (#113).
///
/// Inputs: The database, this device and the database key's cipher.
/// Outputs: The setup: this platform's reader, the call watch's `read_names` switch (read at each
/// recording's start, so Settings apply to the next recording), sealed `speaker-names`
/// attachments in the `slides` folder beside the database, and a 2 s interval; or the error of
/// a store that cannot be opened.
/// Side effects: Opens a note-repository connection for the attachments.
fn speaker_names_setup(
    database_path: &Path,
    device_id: DeviceId,
    embedding_cipher: &Arc<dyn EmbeddingCipher>,
) -> Result<crate::speaker_name_watcher::SpeakerNamesSetup> {
    let attachment_repository = open_note_repository(
        database_path,
        device_id.clone(),
        Arc::clone(embedding_cipher),
    )
    .context("unable to open the note repository for call window names")?;
    let sealed_files = attachment_repository.sealed_files();
    let slide_directory = database_path
        .parent()
        .map_or_else(PathBuf::new, Path::to_path_buf)
        .join("slides");
    let settings_file = crate::call_watch_commands::CallWatchSettings::file_for(database_path);
    Ok(crate::speaker_name_watcher::SpeakerNamesSetup {
        reader_factory: Arc::new(crate::adapters::accessibility::platform_reader),
        enabled: Box::new(move || {
            crate::call_watch_commands::CallWatchSettings::load(&settings_file).read_names
        }),
        sink: Box::new(
            crate::adapters::storage::slide_attachment_store::SlideAttachmentStore::new(
                slide_directory,
                attachment_repository,
                device_id,
                sealed_files,
            ),
        ),
        sample_interval: std::time::Duration::from_secs(2),
    })
}

/// Which engine the after-meeting pass transcribes with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PassEngineChoice {
    /// whisper.cpp with this model.
    WhisperCpp(PathBuf),
    /// The NPU (#75), with whisper.cpp and the live model if the NPU will not open.
    Npu {
        /// The downloaded NPU folder.
        directory: PathBuf,
        /// The multilingual model supplying the vocabulary.
        vocabulary_model: PathBuf,
        /// The whisper.cpp model used when the NPU cannot.
        fallback_model: PathBuf,
    },
}

/// Chooses the after-meeting pass's engine.
///
/// Inputs: The configuration. Outputs: A model chosen for the pass runs on whisper.cpp;
/// otherwise the pass uses what live transcription uses: the NPU when it is on, else the live
/// whisper.cpp model. `None` when there is no model. Side effects: None.
pub fn pass_engine_choice(config: &PipelineConfig) -> Option<PassEngineChoice> {
    let model = config.transcript_pass_model_path()?.to_path_buf();
    match (&config.transcript_pass_model_path, &config.npu_whisper) {
        (None, Some((directory, vocabulary_model))) => Some(PassEngineChoice::Npu {
            directory: directory.clone(),
            vocabulary_model: vocabulary_model.clone(),
            fallback_model: model,
        }),
        _ => Some(PassEngineChoice::WhisperCpp(model)),
    }
}

/// Starts the after-meeting worker (#87, #88) when the pass is on.
///
/// Inputs: The database (its folder holds `retained-audio`), this device, the database key's
/// cipher, the configuration, and the P2P link (`None` without P2P).
/// Outputs: The pass's store and control for the recorder, or `None` when the pass is off, there
/// is neither a local model nor a P2P link, or its thread cannot start (recordings then keep
/// nothing).
/// Side effects: Spawns the worker thread, which at once looks for meetings left from earlier runs.
/// With a local model the worker runs passes (a phone only while charging, as the platform
/// reports it); without one but with P2P, it keeps speech only to send it to a paired device
/// that combines the meeting. With P2P, the link sends this device's speech and stores the
/// speech paired devices send here.
pub fn start_transcript_pass(
    database_path: &Path,
    device_id: DeviceId,
    embedding_cipher: Arc<dyn EmbeddingCipher>,
    config: &PipelineConfig,
    speech_link: Option<Arc<dyn SpeechLink>>,
) -> Option<TranscriptPass> {
    if !config.transcript_pass() {
        return None;
    }
    let engine_choice = pass_engine_choice(config).filter(|_| cfg!(feature = "local-ai"));
    if engine_choice.is_none() && speech_link.is_none() {
        return None;
    }
    let store = Arc::new(RetainedAudioStore::new(
        retained_audio_directory(database_path),
        Arc::clone(&embedding_cipher),
    ));
    let control = Arc::new(RefinementControl::new(cfg!(target_os = "android")));
    let repository_path = database_path.to_path_buf();
    let repository_device = device_id.clone();
    let check_cipher = Arc::clone(&embedding_cipher);
    let repository_opener = move || -> RepositoryFactory {
        let repository_path = repository_path.clone();
        let repository_device = repository_device.clone();
        let embedding_cipher = Arc::clone(&embedding_cipher);
        Box::new(move || {
            Ok(Box::new(open_note_repository(
                &repository_path,
                repository_device.clone(),
                Arc::clone(&embedding_cipher),
            )?) as Box<dyn NoteRepository>)
        })
    };
    let worker = match engine_choice {
        Some(engine_choice) => {
            let efficient_gpu = config.efficient_gpu;
            let make_engine: EngineFactory = Box::new(move |audio_source| match &engine_choice {
                PassEngineChoice::WhisperCpp(model_path) => {
                    transcript_pass_engine(model_path, efficient_gpu, audio_source)
                }
                PassEngineChoice::Npu {
                    directory,
                    vocabulary_model,
                    fallback_model,
                } => npu_pass_engine(directory, vocabulary_model, &audio_source).map_or_else(
                    || transcript_pass_engine(fallback_model, efficient_gpu, audio_source),
                    Ok,
                ),
            });
            RefinementWorker::new(
                Arc::clone(&store),
                Arc::clone(&control),
                make_engine,
                repository_opener(),
                device_id.clone(),
            )
            .with_glossary_source(crate::transcription_settings::glossary_source(
                settings_file_for(database_path),
            ))
            .with_speed_check(speed_check(database_path, &device_id, &check_cipher))
        }
        None => RefinementWorker::without_pass(
            Arc::clone(&store),
            Arc::clone(&control),
            repository_opener(),
            device_id.clone(),
        ),
    }
    .with_keep_for_source(crate::transcription_settings::keep_for_source(
        settings_file_for(database_path),
    ));
    if let Some(link) = speech_link {
        link.accept_speech_with(Box::new(RetainedSpeechInbox::new(
            Arc::clone(&store),
            repository_opener(),
            device_id,
            Arc::clone(&control),
        )));
        control.set_speech_sender(link as Arc<dyn SpeechSender>);
    }
    let writer = match RetainedSpeechWriter::spawn(Arc::clone(&store)) {
        Ok(writer) => Arc::new(writer),
        Err(error) => {
            tracing::warn!(error = %error, "speech could not be kept for the after-meeting pass");
            return None;
        }
    };
    match worker.spawn() {
        Ok(_) => Some(TranscriptPass { writer, control }),
        Err(error) => {
            tracing::warn!(error = %error, "the after-meeting transcript pass could not start");
            None
        }
    }
}

/// Builds the speed check (#109) the worker runs after its rounds.
///
/// Inputs: The database, this device and the database key's cipher.
/// Outputs: The check, which reads Automatic's model from the settings each time.
/// Side effects: None until it runs.
fn speed_check(
    database_path: &Path,
    device_id: &DeviceId,
    embedding_cipher: &Arc<dyn EmbeddingCipher>,
) -> crate::speed_check::SpeedCheck {
    let target_database = database_path.to_path_buf();
    let metrics_database = database_path.to_path_buf();
    let metrics_device = device_id.clone();
    let metrics_cipher = Arc::clone(embedding_cipher);
    crate::speed_check::SpeedCheck::new(
        Box::new(move || crate::ffi_transcription::speed_check_target(&target_database)),
        Box::new(|model_path, efficient_gpu, audio_source| {
            transcript_pass_engine(model_path, efficient_gpu, audio_source)
        }),
        Box::new(move || {
            Ok(Box::new(open_note_repository(
                &metrics_database,
                metrics_device.clone(),
                Arc::clone(&metrics_cipher),
            )?)
                as Box<dyn crate::ports::telemetry::MetricsRepository>)
        }),
        crate::speed_check::record_file_for(database_path),
        device_id.clone(),
    )
}

/// Loads the after-meeting pass's model with the live engine's GPU policy.
///
/// Inputs: The model file, the efficient-mode choice, and the store the engine reads windows from.
/// Outputs: The engine, or why it cannot load. Side effects: Loads the model into memory.
#[cfg(feature = "local-ai")]
fn transcript_pass_engine(
    model_path: &Path,
    efficient_gpu: bool,
    audio_source: Arc<ChunkAudioStore>,
) -> Result<Box<dyn TranscriptionEngine>> {
    use crate::adapters::ai::local_whisper_cpp::{
        LocalWhisperCppConfig, LocalWhisperCppTranscriptionEngine,
    };
    let use_gpu = whisper_gpu_allowed() && (!cfg!(target_os = "android") || efficient_gpu);
    let config = LocalWhisperCppConfig::new(
        model_path.to_path_buf(),
        crate::adapters::ai::compute_resources::preferred_inference_threads(),
        use_gpu,
        None,
    )
    .context("invalid transcript pass configuration")?
    .with_efficient_gpu(efficient_gpu);
    let engine = LocalWhisperCppTranscriptionEngine::new(config, audio_source as _)
        .context("unable to load the transcript pass model")?;
    Ok(Box::new(engine))
}

/// Opens the NPU engine for one round of the after-meeting pass (#75).
///
/// Inputs: The NPU folder, the vocabulary model and the store the engine reads windows from.
/// Outputs: The engine; `None` when the NPU will not open (logged) or this build lacks it.
/// Side effects: Loads the model onto the NPU (about 2 s).
fn npu_pass_engine(
    directory: &Path,
    vocabulary_model: &Path,
    audio_source: &Arc<ChunkAudioStore>,
) -> Option<Box<dyn TranscriptionEngine>> {
    #[cfg(feature = "npu-whisper")]
    {
        use crate::adapters::ai::npu_whisper::{
            install::NpuInstall,
            qnn_engine::{NpuWhisperFiles, QnnWhisperEngine},
        };
        let install = NpuInstall::new(directory);
        let files = NpuWhisperFiles {
            model_dir: install.model_dir(),
            qnn_dir: install.qnn_dir(),
            vocabulary_model: vocabulary_model.to_path_buf(),
        };
        match QnnWhisperEngine::open(&files, None, Arc::clone(audio_source) as _) {
            Ok(engine) => Some(Box::new(engine)),
            Err(error) => {
                tracing::warn!(%error, "the NPU could not be used for the after-meeting pass");
                None
            }
        }
    }
    #[cfg(not(feature = "npu-whisper"))]
    {
        let _ = (directory, vocabulary_model, audio_source);
        None
    }
}

/// Reports that this build has no local engine for the pass.
#[cfg(not(feature = "local-ai"))]
fn transcript_pass_engine(
    _model_path: &Path,
    _efficient_gpu: bool,
    _audio_source: Arc<ChunkAudioStore>,
) -> Result<Box<dyn TranscriptionEngine>> {
    bail!("the transcript pass needs the local-ai feature")
}
