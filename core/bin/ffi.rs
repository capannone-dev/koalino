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
        observability::{LoggingConfiguration, LoggingRuntime, initialize_logging},
        storage::{
            in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
        },
    },
    application::voice_identity_manager::VoiceIdentityManager,
    domain::models::{AudioConfig, DeviceId, MeetingId, UnassignedVoiceClip},
    ports::{audio::AudioCapture, storage::NoteRepository},
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

/// Stores adapter-owned state required by the primitive Flutter C ABI commands.
struct FfiRuntime {
    local_recorder: LocalRecorder,
    logging_runtime: Option<LoggingRuntime>,
    note_repository: SqliteNoteRepository,
    voice_identity_manager: VoiceIdentityManager<InMemoryVoiceVectorDB>,
    review_clip_directory: PathBuf,
    loaded_review_clips: HashMap<Uuid, UnassignedVoiceClip>,
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
        Ok(Self {
            local_recorder,
            logging_runtime: None,
            note_repository,
            voice_identity_manager: VoiceIdentityManager::new(InMemoryVoiceVectorDB::new()),
            review_clip_directory,
            loaded_review_clips: HashMap::new(),
        })
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

/// Escapes a platform path for the narrow JSON response without accepting arbitrary JSON values.
///
/// Inputs: `path` is an absolute local file-system path.
/// Outputs: JSON string content with required quote and backslash escapes.
/// Side effects: None.
fn json_escape_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
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
    use super::{FFI_INVALID_ARGUMENT, FFI_OPERATION_REJECTED, FFI_SUCCESS, FfiRuntime};

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

    /// Verifies that an empty logging directive is rejected before logging initialization.
    #[test]
    fn rejects_an_empty_logging_directive() {
        let mut ffi_runtime = FfiRuntime::new()
            .unwrap_or_else(|status| panic!("fixture runtime must initialize: status {status}"));

        assert_eq!(ffi_runtime.set_log_level(""), FFI_INVALID_ARGUMENT);
    }
}
