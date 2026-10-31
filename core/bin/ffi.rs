//! Exposes a C-compatible recording and logging boundary for Flutter without sharing Rust structs.

use std::{
    collections::HashMap,
    ffi::{CStr, CString, c_char, c_int},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::Duration,
};

use crate::{
    adapters::{
        audio::live_microphone::LiveMicrophone,
        export::{default_export_directory, default_exporters},
        media::local_file_metadata::LocalFileMetadataExtractor,
        storage::{
            attachment_file_store::AttachmentFileStore, sqlite_adapter::SqliteNoteRepository,
            sqlite_voice_vector_db::SqliteVoiceVectorDB,
        },
    },
    application::transcription_routing::RemoteTranscriptionGateway,
    application::{
        attachment_manager::AttachmentManager,
        cluster_review::{
            ClusterReviewError, assign_cluster_to_named_identity, find_cluster_review_clip,
            list_unknown_clusters, load_voice_library,
        },
        crypto::key_manager::KeyManagerConfig,
        export_manager::{
            ExportManager, ExportWriter, UNATTRIBUTED_SPEAKER_LABEL, UNKNOWN_SPEAKER_LABEL,
        },
        meeting_rename::{MeetingRenameError, rename_meeting},
        p2p_service::P2pControl,
        unknown_speaker_clusterer::ClusteringError,
        voice_identity_manager::VoiceIdentityManager,
    },
    device_identity::{DEVICE_ID_VARIABLE, resolve_device_id},
    domain::transcription_language::TranscriptionLanguage,
    domain::{
        errors::ExportError,
        models::{
            Attachment, AudioState, ExportFormat, ExportOptions, MeetingId, MeetingMetadata,
            MeetingTitle, SpeakerIdentity, TaskMetrics, TaskType, TranscriptSegment,
            UnassignedVoiceClip,
        },
    },
    ffi_language::default_language,
    ffi_p2p::{p2p_command_json, p2p_status_json},
    ffi_settings::{APP_SETTINGS, ConfiguredSettings, SettingsError, app_setting},
    p2p_composition::{P2pConfig, P2pHandle, spawn_p2p_task},
    pipeline_composition::{PipelineConfig, spawn_recording_controller},
    ports::{
        audio::MicrophoneSource,
        media::MetadataExtractor,
        storage::{AttachmentRepository, NoteRepository},
        telemetry::MetricsRepository,
        voice::VoiceVectorDB,
    },
    recording_controller::{RecordingControlError, RecordingController},
    storage_composition::{EncryptedStores, open_encrypted_stores_with_keychain},
    transcription_settings::{
        OpenAiKeyStore, TranscriptionPaths, apply_saved_settings, load_settings,
    },
};
use uuid::Uuid;

/// Indicates that a C ABI request completed successfully.
pub const FFI_SUCCESS: c_int = 0;
/// Indicates that a C ABI request received malformed input, including a null or invalid UTF-8 string.
pub const FFI_INVALID_ARGUMENT: c_int = 1;
/// Indicates that the Rust audio or logging operation rejected the request.
pub const FFI_OPERATION_REJECTED: c_int = 2;
/// Indicates that the bridge could not acquire its process-local synchronization state.
pub const FFI_RUNTIME_UNAVAILABLE: c_int = 3;
/// Indicates that the meeting pipeline could not be built, e.g. no transcription engine is
/// configured (see the Rust logs for the variables to set).
pub const FFI_PIPELINE_UNAVAILABLE: c_int = 4;
/// Indicates that the requested meeting does not exist (returned by `note_taker_rename_meeting`).
pub const FFI_MEETING_NOT_FOUND: c_int = 5;
/// Indicates that the platform keychain holding the database key is missing, locked, or refused
/// access (for example a dismissed unlock prompt). The app shows the error with Retry; there is
/// no key-file fallback.
pub const FFI_KEYCHAIN_UNAVAILABLE: c_int = 6;
/// Indicates that the microphone could not be opened when a recording started or resumed: no
/// input device, microphone access denied (Android permission, Windows privacy setting), or an
/// unsupported format. The recording is not started (Issue #45).
pub const FFI_MICROPHONE_UNAVAILABLE: c_int = 7;
/// Returns the portable Idle state from `note_taker_get_current_state`.
pub const FFI_AUDIO_STATE_IDLE: c_int = 10;
/// Returns the portable Recording state from `note_taker_get_current_state`.
pub const FFI_AUDIO_STATE_RECORDING: c_int = 11;
/// Returns the portable Paused state from `note_taker_get_current_state`.
pub const FFI_AUDIO_STATE_PAUSED: c_int = 12;
/// Returns the portable Interrupted state from `note_taker_get_current_state`.
pub const FFI_AUDIO_STATE_INTERRUPTED: c_int = 13;
/// Returns the portable Stopped state from `note_taker_get_current_state`.
pub const FFI_AUDIO_STATE_STOPPED: c_int = 14;
/// Returns the portable Error state from `note_taker_get_current_state`.
pub const FFI_AUDIO_STATE_ERROR: c_int = 15;

/// Converts a portable state to the stable integer C ABI used by background services.
///
/// Inputs: `audio_state` is a Rust-owned portable lifecycle value.
/// Outputs: Its documented `FFI_AUDIO_STATE_*` value.
/// Side effects: None.
fn audio_state_code(audio_state: crate::domain::models::AudioState) -> c_int {
    match audio_state {
        crate::domain::models::AudioState::Idle => FFI_AUDIO_STATE_IDLE,
        crate::domain::models::AudioState::Recording => FFI_AUDIO_STATE_RECORDING,
        crate::domain::models::AudioState::Paused => FFI_AUDIO_STATE_PAUSED,
        crate::domain::models::AudioState::Interrupted => FFI_AUDIO_STATE_INTERRUPTED,
        crate::domain::models::AudioState::Stopped => FFI_AUDIO_STATE_STOPPED,
        crate::domain::models::AudioState::Error => FFI_AUDIO_STATE_ERROR,
    }
}

/// Maps a recording command result to its C status.
///
/// Inputs: The controller's result.
/// Outputs: `FFI_SUCCESS`, `FFI_PIPELINE_UNAVAILABLE` when the worker is unavailable, or
/// `FFI_OPERATION_REJECTED` for lifecycle, audio, and session rejections.
/// Side effects: Logs the rejection reason (never audio or transcript text).
fn recording_status<Value>(result: Result<Value, RecordingControlError>) -> c_int {
    use crate::application::recording_manager::RecordingManagerError;

    match result {
        Ok(_) => FFI_SUCCESS,
        Err(error) => {
            tracing::warn!(error = %error, "Recording command rejected");
            match error {
                RecordingControlError::Pipeline(
                    RecordingManagerError::PipelineUnavailable { .. }
                    | RecordingManagerError::WorkerStopped,
                ) => FFI_PIPELINE_UNAVAILABLE,
                _ => FFI_OPERATION_REJECTED,
            }
        }
    }
}

/// Copies caller-owned PCM samples into Rust memory.
///
/// Inputs: `samples` points to `sample_count` contiguous `int16_t` values; it may be null only
/// when `sample_count` is 0.
/// Outputs: The copied samples, or `None` for a null pointer with a nonzero count.
/// Side effects: None; the pointer is not retained.
///
/// # Safety
/// A non-null `samples` must be valid for reading `sample_count` aligned `i16` values.
unsafe fn pcm_samples_from_raw(samples: *const i16, sample_count: usize) -> Option<Vec<i16>> {
    if sample_count == 0 {
        return Some(Vec::new());
    }
    if samples.is_null() {
        return None;
    }
    Some(unsafe { std::slice::from_raw_parts(samples, sample_count) }.to_vec())
}

/// Returns a fixed transcript without reading audio, standing in for a real engine in tests.
#[cfg(test)]
struct FixedTranscriptionEngine;

#[cfg(test)]
impl crate::ports::ai::TranscriptionEngine for FixedTranscriptionEngine {
    /// Returns "transcribed chunk" for the recording's meeting.
    fn transcribe(
        &self,
        audio_recording: &crate::domain::models::AudioRecording,
        _transcription_context: &crate::domain::models::TranscriptionContext,
    ) -> Result<crate::domain::models::Transcript, crate::domain::errors::TranscriptionError> {
        Ok(crate::domain::models::Transcript::new(
            audio_recording.meeting_id().clone(),
            "transcribed chunk",
        ))
    }
}

/// Builds a time-chunked controller around `FixedTranscriptionEngine` for FFI tests.
///
/// Inputs: The database file and cipher to share with the runtime, or `None` for a separate
/// in-memory database.
/// Outputs: The controller, or the storage error.
/// Side effects: Spawns a pipeline worker; opens SQLite on the given file when provided.
#[cfg(test)]
fn test_recording_controller(
    shared_database: Option<(
        PathBuf,
        std::sync::Arc<dyn crate::ports::crypto::EmbeddingCipher>,
    )>,
) -> anyhow::Result<RecordingController> {
    use crate::{
        adapters::audio::chunk_audio_store::ChunkAudioStore,
        application::{
            crypto::voice_encryptor::ephemeral_cipher, pipeline::pipeline_factory::PipelineFactory,
            recording_manager::RecordingManager,
        },
        recording_controller::ChunkingMode,
    };

    let recording_manager = RecordingManager::spawn(move || -> anyhow::Result<_> {
        let note_repository = match shared_database {
            Some((database_path, embedding_cipher)) => SqliteNoteRepository::open(
                database_path,
                crate::domain::models::DeviceId::new("ffi-test"),
                embedding_cipher,
            )?,
            None => SqliteNoteRepository::open_in_memory(ephemeral_cipher())?,
        };
        Ok(PipelineFactory::new(
            FixedTranscriptionEngine,
            note_repository,
        ))
    })?;
    Ok(RecordingController::new(
        recording_manager,
        std::sync::Arc::new(ChunkAudioStore::new()),
        ChunkingMode::TimeBased,
        crate::domain::models::DeviceId::new("ffi-test"),
    ))
}

/// Builds the recording controller (pipeline worker plus capture driver) on first use.
type RecordingControllerBuilder = Box<dyn Fn() -> anyhow::Result<RecordingController> + Send>;

/// Stores adapter-owned state required by the primitive Flutter C ABI commands.
struct FfiRuntime {
    /// Built on the first recording start and reused for every later recording.
    recording_controller: Option<RecordingController>,
    /// Composes the meeting pipeline; retried on the next start if it fails.
    recording_controller_builder: RecordingControllerBuilder,
    note_repository: SqliteNoteRepository,
    voice_identity_manager: VoiceIdentityManager<SqliteVoiceVectorDB>,
    review_clip_directory: PathBuf,
    loaded_review_clips: HashMap<Uuid, UnassignedVoiceClip>,
    pending_attachment_imports: HashMap<Uuid, PendingAttachmentImport>,
    /// Where `note_taker_attach_files` copies attached files and photos (Issue #54).
    attachment_directory: PathBuf,
    export_writer: ExportWriter,
    /// The P2P pairing and sync task, when enabled and started (Issue #43). Shared so a command
    /// can wait for the task without holding the runtime lock.
    p2p: Option<Arc<P2pHandle>>,
    /// Where recordings capture audio from; `None` when the host pushes PCM itself
    /// (`note_taker_push_audio_pcm`) or the build has no `microphone` feature (Issue #45).
    microphone_source: Option<Arc<dyn MicrophoneSource>>,
    /// The microphone while a recording captures from it.
    live_microphone: Option<LiveMicrophone>,
    /// Whether a drain ticker is running for the open microphone.
    microphone_ticker_running: bool,
    /// The language chosen for the next recording or the current one; a stop clears it.
    recording_language: Option<TranscriptionLanguage>,
    /// Starts the thread that drains the microphone into the recording (a no-op in tests,
    /// which drain by hand).
    spawn_microphone_ticker: fn(),
}

/// How often the ticker moves microphone audio into the recording.
const MICROPHONE_DRAIN_INTERVAL: Duration = Duration::from_millis(100);

/// Options accepted by `note_taker_export_meeting`; omitted fields use defaults.
#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FfiExportOptions {
    /// Defaults to `true`.
    include_attachments: Option<bool>,
    /// Defaults to `true`.
    include_metadata: Option<bool>,
    /// Explicit destination file; defaults to a generated name in the export directory.
    output_path: Option<PathBuf>,
}

/// Builds the JSON error object returned by the JSON-returning export and cluster commands.
///
/// Inputs: A stable machine-readable `error_code` and a human-readable `message`.
/// Outputs: `{"ok":false,"error_code":…,"error":…}` as a string.
/// Side effects: None.
fn export_error_json(error_code: &str, message: &str) -> String {
    serde_json::json!({ "ok": false, "error_code": error_code, "error": message }).to_string()
}

/// Serializes the meeting fields shared by `note_taker_get_meetings` and `note_taker_get_meeting`.
///
/// Inputs: Persisted meeting metadata.
/// Inputs: Persisted meeting metadata and the meeting's transcript segments.
/// Outputs: `{"meeting_id","title","created_at","duration_ms"}` with the display title, RFC 3339
/// UTC time, and the latest segment end (0 without segments).
/// Side effects: None.
fn meeting_summary_json(
    meeting_metadata: &MeetingMetadata,
    segments: &[TranscriptSegment],
) -> serde_json::Value {
    serde_json::json!({
        "meeting_id": meeting_metadata.meeting_id().as_str(),
        "title": meeting_metadata.display_title(),
        "created_at": meeting_metadata.created_at().to_rfc3339(),
        "duration_ms": segments
            .iter()
            .map(TranscriptSegment::end_time_ms)
            .max()
            .unwrap_or_default(),
    })
}

/// Validates the arguments of `note_taker_start_recording_with_title`.
///
/// Inputs: The decoded meeting ID and title; `None` means the caller passed null.
/// Outputs: The trimmed meeting ID (`None` generates one) and validated title, or
/// `FFI_INVALID_ARGUMENT` for a blank meeting ID or an invalid title.
/// Side effects: None.
fn titled_start_arguments(
    meeting_id: Option<&str>,
    title: Option<&str>,
) -> Result<(Option<MeetingId>, Option<MeetingTitle>), c_int> {
    let meeting_id = match meeting_id.map(str::trim) {
        Some("") => return Err(FFI_INVALID_ARGUMENT),
        Some(meeting_id) => Some(MeetingId::new(meeting_id)),
        None => None,
    };
    let title = title
        .map(MeetingTitle::parse)
        .transpose()
        .map_err(|_| FFI_INVALID_ARGUMENT)?;
    Ok((meeting_id, title))
}

/// Retains only metadata necessary to persist one explicitly user-approved attachment link.
struct PendingAttachmentImport {
    file_path: String,
    original_timestamp: chrono::DateTime<chrono::Utc>,
}

impl FfiRuntime {
    /// Creates the adapter-owned state for one process-local Flutter bridge.
    ///
    /// Inputs: None; the portable 16 kHz mono capture format is fixed for this initial bridge.
    /// Outputs: A ready runtime or an audio configuration error encoded as a C status by callers.
    /// Side effects: Unlocks the embedding encryption key (`NOTE_TAKER_ENCRYPTION_PASSPHRASE`,
    /// else the platform keychain or key file) and opens the note repository and the voice store on the same SQLite
    /// file. The meeting pipeline is composed later, on the first recording start.
    fn new() -> Result<Self, c_int> {
        // First, so failures while opening the stores reach the log file (Issue #46).
        crate::ffi_logging::start_app_logging();
        Self::from_settings(&app_setting)
    }

    /// Opens the runtime from `setting`, which resolves `NOTE_TAKER_*` names.
    ///
    /// Inputs: `setting` returns a variable's value: in production the app's configuration over
    /// the environment (`app_setting`), in tests a fixed map.
    /// Outputs: A ready runtime, or `FFI_OPERATION_REJECTED` when the device ID, the key, or the
    /// stores cannot be opened.
    /// Side effects: Creates the database's directory and the review-clip directory, unlocks the
    /// key, opens the stores, and starts the P2P task unless `NOTE_TAKER_P2P_ENABLED=false`. A P2P
    /// task that fails to start is logged and left out, as in the daemon: notes still work.
    fn from_settings(setting: &dyn Fn(&str) -> Option<String>) -> Result<Self, c_int> {
        let database_path = setting("NOTE_TAKER_DATABASE_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("note_taker.sqlite3"));
        if let Some(database_directory) = database_path.parent()
            && !database_directory.as_os_str().is_empty()
        {
            fs::create_dir_all(database_directory).map_err(|_| FFI_OPERATION_REJECTED)?;
        }
        let device_id =
            resolve_device_id(setting(DEVICE_ID_VARIABLE), &database_path).map_err(|error| {
                tracing::error!(error = %format!("{error:#}"), "unable to resolve the device ID");
                FFI_OPERATION_REJECTED
            })?;
        let EncryptedStores {
            note_repository,
            voice_store,
            embedding_cipher,
        } = open_encrypted_stores_with_keychain(
            &database_path,
            device_id.clone(),
            KeyManagerConfig::from_lookup(setting),
            || runtime_keychain(provided_encryption_key(), cfg!(target_os = "android")),
        )
        .map_err(|error| {
            tracing::error!(error = %format!("{error:#}"), "unable to open encrypted stores");
            store_open_status(&error)
        })?;
        let review_clip_directory = setting("NOTE_TAKER_REVIEW_CLIP_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("voice_review_clips"));
        fs::create_dir_all(&review_clip_directory).map_err(|_| FFI_OPERATION_REJECTED)?;
        let export_directory = setting("NOTE_TAKER_EXPORT_DIR")
            .filter(|directory| !directory.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(default_export_directory);
        let p2p = start_p2p(setting, &database_path, &device_id, &embedding_cipher);
        let pipeline_config = PipelineConfig::from_lookup(setting);
        let transcription_paths = TranscriptionPaths::from_lookup(setting);
        let pipeline_database_path = database_path.clone();
        let remote_gateway = p2p.clone();
        let recording_controller_builder: RecordingControllerBuilder = Box::new(move || {
            spawn_recording_controller(
                &pipeline_database_path,
                device_id.clone(),
                std::sync::Arc::clone(&embedding_cipher),
                match &pipeline_config {
                    Ok(pipeline_config) => {
                        with_saved_transcription(pipeline_config.clone(), &transcription_paths)
                    }
                    Err(error) => return Err(anyhow::anyhow!("{error:#}")),
                },
                remote_gateway
                    .clone()
                    .map(|task| task as Arc<dyn RemoteTranscriptionGateway>),
            )
        });
        let mut runtime = Self::from_parts(
            recording_controller_builder,
            note_repository,
            voice_store,
            review_clip_directory,
            ExportWriter::new(default_exporters(), export_directory),
        );
        runtime.p2p = p2p;
        runtime.attachment_directory = setting("NOTE_TAKER_ATTACHMENT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("attachments"));
        runtime.microphone_source = default_microphone_source();
        runtime.spawn_microphone_ticker = spawn_global_microphone_ticker;
        Ok(runtime)
    }

    /// Assembles runtime state from already-opened adapters.
    ///
    /// Inputs: The recorder, note repository, voice store (persisting speaker names and voice
    /// variants), review-clip directory, and export writer.
    /// Outputs: The runtime with empty caches.
    /// Side effects: None.
    fn from_parts(
        recording_controller_builder: RecordingControllerBuilder,
        note_repository: SqliteNoteRepository,
        voice_store: SqliteVoiceVectorDB,
        review_clip_directory: PathBuf,
        export_writer: ExportWriter,
    ) -> Self {
        Self {
            recording_controller: None,
            recording_controller_builder,
            note_repository,
            voice_identity_manager: VoiceIdentityManager::new(voice_store),
            review_clip_directory,
            loaded_review_clips: HashMap::new(),
            pending_attachment_imports: HashMap::new(),
            attachment_directory: PathBuf::from("attachments"),
            export_writer,
            p2p: None,
            microphone_source: None,
            live_microphone: None,
            microphone_ticker_running: false,
            recording_language: None,
            spawn_microphone_ticker: || {},
        }
    }

    /// Creates a runtime over in-memory SQLite that exports into `export_directory`.
    ///
    /// Inputs: The directory for generated export filenames.
    /// Outputs: A runtime that never touches the developer database, or a C status.
    /// Side effects: Allocates in-memory SQLite; creates no directories.
    #[cfg(test)]
    fn in_memory_for_tests(export_directory: &Path) -> Result<Self, c_int> {
        let embedding_cipher = crate::application::crypto::voice_encryptor::ephemeral_cipher();
        let note_repository =
            SqliteNoteRepository::open_in_memory(std::sync::Arc::clone(&embedding_cipher))
                .map_err(|_| FFI_OPERATION_REJECTED)?;
        let voice_store = SqliteVoiceVectorDB::open_in_memory(embedding_cipher)
            .map_err(|_| FFI_OPERATION_REJECTED)?;
        Ok(Self::from_parts(
            Box::new(|| test_recording_controller(None)),
            note_repository,
            voice_store,
            export_directory.join("review-clips"),
            ExportWriter::new(default_exporters(), export_directory),
        )
        .with_attachment_directory(export_directory.join("attachments")))
    }

    /// Creates a runtime over the SQLite file at `database_path`, as one app launch would.
    ///
    /// Inputs: The database file (created when missing) and the export directory, which also
    /// holds the random key file in `keys/`.
    /// Outputs: A runtime whose stores persist in `database_path`, or a C status.
    /// Side effects: Opens or creates the SQLite file and the key file.
    #[cfg(test)]
    fn open_for_tests(database_path: &Path, export_directory: &Path) -> Result<Self, c_int> {
        let EncryptedStores {
            note_repository,
            voice_store,
            embedding_cipher,
        } = crate::storage_composition::open_encrypted_stores(
            database_path,
            crate::domain::models::DeviceId::new("ffi-test"),
            crate::storage_composition::key_file_config_for_tests(&export_directory.join("keys")),
        )
        .map_err(|_| FFI_OPERATION_REJECTED)?;
        let worker_database_path = database_path.to_path_buf();
        Ok(Self::from_parts(
            Box::new(move || {
                test_recording_controller(Some((
                    worker_database_path.clone(),
                    std::sync::Arc::clone(&embedding_cipher),
                )))
            }),
            note_repository,
            voice_store,
            export_directory.join("review-clips"),
            ExportWriter::new(default_exporters(), export_directory),
        )
        .with_attachment_directory(export_directory.join("attachments")))
    }

    /// Copies attached files under `attachment_directory` instead of `attachments`.
    ///
    /// Inputs: The attachment root. Outputs: The runtime using it. Side effects: None.
    #[cfg(test)]
    fn with_attachment_directory(mut self, attachment_directory: PathBuf) -> Self {
        self.attachment_directory = attachment_directory;
        self
    }

    /// Replaces the pipeline builder, e.g. with one that fails or counts its calls.
    ///
    /// Inputs: The builder invoked on the first recording start.
    /// Outputs: The runtime using it.
    /// Side effects: None.
    #[cfg(test)]
    fn with_recording_controller_builder(
        mut self,
        recording_controller_builder: RecordingControllerBuilder,
    ) -> Self {
        self.recording_controller_builder = recording_controller_builder;
        self
    }

    /// Records from `microphone_source` instead of PCM pushed by the host.
    ///
    /// Inputs: The source opened on each start or resume.
    /// Outputs: The runtime using it.
    /// Side effects: None.
    #[cfg(test)]
    fn with_microphone_source(mut self, microphone_source: Arc<dyn MicrophoneSource>) -> Self {
        self.microphone_source = Some(microphone_source);
        self
    }

    /// Exports one meeting through the shared `ExportManager` and describes the result as JSON.
    ///
    /// Inputs: `meeting_id` (non-empty), `format` (`markdown`/`md`, `pdf`, `json`), and
    /// `options_json` (empty or a `FfiExportOptions` object).
    /// Outputs: `{"ok":true,"file_path","format","size_bytes","metadata":{…}}` on success, or
    /// `{"ok":false,"error_code","error"}` where `error_code` is `invalid_argument`,
    /// `unsupported_format`, `meeting_not_found`, or `export_failed`.
    /// Side effects: Reads SQLite, copies attachments, and writes the export file synchronously;
    /// Flutter must call it from a background isolate.
    fn export_meeting(&self, meeting_id: &str, format: &str, options_json: &str) -> String {
        if meeting_id.trim().is_empty() {
            return export_error_json("invalid_argument", "meeting_id must not be empty");
        }
        let export_format = match format.parse::<ExportFormat>() {
            Ok(export_format) => export_format,
            Err(error) => return export_error_json("invalid_argument", &error.to_string()),
        };
        let ffi_options = if options_json.trim().is_empty() {
            FfiExportOptions::default()
        } else {
            match serde_json::from_str::<FfiExportOptions>(options_json) {
                Ok(ffi_options) => ffi_options,
                Err(error) => {
                    return export_error_json(
                        "invalid_argument",
                        &format!("options_json is invalid: {error}"),
                    );
                }
            }
        };
        let export_options = ExportOptions::new(
            export_format,
            ffi_options.include_attachments.unwrap_or(true),
            ffi_options.include_metadata.unwrap_or(true),
            None,
        );
        let export_manager = ExportManager::new(
            &self.note_repository,
            &self.note_repository,
            self.voice_identity_manager.vector_database(),
            &self.export_writer,
        );
        match export_manager.export_meeting(
            &MeetingId::new(meeting_id),
            &export_options,
            ffi_options.output_path.as_deref(),
        ) {
            Ok(export_result) => serde_json::json!({
                "ok": true,
                "file_path": export_result.file_path(),
                "format": export_result.format().as_str(),
                "size_bytes": export_result.size_bytes(),
                "metadata": export_result
                    .metadata()
                    .iter()
                    .map(|(metadata_key, metadata_value)| {
                        (metadata_key.clone(), serde_json::Value::from(metadata_value.as_str()))
                    })
                    .collect::<serde_json::Map<String, serde_json::Value>>()
            })
            .to_string(),
            Err(error) => {
                let error_code = match error.downcast_ref::<ExportError>() {
                    Some(ExportError::MeetingNotFound { .. }) => "meeting_not_found",
                    Some(ExportError::UnsupportedFormat { .. }) => "unsupported_format",
                    Some(ExportError::UnknownFormat { .. } | ExportError::InvalidDateRange) => {
                        "invalid_argument"
                    }
                    _ => "export_failed",
                };
                export_error_json(error_code, &format!("{error:#}"))
            }
        }
    }

    /// Loads clip metadata, writes only local review files, and serializes metadata as JSON.
    ///
    /// Inputs: `meeting_id` identifies a locally persisted meeting.
    /// Outputs: JSON array containing IDs, ranges, weak candidates, and absolute local paths.
    /// Side effects: Reads SQLite, writes temporary review files, and caches clips for action requests.
    fn get_unassigned_clips(&mut self, meeting_id: &str) -> Result<String, c_int> {
        let clips = self
            .note_repository
            .get_unassigned_clips(&MeetingId::new(meeting_id))
            .map_err(|_| FFI_OPERATION_REJECTED)?;
        let mut json_clips = Vec::with_capacity(clips.len());
        for clip in clips {
            let clip_path = self
                .review_clip_directory
                .join(format!("{}.wav", clip.clip_id()));
            fs::write(&clip_path, clip.audio_bytes()).map_err(|_| FFI_OPERATION_REJECTED)?;
            let absolute_path = fs::canonicalize(&clip_path).map_err(|_| FFI_OPERATION_REJECTED)?;
            let weak_match_json = match clip.weak_match_candidate() {
                Some(candidate) => format!(
                    ",\"weak_match\":{{\"identity_id\":\"{}\",\"confidence_score\":{}}}",
                    candidate.identity_id(),
                    candidate.confidence_score()
                ),
                None => String::new(),
            };
            json_clips.push(format!(
                "{{\"clip_id\":\"{}\",\"start_time_ms\":{},\"end_time_ms\":{},\"local_file_path\":\"{}\"{}}}",
                clip.clip_id(), clip.start_time_ms(), clip.end_time_ms(), json_escape_path(&absolute_path), weak_match_json,
            ));
            self.loaded_review_clips.insert(clip.clip_id(), clip);
        }
        Ok(format!("[{}]", json_clips.join(",")))
    }

    /// Delegates candidate ranking to `AttachmentManager` and serializes only display-safe JSON.
    ///
    /// Inputs: `file_path` is a non-empty path selected by the Flutter platform picker.
    /// Outputs: Ranked JSON suggestions with one opaque attachment ID, or a C status on failure.
    /// Side effects: Reads metadata through its adapter and caches no file contents for confirmation.
    fn process_attachment(&mut self, file_path: &str) -> Result<String, c_int> {
        let metadata_extractor = LocalFileMetadataExtractor;
        let original_timestamp = metadata_extractor
            .extract_timestamp(file_path)
            .map_err(|_| FFI_OPERATION_REJECTED)?;
        let attachment_manager = AttachmentManager::new(metadata_extractor, &self.note_repository);
        let suggestions = attachment_manager
            .suggest_meetings(file_path)
            .map_err(|_| FFI_OPERATION_REJECTED)?;
        let attachment_id = Uuid::new_v4();
        self.pending_attachment_imports.insert(
            attachment_id,
            PendingAttachmentImport {
                file_path: String::from(file_path),
                original_timestamp,
            },
        );
        let serialized_suggestions = suggestions
            .iter()
            .map(|suggestion| {
                format!(
                    "{{\"attachment_id\":\"{}\",\"meeting_id\":\"{}\",\"meeting_start_time\":\"{}\",\"meeting_end_time\":\"{}\",\"confidence_score\":{}}}",
                    attachment_id,
                    json_escape(suggestion.meeting_id().as_str()),
                    suggestion.meeting_start_time().to_rfc3339(),
                    suggestion.meeting_end_time().to_rfc3339(),
                    suggestion.confidence_score(),
                )
            })
            .collect::<Vec<_>>();
        Ok(format!("[{}]", serialized_suggestions.join(",")))
    }

    /// Persists a link chosen from the immediately preceding Rust-generated suggestion response.
    ///
    /// Inputs: `attachment_id` must be pending and `meeting_id` must identify a stored meeting.
    /// Outputs: A documented C status describing validation or persistence success.
    /// Side effects: Saves attachment metadata locally and removes the pending import on success.
    fn confirm_attachment_link(&mut self, attachment_id: &str, meeting_id: &str) -> c_int {
        let parsed_attachment_id = match Uuid::parse_str(attachment_id) {
            Ok(value) => value,
            Err(_) => return FFI_INVALID_ARGUMENT,
        };
        if meeting_id.is_empty() {
            return FFI_INVALID_ARGUMENT;
        }
        let Some(pending_import) = self.pending_attachment_imports.get(&parsed_attachment_id)
        else {
            return FFI_OPERATION_REJECTED;
        };
        let attachment = Attachment::new(
            parsed_attachment_id,
            Some(MeetingId::new(meeting_id)),
            &pending_import.file_path,
            attachment_mime_type(&pending_import.file_path),
            pending_import.original_timestamp,
            self.note_repository_device_id(),
        );
        match self.note_repository.save_attachment(&attachment) {
            Ok(()) => {
                self.pending_attachment_imports
                    .remove(&parsed_attachment_id);
                FFI_SUCCESS
            }
            Err(_) => FFI_OPERATION_REJECTED,
        }
    }

    /// Returns safe source-device metadata for a local attachment record.
    ///
    /// Inputs: None.
    /// Outputs: The device ID the note repository was opened with (see `device_identity`).
    /// Side effects: None.
    fn note_repository_device_id(&self) -> String {
        self.note_repository.device_id().as_str().to_owned()
    }

    /// Delegates a user-approved candidate to the existing voice identity application service.
    ///
    /// Inputs: `clip_id` must have been returned by this runtime and `identity_id` must be a UUID.
    /// Outputs: A C status describing validation or persistence success.
    /// Side effects: Stores one derived voice-signature variant through `VoiceIdentityManager`.
    fn confirm_weak_match(&mut self, clip_id: &str, identity_id: &str) -> c_int {
        let parsed_clip_id = match Uuid::parse_str(clip_id) {
            Ok(value) => value,
            Err(_) => return FFI_INVALID_ARGUMENT,
        };
        let parsed_identity_id = match Uuid::parse_str(identity_id) {
            Ok(value) => value,
            Err(_) => return FFI_INVALID_ARGUMENT,
        };
        let Some(clip) = self.loaded_review_clips.get(&parsed_clip_id) else {
            return FFI_OPERATION_REJECTED;
        };
        if clip.derived_embedding().is_empty() {
            return FFI_OPERATION_REJECTED;
        }
        self.voice_identity_manager
            .confirm_weak_match(parsed_identity_id, clip.derived_embedding())
            .map(|_| FFI_SUCCESS)
            .unwrap_or(FFI_OPERATION_REJECTED)
    }

    /// Creates a named identity with a fresh UUID and delegates its initial signature to the manager.
    ///
    /// Inputs: `clip_id` must be loaded and `display_name` must be non-empty user-facing text
    /// (surrounding whitespace is trimmed).
    /// Outputs: A C status indicating whether the identity and its signature variant were persisted.
    /// Side effects: Saves the identity name in the voice store, then stores one derived
    /// voice-signature variant for it.
    fn assign_new_identity(&mut self, clip_id: &str, display_name: &str) -> c_int {
        let trimmed_display_name = display_name.trim();
        if trimmed_display_name.is_empty() {
            return FFI_INVALID_ARGUMENT;
        }
        let parsed_clip_id = match Uuid::parse_str(clip_id) {
            Ok(value) => value,
            Err(_) => return FFI_INVALID_ARGUMENT,
        };
        let Some(clip) = self.loaded_review_clips.get(&parsed_clip_id) else {
            return FFI_OPERATION_REJECTED;
        };
        if clip.derived_embedding().is_empty() {
            return FFI_OPERATION_REJECTED;
        }
        let identity = SpeakerIdentity::new(Uuid::new_v4(), trimmed_display_name);
        if self
            .voice_identity_manager
            .vector_database()
            .save_speaker_identity(&identity)
            .is_err()
        {
            return FFI_OPERATION_REJECTED;
        }
        self.voice_identity_manager
            .confirm_weak_match(identity.identity_id(), clip.derived_embedding())
            .map(|_| FFI_SUCCESS)
            .unwrap_or(FFI_OPERATION_REJECTED)
    }

    /// Lists a meeting's unresolved unknown-speaker clusters as JSON for the Voice Gallery.
    ///
    /// Inputs: `meeting_id` is a non-blank meeting identifier; an unknown meeting has no clusters.
    /// Outputs: A JSON array of `{"cluster_id","meeting_id","total_duration_ms","chunk_count",
    /// "created_at"}` objects in creation order, each with an absolute `review_clip_path` when
    /// the cluster still has a review clip; or `{"ok":false,"error_code","error"}` where
    /// `error_code` is `invalid_argument` or `list_failed`.
    /// Side effects: Reads SQLite and writes each cluster's review clip into the review-clip
    /// directory. A clip that cannot be written is omitted (logged by cluster ID only). Embeddings
    /// never leave Rust.
    fn get_unknown_clusters(&mut self, meeting_id: &str) -> String {
        if meeting_id.trim().is_empty() {
            return export_error_json("invalid_argument", "meeting_id must not be empty");
        }
        let typed_meeting_id = MeetingId::new(meeting_id);
        let summaries = match list_unknown_clusters(&self.note_repository, &typed_meeting_id) {
            Ok(summaries) => summaries,
            Err(error) => return export_error_json("list_failed", &error.to_string()),
        };
        if summaries.is_empty() {
            // Clips are only read for meetings that have clusters; the repository rejects
            // clip reads for unknown meetings, which simply have no clusters here.
            return String::from("[]");
        }
        let review_clips = match self.note_repository.get_unassigned_clips(&typed_meeting_id) {
            Ok(review_clips) => review_clips,
            Err(error) => return export_error_json("list_failed", &error.to_string()),
        };
        let serialized_clusters = summaries
            .iter()
            .map(|summary| {
                let mut cluster_json = serde_json::json!({
                    "cluster_id": summary.cluster_id().to_string(),
                    "meeting_id": summary.meeting_id().as_str(),
                    "total_duration_ms": summary.total_duration_ms(),
                    "chunk_count": summary.chunk_count(),
                    "created_at": summary.created_at().to_rfc3339(),
                });
                let cluster_clip = review_clips
                    .iter()
                    .find(|clip| clip.cluster_id() == Some(summary.cluster_id()));
                if let Some(clip) = cluster_clip {
                    match self.materialize_review_clip(clip) {
                        Ok(clip_path) => {
                            cluster_json["review_clip_path"] =
                                serde_json::Value::from(clip_path.to_string_lossy().into_owned());
                        }
                        Err(_) => tracing::warn!(
                            cluster_id = %summary.cluster_id(),
                            "unable to materialize cluster review clip"
                        ),
                    }
                }
                cluster_json
            })
            .collect::<Vec<_>>();
        serde_json::Value::Array(serialized_clusters).to_string()
    }

    /// Describes every meeting's unresolved clusters and the saved people as JSON (Issue #63).
    ///
    /// Outputs: `{"ok":true,"unknown_clusters":[{"cluster_id","meeting_id","meeting_title",
    /// "total_duration_ms","chunk_count","created_at","has_review_clip"}],"saved_people":
    /// [{"identity_id","display_name","voice_variant_count"}]}`, or `{"ok":false,"error_code":
    /// "list_failed","error"}`. `meeting_title` is null for an untitled meeting.
    /// Side effects: Reads SQLite. Writes no audio, and embeddings never leave Rust.
    fn get_voice_library(&self) -> String {
        let library = match load_voice_library(
            &self.note_repository,
            self.voice_identity_manager.vector_database(),
        ) {
            Ok(library) => library,
            Err(error) => return export_error_json("list_failed", &error.to_string()),
        };
        serde_json::json!({
            "ok": true,
            "unknown_clusters": library.unknown_clusters().iter().map(|cluster| serde_json::json!({
                "cluster_id": cluster.summary().cluster_id().to_string(),
                "meeting_id": cluster.summary().meeting_id().as_str(),
                "meeting_title": cluster.meeting_title(),
                "total_duration_ms": cluster.summary().total_duration_ms(),
                "chunk_count": cluster.summary().chunk_count(),
                "created_at": cluster.summary().created_at().to_rfc3339(),
                "has_review_clip": cluster.has_review_clip(),
            })).collect::<Vec<_>>(),
            "saved_people": library.saved_people().iter().map(|person| serde_json::json!({
                "identity_id": person.identity_id().to_string(),
                "display_name": person.display_name(),
                "voice_variant_count": person.voice_variant_count(),
            })).collect::<Vec<_>>(),
        })
        .to_string()
    }

    /// Writes one cluster's review clip to a playable file, only when the owner asks to hear it.
    ///
    /// Inputs: `cluster_id` is a UUID string.
    /// Outputs: `{"ok":true,"review_clip_path"}`, or `{"ok":false,"error_code","error"}` where
    /// `error_code` is `invalid_argument`, `clip_not_found`, or `clip_unavailable`.
    /// Side effects: Decrypts the clip and writes it to the review-clip directory; assigning the
    /// cluster deletes the file.
    fn prepare_cluster_review_clip(&self, cluster_id: &str) -> String {
        let Ok(parsed_cluster_id) = Uuid::parse_str(cluster_id) else {
            return export_error_json("invalid_argument", "cluster_id must be a UUID");
        };
        match find_cluster_review_clip(&self.note_repository, parsed_cluster_id) {
            Ok(Some(clip)) => match self.materialize_review_clip(&clip) {
                Ok(clip_path) => serde_json::json!({
                    "ok": true,
                    "review_clip_path": clip_path.to_string_lossy(),
                })
                .to_string(),
                Err(_) => export_error_json("clip_unavailable", "unable to write the review clip"),
            },
            Ok(None) => export_error_json("clip_not_found", "this voice has no review clip"),
            Err(error) => export_error_json("clip_unavailable", &error.to_string()),
        }
    }

    /// Writes one review clip as `<clip_id>.wav` in the review-clip directory.
    ///
    /// Inputs: A clip loaded from SQLite.
    /// Outputs: The clip's absolute path, or the I/O error.
    /// Side effects: Creates the review-clip directory if needed and writes temporary raw audio.
    fn materialize_review_clip(&self, clip: &UnassignedVoiceClip) -> std::io::Result<PathBuf> {
        fs::create_dir_all(&self.review_clip_directory)?;
        let clip_path = self
            .review_clip_directory
            .join(format!("{}.wav", clip.clip_id()));
        fs::write(&clip_path, clip.audio_bytes())?;
        fs::canonicalize(&clip_path)
    }

    /// Assigns an unknown-speaker cluster to a named identity and describes the outcome as JSON.
    ///
    /// Inputs: `cluster_id` is a UUID string; `identity_name` is the person's non-blank name.
    /// Outputs: `{"ok":true,"cluster_id","identity_id","identity_name","identity_created",
    /// "variants_stored","segments_updated"}`, or `{"ok":false,"error_code","error"}` where
    /// `error_code` is `invalid_argument`, `cluster_not_found`, `cluster_already_resolved`, or
    /// `assignment_failed`.
    /// Side effects: Through `cluster_review`, stores voice variants, relinks transcript
    /// segments, resolves the cluster, and deletes its review clip rows; then deletes the
    /// cluster's materialized review files from the review-clip directory.
    fn assign_cluster(&mut self, cluster_id: &str, identity_name: &str) -> String {
        let Ok(parsed_cluster_id) = Uuid::parse_str(cluster_id) else {
            return export_error_json("invalid_argument", "cluster_id must be a UUID");
        };
        let cluster_clip_ids = self.review_clip_ids_for_cluster(parsed_cluster_id);
        match assign_cluster_to_named_identity(
            &self.note_repository,
            self.voice_identity_manager.vector_database(),
            parsed_cluster_id,
            identity_name,
        ) {
            Ok(assignment) => {
                for clip_id in cluster_clip_ids {
                    self.loaded_review_clips.remove(&clip_id);
                    let clip_path = self.review_clip_directory.join(format!("{clip_id}.wav"));
                    if let Err(error) = fs::remove_file(&clip_path)
                        && error.kind() != std::io::ErrorKind::NotFound
                    {
                        tracing::warn!(clip_id = %clip_id, "unable to delete resolved review clip file");
                    }
                }
                serde_json::json!({
                    "ok": true,
                    "cluster_id": assignment.cluster_id().to_string(),
                    "identity_id": assignment.identity_id().to_string(),
                    "identity_name": assignment.identity_name(),
                    "identity_created": assignment.identity_created(),
                    "variants_stored": assignment.variants_stored(),
                    "segments_updated": assignment.segments_updated(),
                })
                .to_string()
            }
            Err(error) => {
                let error_code = match &error {
                    ClusterReviewError::BlankIdentityName => "invalid_argument",
                    ClusterReviewError::Clustering(ClusteringError::ClusterNotFound { .. }) => {
                        "cluster_not_found"
                    }
                    ClusterReviewError::Clustering(ClusteringError::ClusterAlreadyResolved {
                        ..
                    }) => "cluster_already_resolved",
                    ClusterReviewError::Clustering(_) => "assignment_failed",
                };
                export_error_json(error_code, &error.to_string())
            }
        }
    }

    /// Returns the IDs of the review clips that belong to one cluster.
    ///
    /// Inputs: `cluster_id` identifies a cluster, which need not exist.
    /// Outputs: Matching clip IDs; empty when the cluster or its meeting cannot be read.
    /// Side effects: Reads SQLite.
    fn review_clip_ids_for_cluster(&self, cluster_id: Uuid) -> Vec<Uuid> {
        let Ok(Some(cluster)) = self
            .note_repository
            .load_unknown_speaker_cluster(cluster_id)
        else {
            return Vec::new();
        };
        self.note_repository
            .get_unassigned_clips(cluster.meeting_id())
            .map(|clips| {
                clips
                    .iter()
                    .filter(|clip| clip.cluster_id() == Some(cluster_id))
                    .map(UnassignedVoiceClip::clip_id)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Serializes telemetry-observed device cards without exposing hardware identifiers or task data.
    fn get_devices(&self) -> Result<String, c_int> {
        let metrics = self
            .note_repository
            .get_all_metrics()
            .map_err(|_| FFI_OPERATION_REJECTED)?;
        let mut devices = std::collections::BTreeMap::<String, (Vec<String>, u64)>::new();
        for metric in metrics {
            let device = devices
                .entry(metric.device_id().as_str().to_owned())
                .or_insert((Vec::new(), 0));
            if !device.0.contains(&metric.model_name().to_owned()) {
                device.0.push(metric.model_name().to_owned());
            }
            device.1 = device.1.max(metric.timestamp_epoch_ms());
        }
        Ok(format!("[{}]", devices.into_iter().map(|(device_id, (models, last_seen))| format!("{{\"device_id\":\"{}\",\"supports_vad\":{},\"supported_models\":[{}],\"last_seen\":{}}}", json_escape(&device_id), cfg!(feature = "vad"), models.iter().map(|model| format!("\"{}\"", json_escape(model))).collect::<Vec<_>>().join(","), last_seen)).collect::<Vec<_>>().join(",")))
    }

    /// Serializes aggregate privacy-safe telemetry values for the Flutter summary cards.
    fn get_telemetry_summary(&self) -> Result<String, c_int> {
        let metrics = self
            .note_repository
            .get_all_metrics()
            .map_err(|_| FFI_OPERATION_REJECTED)?;
        let total_tasks = metrics.len() as u64;
        let transcription_ms: u64 = metrics
            .iter()
            .filter(|metric| {
                matches!(
                    metric.task_type(),
                    TaskType::Transcription | TaskType::CloudTranscription
                )
            })
            .map(TaskMetrics::duration_ms)
            .sum();
        let latency_ms: u64 = metrics.iter().map(TaskMetrics::latency_ms).sum();
        let power_mwh: f32 = metrics.iter().map(TaskMetrics::estimated_power_mwh).sum();
        Ok(format!(
            "{{\"total_transcription_time_ms\":{transcription_ms},\"average_latency_ms\":{},\"total_tasks\":{total_tasks},\"power_estimate_mwh\":{power_mwh}}}",
            latency_ms.checked_div(total_tasks).unwrap_or_default()
        ))
    }

    /// Serializes one device's most recent 24-hour metric series without task payloads.
    fn get_device_telemetry(&self, device_id: &str) -> Result<String, c_int> {
        let earliest_timestamp = u64::try_from(chrono::Utc::now().timestamp_millis())
            .unwrap_or_default()
            .saturating_sub(86_400_000);
        let metrics = self
            .note_repository
            .get_metrics_for_device(device_id)
            .map_err(|_| FFI_OPERATION_REJECTED)?;
        let serialized_metrics = metrics.into_iter().filter(|metric| metric.timestamp_epoch_ms() >= earliest_timestamp).map(|metric| format!("{{\"task_type\":\"{}\",\"duration_ms\":{},\"latency_ms\":{},\"power_mwh\":{},\"recorded_at\":{}}}", metric.task_type().as_str(), metric.duration_ms(), metric.latency_ms(), metric.estimated_power_mwh(), metric.timestamp_epoch_ms())).collect::<Vec<_>>();
        Ok(format!(
            "{{\"device_id\":\"{}\",\"metrics\":[{}]}}",
            json_escape(device_id),
            serialized_metrics.join(",")
        ))
    }

    /// Returns the recording controller, composing the meeting pipeline on first use.
    ///
    /// Inputs: None.
    /// Outputs: The controller, or `FFI_PIPELINE_UNAVAILABLE` when the builder fails.
    /// Side effects: The first call spawns the pipeline worker and may load a model (slow; call
    /// recording commands from a background isolate). A failure is logged and retried next time.
    fn recording_controller(&mut self) -> Result<&mut RecordingController, c_int> {
        if self.recording_controller.is_none() {
            let recording_controller = (self.recording_controller_builder)().map_err(|error| {
                tracing::error!(error = %format!("{error:#}"), "Meeting pipeline is unavailable");
                FFI_PIPELINE_UNAVAILABLE
            })?;
            self.recording_controller = Some(recording_controller);
        }
        self.recording_controller
            .as_mut()
            .ok_or(FFI_RUNTIME_UNAVAILABLE)
    }

    /// Starts a recording with a generated meeting ID, or resumes a paused one.
    ///
    /// Inputs: None.
    /// Outputs: A C status.
    /// Side effects: See `start_recording_for`.
    fn start_recording(&mut self) -> c_int {
        self.start_recording_for(None)
    }

    /// Starts a recording into `meeting_id` (created if missing), or resumes a paused one.
    ///
    /// Inputs: The meeting to record into; `None` generates one. Ignored when resuming.
    /// Outputs: See `start_recording_titled`.
    /// Side effects: See `start_recording_titled`; an existing meeting keeps its title.
    fn start_recording_for(&mut self, meeting_id: Option<MeetingId>) -> c_int {
        self.start_recording_titled(meeting_id, None)
    }

    /// Starts an optionally titled recording, or resumes a paused one.
    ///
    /// Inputs: The meeting to record into (`None` generates one) and its title (`None` keeps the
    /// current title); both are ignored when resuming.
    /// Outputs: `FFI_SUCCESS`, `FFI_OPERATION_REJECTED` (e.g. already recording, or the title
    /// could not be stored), or `FFI_PIPELINE_UNAVAILABLE`.
    /// Side effects: Composes the pipeline on first use, opens a meeting session, and stores the
    /// title in SQLite. Records in the language chosen for it, else the default language.
    fn start_recording_titled(
        &mut self,
        meeting_id: Option<MeetingId>,
        title: Option<MeetingTitle>,
    ) -> c_int {
        self.rebuild_pipeline_if_settings_changed();
        let opened_microphone = match self.open_microphone() {
            Ok(opened_microphone) => opened_microphone,
            Err(status) => return status,
        };
        let language = self
            .recording_language
            .unwrap_or_else(|| default_language(&app_setting));
        let status = match self.recording_controller() {
            Ok(recording_controller) => {
                recording_controller.set_language(Some(language));
                recording_status(recording_controller.start_titled(meeting_id, title))
            }
            Err(status) => status,
        };
        if status == FFI_SUCCESS {
            // Fixed for this recording: a later change of the default waits for the next one.
            self.recording_language = Some(language);
        }
        if status != FFI_SUCCESS && opened_microphone {
            if let Some(microphone) = self.live_microphone.take() {
                // Nothing is recording, so the captured audio is discarded.
                drop(microphone.close());
            }
        } else if self.live_microphone.is_some() && !self.microphone_ticker_running {
            self.microphone_ticker_running = true;
            (self.spawn_microphone_ticker)();
        }
        status
    }

    /// Drops an idle pipeline after a Settings change, so this start uses the new model.
    ///
    /// Inputs: None. Outputs: None.
    /// Side effects: Never touches a running or paused recording, whose audio must not be lost;
    /// the change then waits for the next start. Frees the old model's memory.
    fn rebuild_pipeline_if_settings_changed(&mut self) {
        let in_use = self
            .recording_controller
            .as_ref()
            .is_some_and(|controller| {
                !matches!(controller.state(), AudioState::Idle | AudioState::Stopped)
            });
        if in_use {
            return;
        }
        crate::cloud_fallback::reset_cloud_fallback();
        if crate::ffi_transcription::take_pipeline_stale() {
            self.recording_controller = None;
        }
    }

    /// Chooses the language for the next recording, or for the rest of the current one.
    ///
    /// Inputs: The language. Outputs: `FFI_SUCCESS`.
    /// Side effects: Applies to chunks cut from now on. It does not load the pipeline: a start
    /// applies the choice when no recording controller exists yet.
    fn set_recording_language(&mut self, language: TranscriptionLanguage) -> c_int {
        self.recording_language = Some(language);
        if let Some(recording_controller) = self.recording_controller.as_mut() {
            recording_controller.set_language(Some(language));
        }
        FFI_SUCCESS
    }

    /// Opens the microphone for a start or resume, unless the host pushes PCM or it is open.
    ///
    /// Outputs: Whether this call opened it, or `FFI_MICROPHONE_UNAVAILABLE`.
    /// Side effects: Turns the microphone on.
    fn open_microphone(&mut self) -> Result<bool, c_int> {
        let Some(microphone_source) = &self.microphone_source else {
            return Ok(false);
        };
        if self.live_microphone.is_some() {
            return Ok(false);
        }
        match LiveMicrophone::open(microphone_source.as_ref()) {
            Ok(microphone) => {
                self.live_microphone = Some(microphone);
                Ok(true)
            }
            Err(error) => {
                tracing::warn!(error = %error, "Unable to open the microphone");
                Err(FFI_MICROPHONE_UNAVAILABLE)
            }
        }
    }

    /// Moves the microphone's queued audio into the recording; the ticker calls it.
    ///
    /// Outputs: Whether a microphone is still open (the ticker stops when it is not).
    /// Side effects: Pushes PCM into the recorder, which may queue chunks for transcription.
    fn drain_microphone(&mut self) -> bool {
        let Some(microphone) = &self.live_microphone else {
            self.microphone_ticker_running = false;
            return false;
        };
        let blocks = microphone.drain();
        self.push_microphone_blocks(blocks);
        true
    }

    /// Turns the microphone off and moves the audio still queued into the recording.
    ///
    /// Side effects: Closes the stream (waiting for its thread) and pushes the tail.
    fn close_microphone(&mut self) {
        if let Some(microphone) = self.live_microphone.take() {
            let tail = microphone.close();
            self.push_microphone_blocks(tail);
        }
    }

    /// Pushes microphone blocks into the active recording.
    ///
    /// Side effects: As `RecordingController::push_pcm`; failures are logged, never the audio.
    fn push_microphone_blocks(&mut self, blocks: Vec<Vec<i16>>) {
        let Some(recording_controller) = self.recording_controller.as_mut() else {
            return;
        };
        for block in blocks {
            if let Err(error) = recording_controller.push_pcm(block) {
                tracing::warn!(error = %error, "Microphone audio was not accepted");
            }
        }
    }

    /// Lists every local meeting with its display title as JSON.
    ///
    /// Inputs: None.
    /// Outputs: `[{"meeting_id","title","created_at","duration_ms"}]` in repository order, where
    /// `title` is the stored title or the dated fallback, `created_at` is RFC 3339 UTC, and
    /// `duration_ms` is the latest segment end; or
    /// `{"ok":false,"error_code":"list_failed","error"}`.
    /// Side effects: Reads SQLite meeting metadata and segment offsets.
    fn get_meetings(&self) -> String {
        let meeting_ids = match self.note_repository.list_meeting_ids() {
            Ok(meeting_ids) => meeting_ids,
            Err(error) => return export_error_json("list_failed", &error.to_string()),
        };
        let mut serialized_meetings = Vec::with_capacity(meeting_ids.len());
        for meeting_id in meeting_ids {
            let meeting_metadata = match self.note_repository.load_meeting_metadata(&meeting_id) {
                Ok(Some(meeting_metadata)) => meeting_metadata,
                Ok(None) => continue,
                Err(error) => return export_error_json("list_failed", &error.to_string()),
            };
            match self.note_repository.load_transcript_segments(&meeting_id) {
                Ok(segments) => {
                    serialized_meetings.push(meeting_summary_json(&meeting_metadata, &segments));
                }
                Err(error) => return export_error_json("list_failed", &error.to_string()),
            }
        }
        serde_json::Value::Array(serialized_meetings).to_string()
    }

    /// Renames one meeting through the shared meeting-rename use case.
    ///
    /// Inputs: `meeting_id` (non-blank) and `new_title` (unvalidated user text; trimmed, 1–200
    /// characters, no control characters).
    /// Outputs: `FFI_SUCCESS`, `FFI_INVALID_ARGUMENT` for a blank ID or invalid title,
    /// `FFI_MEETING_NOT_FOUND` for an unknown meeting, or `FFI_OPERATION_REJECTED` for storage
    /// failures.
    /// Side effects: Replaces the title in SQLite; invalid input writes nothing. The title is
    /// never logged.
    fn rename_meeting(&mut self, meeting_id: &str, new_title: &str) -> c_int {
        if meeting_id.trim().is_empty() {
            return FFI_INVALID_ARGUMENT;
        }
        match rename_meeting(
            &self.note_repository,
            &MeetingId::new(meeting_id),
            new_title,
        ) {
            Ok(_) => FFI_SUCCESS,
            Err(MeetingRenameError::InvalidTitle(_)) => FFI_INVALID_ARGUMENT,
            Err(MeetingRenameError::MeetingNotFound { .. }) => FFI_MEETING_NOT_FOUND,
            Err(MeetingRenameError::Storage(_)) => FFI_OPERATION_REJECTED,
        }
    }

    /// Attaches files or photos to a meeting (Issue #54).
    ///
    /// Inputs: A non-blank `meeting_id` and a non-empty JSON array of absolute file paths.
    /// Outputs: `{"ok":true,"attached":[<attachment>],"failed":[{"path","error"}]}`, where each
    /// `<attachment>` is as in `get_meeting`; or `{"ok":false,"error_code","error"}` with
    /// `invalid_argument` or `meeting_not_found`. While the meeting is being recorded, attachments
    /// carry its current meeting time; otherwise they go at the end of the meeting.
    /// Side effects: Copies each accepted file privately into the attachment folder and writes
    /// its attachment row; leaves the sources (the caller deletes camera temporaries). Never
    /// logs paths or contents.
    fn attach_files(&mut self, meeting_id: &str, paths_json: &str) -> String {
        if meeting_id.trim().is_empty() {
            return export_error_json("invalid_argument", "meeting_id must not be empty");
        }
        let source_paths = match serde_json::from_str::<Vec<String>>(paths_json) {
            Ok(source_paths) if !source_paths.is_empty() => source_paths,
            _ => {
                return export_error_json(
                    "invalid_argument",
                    "paths must be a non-empty JSON array of strings",
                );
            }
        };
        let typed_meeting_id = MeetingId::new(meeting_id);
        match self
            .note_repository
            .load_meeting_metadata(&typed_meeting_id)
        {
            Ok(Some(_)) => {}
            Ok(None) => {
                return export_error_json(
                    "meeting_not_found",
                    &format!("meeting {meeting_id} does not exist"),
                );
            }
            Err(error) => return export_error_json("load_failed", &error.to_string()),
        }
        let meeting_time_ms = self
            .recording_controller
            .as_ref()
            .and_then(|controller| controller.current_meeting_time_ms(&typed_meeting_id));
        let file_store = AttachmentFileStore::new(
            self.attachment_directory.clone(),
            self.note_repository.device_id().clone(),
        );
        let mut attached = Vec::new();
        let mut failed = Vec::new();
        for source_path in source_paths {
            match file_store.import_file(
                &self.note_repository,
                &typed_meeting_id,
                Path::new(&source_path),
                meeting_time_ms,
            ) {
                Ok(attachment) => attached.push(attachment_json(&attachment)),
                Err(error) => failed.push(serde_json::json!({
                    "path": source_path,
                    "error": error.to_string(),
                })),
            }
        }
        serde_json::json!({ "ok": true, "attached": attached, "failed": failed }).to_string()
    }

    /// Describes one meeting with its display title, duration, segments, and flat transcript.
    ///
    /// Inputs: A non-blank `meeting_id`.
    /// Outputs: `{"meeting_id","title","created_at","duration_ms","transcript","segments",
    /// "attachments"}`, where `transcript` is null until a flat transcript is stored, `segments` is
    /// `[{"start_time_ms","end_time_ms","speaker","text"}]` in timeline order (`speaker` is the
    /// voice-store name, `Unknown speaker`, or `Unattributed`, as in exports), and `attachments`
    /// is `[{"attachment_id","file_name","file_path","mime_type","meeting_time_ms"}]` in the
    /// order they were added (`meeting_time_ms` is null for the end of the meeting); or
    /// `{"ok":false,"error_code","error"}` where `error_code` is `invalid_argument`,
    /// `meeting_not_found`, or `load_failed`.
    /// Side effects: Reads SQLite, including speaker names from the voice store; never returns
    /// embeddings.
    fn get_meeting(&self, meeting_id: &str) -> String {
        if meeting_id.trim().is_empty() {
            return export_error_json("invalid_argument", "meeting_id must not be empty");
        }
        let typed_meeting_id = MeetingId::new(meeting_id);
        let meeting_metadata = match self
            .note_repository
            .load_meeting_metadata(&typed_meeting_id)
        {
            Ok(Some(meeting_metadata)) => meeting_metadata,
            Ok(None) => {
                return export_error_json(
                    "meeting_not_found",
                    &format!("meeting {meeting_id} does not exist"),
                );
            }
            Err(error) => return export_error_json("load_failed", &error.to_string()),
        };
        let transcript = match self.note_repository.load_transcript(&typed_meeting_id) {
            Ok(transcript) => transcript,
            Err(error) => return export_error_json("load_failed", &error.to_string()),
        };
        let segments = match self
            .note_repository
            .load_transcript_segments(&typed_meeting_id)
        {
            Ok(segments) => segments,
            Err(error) => return export_error_json("load_failed", &error.to_string()),
        };
        let speaker_names: HashMap<Uuid, String> = match self
            .voice_identity_manager
            .vector_database()
            .list_speaker_identities()
        {
            Ok(identities) => identities
                .into_iter()
                .map(|identity| (identity.identity_id(), identity.display_name().to_owned()))
                .collect(),
            Err(error) => return export_error_json("load_failed", &error.to_string()),
        };
        let attachments = match self
            .note_repository
            .get_attachments_for_meeting(&typed_meeting_id)
        {
            Ok(attachments) => attachments,
            Err(error) => return export_error_json("load_failed", &error.to_string()),
        };
        let mut meeting_json = meeting_summary_json(&meeting_metadata, &segments);
        meeting_json["attachments"] = attachments.iter().map(attachment_json).collect();
        meeting_json["transcript"] = transcript
            .map(|transcript| serde_json::Value::from(transcript.content()))
            .unwrap_or(serde_json::Value::Null);
        meeting_json["segments"] = segments
            .iter()
            .map(|segment| {
                let speaker = match segment.speaker_identity_id() {
                    Some(identity_id) => speaker_names
                        .get(&identity_id)
                        .map_or(UNKNOWN_SPEAKER_LABEL, String::as_str),
                    None => UNATTRIBUTED_SPEAKER_LABEL,
                };
                serde_json::json!({
                    "start_time_ms": segment.start_time_ms(),
                    "end_time_ms": segment.end_time_ms(),
                    "speaker": speaker,
                    "text": segment.text(),
                })
            })
            .collect();
        meeting_json.to_string()
    }

    /// Feeds captured 16 kHz mono PCM into the active recording.
    ///
    /// Inputs: Raw samples (never logged).
    /// Outputs: `FFI_SUCCESS` (also while paused, when audio is discarded) or
    /// `FFI_OPERATION_REJECTED` when no recording is active.
    /// Side effects: Buffers audio and queues finished chunks for transcription.
    fn push_audio_pcm(&mut self, samples: Vec<i16>) -> c_int {
        match self.recording_controller.as_mut() {
            Some(recording_controller) => recording_status(recording_controller.push_pcm(samples)),
            None => FFI_OPERATION_REJECTED,
        }
    }

    /// Pauses the active recording.
    ///
    /// Inputs: None.
    /// Outputs: A C status indicating whether an active recording accepted the pause.
    /// Side effects: Later PCM is discarded until a start resumes the recording.
    fn pause_recording(&mut self) -> c_int {
        self.close_microphone();
        match self.recording_controller.as_mut() {
            Some(recording_controller) => recording_status(recording_controller.pause()),
            None => FFI_OPERATION_REJECTED,
        }
    }

    /// Stops the active recording once all captured audio is transcribed.
    ///
    /// Inputs: None.
    /// Outputs: A C status indicating whether a recording was stopped.
    /// Side effects: Blocks (holding the runtime lock) until queued chunks finish, persists
    /// their segments, and deletes unprocessed raw audio. A later start is a new recording.
    fn stop_recording(&mut self) -> c_int {
        self.close_microphone();
        self.recording_language = None;
        match self.recording_controller.as_mut() {
            Some(recording_controller) => recording_status(recording_controller.stop()),
            None => FFI_OPERATION_REJECTED,
        }
    }

    /// Converts a platform-reported OS suspension into a safe recoverable recorder state.
    ///
    /// Inputs: None.
    /// Outputs: Success after a recording enters Interrupted; rejection for inactive states.
    /// Side effects: Queues buffered audio for transcription; a start resumes the recording.
    fn handle_audio_interruption(&mut self) -> c_int {
        self.close_microphone();
        match self.recording_controller.as_mut() {
            Some(recording_controller) => {
                recording_status(recording_controller.handle_interruption())
            }
            None => FFI_OPERATION_REJECTED,
        }
    }

    /// Reads the current recorder lifecycle state without exposing Rust state values over FFI.
    ///
    /// Inputs: None.
    /// Outputs: One documented `FFI_AUDIO_STATE_*` integer (`Idle` before the first recording).
    /// Side effects: None; the runtime mutex serializes this read with lifecycle mutations.
    fn current_recording_state(&self) -> c_int {
        audio_state_code(
            self.recording_controller
                .as_ref()
                .map_or(AudioState::Idle, RecordingController::state),
        )
    }

    /// Returns the meeting being recorded, if any.
    ///
    /// Inputs: None.
    /// Outputs: The meeting ID while a recording is active (including paused).
    /// Side effects: None.
    fn active_meeting_id(&self) -> Option<String> {
        self.recording_controller
            .as_ref()
            .and_then(RecordingController::active_meeting_id)
            .map(|meeting_id| meeting_id.as_str().to_owned())
    }
}

/// Escapes UTF-8 text for the narrow JSON response without accepting arbitrary JSON values.
///
/// Inputs: `value` is text emitted in a JSON string field.
/// Outputs: JSON string content (without the surrounding quotes) with every escape JSON requires:
/// quotes, backslashes, and control characters.
/// Side effects: None.
fn json_escape(value: &str) -> String {
    // serde_json escapes quotes, backslashes, and every control character; replacing only quotes
    // and backslashes produced invalid JSON for paths or IDs containing a newline or tab.
    let quoted_value = serde_json::Value::String(value.to_owned()).to_string();
    quoted_value[1..quoted_value.len() - 1].to_owned()
}

/// Escapes a platform path for the narrow JSON response without accepting arbitrary JSON values.
///
/// Inputs: `path` is an absolute local file-system path.
/// Outputs: JSON string content with every escape JSON requires.
/// Side effects: None.
fn json_escape_path(path: &Path) -> String {
    json_escape(&path.to_string_lossy())
}

/// Describes one attachment for the app.
///
/// Inputs: A stored attachment.
/// Outputs: `{"attachment_id","file_name","file_path","mime_type","meeting_time_ms"}`, where
/// `file_name` is the stored name without the `<attachment UUID>-` prefix imports add.
/// Side effects: None; the file is not read.
fn attachment_json(attachment: &Attachment) -> serde_json::Value {
    let stored_name = Path::new(attachment.file_path())
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let attachment_prefix = format!("{}-", attachment.attachment_id());
    let file_name = stored_name
        .strip_prefix(&attachment_prefix)
        .map_or(stored_name.clone(), str::to_owned);
    serde_json::json!({
        "attachment_id": attachment.attachment_id().to_string(),
        "file_name": file_name,
        "file_path": attachment.file_path(),
        "mime_type": attachment.mime_type(),
        "meeting_time_ms": attachment.meeting_time_ms(),
    })
}

/// Classifies a selected attachment by extension for local metadata persistence.
///
/// Inputs: `file_path` identifies the user-selected local attachment.
/// Outputs: A conservative supported media type, defaulting to binary data.
/// Side effects: None.
fn attachment_mime_type(file_path: &str) -> &'static str {
    match Path::new(file_path)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("pdf") => "application/pdf",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}

/// Starts the P2P task from the runtime's settings, or returns `None`.
///
/// Inputs: The settings lookup, the database the task opens its own connections to, this
/// device's ID, and the verified embedding cipher (it seals the node identity).
/// Outputs: The running task, or `None` when P2P is disabled, misconfigured, or fails to start.
/// Side effects: Spawns the `note-taker-p2p` thread and binds the listen address. A failure is
/// logged without the configured values.
fn start_p2p(
    setting: &dyn Fn(&str) -> Option<String>,
    database_path: &Path,
    device_id: &crate::domain::models::DeviceId,
    embedding_cipher: &Arc<dyn crate::ports::crypto::EmbeddingCipher>,
) -> Option<Arc<P2pHandle>> {
    let mut config = match P2pConfig::from_lookup(setting) {
        Ok(Some(config)) => config,
        Ok(None) => return None,
        Err(error) => {
            tracing::warn!(error = %format!("{error:#}"), "P2P sync is misconfigured and stays off");
            return None;
        }
    };
    // The manifest advertises what the pipeline can run; an invalid pipeline setting is
    // reported by the pipeline itself, so it advertises no models here.
    let mut hosted_pipeline = None;
    if let Ok(pipeline_config) = PipelineConfig::from_lookup(setting) {
        let pipeline_config =
            with_saved_transcription(pipeline_config, &TranscriptionPaths::from_lookup(setting));
        config.capabilities.transcription_models =
            pipeline_config.advertised_transcription_models();
        config.capabilities.local_whisper_model = pipeline_config
            .local_model_path()
            .map(crate::pipeline_composition::local_model_label);
        if !config.capabilities.transcription_models.is_empty() {
            hosted_pipeline = Some(pipeline_config);
        }
    }
    match spawn_p2p_task(
        database_path,
        device_id.clone(),
        Arc::clone(embedding_cipher),
        config,
        hosted_pipeline,
    ) {
        Ok(handle) => Some(Arc::new(handle)),
        Err(error) => {
            tracing::warn!(error = %format!("{error:#}"), "P2P sync could not start");
            None
        }
    }
}

/// Returns the runtime's P2P task without holding the runtime lock afterwards.
///
/// Outputs: `Ok(None)` when P2P is off, or `Err` when the runtime cannot start.
/// Side effects: Opens the runtime on first use.
fn runtime_p2p() -> Result<Option<Arc<P2pHandle>>, c_int> {
    let runtime_mutex = FFI_RUNTIME.get_or_init(|| Mutex::new(None));
    let mut runtime_guard = runtime_mutex.lock().map_err(|_| FFI_RUNTIME_UNAVAILABLE)?;
    if runtime_guard.is_none() {
        *runtime_guard = Some(FfiRuntime::new()?);
    }
    Ok(runtime_guard
        .as_ref()
        .and_then(|runtime| runtime.p2p.clone()))
}

/// The database key the host app handed over (Android: unwrapped from the Android Keystore).
static HOST_ENCRYPTION_KEY: Mutex<Option<zeroize::Zeroizing<Vec<u8>>>> = Mutex::new(None);

/// Returns a copy of the host-provided key, if any.
pub(crate) fn provided_encryption_key() -> Option<Vec<u8>> {
    HOST_ENCRYPTION_KEY
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|key| key.to_vec()))
}

/// Layers the owner's saved transcription choices over the environment's configuration.
///
/// Inputs: The configuration and the paths.
/// Outputs: The configuration to run. An unreadable key store counts as no saved key.
/// Side effects: Reads the settings file and the key store, which may prompt to unlock.
fn with_saved_transcription(config: PipelineConfig, paths: &TranscriptionPaths) -> PipelineConfig {
    let saved = load_settings(&paths.settings_file);
    let saved_key = match OpenAiKeyStore::open(paths, provided_encryption_key()).load() {
        Ok(key) => key.map(|key| key.to_string()),
        Err(error) => {
            tracing::warn!(error = %error, "unable to read the saved OpenAI key");
            None
        }
    };
    apply_saved_settings(config, &saved, paths, saved_key)
}

/// Picks the keychain the runtime unlocks through.
///
/// Inputs: The host-provided key, if any, and whether this is an Android build.
/// Outputs: The host-provided keychain when a key was given or on Android (where it reports the
/// missing key as unavailable, status 6), otherwise the platform keychain.
/// Side effects: None until the keychain is used.
fn runtime_keychain(
    host_key: Option<Vec<u8>>,
    is_android: bool,
) -> Arc<dyn crate::ports::crypto::KeychainStore> {
    if host_key.is_some() || is_android {
        Arc::new(
            crate::adapters::keychain::host_provided_keychain::HostProvidedKeychain::new(
                host_key,
                "Android Keystore",
            ),
        )
    } else {
        Arc::new(crate::adapters::keychain::platform_keychain::PlatformKeychain::new())
    }
}

/// Hands the runtime the database key that the host app manages (Android Keystore).
///
/// Inputs: `key` points to `key_length` bytes, which must be 32. The caller wipes its own copy
/// afterwards.
/// Outputs: `0` when stored, `1` for a null pointer or another length.
/// Side effects: Replaces any earlier key in process memory (wiped on replacement). The next
/// runtime start unlocks with it; a runtime that is already open is unaffected.
///
/// # Safety
///
/// `key` must be null or valid for reads of `key_length` bytes for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_provide_encryption_key(
    key: *const u8,
    key_length: usize,
) -> c_int {
    if key.is_null() {
        return FFI_INVALID_ARGUMENT;
    }
    // SAFETY: non-null and valid for `key_length` reads per the caller contract.
    store_host_encryption_key(unsafe { std::slice::from_raw_parts(key, key_length) })
}

/// Length of the host-provided database key, in bytes (AES-256).
pub(crate) const HOST_ENCRYPTION_KEY_LENGTH: usize = 32;

/// Keeps the host's database key for the next runtime start; shared by the C entry point and
/// the Android JNI method (`ffi_android.rs`).
///
/// Inputs: The key bytes; the caller wipes its own copy afterwards.
/// Outputs: `0` when stored, `1` when the key is not 32 bytes, `3` if the key slot is poisoned.
/// Side effects: Copies the key into a zeroizing buffer, wiping any key it replaces.
pub(crate) fn store_host_encryption_key(key: &[u8]) -> c_int {
    if key.len() != HOST_ENCRYPTION_KEY_LENGTH {
        return FFI_INVALID_ARGUMENT;
    }
    match HOST_ENCRYPTION_KEY.lock() {
        Ok(mut guard) => {
            *guard = Some(zeroize::Zeroizing::new(key.to_vec()));
            FFI_SUCCESS
        }
        Err(_) => FFI_RUNTIME_UNAVAILABLE,
    }
}

/// Maps a failure to open the encrypted stores to its FFI status.
///
/// Inputs: The error from `open_encrypted_stores`, with any context layers.
/// Outputs: `FFI_KEYCHAIN_UNAVAILABLE` when a keychain error caused it, otherwise
/// `FFI_OPERATION_REJECTED`.
/// Side effects: None.
fn store_open_status(error: &anyhow::Error) -> c_int {
    let keychain_failed = error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<crate::domain::errors::CryptoError>(),
            Some(
                crate::domain::errors::CryptoError::Keychain { .. }
                    | crate::domain::errors::CryptoError::KeychainUnavailable { .. }
            )
        )
    });
    if keychain_failed {
        FFI_KEYCHAIN_UNAVAILABLE
    } else {
        FFI_OPERATION_REJECTED
    }
}

/// Applies the app's settings; call once, before any other function.
///
/// Inputs: `configuration_json` is a NUL-terminated UTF-8 JSON object:
/// `{"data_directory": "<absolute app-private directory>", "device_name"?, "whisper_model_path"?,
/// "keychain_enabled"?, "p2p_enabled"?, "p2p_peers"?: [...], "export_directory"?}`. The database,
/// keys, logs, review clips, and (by default) exports go in `data_directory`.
/// Outputs: `FFI_SUCCESS` (0); `FFI_INVALID_ARGUMENT` (1) for a null pointer, invalid JSON, an
/// unknown field, or a relative path; `FFI_OPERATION_REJECTED` (2) when the app is already
/// configured or another call already started the runtime with the environment's settings.
/// Side effects: Stores the settings for the process lifetime. Values are never logged.
///
/// # Safety
/// `configuration_json` must be null or point to a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_configure(configuration_json: *const c_char) -> c_int {
    let Some(configuration_json) = (unsafe { ffi_string(configuration_json) }) else {
        return FFI_INVALID_ARGUMENT;
    };
    let settings = match ConfiguredSettings::from_json(configuration_json) {
        Ok(settings) => settings,
        Err(_) => return FFI_INVALID_ARGUMENT,
    };
    let runtime_mutex = FFI_RUNTIME.get_or_init(|| Mutex::new(None));
    let Ok(runtime_guard) = runtime_mutex.lock() else {
        return FFI_RUNTIME_UNAVAILABLE;
    };
    if runtime_guard.is_some() {
        return FFI_OPERATION_REJECTED;
    }
    match APP_SETTINGS.configure(settings) {
        Ok(()) => FFI_SUCCESS,
        Err(SettingsError::AlreadyConfigured | SettingsError::Invalid(_)) => FFI_OPERATION_REJECTED,
    }
}

/// Opens the runtime now instead of at the first other call, so P2P sync runs from app start.
///
/// Inputs: None; call after `note_taker_configure`.
/// Outputs: `0` once the runtime is open (also when it already was), or the status that opening
/// it failed with: `6` when the keychain is missing, locked, or refused (call again to retry),
/// `2` when the database, keys, or settings are otherwise unusable.
/// Side effects: Opens the database and key store, and starts the P2P task when it is enabled.
/// Blocks while they open, so callers use a background thread.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_start_runtime() -> c_int {
    with_runtime(|_| FFI_SUCCESS)
}

/// Returns the P2P status for the Devices screen as caller-freed JSON.
///
/// Inputs: None.
/// Outputs: An owned JSON string (free with `note_taker_free_string`): the same object as
/// `GET /api/p2p/status`, `{"enabled":false}` when P2P is off, or
/// `{"ok":false,"error_code":"runtime_unavailable",...}`. Null only if allocation fails.
/// Side effects: Opens the runtime (and so starts P2P) on first use.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_p2p_status() -> *mut c_char {
    let response_json = match runtime_p2p() {
        Ok(p2p) => p2p_status_json(p2p.as_deref().map(|handle| handle as &dyn P2pControl)),
        Err(_) => export_error_json("runtime_unavailable", "the native runtime is unavailable"),
    };
    CString::new(response_json)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Runs one Devices-screen command (pair, confirm, accept, reject, revoke...) as JSON.
///
/// Inputs: `request_json` is a NUL-terminated UTF-8 object tagged by `command`; see `ffi_p2p`.
/// Outputs: An owned JSON answer (free with `note_taker_free_string`): `{"ok":true,...}` or
/// `{"ok":false,"error_code",...}` with `invalid_request`, `invalid_peer_id`, `unavailable`,
/// `refused`, or `runtime_unavailable`. Null only if allocation fails.
/// Side effects: Those of the command. Blocks until the P2P task answers, without holding the
/// runtime lock, so recording and other calls continue meanwhile.
///
/// # Safety
/// `request_json` must be null or point to a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_p2p_command(request_json: *const c_char) -> *mut c_char {
    let response_json = match unsafe { ffi_string(request_json) } {
        None => export_error_json("invalid_request", "the request must be a UTF-8 C string"),
        Some(request_json) => match runtime_p2p() {
            Ok(p2p) => p2p_command_json(
                p2p.as_deref().map(|handle| handle as &dyn P2pControl),
                request_json,
            ),
            Err(_) => export_error_json("runtime_unavailable", "the native runtime is unavailable"),
        },
    };
    CString::new(response_json)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Holds the process-local runtime without exposing it to Dart or other native consumers.
static FFI_RUNTIME: OnceLock<Mutex<Option<FfiRuntime>>> = OnceLock::new();

/// Runs an FFI operation while safely creating and borrowing its process-local runtime.
///
/// Inputs: `operation` receives the mutable runtime only while its mutex is held.
/// Outputs: The C status returned by `operation`, initialization, or lock acquisition.
/// Side effects: Initializes in-memory adapter state on the first successful FFI call.
fn with_runtime(operation: impl FnOnce(&mut FfiRuntime) -> c_int) -> c_int {
    let runtime_mutex = FFI_RUNTIME.get_or_init(|| Mutex::new(None));
    let mut runtime_guard = match runtime_mutex.lock() {
        Ok(runtime_guard) => runtime_guard,
        Err(_) => return FFI_RUNTIME_UNAVAILABLE,
    };

    if runtime_guard.is_none() {
        let created_runtime = match FfiRuntime::new() {
            Ok(created_runtime) => created_runtime,
            Err(status) => return status,
        };
        *runtime_guard = Some(created_runtime);
    }

    match runtime_guard.as_mut() {
        Some(runtime) => operation(runtime),
        None => FFI_RUNTIME_UNAVAILABLE,
    }
}

/// Runs an FFI operation that returns owned JSON while keeping the runtime lock scoped to the call.
///
/// Inputs: `operation` receives the process-local runtime after successful initialization.
/// Outputs: Serialized metadata or a documented C status.
/// Side effects: May initialize persistent adapter state and execute the supplied operation.
fn with_runtime_json(
    operation: impl FnOnce(&mut FfiRuntime) -> Result<String, c_int>,
) -> Result<String, c_int> {
    let runtime_mutex = FFI_RUNTIME.get_or_init(|| Mutex::new(None));
    let mut runtime_guard = runtime_mutex.lock().map_err(|_| FFI_RUNTIME_UNAVAILABLE)?;
    if runtime_guard.is_none() {
        *runtime_guard = Some(FfiRuntime::new()?);
    }
    match runtime_guard.as_mut() {
        Some(runtime) => operation(runtime),
        None => Err(FFI_RUNTIME_UNAVAILABLE),
    }
}

/// Starts the Rust-owned recording session for a Flutter caller.
///
/// Inputs: None.
/// Outputs: `FFI_SUCCESS` on an accepted transition or a documented nonzero C status.
/// Side effects: Creates the process-local bridge if needed and starts its local audio recorder.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_start_recording() -> c_int {
    with_runtime(FfiRuntime::start_recording)
}

/// Pauses the Rust-owned recording session from UI, lock-screen, or background service threads.
///
/// Inputs: None.
/// Outputs: `FFI_SUCCESS` on an accepted pause or a documented nonzero status.
/// Side effects: Acquires the process runtime mutex and pauses its portable recorder.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_pause_recording() -> c_int {
    with_runtime(FfiRuntime::pause_recording)
}

/// Stops the Rust-owned recording session from UI, lock-screen, or background service threads.
///
/// Inputs: None.
/// Outputs: `FFI_SUCCESS` on an accepted terminal stop or a documented nonzero status.
/// Side effects: Acquires the process runtime mutex and stops its portable recorder.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_stop_recording() -> c_int {
    with_runtime(FfiRuntime::stop_recording)
}

/// Chooses the language for the next recording, or for the rest of the current one.
///
/// Inputs: `language` is `auto` or a Whisper code.
/// Outputs: `FFI_SUCCESS`, or `FFI_INVALID_ARGUMENT` (null, not UTF-8, or unknown).
/// Side effects: Applies to audio chunks cut from now on; a stop clears the choice, and the next
/// recording uses the default language.
///
/// # Safety
/// `language` must be null or point to a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_set_recording_language(language: *const c_char) -> c_int {
    let parsed =
        unsafe { ffi_string(language) }.and_then(|text| TranscriptionLanguage::parse(text).ok());
    match parsed {
        Some(language) => with_runtime(|runtime| runtime.set_recording_language(language)),
        None => FFI_INVALID_ARGUMENT,
    }
}

/// Reports an operating-system audio stream suspension from a background service thread.
///
/// Inputs: None.
/// Outputs: `FFI_SUCCESS` once a recording has entered recoverable Interrupted state.
/// Side effects: Acquires the process runtime mutex and stops routing audio until resume.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_handle_audio_interruption() -> c_int {
    with_runtime(FfiRuntime::handle_audio_interruption)
}

/// Starts a recording into a specific meeting, creating the meeting if it does not exist.
///
/// Inputs: `meeting_id` is a non-null, non-blank NUL-terminated UTF-8 string.
/// Outputs: `FFI_SUCCESS`, `FFI_INVALID_ARGUMENT`, `FFI_OPERATION_REJECTED` (a recording is
/// already active), or `FFI_PIPELINE_UNAVAILABLE` (no transcription engine is configured).
/// Side effects: Composes the meeting pipeline on first use (may load a model; call it from a
/// background isolate) and opens a pipeline session for the meeting.
///
/// # Safety
/// `meeting_id` must be null or point to a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_start_recording_for_meeting(
    meeting_id: *const c_char,
) -> c_int {
    match unsafe { ffi_string(meeting_id) } {
        Some(meeting_id) if !meeting_id.trim().is_empty() => with_runtime(|runtime| {
            runtime.start_recording_for(Some(MeetingId::new(meeting_id.trim())))
        }),
        _ => FFI_INVALID_ARGUMENT,
    }
}

/// Starts a recording with an optional meeting ID and title (Issue #35).
///
/// Inputs: `meeting_id` is null (generate `meeting-<uuid>`) or a non-blank NUL-terminated UTF-8
/// string; `title` is null (keep the meeting's title; new meetings show a dated fallback) or a
/// NUL-terminated UTF-8 title of at most 200 characters without control characters.
/// Outputs: `FFI_SUCCESS`, `FFI_INVALID_ARGUMENT` (invalid UTF-8, blank ID, or invalid title),
/// `FFI_OPERATION_REJECTED` (a recording is already active, or the title could not be stored),
/// or `FFI_PIPELINE_UNAVAILABLE`. Both arguments are ignored when resuming a paused recording.
/// Side effects: Composes the meeting pipeline on first use (call it from a background isolate),
/// opens a pipeline session, and stores the title in SQLite. The title is never logged.
///
/// # Safety
/// Each pointer must be null or point to a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_start_recording_with_title(
    meeting_id: *const c_char,
    title: *const c_char,
) -> c_int {
    let decoded_meeting_id = match (meeting_id.is_null(), unsafe { ffi_string(meeting_id) }) {
        (true, _) => None,
        (false, Some(meeting_id)) => Some(meeting_id),
        (false, None) => return FFI_INVALID_ARGUMENT,
    };
    let decoded_title = match (title.is_null(), unsafe { ffi_string(title) }) {
        (true, _) => None,
        (false, Some(title)) => Some(title),
        (false, None) => return FFI_INVALID_ARGUMENT,
    };
    match titled_start_arguments(decoded_meeting_id, decoded_title) {
        Ok((meeting_id, title)) => {
            with_runtime(|runtime| runtime.start_recording_titled(meeting_id, title))
        }
        Err(status) => status,
    }
}

/// Lists every local meeting with its title as caller-freed JSON.
///
/// Inputs: None.
/// Outputs: An owned JSON string (free with `note_taker_free_string`):
/// `[{"meeting_id","title","created_at"}]`, where an untitled meeting's `title` is its dated
/// fallback such as `Meeting 2026-10-27 09:05 UTC`; or `{"ok":false,"error_code","error"}`.
/// Null only if the response cannot be allocated.
/// Side effects: Reads SQLite meeting metadata.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_get_meetings() -> *mut c_char {
    let response_json =
        with_runtime_json(|runtime| Ok(runtime.get_meetings())).unwrap_or_else(|_| {
            export_error_json("runtime_unavailable", "the native runtime is unavailable")
        });
    CString::new(response_json)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Describes one meeting, including its title, as caller-freed JSON.
///
/// Inputs: `meeting_id` is a non-null, non-blank NUL-terminated UTF-8 string.
/// Outputs: An owned JSON string (free with `note_taker_free_string`):
/// `{"meeting_id","title","created_at","transcript"}`, or `{"ok":false,"error_code","error"}`
/// where `error_code` is `invalid_argument`, `meeting_not_found`, `load_failed`, or
/// `runtime_unavailable`. Null only if the response cannot be allocated.
/// Side effects: Reads SQLite.
///
/// # Safety
/// `meeting_id` must be null or point to a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_get_meeting(meeting_id: *const c_char) -> *mut c_char {
    let response_json = match unsafe { ffi_string(meeting_id) } {
        Some(meeting_id) => with_runtime_json(|runtime| Ok(runtime.get_meeting(meeting_id)))
            .unwrap_or_else(|_| {
                export_error_json("runtime_unavailable", "the native runtime is unavailable")
            }),
        None => export_error_json(
            "invalid_argument",
            "meeting_id must be a valid UTF-8 C string",
        ),
    };
    CString::new(response_json)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Attaches files or camera photos to a meeting (Issue #54).
///
/// Inputs: `meeting_id` and `paths_json` (a JSON array of absolute file paths) are non-null
/// NUL-terminated UTF-8 strings.
/// Outputs: A caller-freed JSON response as described by `attach_files`: the attached files and
/// those that failed, or `{"ok":false,"error_code","error"}` (`invalid_argument`,
/// `meeting_not_found`, `runtime_unavailable`); null only if the response cannot be built.
/// Side effects: Copies the files into the app's attachment folder and links them to the
/// meeting, at its current meeting time while it is being recorded. The sources are left.
///
/// # Safety
/// Both pointers must be null or valid NUL-terminated C strings for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_attach_files(
    meeting_id: *const c_char,
    paths_json: *const c_char,
) -> *mut c_char {
    let response_json = match (unsafe { ffi_string(meeting_id) }, unsafe {
        ffi_string(paths_json)
    }) {
        (Some(meeting_id), Some(paths_json)) => {
            with_runtime_json(|runtime| Ok(runtime.attach_files(meeting_id, paths_json)))
                .unwrap_or_else(|_| {
                    export_error_json("runtime_unavailable", "the native runtime is unavailable")
                })
        }
        _ => export_error_json(
            "invalid_argument",
            "meeting_id and paths_json must be valid UTF-8 C strings",
        ),
    };
    CString::new(response_json)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Renames one meeting.
///
/// Inputs: `meeting_id` and `new_title` are non-null NUL-terminated UTF-8 strings. The meeting
/// ID must not be blank; the title is trimmed and must then be 1–200 characters without control
/// characters (such as newlines), the same rules as `note_taker_start_recording_with_title`.
/// Outputs: `FFI_SUCCESS` (0); `FFI_INVALID_ARGUMENT` (1) for a null or non-UTF-8 pointer, a
/// blank meeting ID, or an invalid title; `FFI_OPERATION_REJECTED` (2) for a storage failure;
/// `FFI_RUNTIME_UNAVAILABLE` (3) when the native runtime cannot start; `FFI_MEETING_NOT_FOUND`
/// (5) for an unknown meeting.
/// Side effects: Opens the runtime on first use and replaces the meeting's title in SQLite. Null
/// pointers are rejected before the runtime is touched. The title is never logged.
///
/// # Safety
/// Each pointer must be null or point to a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_rename_meeting(
    meeting_id: *const c_char,
    new_title: *const c_char,
) -> c_int {
    let (Some(meeting_id), Some(new_title)) = (unsafe { ffi_string(meeting_id) }, unsafe {
        ffi_string(new_title)
    }) else {
        return FFI_INVALID_ARGUMENT;
    };
    with_runtime(|runtime| runtime.rename_meeting(meeting_id, new_title))
}

/// Feeds captured 16 kHz mono signed 16-bit PCM into the active recording.
///
/// Inputs: `samples` points to `sample_count` samples (null is allowed only for a count of 0).
/// Outputs: `FFI_SUCCESS` (audio is discarded while paused), `FFI_INVALID_ARGUMENT`, or
/// `FFI_OPERATION_REJECTED` when no recording is active.
/// Side effects: Copies the samples, buffers them, and queues finished chunks for background
/// transcription; it never waits for transcription. Audio is never logged.
///
/// # Safety
/// A non-null `samples` must be valid for reading `sample_count` `int16_t` values during this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_push_audio_pcm(
    samples: *const i16,
    sample_count: usize,
) -> c_int {
    match unsafe { pcm_samples_from_raw(samples, sample_count) } {
        Some(samples) => with_runtime(|runtime| runtime.push_audio_pcm(samples)),
        None => FFI_INVALID_ARGUMENT,
    }
}

/// Returns the meeting ID of the active recording as a caller-freed C string.
///
/// Inputs: None.
/// Outputs: The meeting ID (free with `note_taker_free_string`), or null when no recording is
/// active or the runtime is unavailable.
/// Side effects: May lazily initialize the process runtime.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_get_active_meeting_id() -> *mut c_char {
    with_runtime_json(|runtime| runtime.active_meeting_id().ok_or(FFI_OPERATION_REJECTED))
        .ok()
        .and_then(|meeting_id| CString::new(meeting_id).ok())
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Returns the lock-screen-safe current portable recorder state without waking Flutter UI state.
///
/// Inputs: None.
/// Outputs: A documented `FFI_AUDIO_STATE_*` integer, or `FFI_RUNTIME_UNAVAILABLE` on lock/init failure.
/// Side effects: May lazily initialize the process runtime, then performs only a mutex-protected read.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_get_current_state() -> c_int {
    with_runtime(|runtime| runtime.current_recording_state())
}

/// Returns telemetry-observed device cards as caller-freed UTF-8 JSON.
///
/// Inputs: None.
/// Outputs: An owned JSON array or null when the local runtime is unavailable.
/// Side effects: Reads privacy-safe SQLite telemetry metadata only.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_get_devices() -> *mut c_char {
    with_runtime_json(|runtime| runtime.get_devices())
        .ok()
        .and_then(|json| CString::new(json).ok())
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Returns aggregate telemetry summary values as caller-freed UTF-8 JSON.
///
/// Inputs: None.
/// Outputs: An owned JSON object or null when the local runtime is unavailable.
/// Side effects: Reads privacy-safe SQLite telemetry metadata only.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_get_telemetry_summary() -> *mut c_char {
    with_runtime_json(|runtime| runtime.get_telemetry_summary())
        .ok()
        .and_then(|json| CString::new(json).ok())
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Returns a device's recent telemetry series as caller-freed UTF-8 JSON.
///
/// Inputs: `device_id` is a valid non-empty NUL-terminated UTF-8 identifier.
/// Outputs: An owned JSON object or null for invalid input/runtime failures.
/// Side effects: Reads the last 24 hours of privacy-safe SQLite telemetry metadata.
///
/// # Safety
/// `device_id` must be null or point to a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_get_device_telemetry(device_id: *const c_char) -> *mut c_char {
    let Some(device_id) = (unsafe { ffi_string(device_id) }).filter(|value| !value.is_empty())
    else {
        return std::ptr::null_mut();
    };
    with_runtime_json(|runtime| runtime.get_device_telemetry(device_id))
        .ok()
        .and_then(|json| CString::new(json).ok())
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Returns clip metadata and absolute local audio paths as caller-freed UTF-8 JSON.
///
/// Inputs: `meeting_id` points to a valid NUL-terminated UTF-8 identifier.
/// Outputs: An owned JSON string pointer or null for invalid input/runtime failures.
/// Side effects: Reads repository clips and materializes local playback files; never returns audio bytes.
///
/// # Safety
/// `meeting_id` must be null or point to a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_get_unassigned_clips(meeting_id: *const c_char) -> *mut c_char {
    if meeting_id.is_null() {
        return std::ptr::null_mut();
    }
    let meeting_id = match unsafe { CStr::from_ptr(meeting_id) }.to_str() {
        Ok(value) if !value.is_empty() => value,
        _ => return std::ptr::null_mut(),
    };
    let json = match with_runtime_json(|runtime| runtime.get_unassigned_clips(meeting_id)) {
        Ok(value) => value,
        Err(_) => return std::ptr::null_mut(),
    };
    CString::new(json)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Returns ranked meeting suggestions for a picker-selected attachment as caller-freed UTF-8 JSON.
///
/// Inputs: `file_path` points to a non-empty NUL-terminated UTF-8 local path.
/// Outputs: An owned JSON array with opaque `attachment_id` and meeting suggestion fields, or null.
/// Side effects: Delegates suggestion ranking to `AttachmentManager` and caches metadata for approval.
///
/// # Safety
/// `file_path` must be null or point to a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_process_attachment(file_path: *const c_char) -> *mut c_char {
    let Some(file_path) = (unsafe { ffi_string(file_path) }).filter(|value| !value.is_empty())
    else {
        return std::ptr::null_mut();
    };
    let json = match with_runtime_json(|runtime| runtime.process_attachment(file_path)) {
        Ok(value) => value,
        Err(_) => return std::ptr::null_mut(),
    };
    match CString::new(json) {
        Ok(value) => value.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Persists the meeting link selected from a prior attachment suggestion response.
///
/// Inputs: Both pointers are NUL-terminated UTF-8 strings returned/selected through Flutter UI.
/// Outputs: `FFI_SUCCESS` after persistence or a documented nonzero rejection status.
/// Side effects: Saves attachment metadata and removes the runtime's pending import on success.
///
/// # Safety
/// Both pointers must be null or valid NUL-terminated C strings for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_confirm_attachment_link(
    attachment_id: *const c_char,
    meeting_id: *const c_char,
) -> c_int {
    let (Some(attachment_id), Some(meeting_id)) = (unsafe { ffi_string(attachment_id) }, unsafe {
        ffi_string(meeting_id)
    }) else {
        return FFI_INVALID_ARGUMENT;
    };
    with_runtime(|runtime| runtime.confirm_attachment_link(attachment_id, meeting_id))
}

/// Confirms a weak match through the existing voice identity application service.
///
/// Inputs: Both pointers are NUL-terminated UTF-8 UUID strings.
/// Outputs: A documented C status.
/// Side effects: May persist a derived signature variant; no audio bytes cross FFI.
///
/// # Safety
/// Both pointers must be null or valid NUL-terminated C strings for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_confirm_weak_match(
    clip_id: *const c_char,
    identity_id: *const c_char,
) -> c_int {
    let (Some(clip_id), Some(identity_id)) = (unsafe { ffi_string(clip_id) }, unsafe {
        ffi_string(identity_id)
    }) else {
        return FFI_INVALID_ARGUMENT;
    };
    with_runtime(|runtime| runtime.confirm_weak_match(clip_id, identity_id))
}

/// Creates an identity from a reviewed clip through the existing voice identity application service.
///
/// Inputs: `clip_id` and `display_name` are NUL-terminated UTF-8 strings.
/// Outputs: A documented C status.
/// Side effects: May persist one derived signature variant; no audio bytes cross FFI.
///
/// # Safety
/// Both pointers must be null or valid NUL-terminated C strings for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_assign_new_identity(
    clip_id: *const c_char,
    display_name: *const c_char,
) -> c_int {
    let (Some(clip_id), Some(display_name)) = (unsafe { ffi_string(clip_id) }, unsafe {
        ffi_string(display_name)
    }) else {
        return FFI_INVALID_ARGUMENT;
    };
    with_runtime(|runtime| runtime.assign_new_identity(clip_id, display_name))
}

/// Lists a meeting's unresolved unknown-speaker clusters as caller-freed UTF-8 JSON.
///
/// Inputs: `meeting_id` is a non-null NUL-terminated UTF-8 meeting identifier.
/// Outputs: An owned JSON string (free with `note_taker_free_string`): an array of cluster
/// summaries with optional absolute `review_clip_path`, or `{"ok":false,"error_code","error"}`
/// with `invalid_argument`, `list_failed`, or `runtime_unavailable`. Returns null only if the
/// response cannot be allocated as a C string.
/// Side effects: Reads SQLite and materializes review clips locally; never returns audio bytes or
/// embeddings. Runs synchronously, so Flutter should call it from a background isolate.
///
/// # Safety
/// `meeting_id` must be null or point to a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_get_unknown_clusters(meeting_id: *const c_char) -> *mut c_char {
    let response_json = match unsafe { ffi_string(meeting_id) } {
        Some(meeting_id) => {
            with_runtime_json(|runtime| Ok(runtime.get_unknown_clusters(meeting_id)))
                .unwrap_or_else(|_| {
                    export_error_json("runtime_unavailable", "the native runtime is unavailable")
                })
        }
        None => export_error_json(
            "invalid_argument",
            "meeting_id must be a valid UTF-8 C string",
        ),
    };
    CString::new(response_json)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Returns every meeting's unresolved clusters and the saved people as caller-freed JSON.
///
/// Inputs: None.
/// Outputs: An owned JSON string (free with `note_taker_free_string`), as described on
/// `FfiRuntime::get_voice_library`, or `{"ok":false,"error_code":"runtime_unavailable","error"}`.
/// Returns null only if the response cannot be allocated as a C string.
/// Side effects: Reads SQLite synchronously; Flutter should call it from a background isolate.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_get_voice_library() -> *mut c_char {
    let response_json = with_runtime_json(|runtime| Ok(runtime.get_voice_library()))
        .unwrap_or_else(|_| {
            export_error_json("runtime_unavailable", "the native runtime is unavailable")
        });
    CString::new(response_json)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Writes a cluster's review clip to a playable file and returns caller-freed JSON with its path.
///
/// Inputs: `cluster_id` is a non-null NUL-terminated UTF-8 UUID.
/// Outputs: An owned JSON string (free with `note_taker_free_string`), as described on
/// `FfiRuntime::prepare_cluster_review_clip`. Returns null only if the response cannot be
/// allocated as a C string.
/// Side effects: Decrypts the clip to the review-clip directory, synchronously.
///
/// # Safety
/// `cluster_id` must be null or point to a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_prepare_cluster_review_clip(
    cluster_id: *const c_char,
) -> *mut c_char {
    let response_json = match unsafe { ffi_string(cluster_id) } {
        Some(cluster_id) => {
            with_runtime_json(|runtime| Ok(runtime.prepare_cluster_review_clip(cluster_id)))
                .unwrap_or_else(|_| {
                    export_error_json("runtime_unavailable", "the native runtime is unavailable")
                })
        }
        None => export_error_json(
            "invalid_argument",
            "cluster_id must be a valid UTF-8 C string",
        ),
    };
    CString::new(response_json)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Assigns an unknown-speaker cluster to a named identity and returns caller-freed result JSON.
///
/// Inputs: `cluster_id` (a UUID) and `identity_name` (non-blank) are non-null NUL-terminated
/// UTF-8 strings.
/// Outputs: An owned JSON string (free with `note_taker_free_string`): `{"ok":true,"cluster_id",
/// "identity_id","identity_name","identity_created","variants_stored","segments_updated"}` or
/// `{"ok":false,"error_code","error"}` with `invalid_argument`, `cluster_not_found`,
/// `cluster_already_resolved`, `assignment_failed`, or `runtime_unavailable`. Returns null only
/// if the response cannot be allocated as a C string.
/// Side effects: Stores voice variants, relinks transcript segments, resolves the cluster, and
/// deletes its review audio, synchronously; Flutter should call it from a background isolate.
///
/// # Safety
/// Both pointers must be null or point to valid NUL-terminated C strings for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_assign_cluster(
    cluster_id: *const c_char,
    identity_name: *const c_char,
) -> *mut c_char {
    let response_json = match (unsafe { ffi_string(cluster_id) }, unsafe {
        ffi_string(identity_name)
    }) {
        (Some(cluster_id), Some(identity_name)) => {
            with_runtime_json(|runtime| Ok(runtime.assign_cluster(cluster_id, identity_name)))
                .unwrap_or_else(|_| {
                    export_error_json("runtime_unavailable", "the native runtime is unavailable")
                })
        }
        _ => export_error_json(
            "invalid_argument",
            "cluster_id and identity_name must be valid UTF-8 C strings",
        ),
    };
    CString::new(response_json)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Converts a borrowed C string to UTF-8 without retaining its pointer.
///
/// Inputs: `value` is a nullable C string pointer.
/// Outputs: A borrowed UTF-8 string for the duration of the FFI call.
/// Side effects: None.
///
/// # Safety
/// Non-null `value` must point to a valid NUL-terminated C string.
unsafe fn ffi_string<'a>(value: *const c_char) -> Option<&'a str> {
    if value.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(value) }.to_str().ok()
}

/// Exports one meeting to Markdown, PDF, or JSON and returns caller-freed result JSON.
///
/// Inputs: `meeting_id` and `format` are non-null NUL-terminated UTF-8 strings; `options_json` is
/// null, empty, or a JSON object with optional `include_attachments`, `include_metadata`, and
/// `output_path`.
/// Outputs: An owned JSON string (free with `note_taker_free_string`): `{"ok":true,"file_path",
/// "format","size_bytes","metadata"}` or `{"ok":false,"error_code","error"}`. Returns null only if
/// the response cannot be allocated as a C string.
/// Side effects: Reads SQLite, copies attachments, and writes the export file synchronously, so
/// Flutter must call it from a background isolate.
///
/// # Safety
/// Each pointer must be null or point to a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_export_meeting(
    meeting_id: *const c_char,
    format: *const c_char,
    options_json: *const c_char,
) -> *mut c_char {
    let response_json = match (unsafe { ffi_string(meeting_id) }, unsafe {
        ffi_string(format)
    }) {
        (Some(meeting_id), Some(format)) => {
            let options_json = unsafe { ffi_string(options_json) }.unwrap_or("");
            with_runtime_json(|runtime| {
                Ok(runtime.export_meeting(meeting_id, format, options_json))
            })
            .unwrap_or_else(|_| {
                export_error_json("runtime_unavailable", "the native runtime is unavailable")
            })
        }
        _ => export_error_json(
            "invalid_argument",
            "meeting_id and format must be valid UTF-8 C strings",
        ),
    };
    CString::new(response_json)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Frees a JSON string returned by `note_taker_get_unassigned_clips`.
///
/// Inputs: `value` must be null or a pointer returned by this library and not previously freed.
/// Outputs: None.
/// Side effects: Releases native heap memory.
///
/// # Safety
/// Passing any pointer not returned by this library is undefined behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_free_string(value: *mut c_char) {
    if !value.is_null() {
        unsafe {
            drop(CString::from_raw(value));
        }
    }
}

/// The platform's default microphone, when built with the `microphone` feature.
fn default_microphone_source() -> Option<Arc<dyn MicrophoneSource>> {
    #[cfg(feature = "microphone")]
    {
        Some(Arc::new(
            crate::adapters::audio::cpal_microphone::CpalMicrophone::new(),
        ))
    }
    #[cfg(not(feature = "microphone"))]
    {
        None
    }
}

/// Starts the thread that drains the global runtime's microphone every 100 ms.
///
/// It takes the runtime lock only to drain, and never holds anything while waiting for it, so
/// a stop (which holds the lock while transcribing) cannot deadlock with it. It ends once no
/// microphone is open.
/// Side effects: Spawns the `microphone-drain` thread.
fn spawn_global_microphone_ticker() {
    let spawned = thread::Builder::new()
        .name(String::from("microphone-drain"))
        .spawn(|| {
            loop {
                thread::sleep(MICROPHONE_DRAIN_INTERVAL);
                let still_open = with_runtime(|runtime| c_int::from(runtime.drain_microphone()));
                if still_open != 1 {
                    break;
                }
            }
        });
    if spawned.is_err() {
        tracing::error!("Unable to start the microphone drain thread");
    }
}

#[cfg(test)]
#[path = "ffi_microphone_tests.rs"]
mod ffi_microphone_tests;

#[cfg(test)]
mod ffi_contract_tests {
    use super::{
        FFI_AUDIO_STATE_IDLE, FFI_AUDIO_STATE_INTERRUPTED, FFI_AUDIO_STATE_PAUSED,
        FFI_AUDIO_STATE_RECORDING, FFI_AUDIO_STATE_STOPPED, FFI_INVALID_ARGUMENT,
        FFI_MEETING_NOT_FOUND, FFI_OPERATION_REJECTED, FFI_PIPELINE_UNAVAILABLE, FFI_SUCCESS,
        FfiRuntime, note_taker_rename_meeting, pcm_samples_from_raw, titled_start_arguments,
    };
    use crate::{
        application::unknown_speaker_clusterer::{ClusteringConfig, UnknownSpeakerClusterer},
        domain::models::{
            DeviceId, MeetingId, MeetingTitle, SpeakerIdentity, TaskMetrics, TaskType,
            TranscriptSegment, UnassignedVoiceClip,
        },
        ports::{storage::NoteRepository, telemetry::MetricsRepository, voice::VoiceVectorDB},
    };

    /// Builds a runtime backed by in-memory SQLite exporting into a unique temp directory.
    fn export_runtime() -> (FfiRuntime, std::path::PathBuf) {
        let export_directory =
            std::env::temp_dir().join(format!("note-taker-ffi-export-{}", uuid::Uuid::new_v4()));
        let ffi_runtime = FfiRuntime::in_memory_for_tests(&export_directory)
            .unwrap_or_else(|status| panic!("fixture runtime must initialize: status {status}"));
        (ffi_runtime, export_directory)
    }

    /// Parses an FFI export response.
    fn parse_response(response_json: &str) -> serde_json::Value {
        serde_json::from_str(response_json)
            .unwrap_or_else(|error| panic!("FFI export response must be JSON: {error}"))
    }

    /// Verifies the FFI export writes through ExportManager and returns result metadata as JSON.
    #[test]
    fn export_meeting_returns_result_json_for_a_stored_meeting() {
        let (ffi_runtime, export_directory) = export_runtime();
        let meeting_id = MeetingId::new("ffi-export-meeting");
        ffi_runtime
            .note_repository
            .create_meeting(&meeting_id, &[1])
            .unwrap_or_else(|error| panic!("fixture meeting must persist: {error}"));
        ffi_runtime
            .note_repository
            .save_transcript_segments(
                &meeting_id,
                &[TranscriptSegment::new(
                    meeting_id.clone(),
                    0,
                    1_000,
                    None,
                    "From Flutter.",
                )],
            )
            .unwrap_or_else(|error| panic!("fixture segments must persist: {error}"));

        let response = parse_response(&ffi_runtime.export_meeting(
            "ffi-export-meeting",
            "json",
            r#"{"include_metadata":false}"#,
        ));
        let file_path = response["file_path"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let exported_json = std::fs::read_to_string(&file_path).unwrap_or_default();
        let _ = std::fs::remove_dir_all(&export_directory);

        assert_eq!(response["ok"], true);
        assert_eq!(response["format"], "json");
        assert!(response["size_bytes"].as_u64().unwrap_or_default() > 0);
        assert_eq!(response["metadata"]["meeting_count"], "1");
        assert!(file_path.starts_with(&*export_directory.to_string_lossy()));
        assert!(exported_json.contains("From Flutter."));
        assert!(!exported_json.contains("\"title\""));
    }

    /// Verifies explicit output paths are honored and empty options use defaults.
    #[test]
    fn export_meeting_honors_output_path_option() {
        let (ffi_runtime, export_directory) = export_runtime();
        ffi_runtime
            .note_repository
            .create_meeting(&MeetingId::new("ffi-output"), &[1])
            .unwrap_or_else(|error| panic!("fixture meeting must persist: {error}"));
        let output_path = export_directory.join("custom.md");

        let response = parse_response(&ffi_runtime.export_meeting(
            "ffi-output",
            "markdown",
            &serde_json::json!({ "output_path": output_path }).to_string(),
        ));
        let output_exists = output_path.exists();
        let default_response = parse_response(&ffi_runtime.export_meeting("ffi-output", "md", ""));
        let _ = std::fs::remove_dir_all(&export_directory);

        assert_eq!(response["ok"], true);
        assert!(output_exists);
        assert_eq!(default_response["ok"], true);
    }

    /// Verifies invalid input and missing meetings produce structured error JSON, never a crash.
    #[test]
    fn export_meeting_reports_structured_errors() {
        let (ffi_runtime, export_directory) = export_runtime();

        for (meeting_id, format, options_json, expected_code) in [
            ("missing", "json", "{}", "meeting_not_found"),
            ("missing", "docx", "{}", "invalid_argument"),
            ("missing", "srt", "{}", "unsupported_format"),
            ("missing", "json", "{not json", "invalid_argument"),
            ("", "json", "{}", "invalid_argument"),
        ] {
            let response =
                parse_response(&ffi_runtime.export_meeting(meeting_id, format, options_json));
            assert_eq!(response["ok"], false);
            assert_eq!(
                response["error_code"], expected_code,
                "{format} {options_json}"
            );
            assert!(
                response["error"]
                    .as_str()
                    .is_some_and(|message| !message.is_empty())
            );
        }
        assert!(!export_directory.exists());
    }

    /// Verifies telemetry FFI JSON is derived from in-memory SQLite without exposing task payloads.
    #[test]
    fn telemetry_ffi_serializes_devices_summary_and_recent_device_metrics() {
        let ffi_runtime = FfiRuntime::in_memory_for_tests(&std::env::temp_dir())
            .unwrap_or_else(|status| panic!("fixture runtime must initialize: status {status}"));
        let timestamp_epoch_ms =
            u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or_default();
        ffi_runtime
            .note_repository
            .save_metrics(TaskMetrics::new(
                uuid::Uuid::new_v4(),
                DeviceId::new("ffi-device"),
                TaskType::Transcription,
                "whisper",
                900,
                90,
                1.5,
                timestamp_epoch_ms,
            ))
            .unwrap_or_else(|error| panic!("fixture metric must save: {error}"));

        assert!(
            ffi_runtime
                .get_devices()
                .unwrap_or_default()
                .contains("ffi-device")
        );
        assert!(
            ffi_runtime
                .get_telemetry_summary()
                .unwrap_or_default()
                .contains("total_tasks")
        );
        assert!(
            ffi_runtime
                .get_device_telemetry("ffi-device")
                .unwrap_or_default()
                .contains("duration_ms")
        );
    }

    /// Verifies that the bridge reports the first portable start as successful.
    #[test]
    fn starts_an_idle_recorder_through_the_c_boundary_runtime() {
        let mut ffi_runtime = FfiRuntime::in_memory_for_tests(&std::env::temp_dir())
            .unwrap_or_else(|status| panic!("fixture runtime must initialize: status {status}"));

        assert_eq!(ffi_runtime.start_recording(), FFI_SUCCESS);
    }

    /// Verifies that the bridge preserves the audio port's terminal duplicate-start rejection.
    #[test]
    fn rejects_a_second_start_without_exposing_the_rust_audio_error() {
        let mut ffi_runtime = FfiRuntime::in_memory_for_tests(&std::env::temp_dir())
            .unwrap_or_else(|status| panic!("fixture runtime must initialize: status {status}"));

        assert_eq!(ffi_runtime.start_recording(), FFI_SUCCESS);
        assert_eq!(ffi_runtime.start_recording(), FFI_OPERATION_REJECTED);
    }

    /// Verifies lifecycle commands share one serialized runtime and expose portable state codes.
    #[test]
    fn background_lifecycle_commands_are_serializable_and_report_portable_state() {
        let mut ffi_runtime = FfiRuntime::in_memory_for_tests(&std::env::temp_dir())
            .unwrap_or_else(|status| panic!("fixture runtime must initialize: status {status}"));

        assert_eq!(ffi_runtime.start_recording(), FFI_SUCCESS);
        assert_eq!(
            ffi_runtime.current_recording_state(),
            FFI_AUDIO_STATE_RECORDING
        );
        assert_eq!(ffi_runtime.pause_recording(), FFI_SUCCESS);
        assert_eq!(
            ffi_runtime.current_recording_state(),
            FFI_AUDIO_STATE_PAUSED
        );
        assert_eq!(ffi_runtime.start_recording(), FFI_SUCCESS);
        assert_eq!(ffi_runtime.handle_audio_interruption(), FFI_SUCCESS);
        assert_eq!(
            ffi_runtime.current_recording_state(),
            FFI_AUDIO_STATE_INTERRUPTED
        );
        assert_eq!(ffi_runtime.start_recording(), FFI_SUCCESS);
        assert_eq!(ffi_runtime.stop_recording(), FFI_SUCCESS);
        assert_eq!(
            ffi_runtime.current_recording_state(),
            FFI_AUDIO_STATE_STOPPED
        );
    }

    /// Seeds one meeting with a two-chunk cluster that owns a review clip and one segment.
    ///
    /// Outputs: The cluster ID and the review clip ID.
    fn seed_cluster(ffi_runtime: &FfiRuntime, meeting_id: &MeetingId) -> (uuid::Uuid, uuid::Uuid) {
        ffi_runtime
            .note_repository
            .create_meeting(meeting_id, &[])
            .unwrap_or_else(|error| panic!("fixture meeting must persist: {error}"));
        let clusterer =
            UnknownSpeakerClusterer::new(&ffi_runtime.note_repository, ClusteringConfig::default());
        let cluster_id = clusterer
            .find_or_create_cluster(meeting_id, &[1.0, 0.0], String::from("c1"), 1_000)
            .unwrap_or_else(|error| panic!("fixture chunk must cluster: {error}"))
            .cluster_id();
        clusterer
            .find_or_create_cluster(meeting_id, &[0.9, 0.3], String::from("c2"), 1_500)
            .unwrap_or_else(|error| panic!("fixture chunk must cluster: {error}"));
        let review_clip_id = uuid::Uuid::new_v4();
        ffi_runtime
            .note_repository
            .save_unassigned_clip(
                UnassignedVoiceClip::new(review_clip_id, meeting_id.clone(), 0, 1_000, vec![7, 8])
                    .with_cluster_id(Some(cluster_id)),
            )
            .unwrap_or_else(|error| panic!("fixture clip must persist: {error}"));
        ffi_runtime
            .note_repository
            .save_transcript_segments(
                meeting_id,
                &[
                    TranscriptSegment::new(meeting_id.clone(), 0, 1_000, None, "hi")
                        .with_unknown_cluster_id(Some(cluster_id)),
                ],
            )
            .unwrap_or_else(|error| panic!("fixture segments must persist: {error}"));
        (cluster_id, review_clip_id)
    }

    /// Verifies cluster listing serializes summaries and a playable review clip path as JSON.
    #[test]
    fn unknown_clusters_serialize_summaries_with_review_clip_paths() {
        let (mut ffi_runtime, export_directory) = export_runtime();
        let meeting_id = MeetingId::new("ffi-cluster-meeting");
        let (cluster_id, review_clip_id) = seed_cluster(&ffi_runtime, &meeting_id);

        let response = parse_response(&ffi_runtime.get_unknown_clusters("ffi-cluster-meeting"));
        let review_clip_path = response[0]["review_clip_path"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let review_clip_bytes = std::fs::read(&review_clip_path).unwrap_or_default();
        let empty_response = parse_response(&ffi_runtime.get_unknown_clusters("other-meeting"));
        let invalid_response = parse_response(&ffi_runtime.get_unknown_clusters(" "));
        let _ = std::fs::remove_dir_all(&export_directory);

        assert_eq!(response.as_array().map(Vec::len), Some(1));
        assert_eq!(response[0]["cluster_id"], cluster_id.to_string());
        assert_eq!(response[0]["meeting_id"], "ffi-cluster-meeting");
        assert_eq!(response[0]["total_duration_ms"], 2_500);
        assert_eq!(response[0]["chunk_count"], 2);
        assert!(response[0]["created_at"].is_string());
        assert!(response[0].get("representative_embedding").is_none());
        assert!(review_clip_path.ends_with(&format!("{review_clip_id}.wav")));
        assert_eq!(review_clip_bytes, vec![7, 8]);
        assert_eq!(empty_response, serde_json::json!([]));
        assert_eq!(invalid_response["ok"], false);
        assert_eq!(invalid_response["error_code"], "invalid_argument");
    }

    /// Verifies the library JSON spans meetings without writing audio or exposing embeddings, and
    /// that a clip becomes a file only when requested.
    #[test]
    fn voice_library_lists_clusters_and_people_and_clips_are_written_on_request() {
        let (mut ffi_runtime, export_directory) = export_runtime();
        let (first_cluster_id, _) = seed_cluster(&ffi_runtime, &MeetingId::new("ffi-library-1"));
        let (assigned_cluster_id, _) = seed_cluster(&ffi_runtime, &MeetingId::new("ffi-library-2"));
        parse_response(&ffi_runtime.assign_cluster(&assigned_cluster_id.to_string(), "Ada"));
        let clip_files = |runtime: &FfiRuntime| {
            std::fs::read_dir(&runtime.review_clip_directory)
                .map(|entries| entries.count())
                .unwrap_or(0)
        };

        let library = parse_response(&ffi_runtime.get_voice_library());
        let files_after_listing = clip_files(&ffi_runtime);
        let clip =
            parse_response(&ffi_runtime.prepare_cluster_review_clip(&first_cluster_id.to_string()));
        let clip_bytes = std::fs::read(clip["review_clip_path"].as_str().unwrap_or_default())
            .unwrap_or_default();
        let missing = parse_response(
            &ffi_runtime.prepare_cluster_review_clip(&uuid::Uuid::new_v4().to_string()),
        );
        let invalid = parse_response(&ffi_runtime.prepare_cluster_review_clip("nope"));
        let _ = std::fs::remove_dir_all(&export_directory);

        assert_eq!(library["ok"], true);
        let clusters = library["unknown_clusters"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0]["cluster_id"], first_cluster_id.to_string());
        assert_eq!(clusters[0]["has_review_clip"], true);
        assert!(clusters[0].get("representative_embedding").is_none());
        assert_eq!(library["saved_people"][0]["display_name"], "Ada");
        assert_eq!(library["saved_people"][0]["voice_variant_count"], 2);
        assert_eq!(files_after_listing, 0);
        assert_eq!(clip_bytes, vec![7, 8]);
        assert_eq!(missing["error_code"], "clip_not_found");
        assert_eq!(invalid["error_code"], "invalid_argument");
    }

    /// Verifies assignment returns the summary JSON and deletes the materialized review audio.
    #[test]
    fn assign_cluster_returns_success_json_and_removes_review_audio() {
        let (mut ffi_runtime, export_directory) = export_runtime();
        let meeting_id = MeetingId::new("ffi-assign-meeting");
        let (cluster_id, _) = seed_cluster(&ffi_runtime, &meeting_id);
        let listed = parse_response(&ffi_runtime.get_unknown_clusters("ffi-assign-meeting"));
        let review_clip_path =
            std::path::PathBuf::from(listed[0]["review_clip_path"].as_str().unwrap_or_default());
        let review_clip_existed = review_clip_path.exists();

        let response =
            parse_response(&ffi_runtime.assign_cluster(&cluster_id.to_string(), "Grace Hopper"));
        let review_clip_remains = review_clip_path.exists();
        let relisted = parse_response(&ffi_runtime.get_unknown_clusters("ffi-assign-meeting"));
        let _ = std::fs::remove_dir_all(&export_directory);

        assert!(review_clip_existed);
        assert_eq!(response["ok"], true);
        assert_eq!(response["cluster_id"], cluster_id.to_string());
        assert_eq!(response["identity_name"], "Grace Hopper");
        assert_eq!(response["identity_created"], true);
        assert_eq!(response["variants_stored"], 2);
        assert_eq!(response["segments_updated"], 1);
        assert!(
            response["identity_id"]
                .as_str()
                .and_then(|identity_id| uuid::Uuid::parse_str(identity_id).ok())
                .is_some()
        );
        assert!(
            !review_clip_remains,
            "raw review audio must be deleted after assignment"
        );
        assert_eq!(relisted, serde_json::json!([]));
    }

    /// Verifies a name assigned in one app launch is reused, not duplicated, in the next launch.
    #[test]
    fn assigned_cluster_names_persist_across_runtimes() {
        let test_directory =
            std::env::temp_dir().join(format!("note-taker-ffi-persist-{}", uuid::Uuid::new_v4()));
        let database_path = test_directory.join("note_taker.sqlite3");
        std::fs::create_dir_all(&test_directory)
            .unwrap_or_else(|error| panic!("test directory must be created: {error}"));
        let open_runtime = || {
            FfiRuntime::open_for_tests(&database_path, &test_directory)
                .unwrap_or_else(|status| panic!("file runtime must open: status {status}"))
        };
        let first_response = {
            let mut first_launch = open_runtime();
            let (cluster_id, _) = seed_cluster(&first_launch, &MeetingId::new("ffi-first-launch"));
            parse_response(&first_launch.assign_cluster(&cluster_id.to_string(), "Grace Hopper"))
        };

        let mut second_launch = open_runtime();
        let (cluster_id, _) = seed_cluster(&second_launch, &MeetingId::new("ffi-second-launch"));
        let second_response =
            parse_response(&second_launch.assign_cluster(&cluster_id.to_string(), "grace HOPPER"));
        let stored_identities = second_launch
            .voice_identity_manager
            .list_identities_with_variant_counts()
            .unwrap_or_else(|error| panic!("identities must list: {error}"));
        drop(second_launch);
        let _ = std::fs::remove_dir_all(&test_directory);

        assert_eq!(first_response["identity_created"], true);
        assert_eq!(second_response["ok"], true);
        assert_eq!(
            second_response["identity_id"],
            first_response["identity_id"]
        );
        assert_eq!(second_response["identity_name"], "Grace Hopper");
        assert_eq!(second_response["identity_created"], false);
        assert_eq!(stored_identities.len(), 1);
        assert_eq!(stored_identities[0].0.display_name(), "Grace Hopper");
        assert_eq!(stored_identities[0].1, 4);
    }

    /// Verifies `assign_new_identity` stores the typed display name with the first variant.
    #[test]
    fn assign_new_identity_persists_the_display_name() {
        let (mut ffi_runtime, export_directory) = export_runtime();
        let meeting_id = MeetingId::new("ffi-new-identity-meeting");
        let clip_id = uuid::Uuid::new_v4();
        ffi_runtime
            .note_repository
            .create_meeting(&meeting_id, &[1])
            .unwrap_or_else(|error| panic!("fixture meeting must persist: {error}"));
        ffi_runtime
            .note_repository
            .save_unassigned_clip(UnassignedVoiceClip::with_review_context(
                clip_id,
                meeting_id.clone(),
                0,
                1_000,
                vec![7, 8],
                vec![1.0, 0.0],
                None,
            ))
            .unwrap_or_else(|error| panic!("fixture clip must persist: {error}"));
        std::fs::create_dir_all(&ffi_runtime.review_clip_directory)
            .unwrap_or_else(|error| panic!("review clip directory must be created: {error}"));
        let loaded_clips = ffi_runtime.get_unassigned_clips("ffi-new-identity-meeting");

        let status = ffi_runtime.assign_new_identity(&clip_id.to_string(), " Ada Lovelace ");
        let stored_identities = ffi_runtime
            .voice_identity_manager
            .list_identities_with_variant_counts()
            .unwrap_or_else(|error| panic!("identities must list: {error}"));
        let _ = std::fs::remove_dir_all(&export_directory);

        assert!(loaded_clips.is_ok());
        assert_eq!(status, FFI_SUCCESS);
        assert_eq!(stored_identities.len(), 1);
        assert_eq!(stored_identities[0].0.display_name(), "Ada Lovelace");
        assert_eq!(stored_identities[0].1, 1);
    }

    /// Verifies assignment failures are reported as structured error JSON.
    #[test]
    fn assign_cluster_reports_structured_errors() {
        let (mut ffi_runtime, export_directory) = export_runtime();
        let meeting_id = MeetingId::new("ffi-error-meeting");
        let (cluster_id, _) = seed_cluster(&ffi_runtime, &meeting_id);
        let cluster_text = cluster_id.to_string();

        let malformed = parse_response(&ffi_runtime.assign_cluster("not-a-uuid", "Grace"));
        let blank = parse_response(&ffi_runtime.assign_cluster(&cluster_text, "  "));
        let missing =
            parse_response(&ffi_runtime.assign_cluster(&uuid::Uuid::new_v4().to_string(), "Grace"));
        let first = parse_response(&ffi_runtime.assign_cluster(&cluster_text, "Grace"));
        let repeated = parse_response(&ffi_runtime.assign_cluster(&cluster_text, "Grace"));
        let _ = std::fs::remove_dir_all(&export_directory);

        for (response, expected_code) in [
            (&malformed, "invalid_argument"),
            (&blank, "invalid_argument"),
            (&missing, "cluster_not_found"),
            (&repeated, "cluster_already_resolved"),
        ] {
            assert_eq!(response["ok"], false, "{response}");
            assert_eq!(response["error_code"], expected_code, "{response}");
            assert!(response["error"].is_string(), "{response}");
        }
        assert_eq!(first["ok"], true);
    }

    /// Verifies a pipeline that cannot be built is reported with its own status, not a crash.
    #[test]
    fn start_reports_an_unavailable_pipeline() {
        let mut ffi_runtime = FfiRuntime::in_memory_for_tests(&std::env::temp_dir())
            .unwrap_or_else(|status| panic!("fixture runtime must initialize: status {status}"))
            .with_recording_controller_builder(Box::new(|| {
                Err(anyhow::anyhow!("no transcription engine is configured"))
            }));

        assert_eq!(ffi_runtime.start_recording(), FFI_PIPELINE_UNAVAILABLE);
        assert_eq!(ffi_runtime.current_recording_state(), FFI_AUDIO_STATE_IDLE);
    }

    /// Verifies PCM is rejected without a recording, and accepted once one is active.
    #[test]
    fn push_audio_requires_an_active_recording() {
        let mut ffi_runtime = FfiRuntime::in_memory_for_tests(&std::env::temp_dir())
            .unwrap_or_else(|status| panic!("fixture runtime must initialize: status {status}"));

        assert_eq!(
            ffi_runtime.push_audio_pcm(vec![0; 160]),
            FFI_OPERATION_REJECTED
        );
        assert_eq!(ffi_runtime.start_recording(), FFI_SUCCESS);
        assert_eq!(ffi_runtime.push_audio_pcm(vec![0; 160]), FFI_SUCCESS);
    }

    /// Verifies raw PCM pointers are validated before any sample is read.
    #[test]
    fn pcm_pointers_are_validated() {
        let samples = [1_i16, 2, 3];

        assert_eq!(unsafe { pcm_samples_from_raw(std::ptr::null(), 4) }, None);
        assert_eq!(
            unsafe { pcm_samples_from_raw(std::ptr::null(), 0) },
            Some(Vec::new())
        );
        assert_eq!(
            unsafe { pcm_samples_from_raw(samples.as_ptr(), samples.len()) },
            Some(vec![1, 2, 3])
        );
    }

    /// Verifies the full FFI flow: start for a meeting, push PCM, stop, and read the persisted
    /// segment through the runtime's own repository on the same database file.
    #[test]
    fn recorded_audio_is_transcribed_into_the_meeting() {
        let test_directory =
            std::env::temp_dir().join(format!("note-taker-ffi-pipeline-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&test_directory)
            .unwrap_or_else(|error| panic!("test directory must be created: {error}"));
        let mut ffi_runtime =
            FfiRuntime::open_for_tests(&test_directory.join("notes.sqlite3"), &test_directory)
                .unwrap_or_else(|status| {
                    panic!("fixture runtime must initialize: status {status}")
                });

        assert_eq!(
            ffi_runtime.start_recording_for(Some(MeetingId::new("ffi-live"))),
            FFI_SUCCESS
        );
        assert_eq!(ffi_runtime.active_meeting_id().as_deref(), Some("ffi-live"));
        assert_eq!(ffi_runtime.push_audio_pcm(vec![100; 32_000]), FFI_SUCCESS);
        assert_eq!(ffi_runtime.stop_recording(), FFI_SUCCESS);
        assert_eq!(ffi_runtime.active_meeting_id(), None);

        let segments = ffi_runtime
            .note_repository
            .load_transcript_segments(&MeetingId::new("ffi-live"))
            .unwrap_or_else(|error| panic!("segments must load: {error}"));
        drop(ffi_runtime);
        let _ = std::fs::remove_dir_all(&test_directory);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text(), "transcribed chunk");
        assert_eq!(
            (segments[0].start_time_ms(), segments[0].end_time_ms()),
            (0, 2_000)
        );
    }

    /// Verifies a rename with valid inputs succeeds and `get_meeting` then shows the trimmed title.
    #[test]
    fn rename_meeting_updates_the_title_in_meeting_json() {
        let (mut ffi_runtime, export_directory) = export_runtime();
        crate::application::meeting_crdt::MeetingCrdtService::new(&ffi_runtime.note_repository)
            .create_meeting(&MeetingId::new("ffi-rename"))
            .unwrap_or_else(|error| panic!("fixture meeting must persist: {error}"));

        let rename_status = ffi_runtime.rename_meeting("ffi-rename", "  Project Sync  ");
        let meeting = parse_response(&ffi_runtime.get_meeting("ffi-rename"));
        let _ = std::fs::remove_dir_all(&export_directory);

        assert_eq!(rename_status, FFI_SUCCESS);
        assert_eq!(meeting["title"], "Project Sync");
    }

    /// Verifies blank IDs and invalid titles return `FFI_INVALID_ARGUMENT`, an unknown meeting
    /// returns `FFI_MEETING_NOT_FOUND`, and none of them changes the stored title.
    #[test]
    fn rename_meeting_rejects_invalid_inputs_and_unknown_meetings() {
        let (mut ffi_runtime, export_directory) = export_runtime();
        crate::application::meeting_crdt::MeetingCrdtService::new(&ffi_runtime.note_repository)
            .create_meeting(&MeetingId::new("ffi-rename"))
            .unwrap_or_else(|error| panic!("fixture meeting must persist: {error}"));
        assert_eq!(
            ffi_runtime.rename_meeting("ffi-rename", "Original"),
            FFI_SUCCESS
        );
        let oversized_title = "x".repeat(MeetingTitle::MAX_CHARACTERS + 1);

        let invalid_statuses: Vec<_> = [
            ("ffi-rename", ""),
            ("ffi-rename", "   "),
            ("ffi-rename", "line one\nline two"),
            ("ffi-rename", "bell\u{0007}"),
            ("ffi-rename", oversized_title.as_str()),
            ("", "Valid"),
            ("   ", "Valid"),
        ]
        .into_iter()
        .map(|(meeting_id, title)| ffi_runtime.rename_meeting(meeting_id, title))
        .collect();
        let missing_status = ffi_runtime.rename_meeting("ffi-missing", "Orphan");
        let meeting = parse_response(&ffi_runtime.get_meeting("ffi-rename"));
        let missing_meeting = parse_response(&ffi_runtime.get_meeting("ffi-missing"));
        let _ = std::fs::remove_dir_all(&export_directory);

        assert!(
            invalid_statuses
                .iter()
                .all(|status| *status == FFI_INVALID_ARGUMENT),
            "{invalid_statuses:?}"
        );
        assert_eq!(missing_status, FFI_MEETING_NOT_FOUND);
        assert_eq!(meeting["title"], "Original");
        assert_eq!(missing_meeting["error_code"], "meeting_not_found");
    }

    /// Verifies null pointers are rejected before the runtime is touched.
    #[test]
    fn rename_meeting_rejects_null_pointers() {
        let meeting_id = std::ffi::CString::new("ffi-rename")
            .unwrap_or_else(|error| panic!("fixture C string must build: {error}"));
        let title = std::ffi::CString::new("Title")
            .unwrap_or_else(|error| panic!("fixture C string must build: {error}"));

        // SAFETY: Each pointer is null or a valid NUL-terminated string alive for the call.
        let statuses = unsafe {
            [
                note_taker_rename_meeting(std::ptr::null(), title.as_ptr()),
                note_taker_rename_meeting(meeting_id.as_ptr(), std::ptr::null()),
                note_taker_rename_meeting(std::ptr::null(), std::ptr::null()),
            ]
        };

        assert_eq!(statuses, [FFI_INVALID_ARGUMENT; 3]);
    }

    /// Verifies a titled start persists the title, and `get_meetings`/`get_meeting` return it
    /// alongside untitled meetings' dated fallback.
    /// A chosen language lasts until a stop, so the next recording uses the default again.
    #[test]
    fn a_stop_clears_the_chosen_language() {
        let test_directory =
            std::env::temp_dir().join(format!("ffi-language-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&test_directory)
            .unwrap_or_else(|error| panic!("test directory must be created: {error}"));
        let mut runtime =
            FfiRuntime::open_for_tests(&test_directory.join("notes.sqlite3"), &test_directory)
                .unwrap_or_else(|status| panic!("file runtime must open: status {status}"));
        let italian =
            crate::domain::transcription_language::TranscriptionLanguage::parse("it").ok();
        let italian = italian.unwrap_or_else(|| panic!("it is valid"));

        assert_eq!(runtime.set_recording_language(italian), FFI_SUCCESS);
        assert_eq!(runtime.recording_language, Some(italian));

        runtime.stop_recording();
        assert_eq!(runtime.recording_language, None);
        let _ = std::fs::remove_dir_all(&test_directory);
    }

    #[test]
    fn start_recording_with_title_persists_it_in_meeting_json() {
        let test_directory =
            std::env::temp_dir().join(format!("note-taker-ffi-title-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&test_directory)
            .unwrap_or_else(|error| panic!("test directory must be created: {error}"));
        let mut ffi_runtime =
            FfiRuntime::open_for_tests(&test_directory.join("notes.sqlite3"), &test_directory)
                .unwrap_or_else(|status| {
                    panic!("fixture runtime must initialize: status {status}")
                });
        ffi_runtime
            .note_repository
            .create_meeting(&MeetingId::new("ffi-untitled"), &[])
            .unwrap_or_else(|error| panic!("fixture meeting must persist: {error}"));
        let title = MeetingTitle::parse("Project Sync")
            .unwrap_or_else(|error| panic!("title must parse: {error}"));

        assert_eq!(
            ffi_runtime.start_recording_titled(Some(MeetingId::new("ffi-titled")), Some(title)),
            FFI_SUCCESS
        );
        assert_eq!(ffi_runtime.push_audio_pcm(vec![100; 32_000]), FFI_SUCCESS);
        assert_eq!(ffi_runtime.stop_recording(), FFI_SUCCESS);
        let meetings = parse_response(&ffi_runtime.get_meetings());
        let titled_meeting = parse_response(&ffi_runtime.get_meeting("ffi-titled"));
        let missing_meeting = parse_response(&ffi_runtime.get_meeting("ffi-missing"));
        drop(ffi_runtime);
        let _ = std::fs::remove_dir_all(&test_directory);

        let listed_titles: std::collections::HashMap<&str, &str> = meetings
            .as_array()
            .unwrap_or_else(|| panic!("meetings must be an array: {meetings}"))
            .iter()
            .filter_map(|meeting| {
                Some((meeting["meeting_id"].as_str()?, meeting["title"].as_str()?))
            })
            .collect();
        assert_eq!(listed_titles.get("ffi-titled"), Some(&"Project Sync"));
        assert!(
            listed_titles
                .get("ffi-untitled")
                .is_some_and(|title| title.starts_with("Meeting ")),
            "{meetings}"
        );
        assert!(meetings[0]["created_at"].is_string(), "{meetings}");
        assert_eq!(titled_meeting["meeting_id"], "ffi-titled");
        assert_eq!(titled_meeting["title"], "Project Sync");
        assert_eq!(titled_meeting["transcript"], serde_json::Value::Null);
        assert_eq!(titled_meeting["duration_ms"], 2_000);
        assert_eq!(
            titled_meeting["segments"],
            serde_json::json!([{
                "start_time_ms": 0,
                "end_time_ms": 2_000,
                "speaker": "Unattributed",
                "text": "transcribed chunk",
            }])
        );
        let untitled_summary = meetings
            .as_array()
            .and_then(|listed| {
                listed
                    .iter()
                    .find(|meeting| meeting["meeting_id"] == "ffi-untitled")
            })
            .unwrap_or_else(|| panic!("the untitled meeting must be listed: {meetings}"));
        assert_eq!(untitled_summary["duration_ms"], 0);
        assert_eq!(missing_meeting["ok"], false);
        assert_eq!(missing_meeting["error_code"], "meeting_not_found");
    }

    /// Verifies meeting detail segments name known speakers from the voice store and label
    /// identities the store no longer knows as unknown.
    #[test]
    fn meeting_detail_segments_resolve_speaker_names() {
        let (ffi_runtime, export_directory) = export_runtime();
        let meeting_id = MeetingId::new("ffi-speakers");
        let alice_identity_id = uuid::Uuid::from_u128(0xA11CE);
        ffi_runtime
            .note_repository
            .create_meeting(&meeting_id, &[1])
            .unwrap_or_else(|error| panic!("fixture meeting must persist: {error}"));
        ffi_runtime
            .voice_identity_manager
            .vector_database()
            .save_speaker_identity(&SpeakerIdentity::new(alice_identity_id, "Alice"))
            .unwrap_or_else(|error| panic!("fixture identity must persist: {error}"));
        ffi_runtime
            .note_repository
            .save_transcript_segments(
                &meeting_id,
                &[
                    TranscriptSegment::new(
                        meeting_id.clone(),
                        0,
                        1_500,
                        Some(alice_identity_id),
                        "Hello.",
                    ),
                    TranscriptSegment::new(
                        meeting_id.clone(),
                        1_500,
                        4_000,
                        Some(uuid::Uuid::from_u128(0xDEAD)),
                        "Hi.",
                    ),
                ],
            )
            .unwrap_or_else(|error| panic!("fixture segments must persist: {error}"));

        let meeting_detail = parse_response(&ffi_runtime.get_meeting("ffi-speakers"));
        let _ = std::fs::remove_dir_all(&export_directory);

        assert_eq!(meeting_detail["duration_ms"], 4_000);
        assert_eq!(meeting_detail["segments"][0]["speaker"], "Alice");
        assert_eq!(meeting_detail["segments"][1]["speaker"], "Unknown speaker");
        assert_eq!(meeting_detail["segments"][1]["text"], "Hi.");
    }

    /// Verifies files attached while a meeting is recorded carry its current meeting time, files
    /// attached afterwards go at the end, and the meeting JSON lists them (Issue #54).
    #[test]
    fn attached_files_are_timed_while_recording_and_listed_with_the_meeting() {
        let test_directory =
            std::env::temp_dir().join(format!("note-taker-ffi-attach-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&test_directory)
            .unwrap_or_else(|error| panic!("test directory must be created: {error}"));
        let mut ffi_runtime =
            FfiRuntime::open_for_tests(&test_directory.join("notes.sqlite3"), &test_directory)
                .unwrap_or_else(|status| {
                    panic!("fixture runtime must initialize: status {status}")
                });
        let source = |name: &str| {
            let path = test_directory.join(name);
            std::fs::write(&path, b"fixture")
                .unwrap_or_else(|error| panic!("fixture file must be written: {error}"));
            path.display().to_string()
        };
        let request = |paths: &[String]| serde_json::json!(paths).to_string();

        assert_eq!(
            ffi_runtime.start_recording_titled(Some(MeetingId::new("ffi-photos")), None),
            FFI_SUCCESS
        );
        assert_eq!(ffi_runtime.push_audio_pcm(vec![100; 32_000]), FFI_SUCCESS);
        let during = parse_response(
            &ffi_runtime.attach_files("ffi-photos", &request(&[source("whiteboard.jpg")])),
        );
        assert_eq!(ffi_runtime.stop_recording(), FFI_SUCCESS);
        let after = parse_response(&ffi_runtime.attach_files(
            "ffi-photos",
            &request(&[source("agenda.pdf"), source("virus.exe")]),
        ));
        let meeting = parse_response(&ffi_runtime.get_meeting("ffi-photos"));
        drop(ffi_runtime);
        let _ = std::fs::remove_dir_all(&test_directory);

        assert_eq!(during["ok"], true, "{during}");
        assert_eq!(during["attached"][0]["meeting_time_ms"], 2_000);
        assert_eq!(
            after["attached"].as_array().map(Vec::len),
            Some(1),
            "{after}"
        );
        assert_eq!(
            after["attached"][0]["meeting_time_ms"],
            serde_json::Value::Null
        );
        assert_eq!(after["failed"][0]["path"], source_name(&after, "virus.exe"));
        let attachments: Vec<(String, String, serde_json::Value)> = meeting["attachments"]
            .as_array()
            .unwrap_or_else(|| panic!("attachments must be listed: {meeting}"))
            .iter()
            .map(|attachment| {
                (
                    attachment["file_name"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                    attachment["mime_type"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                    attachment["meeting_time_ms"].clone(),
                )
            })
            .collect();
        assert_eq!(
            attachments,
            vec![
                (
                    String::from("whiteboard.jpg"),
                    String::from("image/jpeg"),
                    serde_json::json!(2_000)
                ),
                (
                    String::from("agenda.pdf"),
                    String::from("application/pdf"),
                    serde_json::Value::Null
                ),
            ]
        );
    }

    /// Returns the failed path in `response` that ends with `name`, so the test need not rebuild
    /// the temporary path.
    fn source_name(response: &serde_json::Value, name: &str) -> serde_json::Value {
        response["failed"]
            .as_array()
            .and_then(|failed| {
                failed.iter().find(|entry| {
                    entry["path"]
                        .as_str()
                        .is_some_and(|path| path.ends_with(name))
                })
            })
            .map(|entry| entry["path"].clone())
            .unwrap_or(serde_json::Value::Null)
    }

    /// Verifies blank meeting IDs, malformed or empty path lists and unknown meetings are
    /// rejected as a whole.
    #[test]
    fn attach_files_rejects_invalid_requests() {
        let (mut ffi_runtime, export_directory) = export_runtime();
        ffi_runtime
            .note_repository
            .create_meeting(&MeetingId::new("ffi-attach"), &[1])
            .unwrap_or_else(|error| panic!("fixture meeting must persist: {error}"));

        let error_codes: Vec<serde_json::Value> = [
            ("", r#"["/tmp/a.pdf"]"#),
            ("ffi-attach", "not json"),
            ("ffi-attach", r#"{"path":"/tmp/a.pdf"}"#),
            ("ffi-attach", "[]"),
            ("ffi-missing", r#"["/tmp/a.pdf"]"#),
        ]
        .into_iter()
        .map(|(meeting_id, paths_json)| {
            parse_response(&ffi_runtime.attach_files(meeting_id, paths_json))["error_code"].clone()
        })
        .collect();
        let _ = std::fs::remove_dir_all(&export_directory);

        assert_eq!(
            error_codes,
            vec![
                "invalid_argument",
                "invalid_argument",
                "invalid_argument",
                "invalid_argument",
                "meeting_not_found"
            ]
        );
    }

    /// Verifies the titled-start C arguments: null means absent, blank IDs and invalid titles are
    /// rejected, and titles are trimmed.
    #[test]
    fn titled_start_arguments_are_validated() {
        assert_eq!(titled_start_arguments(None, None), Ok((None, None)));
        assert_eq!(
            titled_start_arguments(Some(" standup "), Some(" Daily ")),
            Ok((
                Some(MeetingId::new("standup")),
                MeetingTitle::parse("Daily").ok()
            ))
        );
        assert_eq!(
            titled_start_arguments(Some("  "), None),
            Err(FFI_INVALID_ARGUMENT)
        );
        assert_eq!(
            titled_start_arguments(None, Some("\t")),
            Err(FFI_INVALID_ARGUMENT)
        );
    }

    /// Verifies the pipeline is built once and reused, and a start after stop is a new recording.
    #[test]
    fn the_pipeline_is_built_once_and_reused_across_recordings() {
        let build_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted_builds = std::sync::Arc::clone(&build_count);
        let mut ffi_runtime = FfiRuntime::in_memory_for_tests(&std::env::temp_dir())
            .unwrap_or_else(|status| panic!("fixture runtime must initialize: status {status}"))
            .with_recording_controller_builder(Box::new(move || {
                counted_builds.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                super::test_recording_controller(None)
            }));

        assert_eq!(ffi_runtime.start_recording(), FFI_SUCCESS);
        let first_meeting = ffi_runtime.active_meeting_id();
        assert_eq!(ffi_runtime.stop_recording(), FFI_SUCCESS);
        assert_eq!(
            ffi_runtime.current_recording_state(),
            FFI_AUDIO_STATE_STOPPED
        );
        assert_eq!(ffi_runtime.start_recording(), FFI_SUCCESS);
        assert_eq!(
            ffi_runtime.current_recording_state(),
            FFI_AUDIO_STATE_RECORDING
        );

        assert!(first_meeting.is_some());
        assert_ne!(ffi_runtime.active_meeting_id(), first_meeting);
        assert_eq!(build_count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    /// Verifies JSON string escaping covers control characters, which Linux paths may contain.
    #[test]
    fn json_escape_produces_valid_json_for_any_string() {
        for raw_value in [
            "plain",
            "quote \" and backslash \\",
            "new\nline\ttab\rreturn",
            "nul\u{0}bell\u{7}escape\u{1b}",
            "unicode 会议 🎉 \u{2028}",
        ] {
            let json_document = format!("{{\"value\":\"{}\"}}", super::json_escape(raw_value));
            let parsed: serde_json::Value = serde_json::from_str(&json_document)
                .unwrap_or_else(|error| panic!("{raw_value:?} must stay valid JSON: {error}"));
            assert_eq!(parsed["value"], raw_value);
        }
    }

    /// Builds a runtime the way `note_taker_configure` does, over files in a fresh directory.
    fn configured_runtime(p2p_enabled: bool) -> FfiRuntime {
        let data_directory =
            std::env::temp_dir().join(format!("note-taker-ffi-settings-{}", uuid::Uuid::new_v4()));
        let settings = crate::ffi_settings::ConfiguredSettings::from_json(&format!(
            r#"{{"data_directory":"{}","device_name":"Test phone","keychain_enabled":false,"p2p_enabled":{p2p_enabled}}}"#,
            data_directory.display()
        ))
        .unwrap_or_else(|error| panic!("settings must parse: {error}"));
        let lookup = move |name: &str| {
            settings.lookup(name, |name| {
                (name == "NOTE_TAKER_P2P_LISTEN").then(|| "/ip4/127.0.0.1/tcp/0".to_owned())
            })
        };
        FfiRuntime::from_settings(&lookup)
            .unwrap_or_else(|status| panic!("configured runtime must open: status {status}"))
    }

    /// A configured runtime keeps its database in the data directory and starts P2P under the
    /// configured device name; with P2P disabled it starts none.
    #[test]
    fn a_configured_runtime_starts_p2p_under_its_device_name() {
        let with_p2p = configured_runtime(true);
        let without_p2p = configured_runtime(false);
        let status = parse_response(&crate::ffi_p2p::p2p_status_json(
            with_p2p
                .p2p
                .as_deref()
                .map(|handle| handle as &dyn crate::application::p2p_service::P2pControl),
        ));
        if let Some(handle) = &with_p2p.p2p {
            handle.shutdown();
        }

        assert_eq!(status["enabled"], true, "{status}");
        assert_eq!(status["device_name"], "Test phone");
        assert!(without_p2p.p2p.is_none());
    }

    /// `note_taker_configure` refuses a null pointer and invalid JSON without storing anything.
    #[test]
    fn configure_refuses_invalid_input() {
        let invalid_json = std::ffi::CString::new(r#"{"data_directory":"relative"}"#)
            .unwrap_or_else(|error| panic!("fixture must be a C string: {error}"));

        // SAFETY: null is accepted by contract, and `invalid_json` is a valid C string.
        let (null_status, invalid_status) = unsafe {
            (
                super::note_taker_configure(std::ptr::null()),
                super::note_taker_configure(invalid_json.as_ptr()),
            )
        };

        assert_eq!(null_status, FFI_INVALID_ARGUMENT);
        assert_eq!(invalid_status, FFI_INVALID_ARGUMENT);
    }

    /// A keychain that is missing, locked, or refused gets its own status, so the app can offer
    /// a retry; every other failure to open the stores stays `FFI_OPERATION_REJECTED`.
    #[test]
    fn a_keychain_failure_opening_the_stores_has_its_own_status() {
        use crate::domain::errors::CryptoError;
        let unavailable = anyhow::Error::new(CryptoError::KeychainUnavailable {
            reason: "no Secret Service".to_owned(),
        })
        .context("unable to unlock the voice embedding encryption key");
        let refused = anyhow::Error::new(CryptoError::Keychain {
            reason: "the unlock prompt was dismissed".to_owned(),
        })
        .context("unable to unlock the voice embedding encryption key");
        let wrong_key = anyhow::Error::new(CryptoError::WrongKey).context("unable to unlock");
        let other = anyhow::anyhow!("unable to open the voice store");

        assert_eq!(
            super::store_open_status(&unavailable),
            super::FFI_KEYCHAIN_UNAVAILABLE
        );
        assert_eq!(
            super::store_open_status(&refused),
            super::FFI_KEYCHAIN_UNAVAILABLE
        );
        assert_eq!(super::store_open_status(&wrong_key), FFI_OPERATION_REJECTED);
        assert_eq!(super::store_open_status(&other), FFI_OPERATION_REJECTED);
    }

    /// Serializes the tests that write the process-wide host key, so read-backs see their own.
    static HOST_KEY_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// `note_taker_provide_encryption_key` accepts exactly 32 bytes and refuses anything else.
    #[test]
    fn provide_encryption_key_checks_its_input() {
        let _serialized = HOST_KEY_TEST_LOCK.lock();
        let short_key = [1_u8; 16];
        let key = [2_u8; 32];

        // SAFETY: null is accepted by contract; the arrays are valid for the given lengths.
        let (null_status, short_status, key_status) = unsafe {
            (
                super::note_taker_provide_encryption_key(std::ptr::null(), 32),
                super::note_taker_provide_encryption_key(short_key.as_ptr(), short_key.len()),
                super::note_taker_provide_encryption_key(key.as_ptr(), key.len()),
            )
        };

        assert_eq!(null_status, FFI_INVALID_ARGUMENT);
        assert_eq!(short_status, FFI_INVALID_ARGUMENT);
        assert_eq!(key_status, FFI_SUCCESS);
    }

    /// The store behind both the C and the JNI entry points keeps exactly the bytes it is given
    /// and refuses any other length.
    #[test]
    fn storing_a_host_key_keeps_exactly_32_bytes() {
        let _serialized = HOST_KEY_TEST_LOCK.lock();
        let short_status = super::store_host_encryption_key(&[4_u8; 31]);
        let key_status = super::store_host_encryption_key(&[5_u8; 32]);

        assert_eq!(short_status, FFI_INVALID_ARGUMENT);
        assert_eq!(key_status, FFI_SUCCESS);
        assert_eq!(super::provided_encryption_key(), Some(vec![5_u8; 32]));
    }

    /// The host's key is used when provided; Android without one gets an unavailable keychain
    /// (status 6), never the `keyring` crate, which has no Android store.
    #[test]
    fn the_runtime_keychain_prefers_the_host_key() {
        let provided = super::runtime_keychain(Some(vec![3; 32]), true);
        let android_without_key = super::runtime_keychain(None, true);
        let desktop = super::runtime_keychain(None, false);

        assert_eq!(provided.location(), "Android Keystore");
        assert!(provided.load_key().is_ok_and(|key| key.is_some()));
        assert!(matches!(
            android_without_key.load_key(),
            Err(crate::domain::errors::CryptoError::KeychainUnavailable { .. })
        ));
        assert_ne!(desktop.location(), "Android Keystore");
    }

    /// `note_taker_start_runtime` opens the configured runtime at once, creating its database in
    /// the data directory, and is idempotent; configuring afterwards is refused. This is the only
    /// test that opens the process-wide runtime, so it cannot disturb the others.
    #[test]
    fn start_runtime_opens_the_configured_database_once() {
        let data_directory =
            std::env::temp_dir().join(format!("note-taker-ffi-start-{}", uuid::Uuid::new_v4()));
        let configuration = std::ffi::CString::new(format!(
            r#"{{"data_directory":"{}","device_name":"Test phone","keychain_enabled":false,"p2p_enabled":false}}"#,
            data_directory.display()
        ))
        .unwrap_or_else(|error| panic!("fixture must be a C string: {error}"));

        // SAFETY: `configuration` is a valid C string for both calls.
        let configure_status = unsafe { super::note_taker_configure(configuration.as_ptr()) };
        let first_start = super::note_taker_start_runtime();
        let second_start = super::note_taker_start_runtime();
        // SAFETY: as above.
        let late_configure = unsafe { super::note_taker_configure(configuration.as_ptr()) };

        assert_eq!(configure_status, FFI_SUCCESS);
        assert_eq!(first_start, FFI_SUCCESS);
        assert_eq!(second_start, FFI_SUCCESS);
        assert!(data_directory.join("note_taker.sqlite3").exists());
        assert_eq!(late_configure, FFI_OPERATION_REJECTED);
    }
}
