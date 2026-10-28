//! Carries a log-level change from `cli log-level` to a running daemon through a file.
//!
//! REQUIREMENTS.md 6.1 requires changing verbosity on headless devices from the CLI without a
//! restart. The CLI and the daemon are separate processes that already coordinate through files
//! beside the database (`record stop` uses `<db>.recording-stop`), so the CLI writes the
//! requested `tracing` filter directive to `<db>.log-level` and the daemon polls it and reloads
//! its filter. The directive is validated before it is written, so the daemon never reads a
//! directive the CLI accepted but `tracing` rejects.

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use tracing_subscriber::EnvFilter;

/// How often a daemon checks the control file for a new level.
pub const LOG_LEVEL_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// The `<database>.log-level` file of one database.
#[derive(Clone, Debug)]
pub struct LogLevelControlFile {
    path: PathBuf,
}

impl LogLevelControlFile {
    /// Derives the control file path from the database path.
    ///
    /// Inputs: The SQLite database path. Outputs: `<db>.log-level`. Side effects: None.
    pub fn for_database(database_path: &Path) -> Self {
        let mut path = database_path.as_os_str().to_owned();
        path.push(".log-level");
        Self {
            path: PathBuf::from(path),
        }
    }

    /// Returns the control file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Records `filter_directive` as the requested verbosity for the daemon.
    ///
    /// Inputs: A `tracing` filter directive such as `debug` or `note_taker_core=trace,info`.
    /// Outputs: The error when the directive is blank, contains control characters, or is not
    /// valid `tracing` syntax, or when the file cannot be written.
    /// Side effects: Writes a temporary file and renames it over the control file, so the daemon
    /// never reads a partial directive.
    pub fn request(&self, filter_directive: &str) -> Result<()> {
        let filter_directive = validated_directive(filter_directive)?;
        let mut temporary_path = self.path.as_os_str().to_owned();
        temporary_path.push(".tmp");
        let temporary_path = PathBuf::from(temporary_path);
        fs::write(&temporary_path, filter_directive)
            .with_context(|| format!("unable to write {}", temporary_path.display()))?;
        fs::rename(&temporary_path, &self.path)
            .with_context(|| format!("unable to update {}", self.path.display()))
    }

    /// Reads the requested verbosity.
    ///
    /// Inputs: None. Outputs: `None` when nothing was requested, else the validated directive;
    /// an error when the file is unreadable or holds an invalid directive.
    /// Side effects: Reads the file.
    pub fn read(&self) -> Result<Option<String>> {
        match fs::read_to_string(&self.path) {
            Ok(stored_directive) => validated_directive(&stored_directive)
                .map(|directive| Some(directive.to_owned()))
                .with_context(|| format!("{} holds an invalid log level", self.path.display())),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => {
                Err(error).with_context(|| format!("unable to read {}", self.path.display()))
            }
        }
    }
}

/// Detects new requests in a control file, for a daemon's polling loop.
#[derive(Debug)]
pub struct LogLevelWatcher {
    control_file: LogLevelControlFile,
    /// The raw file content last seen, so each request (valid or not) is reported once.
    last_seen_content: Option<String>,
}

impl LogLevelWatcher {
    /// Starts watching, treating whatever the file holds now as already handled.
    ///
    /// Inputs: The database's control file. Outputs: The watcher. Side effects: Reads the file.
    /// A request left from before start-up is ignored, so `RUST_LOG` decides the start-up level.
    pub fn start(control_file: LogLevelControlFile) -> Self {
        let last_seen_content = fs::read_to_string(control_file.path()).ok();
        Self {
            control_file,
            last_seen_content,
        }
    }

    /// Checks the file once.
    ///
    /// Inputs: None. Outputs: `Some(directive)` when a new valid request appeared, `None` when
    /// nothing changed, or an error (once per change) when the new content is invalid.
    /// Side effects: Reads the file; remembers its content.
    pub fn poll(&mut self) -> Result<Option<String>> {
        let current_content = fs::read_to_string(self.control_file.path()).ok();
        if current_content == self.last_seen_content {
            return Ok(None);
        }
        self.last_seen_content = current_content;
        self.control_file.read()
    }
}

/// Trims `filter_directive` and checks it is a non-blank, printable, valid `tracing` filter.
fn validated_directive(filter_directive: &str) -> Result<&str> {
    let filter_directive = filter_directive.trim();
    if filter_directive.is_empty() || filter_directive.chars().any(char::is_control) {
        bail!("the log level must be a non-blank directive such as info or debug");
    }
    EnvFilter::try_new(filter_directive)
        .with_context(|| format!("{filter_directive:?} is not a valid log level directive"))?;
    Ok(filter_directive)
}

#[cfg(test)]
#[path = "log_level_control_tests.rs"]
mod tests;
