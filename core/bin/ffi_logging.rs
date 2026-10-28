//! The app's log level and log export over the C ABI (REQUIREMENTS 6.1, 6.2; Issue #46).
//!
//! The Flutter app writes the same redacted JSON log files as the daemon, into `<data>/logs`.
//! Logging starts when the runtime is first opened, at the level the owner chose in Settings:
//! that choice is stored in `<db>.log-level` (the file `cli log-level` writes for the daemon), so
//! it survives restarts. The logging runtime lives in a process-wide cell rather than in
//! `FfiRuntime`, because the global subscriber can be installed only once while opening the
//! runtime can be retried (a locked keychain), and because the level can be changed before the
//! runtime opens.

use std::{
    ffi::{CString, c_char, c_int},
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::{
    adapters::{
        export::default_export_directory,
        observability::{
            LogExportError, LogLevelControlFile, LoggingConfiguration, LoggingRuntime,
            export_log_files, initialize_logging,
        },
    },
    device_identity::{DEVICE_ID_VARIABLE, resolve_device_id},
    ffi::{FFI_INVALID_ARGUMENT, FFI_OPERATION_REJECTED, FFI_SUCCESS},
    ffi_settings::app_setting,
};

/// The level a fresh install logs at (REQUIREMENTS 6.1).
pub const DEFAULT_LOG_LEVEL: &str = "info";

/// The process's logging runtime, once started.
static APP_LOGGING: Mutex<Option<LoggingRuntime>> = Mutex::new(None);

/// Where the app's logs, level, and log exports live.
#[derive(Clone, Debug)]
pub struct AppLogSettings {
    database_path: PathBuf,
    log_directory: PathBuf,
    export_directory: PathBuf,
    control_file: LogLevelControlFile,
}

impl AppLogSettings {
    /// Resolves the paths from the app's settings.
    ///
    /// Inputs: `setting` resolves `NOTE_TAKER_*` names (`app_setting` in production). Missing
    /// values fall back like the daemon's: `note_taker.sqlite3`, `logs`, and the platform's export
    /// folder. Outputs: The settings. Side effects: None.
    pub fn from_lookup(setting: &dyn Fn(&str) -> Option<String>) -> Self {
        let database_path = setting("NOTE_TAKER_DATABASE_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("note_taker.sqlite3"));
        let log_directory = setting("NOTE_TAKER_LOG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("logs"));
        let export_directory = setting("NOTE_TAKER_EXPORT_DIR")
            .filter(|directory| !directory.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(default_export_directory);
        Self {
            control_file: LogLevelControlFile::for_database(&database_path),
            database_path,
            log_directory,
            export_directory,
        }
    }

    /// The level in effect: the stored choice, or `DEFAULT_LOG_LEVEL`.
    ///
    /// Inputs: None. Outputs: A valid `tracing` directive; a missing, unreadable, or damaged
    /// control file reads as the default, so the app always has logs. Side effects: Reads the
    /// control file.
    pub fn current_level(&self) -> String {
        match self.control_file.read() {
            Ok(Some(stored_level)) => stored_level,
            Ok(None) | Err(_) => DEFAULT_LOG_LEVEL.to_owned(),
        }
    }

    /// Validates and stores a new level.
    ///
    /// Inputs: A `tracing` directive such as `debug`. Outputs: The trimmed directive, or an error
    /// when it is blank, holds control characters, is not valid syntax, or cannot be written.
    /// Side effects: Replaces the control file atomically; an invalid level writes nothing.
    pub fn store_level(&self, filter_directive: &str) -> Result<String> {
        self.control_file.request(filter_directive)?;
        Ok(filter_directive.trim().to_owned())
    }

    /// Exports the logs and words the outcome as the FFI's JSON answer.
    ///
    /// Inputs: `output_path` is an absolute file path, or `None`/blank for
    /// `<export folder>/note_taker_logs_<YYYYMMDD-HHMMSS>.jsonl` stamped with `now`.
    /// Outputs: `{"ok":true,"file_path","size_bytes","file_count"}` or
    /// `{"ok":false,"error_code","error"}` with `invalid_argument` (a relative path), `no_logs`,
    /// or `export_failed`.
    /// Side effects: Writes the export file (see `export_log_files`).
    pub fn export_json(&self, output_path: Option<&str>, now: DateTime<Utc>) -> String {
        let output_path = match output_path.map(str::trim).filter(|path| !path.is_empty()) {
            Some(path) if Path::new(path).is_absolute() => PathBuf::from(path),
            Some(_) => {
                return error_json("invalid_argument", "output_path must be an absolute path");
            }
            None => self.export_directory.join(format!(
                "note_taker_logs_{}.jsonl",
                now.format("%Y%m%d-%H%M%S")
            )),
        };
        match export_log_files(&self.log_directory, &output_path) {
            Ok(export) => serde_json::json!({
                "ok": true,
                "file_path": export.path.display().to_string(),
                "size_bytes": export.size_bytes,
                "file_count": export.file_count,
            })
            .to_string(),
            Err(LogExportError::NoLogs) => error_json("no_logs", "there are no logs to export yet"),
            Err(error @ LogExportError::Io(_)) => error_json("export_failed", &error.to_string()),
        }
    }

    /// Starts logging at `filter_directive`.
    ///
    /// Inputs: A valid directive. Outputs: The logging runtime, or the error that prevented it
    /// (no device ID, an unwritable log directory, or a subscriber installed elsewhere).
    /// Side effects: Creates the database's directory (for the device ID file) and the log
    /// directory, and installs the process-wide tracing subscriber.
    fn start_logging(&self, filter_directive: &str) -> Result<LoggingRuntime> {
        if let Some(database_directory) = self.database_path.parent()
            && !database_directory.as_os_str().is_empty()
        {
            fs::create_dir_all(database_directory)?;
        }
        let device_id = resolve_device_id(app_setting(DEVICE_ID_VARIABLE), &self.database_path)?;
        initialize_logging(
            LoggingConfiguration::new(device_id, &self.log_directory).with_filter(filter_directive),
        )
    }
}

/// Starts the app's logging at the stored level, once per process.
///
/// Inputs: None; reads the configured settings. Outputs: None; a failure leaves the app without
/// file logs and is retried by the next call (the next runtime open or level change).
/// Side effects: See `AppLogSettings::start_logging`.
pub fn start_app_logging() {
    let Ok(mut app_logging) = APP_LOGGING.lock() else {
        return;
    };
    if app_logging.is_none() {
        let settings = AppLogSettings::from_lookup(&app_setting);
        *app_logging = settings.start_logging(&settings.current_level()).ok();
    }
}

/// Stores a new level and applies it, starting logging when it has not started yet.
///
/// Inputs: A `tracing` directive. Outputs: `FFI_SUCCESS`, `FFI_INVALID_ARGUMENT` for an invalid
/// directive (nothing stored), or `FFI_OPERATION_REJECTED` when it cannot be stored or applied.
/// Side effects: Writes `<db>.log-level`; reloads the process's tracing filter.
fn set_app_log_level(filter_directive: &str) -> c_int {
    let settings = AppLogSettings::from_lookup(&app_setting);
    // Validation runs before the write, so an invalid level never replaces the stored one.
    if LogLevelControlFile::validate(filter_directive).is_err() {
        return FFI_INVALID_ARGUMENT;
    }
    let Ok(stored_level) = settings.store_level(filter_directive) else {
        return FFI_OPERATION_REJECTED;
    };
    let Ok(mut app_logging) = APP_LOGGING.lock() else {
        return FFI_OPERATION_REJECTED;
    };
    match app_logging.as_ref() {
        Some(logging_runtime) => match logging_runtime.set_filter(&stored_level) {
            Ok(()) => FFI_SUCCESS,
            Err(_) => FFI_OPERATION_REJECTED,
        },
        None => match settings.start_logging(&stored_level) {
            Ok(logging_runtime) => {
                *app_logging = Some(logging_runtime);
                FFI_SUCCESS
            }
            Err(_) => FFI_OPERATION_REJECTED,
        },
    }
}

/// Words an export failure.
fn error_json(error_code: &str, message: &str) -> String {
    serde_json::json!({ "ok": false, "error_code": error_code, "error": message }).to_string()
}

/// Moves a Rust string to a caller-freed C string.
fn into_c_string(value: String) -> *mut c_char {
    CString::new(value)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

/// Sets the app's log level from Settings and keeps it for the next start.
///
/// Inputs: `filter_directive` is a NUL-terminated UTF-8 `tracing` directive, such as `error`,
/// `warn`, `info`, `debug`, or `trace`.
/// Outputs: `FFI_SUCCESS`, `FFI_INVALID_ARGUMENT` (null, not UTF-8, or not a valid directive), or
/// `FFI_OPERATION_REJECTED` (it could not be stored or applied).
/// Side effects: Writes `<db>.log-level`, reloads the tracing filter, and starts file logging if
/// it has not started. Does not need the runtime to be open.
///
/// # Safety
/// The caller must pass either a null pointer or a valid NUL-terminated C string, read only for
/// the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_set_log_level(filter_directive: *const c_char) -> c_int {
    if filter_directive.is_null() {
        return FFI_INVALID_ARGUMENT;
    }
    match unsafe { std::ffi::CStr::from_ptr(filter_directive) }.to_str() {
        Ok(filter_directive) => set_app_log_level(filter_directive),
        Err(_) => FFI_INVALID_ARGUMENT,
    }
}

/// Returns the app's log level for Settings.
///
/// Inputs: None. Outputs: An owned C string (free with `note_taker_free_string`): the stored
/// directive, or `info` when none is stored. Null only if allocation fails.
/// Side effects: Reads `<db>.log-level`.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_get_log_level() -> *mut c_char {
    into_c_string(AppLogSettings::from_lookup(&app_setting).current_level())
}

/// Exports the newest week of logs into one JSON-lines file.
///
/// Inputs: `output_path` is null, empty, or a NUL-terminated absolute file path.
/// Outputs: An owned JSON string (free with `note_taker_free_string`):
/// `{"ok":true,"file_path","size_bytes","file_count"}` or `{"ok":false,"error_code","error"}`
/// with `invalid_argument`, `no_logs`, or `export_failed`. Null only if allocation fails.
/// Side effects: Reads the log files and writes the export (to the export folder without a
/// path). Blocking file I/O, so Flutter calls it from a background isolate. Does not need the
/// runtime to be open.
///
/// # Safety
/// `output_path` must be null or point to a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_export_logs(output_path: *const c_char) -> *mut c_char {
    let output_path = if output_path.is_null() {
        None
    } else {
        match unsafe { std::ffi::CStr::from_ptr(output_path) }.to_str() {
            Ok(output_path) => Some(output_path),
            Err(_) => {
                return into_c_string(error_json(
                    "invalid_argument",
                    "output_path must be valid UTF-8",
                ));
            }
        }
    };
    into_c_string(AppLogSettings::from_lookup(&app_setting).export_json(output_path, Utc::now()))
}

#[cfg(test)]
#[path = "ffi_logging_tests.rs"]
mod tests;
