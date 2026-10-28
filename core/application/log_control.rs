//! The daemon's runtime log level and log location, as a port for the HTTP routes (REQUIREMENTS
//! 6.1, 6.2; Issue #46).
//!
//! The web Settings page reads and changes the level and downloads the logs through
//! `/api/settings/log-level` and `/api/logs/export`. The routes depend on this port only, so they
//! are tested without the process-wide tracing subscriber; `LiveLogControl` implements it.

use std::path::PathBuf;

/// Why a level change was refused.
#[derive(Debug, thiserror::Error)]
pub enum LogControlError {
    /// The directive is blank, holds control characters, or is not valid `tracing` syntax.
    #[error("{0}")]
    InvalidLevel(String),
    /// The logging runtime could not apply it.
    #[error("logging is unavailable: {0}")]
    Unavailable(String),
}

/// Reads and changes the running process's log level.
pub trait LogControl: Send + Sync {
    /// The level in effect, as a `tracing` directive such as `info`.
    fn current_level(&self) -> String;

    /// Validates and applies a level until the process restarts.
    ///
    /// Inputs: A directive. Outputs: The trimmed directive now in effect, or why it was refused
    /// (the old level stays). Side effects: Reloads the process's tracing filter.
    fn set_level(&self, filter_directive: &str) -> Result<String, LogControlError>;

    /// The directory holding the daily log files.
    fn log_directory(&self) -> PathBuf;
}
