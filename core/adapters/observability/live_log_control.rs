//! The daemon's live log level, behind the web Settings page (Issue #46).
//!
//! `LiveLogControl` implements the `LogControl` port over a filter reloader, which the daemon
//! builds from its `LoggingRuntime`. The daemon's control-file watcher (`cli log-level`) applies
//! levels through it too, so the page always shows the level in effect. A change lasts until the
//! daemon restarts; `RUST_LOG` sets the start level.

use std::{
    path::PathBuf,
    sync::{Mutex, PoisonError},
};

use anyhow::Result;

use super::{LogLevelControlFile, LoggingRuntime};
use crate::application::log_control::{LogControl, LogControlError};

/// Reloads the tracing filter with a validated directive.
pub type FilterReloader = Box<dyn Fn(&str) -> Result<()> + Send + Sync>;

/// The level in effect and how to change it.
pub struct LiveLogControl {
    reload: FilterReloader,
    current_level: Mutex<String>,
    log_directory: PathBuf,
}

impl LiveLogControl {
    /// Creates the control.
    ///
    /// Inputs: The level logging started at, the log directory, and the reloader.
    /// Outputs: The control. Side effects: None.
    pub fn new(start_level: &str, log_directory: PathBuf, reload: FilterReloader) -> Self {
        Self {
            reload,
            current_level: Mutex::new(start_level.trim().to_owned()),
            log_directory,
        }
    }

    /// Creates the control over the process's logging runtime.
    ///
    /// Inputs: The runtime (kept alive by the control), its start level, and log directory.
    /// Outputs: The control. Side effects: None.
    pub fn for_runtime(
        logging_runtime: LoggingRuntime,
        start_level: &str,
        log_directory: PathBuf,
    ) -> Self {
        Self::new(
            start_level,
            log_directory,
            Box::new(move |directive: &str| logging_runtime.set_filter(directive)),
        )
    }
}

impl LogControl for LiveLogControl {
    fn current_level(&self) -> String {
        self.current_level
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn set_level(&self, filter_directive: &str) -> Result<String, LogControlError> {
        let directive = LogLevelControlFile::validate(filter_directive)
            .map_err(|error| LogControlError::InvalidLevel(format!("{error:#}")))?;
        let mut current_level = self
            .current_level
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        (self.reload)(directive)
            .map_err(|error| LogControlError::Unavailable(format!("{error:#}")))?;
        directive.clone_into(&mut current_level);
        Ok(directive.to_owned())
    }

    fn log_directory(&self) -> PathBuf {
        self.log_directory.clone()
    }
}

#[cfg(test)]
#[path = "live_log_control_tests.rs"]
mod tests;
