//! Exposes a C-compatible recording and logging boundary for Flutter without sharing Rust structs.

use std::{
    env,
    ffi::{CStr, c_char, c_int},
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

use crate::{
    adapters::{
        audio::local_recorder::LocalRecorder,
        observability::{LoggingConfiguration, LoggingRuntime, initialize_logging},
    },
    domain::models::{AudioConfig, DeviceId},
    ports::audio::AudioCapture,
};

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
        Ok(Self {
            local_recorder,
            logging_runtime: None,
        })
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
