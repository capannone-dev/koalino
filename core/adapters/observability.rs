//! Configures JSON tracing output for stdout and a daily rolling local log file.
//!
//! Both sinks use `StructuredJsonFormat`, which stamps the device ID on every entry, groups fields
//! under `context`, and redacts sensitive values (see `structured_json`).

mod log_export;
mod log_level_control;
mod structured_json;

pub use log_export::{LogExport, LogExportError, MAX_EXPORTED_LOG_FILES, export_log_files};
pub use log_level_control::{LOG_LEVEL_POLL_INTERVAL, LogLevelControlFile, LogLevelWatcher};
pub use structured_json::StructuredJsonFormat;

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    EnvFilter, Layer, Registry, layer::SubscriberExt, reload, util::SubscriberInitExt,
};

use crate::domain::models::DeviceId;

/// Configures structured logging for a single device process.
#[derive(Clone, Debug)]
pub struct LoggingConfiguration {
    device_id: DeviceId,
    log_directory: PathBuf,
    filter_directive: String,
    /// Extra filter for stdout only, or `None` when stdout follows `filter_directive`.
    stdout_filter_directive: Option<String>,
}

impl LoggingConfiguration {
    /// Creates a production-safe JSON logging configuration with INFO as the default verbosity.
    ///
    /// Inputs: `device_id` is logged as required metadata; `log_directory` is a local writable directory.
    /// Outputs: A configuration that can initialize stdout and rolling-file tracing sinks.
    /// Side effects: None.
    pub fn new(device_id: DeviceId, log_directory: impl Into<PathBuf>) -> Self {
        Self {
            device_id,
            log_directory: log_directory.into(),
            filter_directive: String::from("info"),
            stdout_filter_directive: None,
        }
    }

    /// Overrides the initial tracing filter using standard tracing directives.
    ///
    /// Inputs: `filter_directive` must be a valid `tracing_subscriber::EnvFilter` directive.
    /// Outputs: The updated configuration.
    /// Side effects: None.
    pub fn with_filter(mut self, filter_directive: impl Into<String>) -> Self {
        self.filter_directive = filter_directive.into();
        self
    }

    /// Restricts stdout further than the file, e.g. for a CLI whose stdout carries command output.
    ///
    /// Inputs: `stdout_filter_directive` must be a valid `EnvFilter` directive. It applies on top of
    /// the main (runtime-reloadable) filter, so stdout never shows more than the file.
    /// Outputs: The updated configuration.
    /// Side effects: None.
    pub fn with_stdout_filter(mut self, stdout_filter_directive: impl Into<String>) -> Self {
        self.stdout_filter_directive = Some(stdout_filter_directive.into());
        self
    }
}

/// Keeps the rolling file worker alive and allows runtime logging-filter changes.
pub struct LoggingRuntime {
    filter_handle: reload::Handle<EnvFilter, Registry>,
    _file_worker_guard: WorkerGuard,
}

impl LoggingRuntime {
    /// Changes logging verbosity without restarting the process.
    ///
    /// Inputs: `filter_directive` must be valid tracing filter syntax, such as `info` or `note_taker_core=trace`.
    /// Outputs: Success or a configuration error if the directive is invalid or the subscriber has shut down.
    /// Side effects: Reloads the process-wide tracing filter for subsequent events.
    pub fn set_filter(&self, filter_directive: &str) -> Result<()> {
        let replacement_filter = EnvFilter::try_new(filter_directive)
            .with_context(|| "logging filter directive is invalid")?;
        self.filter_handle
            .reload(replacement_filter)
            .with_context(|| "logging subscriber is no longer available")
    }
}

/// Installs JSON tracing output to stdout and a daily rolling `app_log` file.
///
/// Inputs: `logging_configuration` identifies the emitting device, local log location, and initial verbosity.
/// Outputs: A runtime guard that must be retained for as long as file logging is required.
/// Side effects: Creates the log directory, installs the global tracing subscriber, writes JSON events to stdout and disk.
pub fn initialize_logging(logging_configuration: LoggingConfiguration) -> Result<LoggingRuntime> {
    create_log_directory(&logging_configuration.log_directory)?;
    let initial_filter = EnvFilter::try_new(&logging_configuration.filter_directive)
        .with_context(|| "initial logging filter directive is invalid")?;
    let (reloadable_filter, filter_handle) = reload::Layer::new(initial_filter);
    let rolling_file_appender =
        tracing_appender::rolling::daily(&logging_configuration.log_directory, "app_log");
    let (non_blocking_file_writer, file_worker_guard) =
        tracing_appender::non_blocking(rolling_file_appender);
    let structured_format = StructuredJsonFormat::new(&logging_configuration.device_id);
    let stdout_filter = logging_configuration
        .stdout_filter_directive
        .as_deref()
        .map(EnvFilter::try_new)
        .transpose()
        .with_context(|| "stdout logging filter directive is invalid")?;
    let stdout_json_layer = tracing_subscriber::fmt::layer()
        .event_format(structured_format.clone())
        .with_writer(std::io::stdout)
        .with_filter(stdout_filter);
    let file_json_layer = tracing_subscriber::fmt::layer()
        .event_format(structured_format)
        .with_writer(non_blocking_file_writer);

    tracing_subscriber::registry()
        .with(reloadable_filter)
        .with(stdout_json_layer)
        .with(file_json_layer)
        .try_init()
        .with_context(|| "global tracing subscriber is already initialized")?;
    tracing::info!("structured logging initialized");

    Ok(LoggingRuntime {
        filter_handle,
        _file_worker_guard: file_worker_guard,
    })
}

/// Ensures the configured local log directory exists before the rolling appender opens its file.
///
/// Inputs: `log_directory` is the directory intended to hold local JSON logs.
/// Outputs: Success or an I/O error with directory context.
/// Side effects: Creates missing directories on the local filesystem.
fn create_log_directory(log_directory: &Path) -> Result<()> {
    fs::create_dir_all(log_directory)
        .with_context(|| format!("unable to create log directory {}", log_directory.display()))
}
