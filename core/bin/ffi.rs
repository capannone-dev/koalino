//! Exposes a C-compatible recording and logging boundary for Flutter without sharing Rust structs.

use std::{
    collections::HashMap,
    env,
    ffi::{CStr, CString, c_char, c_int},
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use crate::{
    adapters::{
        audio::local_recorder::LocalRecorder,
        export::{default_export_directory, default_exporters},
        media::local_file_metadata::LocalFileMetadataExtractor,
        observability::{LoggingConfiguration, LoggingRuntime, initialize_logging},
        storage::{
            in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
        },
    },
    application::{
        attachment_manager::AttachmentManager,
        export_manager::{ExportManager, ExportWriter},
        voice_identity_manager::VoiceIdentityManager,
    },
    domain::{
        errors::{ExportError, StorageError},
        models::{
            Attachment, AudioConfig, DeviceId, ExportFormat, ExportOptions, MeetingId,
            MeetingMetadata, MeetingWindow, TaskMetrics, TaskType, Transcript, TranscriptSegment,
            UnassignedVoiceClip, UnknownSpeakerChunk, UnknownSpeakerCluster,
        },
    },
    ports::{
        audio::AudioCapture,
        media::MetadataExtractor,
        storage::{AttachmentRepository, NoteRepository},
        telemetry::MetricsRepository,
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

/// Stores adapter-owned state required by the primitive Flutter C ABI commands.
struct FfiRuntime {
    local_recorder: LocalRecorder,
    logging_runtime: Option<LoggingRuntime>,
    note_repository: SqliteNoteRepository,
    voice_identity_manager: VoiceIdentityManager<InMemoryVoiceVectorDB>,
    review_clip_directory: PathBuf,
    loaded_review_clips: HashMap<Uuid, UnassignedVoiceClip>,
    pending_attachment_imports: HashMap<Uuid, PendingAttachmentImport>,
    export_writer: ExportWriter,
}

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

/// Builds the JSON error object returned by `note_taker_export_meeting`.
///
/// Inputs: A stable machine-readable `error_code` and a human-readable `message`.
/// Outputs: `{"ok":false,"error_code":…,"error":…}` as a string.
/// Side effects: None.
fn export_error_json(error_code: &str, message: &str) -> String {
    serde_json::json!({ "ok": false, "error_code": error_code, "error": message }).to_string()
}

/// Retains only metadata necessary to persist one explicitly user-approved attachment link.
struct PendingAttachmentImport {
    file_path: String,
    original_timestamp: chrono::DateTime<chrono::Utc>,
}

/// Borrows the SQLite adapter for one `AttachmentManager` invocation without moving FFI state.
impl NoteRepository for &SqliteNoteRepository {
    /// Forwards meeting creation to the borrowed SQLite adapter.
    ///
    /// Inputs: Match `NoteRepository::create_meeting`.
    /// Outputs: The adapter's portable result.
    /// Side effects: Delegated to the SQLite adapter.
    fn create_meeting(
        &self,
        meeting_id: &MeetingId,
        initial_crdt_document: &[u8],
    ) -> Result<(), StorageError> {
        (*self).create_meeting(meeting_id, initial_crdt_document)
    }

    /// Forwards transcript persistence to the borrowed SQLite adapter.
    ///
    /// Inputs: Match `NoteRepository::save_transcript`.
    /// Outputs: The adapter's portable result.
    /// Side effects: Delegated to the SQLite adapter.
    fn save_transcript(&self, transcript: &Transcript) -> Result<(), StorageError> {
        (*self).save_transcript(transcript)
    }

    /// Forwards transcript retrieval to the borrowed SQLite adapter.
    ///
    /// Inputs: Match `NoteRepository::load_transcript`.
    /// Outputs: The adapter's portable result.
    /// Side effects: Delegated to the SQLite adapter.
    fn load_transcript(&self, meeting_id: &MeetingId) -> Result<Option<Transcript>, StorageError> {
        (*self).load_transcript(meeting_id)
    }

    /// Forwards CRDT persistence to the borrowed SQLite adapter.
    ///
    /// Inputs: Match `NoteRepository::save_crdt_document`.
    /// Outputs: The adapter's portable result.
    /// Side effects: Delegated to the SQLite adapter.
    fn save_crdt_document(
        &self,
        meeting_id: &MeetingId,
        crdt_document: &[u8],
    ) -> Result<(), StorageError> {
        (*self).save_crdt_document(meeting_id, crdt_document)
    }

    /// Forwards CRDT retrieval to the borrowed SQLite adapter.
    ///
    /// Inputs: Match `NoteRepository::load_crdt_document`.
    /// Outputs: The adapter's portable result.
    /// Side effects: Delegated to the SQLite adapter.
    fn load_crdt_document(&self, meeting_id: &MeetingId) -> Result<Vec<u8>, StorageError> {
        (*self).load_crdt_document(meeting_id)
    }

    /// Forwards temporary review-clip persistence to the borrowed SQLite adapter.
    ///
    /// Inputs: Match `NoteRepository::save_unassigned_clip`.
    /// Outputs: The adapter's portable result.
    /// Side effects: Delegated to the SQLite adapter.
    fn save_unassigned_clip(&self, clip: UnassignedVoiceClip) -> Result<(), StorageError> {
        (*self).save_unassigned_clip(clip)
    }

    /// Forwards temporary review-clip retrieval to the borrowed SQLite adapter.
    ///
    /// Inputs: Match `NoteRepository::get_unassigned_clips`.
    /// Outputs: The adapter's portable result.
    /// Side effects: Delegated to the SQLite adapter.
    fn get_unassigned_clips(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Vec<UnassignedVoiceClip>, StorageError> {
        (*self).get_unassigned_clips(meeting_id)
    }

    /// Forwards timeline lookups to the borrowed SQLite adapter.
    ///
    /// Inputs: Match `NoteRepository::get_meetings_overlapping`.
    /// Outputs: The adapter's portable result.
    /// Side effects: Delegated to the SQLite adapter.
    fn get_meetings_overlapping(
        &self,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<MeetingWindow>, StorageError> {
        (*self).get_meetings_overlapping(timestamp)
    }

    /// Forwards meeting metadata retrieval to the borrowed SQLite adapter.
    ///
    /// Inputs: Match `NoteRepository::load_meeting_metadata`.
    /// Outputs: The adapter's portable result.
    /// Side effects: Delegated to the SQLite adapter.
    fn load_meeting_metadata(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Option<MeetingMetadata>, StorageError> {
        (*self).load_meeting_metadata(meeting_id)
    }

    /// Forwards transcript segment persistence to the borrowed SQLite adapter.
    ///
    /// Inputs: Match `NoteRepository::save_transcript_segments`.
    /// Outputs: The adapter's portable result.
    /// Side effects: Delegated to the SQLite adapter.
    fn save_transcript_segments(
        &self,
        meeting_id: &MeetingId,
        segments: &[TranscriptSegment],
    ) -> Result<(), StorageError> {
        (*self).save_transcript_segments(meeting_id, segments)
    }

    /// Forwards transcript segment retrieval to the borrowed SQLite adapter.
    ///
    /// Inputs: Match `NoteRepository::load_transcript_segments`.
    /// Outputs: The adapter's portable result.
    /// Side effects: Delegated to the SQLite adapter.
    fn load_transcript_segments(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Vec<TranscriptSegment>, StorageError> {
        (*self).load_transcript_segments(meeting_id)
    }

    /// Forwards unknown-speaker cluster persistence to the borrowed SQLite adapter.
    ///
    /// Inputs: Match `NoteRepository::save_unknown_speaker_cluster_chunk`.
    /// Outputs: The adapter's portable result.
    /// Side effects: Delegated to the SQLite adapter.
    fn save_unknown_speaker_cluster_chunk(
        &self,
        cluster: &UnknownSpeakerCluster,
        chunk: &UnknownSpeakerChunk,
    ) -> Result<(), StorageError> {
        (*self).save_unknown_speaker_cluster_chunk(cluster, chunk)
    }

    /// Forwards meeting cluster retrieval to the borrowed SQLite adapter.
    ///
    /// Inputs: Match `NoteRepository::load_unknown_speaker_clusters`.
    /// Outputs: The adapter's portable result.
    /// Side effects: Delegated to the SQLite adapter.
    fn load_unknown_speaker_clusters(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Vec<UnknownSpeakerCluster>, StorageError> {
        (*self).load_unknown_speaker_clusters(meeting_id)
    }

    /// Forwards single-cluster retrieval to the borrowed SQLite adapter.
    ///
    /// Inputs: Match `NoteRepository::load_unknown_speaker_cluster`.
    /// Outputs: The adapter's portable result.
    /// Side effects: Delegated to the SQLite adapter.
    fn load_unknown_speaker_cluster(
        &self,
        cluster_id: Uuid,
    ) -> Result<Option<UnknownSpeakerCluster>, StorageError> {
        (*self).load_unknown_speaker_cluster(cluster_id)
    }

    /// Forwards cluster member retrieval to the borrowed SQLite adapter.
    ///
    /// Inputs: Match `NoteRepository::load_unknown_speaker_chunks`.
    /// Outputs: The adapter's portable result.
    /// Side effects: Delegated to the SQLite adapter.
    fn load_unknown_speaker_chunks(
        &self,
        cluster_id: Uuid,
    ) -> Result<Vec<UnknownSpeakerChunk>, StorageError> {
        (*self).load_unknown_speaker_chunks(cluster_id)
    }

    /// Forwards cluster resolution to the borrowed SQLite adapter.
    ///
    /// Inputs: Match `NoteRepository::resolve_unknown_speaker_cluster`.
    /// Outputs: The adapter's portable result.
    /// Side effects: Delegated to the SQLite adapter.
    fn resolve_unknown_speaker_cluster(
        &self,
        cluster_id: Uuid,
        identity_id: Uuid,
    ) -> Result<(), StorageError> {
        (*self).resolve_unknown_speaker_cluster(cluster_id, identity_id)
    }
}

impl FfiRuntime {
    /// Creates the adapter-owned state for one process-local Flutter bridge.
    ///
    /// Inputs: None; the portable 16 kHz mono capture format is fixed for this initial bridge.
    /// Outputs: A ready runtime or an audio configuration error encoded as a C status by callers.
    /// Side effects: Allocates an in-memory recorder; it does not open an OS device or capture audio.
    fn new() -> Result<Self, c_int> {
        let local_recorder =
            LocalRecorder::new(AudioConfig::new(16_000, 1)).map_err(|_| FFI_OPERATION_REJECTED)?;
        let database_path = env::var("NOTE_TAKER_DATABASE_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("note_taker.sqlite3"));
        let device_id = DeviceId::new(
            env::var("NOTE_TAKER_DEVICE_ID")
                .unwrap_or_else(|_| String::from("unconfigured-device")),
        );
        let note_repository = SqliteNoteRepository::open(database_path, device_id)
            .map_err(|_| FFI_OPERATION_REJECTED)?;
        let review_clip_directory = env::var("NOTE_TAKER_REVIEW_CLIP_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("voice_review_clips"));
        fs::create_dir_all(&review_clip_directory).map_err(|_| FFI_OPERATION_REJECTED)?;
        Ok(Self::from_parts(
            local_recorder,
            note_repository,
            review_clip_directory,
            ExportWriter::new(default_exporters(), default_export_directory()),
        ))
    }

    /// Assembles runtime state from already-opened adapters.
    ///
    /// Inputs: The recorder, repository, review-clip directory, and export writer.
    /// Outputs: The runtime with empty caches and an in-memory voice store.
    /// Side effects: None.
    fn from_parts(
        local_recorder: LocalRecorder,
        note_repository: SqliteNoteRepository,
        review_clip_directory: PathBuf,
        export_writer: ExportWriter,
    ) -> Self {
        Self {
            local_recorder,
            logging_runtime: None,
            note_repository,
            voice_identity_manager: VoiceIdentityManager::new(InMemoryVoiceVectorDB::new()),
            review_clip_directory,
            loaded_review_clips: HashMap::new(),
            pending_attachment_imports: HashMap::new(),
            export_writer,
        }
    }

    /// Creates a runtime over in-memory SQLite that exports into `export_directory`.
    ///
    /// Inputs: The directory for generated export filenames.
    /// Outputs: A runtime that never touches the developer database, or a C status.
    /// Side effects: Allocates in-memory SQLite; creates no directories.
    #[cfg(test)]
    fn in_memory_for_tests(export_directory: &Path) -> Result<Self, c_int> {
        let local_recorder =
            LocalRecorder::new(AudioConfig::new(16_000, 1)).map_err(|_| FFI_OPERATION_REJECTED)?;
        let note_repository =
            SqliteNoteRepository::open_in_memory().map_err(|_| FFI_OPERATION_REJECTED)?;
        Ok(Self::from_parts(
            local_recorder,
            note_repository,
            export_directory.join("review-clips"),
            ExportWriter::new(default_exporters(), export_directory),
        ))
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
    /// Outputs: The configured local device identifier or the documented fallback value.
    /// Side effects: Reads one environment variable.
    fn note_repository_device_id(&self) -> String {
        env::var("NOTE_TAKER_DEVICE_ID").unwrap_or_else(|_| String::from("unconfigured-device"))
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

    /// Creates a fresh identity UUID and delegates its initial signature to the existing manager.
    ///
    /// Inputs: `clip_id` must be loaded and `display_name` must be non-empty user-facing text.
    /// Outputs: A C status indicating whether a signature variant was persisted.
    /// Side effects: Stores one derived voice-signature variant; the display name remains UI metadata.
    fn assign_new_identity(&mut self, clip_id: &str, display_name: &str) -> c_int {
        if display_name.trim().is_empty() {
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
        self.voice_identity_manager
            .confirm_weak_match(Uuid::new_v4(), clip.derived_embedding())
            .map(|_| FFI_SUCCESS)
            .unwrap_or(FFI_OPERATION_REJECTED)
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

    /// Starts the recorder through the portable audio-capture port.
    ///
    /// Inputs: None.
    /// Outputs: A C status indicating whether the state transition was accepted.
    /// Side effects: Transitions the Rust-owned recorder to recording and emits its safe state log.
    fn start_recording(&mut self) -> c_int {
        self.local_recorder
            .start()
            .map(|()| FFI_SUCCESS)
            .unwrap_or(FFI_OPERATION_REJECTED)
    }

    /// Pauses the recorder through the shared portable audio-capture state machine.
    ///
    /// Inputs: None.
    /// Outputs: A C status indicating whether an active recording accepted the pause.
    /// Side effects: Stops local chunk routing while preserving a session for resume.
    fn pause_recording(&mut self) -> c_int {
        self.local_recorder
            .pause()
            .map(|()| FFI_SUCCESS)
            .unwrap_or(FFI_OPERATION_REJECTED)
    }

    /// Stops the recorder through the shared portable audio-capture state machine.
    ///
    /// Inputs: None.
    /// Outputs: A C status indicating whether the session accepted its terminal stop.
    /// Side effects: Permanently stops chunk routing for this runtime's recorder instance.
    fn stop_recording(&mut self) -> c_int {
        self.local_recorder
            .stop()
            .map(|()| FFI_SUCCESS)
            .unwrap_or(FFI_OPERATION_REJECTED)
    }

    /// Converts a platform-reported OS suspension into a safe recoverable recorder state.
    ///
    /// Inputs: None.
    /// Outputs: Success after a recording enters Interrupted; rejection for inactive states.
    /// Side effects: Stops chunk routing while retaining the recorder for a later start/resume.
    fn handle_audio_interruption(&mut self) -> c_int {
        match self.local_recorder.handle_interruption() {
            Err(crate::domain::errors::AudioError::Interrupted) => FFI_SUCCESS,
            Ok(()) | Err(_) => FFI_OPERATION_REJECTED,
        }
    }

    /// Reads the current recorder lifecycle state without exposing Rust state values over FFI.
    ///
    /// Inputs: None.
    /// Outputs: One documented `FFI_AUDIO_STATE_*` integer.
    /// Side effects: None; the runtime mutex serializes this read with lifecycle mutations.
    fn current_recording_state(&self) -> c_int {
        audio_state_code(self.local_recorder.state())
    }

    /// Updates tracing verbosity while retaining the logging worker for the process lifetime.
    ///
    /// Inputs: `filter_directive` is a valid non-empty tracing filter directive.
    /// Outputs: A C status indicating whether the filter could be installed.
    /// Side effects: Lazily installs JSON stdout/rolling-file logging and reloads its filter.
    fn set_log_level(&mut self, filter_directive: &str) -> c_int {
        if filter_directive.is_empty() {
            return FFI_INVALID_ARGUMENT;
        }

        if self.logging_runtime.is_none() {
            let device_id = DeviceId::new(
                env::var("NOTE_TAKER_DEVICE_ID")
                    .unwrap_or_else(|_| String::from("unconfigured-device")),
            );
            let log_directory = env::var("NOTE_TAKER_LOG_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("logs"));
            let logging_configuration =
                LoggingConfiguration::new(device_id, log_directory).with_filter(filter_directive);
            let logging_runtime = match initialize_logging(logging_configuration) {
                Ok(logging_runtime) => logging_runtime,
                Err(_) => return FFI_OPERATION_REJECTED,
            };
            self.logging_runtime = Some(logging_runtime);
        }

        match &self.logging_runtime {
            Some(logging_runtime) => logging_runtime
                .set_filter(filter_directive)
                .map(|()| FFI_SUCCESS)
                .unwrap_or(FFI_OPERATION_REJECTED),
            None => FFI_RUNTIME_UNAVAILABLE,
        }
    }
}

/// Escapes UTF-8 text for the narrow JSON response without accepting arbitrary JSON values.
///
/// Inputs: `value` is text emitted in a JSON string field.
/// Outputs: JSON string content with required quote and backslash escapes.
/// Side effects: None.
fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Escapes a platform path for the narrow JSON response without accepting arbitrary JSON values.
///
/// Inputs: `path` is an absolute local file-system path.
/// Outputs: JSON string content with required quote and backslash escapes.
/// Side effects: None.
fn json_escape_path(path: &Path) -> String {
    json_escape(&path.to_string_lossy())
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

/// Reports an operating-system audio stream suspension from a background service thread.
///
/// Inputs: None.
/// Outputs: `FFI_SUCCESS` once a recording has entered recoverable Interrupted state.
/// Side effects: Acquires the process runtime mutex and stops routing audio until resume.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_handle_audio_interruption() -> c_int {
    with_runtime(FfiRuntime::handle_audio_interruption)
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

/// Applies a Rust tracing filter supplied by a Flutter caller.
///
/// Inputs: `filter_directive` must point to a NUL-terminated UTF-8 C string for this call's duration.
/// Outputs: `FFI_SUCCESS` on success or a documented nonzero C status.
/// Side effects: May initialize local JSON logging and reload its verbosity without restarting Rust.
///
/// # Safety
/// The caller must pass either a null pointer or a valid NUL-terminated C string. The pointer is
/// read only for the duration of this call and is never retained.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_set_log_level(filter_directive: *const c_char) -> c_int {
    if filter_directive.is_null() {
        return FFI_INVALID_ARGUMENT;
    }

    let filter_directive = match unsafe { CStr::from_ptr(filter_directive) }.to_str() {
        Ok(filter_directive) => filter_directive,
        Err(_) => return FFI_INVALID_ARGUMENT,
    };
    with_runtime(|runtime| runtime.set_log_level(filter_directive))
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

#[cfg(test)]
mod ffi_contract_tests {
    use super::{
        FFI_AUDIO_STATE_INTERRUPTED, FFI_AUDIO_STATE_PAUSED, FFI_AUDIO_STATE_RECORDING,
        FFI_AUDIO_STATE_STOPPED, FFI_INVALID_ARGUMENT, FFI_OPERATION_REJECTED, FFI_SUCCESS,
        FfiRuntime,
    };
    use crate::{
        domain::models::{DeviceId, MeetingId, TaskMetrics, TaskType, TranscriptSegment},
        ports::{storage::NoteRepository, telemetry::MetricsRepository},
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
        let ffi_runtime = FfiRuntime::new()
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
        let mut ffi_runtime = FfiRuntime::new()
            .unwrap_or_else(|status| panic!("fixture runtime must initialize: status {status}"));

        assert_eq!(ffi_runtime.start_recording(), FFI_SUCCESS);
    }

    /// Verifies that the bridge preserves the audio port's terminal duplicate-start rejection.
    #[test]
    fn rejects_a_second_start_without_exposing_the_rust_audio_error() {
        let mut ffi_runtime = FfiRuntime::new()
            .unwrap_or_else(|status| panic!("fixture runtime must initialize: status {status}"));

        assert_eq!(ffi_runtime.start_recording(), FFI_SUCCESS);
        assert_eq!(ffi_runtime.start_recording(), FFI_OPERATION_REJECTED);
    }

    /// Verifies lifecycle commands share one serialized runtime and expose portable state codes.
    #[test]
    fn background_lifecycle_commands_are_serializable_and_report_portable_state() {
        let mut ffi_runtime = FfiRuntime::new()
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

    /// Verifies that an empty logging directive is rejected before logging initialization.
    #[test]
    fn rejects_an_empty_logging_directive() {
        let mut ffi_runtime = FfiRuntime::new()
            .unwrap_or_else(|status| panic!("fixture runtime must initialize: status {status}"));

        assert_eq!(ffi_runtime.set_log_level(""), FFI_INVALID_ARGUMENT);
    }
}
