//! Composes the live meeting pipeline from environment configuration for the FFI, CLI, and daemon.
//!
//! This is the only place that names concrete AI adapters. It reads the pipeline variables,
//! selects a transcription engine (whisper.cpp in `local-ai` builds, or OpenAI when a key is
//! configured), and spawns a `RecordingController` whose worker owns its own SQLite connections
//! (sharing the already-unlocked embedding cipher) and the encrypted voice library as matcher.
//! No diarization adapter exists yet, so segments are `Unattributed`; requesting diarization only
//! logs that. Model loading happens on the worker thread, never on the caller's.
//!
//! | Variable | Values | Default |
//! | --- | --- | --- |
//! | `NOTE_TAKER_TRANSCRIPTION_PROVIDER` | `auto`, `local`, `openai` | `auto` (local model, else OpenAI key) |
//! | `NOTE_TAKER_WHISPER_MODEL` | path to a GGML/GGUF model (`local-ai` builds) | unset |
//! | `NOTE_TAKER_WHISPER_LANGUAGE` | `auto` or a 2–3 letter Whisper language code (`it`, `en`) | `auto` |
//! | `OPENAI_API_KEY` | OpenAI key (never logged) | unset |
//! | `NOTE_TAKER_OPENAI_WHISPER_MODEL`, `NOTE_TAKER_OPENAI_LANGUAGE` | OpenAI model and language | `whisper-1`, auto |
//! | `NOTE_TAKER_ENABLE_DIARIZATION` | boolean | `false` |
//! | `NOTE_TAKER_VAD_ENABLED` | boolean; `false` cuts near every 15 s | `true` |

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, anyhow, bail};

use crate::{
    adapters::{
        ai::openai_whisper_adapter::{
            OpenAiWhisperConfig, OpenAiWhisperTranscriptionEngine, ReqwestOpenAiHttpClient,
        },
        audio::{chunk_audio_store::ChunkAudioStore, vad_chunker::VadConfig},
        storage::sqlite_voice_vector_db::SqliteVoiceVectorDB,
    },
    application::{
        pipeline::{
            pipeline_factory::{NoDiarizationEngine, PipelineFactory},
            task_metrics::{TaskMetricsRecorder, TaskProfile},
        },
        recording_manager::RecordingManager,
        voice_identity_manager::VoiceIdentityManager,
    },
    domain::{
        errors::TranscriptionError,
        models::{
            AudioRecording, DeviceId, ExternalMediaChunk, TaskType, Transcript,
            TranscriptionContext,
        },
    },
    ports::{ai::TranscriptionEngine, crypto::EmbeddingCipher},
    recording_controller::{ChunkingMode, RecordingController},
    storage_composition::open_note_repository,
};

#[cfg(test)]
#[path = "pipeline_composition_tests.rs"]
mod tests;

/// Estimated draw of on-device whisper.cpp, which runs on the CPU: the same 5 W the compute
/// router assumes for CPU work.
pub const LOCAL_TRANSCRIPTION_POWER_MILLIWATTS: f32 = 5_000.0;
/// Estimated draw while a chunk is uploaded and the provider transcribes it: the device mostly
/// waits on the network, so a fifth of the local estimate.
pub const CLOUD_TRANSCRIPTION_POWER_MILLIWATTS: f32 = 1_000.0;

/// Selects the transcription provider.
pub const TRANSCRIPTION_PROVIDER_VARIABLE: &str = "NOTE_TAKER_TRANSCRIPTION_PROVIDER";
/// Path of the local whisper.cpp model.
pub const WHISPER_MODEL_VARIABLE: &str = "NOTE_TAKER_WHISPER_MODEL";
/// OpenAI API key.
pub const OPENAI_API_KEY_VARIABLE: &str = "OPENAI_API_KEY";
/// OpenAI transcription model.
pub const OPENAI_MODEL_VARIABLE: &str = "NOTE_TAKER_OPENAI_WHISPER_MODEL";
/// Local Whisper transcription language: `auto` or a Whisper language code such as `it` or `en`.
pub const WHISPER_LANGUAGE_VARIABLE: &str = "NOTE_TAKER_WHISPER_LANGUAGE";

/// OpenAI transcription language.
pub const OPENAI_LANGUAGE_VARIABLE: &str = "NOTE_TAKER_OPENAI_LANGUAGE";
/// Requests speaker diarization.
pub const ENABLE_DIARIZATION_VARIABLE: &str = "NOTE_TAKER_ENABLE_DIARIZATION";
/// Enables speech-pause chunking.
pub const VAD_ENABLED_VARIABLE: &str = "NOTE_TAKER_VAD_ENABLED";

/// Which transcription engine the pipeline should use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranscriptionProvider {
    /// A local model when configured, otherwise OpenAI when a key is configured.
    Auto,
    /// Only the local whisper.cpp model.
    Local,
    /// Only OpenAI (chunk audio leaves the device).
    OpenAi,
}

/// Pipeline settings read from the environment.
#[derive(Clone)]
pub struct PipelineConfig {
    transcription_provider: TranscriptionProvider,
    whisper_model_path: Option<PathBuf>,
    /// Always passed to whisper.cpp, whose own default (`en`) would force English output.
    whisper_language: String,
    /// Secret; excluded from `Debug`.
    openai_api_key: Option<String>,
    openai_model: String,
    openai_language: Option<String>,
    diarization_requested: bool,
    vad_enabled: bool,
}

impl std::fmt::Debug for PipelineConfig {
    /// Prints the configuration with the API key redacted.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PipelineConfig")
            .field("transcription_provider", &self.transcription_provider)
            .field("whisper_model_path", &self.whisper_model_path)
            .field("whisper_language", &self.whisper_language)
            .field(
                "openai_api_key",
                &self.openai_api_key.as_ref().map(|_| "<redacted>"),
            )
            .field("openai_model", &self.openai_model)
            .field("openai_language", &self.openai_language)
            .field("diarization_requested", &self.diarization_requested)
            .field("vad_enabled", &self.vad_enabled)
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
        let transcription_provider = match read(TRANSCRIPTION_PROVIDER_VARIABLE)
            .map(|value| value.trim().to_ascii_lowercase())
            .as_deref()
        {
            None | Some("auto") => TranscriptionProvider::Auto,
            Some("local") => TranscriptionProvider::Local,
            Some("openai") => TranscriptionProvider::OpenAi,
            Some(_) => {
                bail!("{TRANSCRIPTION_PROVIDER_VARIABLE} must be one of auto, local, or openai")
            }
        };
        Ok(Self {
            transcription_provider,
            whisper_model_path: read(WHISPER_MODEL_VARIABLE).map(PathBuf::from),
            whisper_language: parse_whisper_language(read(WHISPER_LANGUAGE_VARIABLE))?,
            openai_api_key: read(OPENAI_API_KEY_VARIABLE),
            openai_model: read(OPENAI_MODEL_VARIABLE).unwrap_or_else(|| "whisper-1".to_owned()),
            openai_language: read(OPENAI_LANGUAGE_VARIABLE),
            diarization_requested: parse_flag(
                ENABLE_DIARIZATION_VARIABLE,
                read(ENABLE_DIARIZATION_VARIABLE),
                false,
            )?,
            vad_enabled: parse_flag(VAD_ENABLED_VARIABLE, read(VAD_ENABLED_VARIABLE), true)?,
        })
    }

    /// Returns the local Whisper language: `auto` (detect per chunk) or a lowercase language code.
    pub fn whisper_language(&self) -> &str {
        &self.whisper_language
    }

    /// Returns the selected transcription provider.
    pub fn transcription_provider(&self) -> TranscriptionProvider {
        self.transcription_provider
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
/// 2–3 letter language code; anything else is an error naming the variable.
/// Side effects: None.
fn parse_whisper_language(value: Option<String>) -> Result<String> {
    let Some(value) = value else {
        return Ok("auto".to_owned());
    };
    let language = value.trim().to_ascii_lowercase();
    let is_language_code =
        (2..=3).contains(&language.len()) && language.bytes().all(|byte| byte.is_ascii_lowercase());
    if language == "auto" || is_language_code {
        Ok(language)
    } else {
        bail!(
            "{WHISPER_LANGUAGE_VARIABLE} must be auto or a Whisper language code such as it or en"
        )
    }
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
    /// Cloud transcription through OpenAI.
    OpenAi(OpenAiWhisperTranscriptionEngine),
    /// Local whisper.cpp inference.
    #[cfg(feature = "local-ai")]
    Local(crate::adapters::ai::local_whisper_cpp::LocalWhisperCppTranscriptionEngine),
}

impl TranscriptionEngine for SelectedTranscriptionEngine {
    /// Delegates to the selected engine.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        match self {
            Self::OpenAi(engine) => engine.transcribe(audio_recording, transcription_context),
            #[cfg(feature = "local-ai")]
            Self::Local(engine) => engine.transcribe(audio_recording, transcription_context),
        }
    }

    /// Delegates to the selected engine.
    fn transcribe_external_media(
        &self,
        media_chunk: &ExternalMediaChunk,
        audio_recording: &AudioRecording,
    ) -> Result<Transcript, TranscriptionError> {
        match self {
            Self::OpenAi(engine) => engine.transcribe_external_media(media_chunk, audio_recording),
            #[cfg(feature = "local-ai")]
            Self::Local(engine) => engine.transcribe_external_media(media_chunk, audio_recording),
        }
    }
}

/// Builds the transcription engine selected by `config`.
///
/// Inputs: The configuration and the audio store the engine reads chunks from.
/// Outputs: The engine, or an error naming the variables that would configure one.
/// Side effects: Loads the local model when selected (slow; run it off the UI thread). Creates
/// an HTTP client for OpenAI without sending any request.
pub fn select_transcription_engine(
    config: &PipelineConfig,
    chunk_audio_store: &Arc<ChunkAudioStore>,
) -> Result<SelectedTranscriptionEngine> {
    match config.transcription_provider {
        TranscriptionProvider::Local => local_engine(config, chunk_audio_store),
        TranscriptionProvider::OpenAi => openai_engine(config, chunk_audio_store),
        TranscriptionProvider::Auto => {
            if config.whisper_model_path.is_some() {
                match local_engine(config, chunk_audio_store) {
                    Ok(engine) => return Ok(engine),
                    Err(error) if config.openai_api_key.is_some() => tracing::warn!(
                        error = %format!("{error:#}"),
                        "Local transcription unavailable; falling back to OpenAI"
                    ),
                    Err(error) => return Err(error),
                }
            }
            if config.openai_api_key.is_none() {
                bail!(
                    "no transcription engine is configured: set {WHISPER_MODEL_VARIABLE} \
                     (local-ai builds) for on-device transcription or {OPENAI_API_KEY_VARIABLE} \
                     for cloud transcription"
                );
            }
            openai_engine(config, chunk_audio_store)
        }
    }
}

/// Describes how the selected engine's transcriptions appear in telemetry (Issue #53).
///
/// Inputs: The selected engine and the configuration it was built from.
/// Outputs: `CloudTranscription` with the OpenAI model for the cloud engine, `Transcription`
/// with the model file's name for the local one.
/// Side effects: None.
pub fn transcription_task_profile(
    engine: &SelectedTranscriptionEngine,
    config: &PipelineConfig,
) -> TaskProfile {
    match engine {
        SelectedTranscriptionEngine::OpenAi(_) => TaskProfile::new(
            TaskType::CloudTranscription,
            config.openai_model.clone(),
            CLOUD_TRANSCRIPTION_POWER_MILLIWATTS,
        ),
        #[cfg(feature = "local-ai")]
        SelectedTranscriptionEngine::Local(_) => TaskProfile::new(
            TaskType::Transcription,
            config
                .whisper_model_path
                .as_deref()
                .map_or_else(|| "whisper.cpp".to_owned(), local_model_label),
            LOCAL_TRANSCRIPTION_POWER_MILLIWATTS,
        ),
    }
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

/// Builds the OpenAI engine.
///
/// Inputs: The configuration (requires the API key) and the chunk audio store.
/// Outputs: The engine, or an error naming `OPENAI_API_KEY`.
/// Side effects: Creates an HTTP client; logs that chunk audio will leave the device.
fn openai_engine(
    config: &PipelineConfig,
    chunk_audio_store: &Arc<ChunkAudioStore>,
) -> Result<SelectedTranscriptionEngine> {
    let api_key = config.openai_api_key.clone().ok_or_else(|| {
        anyhow!("OpenAI transcription requires {OPENAI_API_KEY_VARIABLE} to be set")
    })?;
    let openai_config = OpenAiWhisperConfig::new(
        api_key,
        config.openai_model.clone(),
        config.openai_language.clone(),
    )
    .context("invalid OpenAI transcription configuration")?;
    let http_client =
        ReqwestOpenAiHttpClient::new().context("unable to create the OpenAI HTTP client")?;
    tracing::warn!("Cloud transcription selected: recorded chunk audio is sent to OpenAI");
    Ok(SelectedTranscriptionEngine::OpenAi(
        OpenAiWhisperTranscriptionEngine::with_dependencies(
            openai_config,
            Arc::clone(chunk_audio_store) as _,
            Arc::new(http_client),
        ),
    ))
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
    let thread_count = std::thread::available_parallelism().map_or(4, usize::from);
    // An explicit language is required: whisper.cpp defaults to English, so Italian meetings
    // would otherwise come out as English text. `auto` makes it detect the language per chunk.
    let local_config = LocalWhisperCppConfig::new(
        model_path,
        thread_count,
        false,
        Some(config.whisper_language.clone()),
    )
    .context("invalid local transcription configuration")?;
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

/// Starts the pipeline worker and returns an idle recording controller.
///
/// Inputs: The SQLite path (the same database the caller already opened), the device ID, the
/// verified embedding cipher from `open_encrypted_stores`, and the pipeline configuration.
/// Outputs: The controller, or an error explaining which dependency is missing.
/// Side effects: Spawns the worker thread, which selects (and may load) the transcription
/// engine and opens its own note-repository, telemetry and voice-store connections; every
/// transcribed chunk then saves one telemetry row. Blocks until ready.
pub fn spawn_recording_controller(
    database_path: &Path,
    device_id: DeviceId,
    embedding_cipher: Arc<dyn EmbeddingCipher>,
    config: PipelineConfig,
) -> Result<RecordingController> {
    let chunk_audio_store = Arc::new(ChunkAudioStore::new());
    let engine_audio_store = Arc::clone(&chunk_audio_store);
    let chunking_mode = config.chunking_mode();
    let worker_database_path = database_path.to_path_buf();
    let worker_device_id = device_id.clone();
    let recording_manager = RecordingManager::spawn(move || -> Result<_> {
        let transcription_engine = select_transcription_engine(&config, &engine_audio_store)?;
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
        let task_metrics = TaskMetricsRecorder::new(
            Box::new(metrics_repository),
            worker_device_id,
            transcription_task_profile(&transcription_engine, &config),
        );
        let voice_store = SqliteVoiceVectorDB::open(&worker_database_path, embedding_cipher)
            .context("unable to open the pipeline voice store")?;
        if config.diarization_requested {
            tracing::warn!(
                "Diarization was requested, but no diarization engine is available in this \
                 build; segments will be unattributed"
            );
        }
        Ok(PipelineFactory::new(transcription_engine, note_repository)
            .with_diarization_engine(None::<NoDiarizationEngine>)
            .with_voice_matcher(Some(VoiceIdentityManager::new(voice_store)))
            .with_task_metrics(task_metrics))
    })?;
    Ok(RecordingController::new(
        recording_manager,
        chunk_audio_store,
        chunking_mode,
        device_id,
    ))
}
