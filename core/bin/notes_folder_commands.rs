//! Runs the notes folder (#98) for a host: the app over the C ABI, or the daemon.
//!
//! `NotesFolderKeeper` owns the settings file and the round state beside the database, runs a
//! round when the owner keeps the folder on, and remembers how the last round went. Its
//! `command` answers the Settings screens with JSON:
//!
//! | `command` | Fields | Answer |
//! | --- | --- | --- |
//! | `get` | | the status |
//! | `set` | `enabled`, `path`? | the status (turning it on writes the folder at once) |
//! | `sync_now` | | the status, after a round |
//!
//! The status is `{"ok":true,"enabled","path","suggested_path","last_round_at","last_error",
//! "written","imported"}`. Failures are `{"ok":false,"error_code","error"}` with
//! `invalid_request`, `invalid_folder`, `folder_failed`, `storage_failed` or
//! `runtime_unavailable`. `spawn` runs rounds every `ROUND_INTERVAL` on a thread of its own.
//! Nothing here logs file names or text.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
    thread,
};

use chrono::{DateTime, Utc};
use serde_json::{Value, json};

use crate::{
    auto_summaries::AutoSummaryWake,
    notes_folder::{
        FolderState, NotesFolderError, NotesFolderSettings, ROUND_INTERVAL, RoundReport,
        validate_root,
    },
};

/// Runs one round with the host's store; the keeper passes the folder and the state.
pub type RoundRunner<'runner> =
    dyn FnMut(&Path, &mut FolderState) -> Result<RoundReport, NotesFolderError> + 'runner;

/// A round runner that can move to the keeper's thread.
pub type SendRoundRunner = Box<
    dyn FnMut(&Path, &mut FolderState) -> Result<RoundReport, NotesFolderError> + Send + 'static,
>;

/// How the last round went.
#[derive(Clone, Debug, Default)]
struct LastRound {
    at: Option<DateTime<Utc>>,
    error: Option<&'static str>,
    written: usize,
    imported: usize,
}

/// The folder's settings, state and last round for one database.
pub struct NotesFolderKeeper {
    settings_file: PathBuf,
    state_file: PathBuf,
    /// The suggested folder: `<home>/NoteTaker`, when there is a home folder.
    suggested: Option<PathBuf>,
    /// The state, loaded on the first round; one round at a time.
    state: Mutex<Option<FolderState>>,
    last: Mutex<LastRound>,
}

impl NotesFolderKeeper {
    /// Creates the keeper for a database.
    ///
    /// Inputs: The database path (the settings and state live beside it) and the home folder,
    /// for the suggested location. Side effects: None.
    pub fn for_database(database_path: &Path, home: Option<PathBuf>) -> Self {
        Self {
            settings_file: NotesFolderSettings::file_for(database_path),
            state_file: FolderState::file_for(database_path),
            suggested: home.map(|home| home.join("NoteTaker")),
            state: Mutex::new(None),
            last: Mutex::new(LastRound::default()),
        }
    }

    /// The saved settings; unreadable settings count as off.
    pub fn settings(&self) -> NotesFolderSettings {
        NotesFolderSettings::load(&self.settings_file).unwrap_or_default()
    }

    /// Runs a round when the folder is on.
    ///
    /// Inputs: `runner` runs `notes_folder::run_round` against the host's store.
    /// Outputs: What the round did; `None` when the folder is off. Side effects: Those of the
    /// round; saves the state beside the database and records how the round went (a failure is
    /// logged with its kind only).
    pub fn round(
        &self,
        runner: &mut RoundRunner<'_>,
    ) -> Option<Result<RoundReport, NotesFolderError>> {
        let settings = self.settings();
        let root = settings.active_root()?.to_path_buf();
        let mut guard = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let state = guard.get_or_insert_with(|| FolderState::load(&self.state_file));
        let result = runner(&root, state);
        // A failed round may have done part of its work; the state says what.
        let saved = state.save(&self.state_file);
        drop(guard);
        let result = result.and_then(|report| saved.map(|()| report));
        let mut last = self.last.lock().unwrap_or_else(PoisonError::into_inner);
        last.at = Some(Utc::now());
        match &result {
            Ok(report) => {
                last.error = None;
                last.written = report.written;
                last.imported = report.imported;
            }
            Err(error) => {
                last.error = Some(error_code(error));
                tracing::warn!(error_code = error_code(error), "notes folder round failed");
            }
        }
        Some(result)
    }

    /// Answers a Settings command (see the module header).
    ///
    /// Inputs: The request JSON and the round runner, used by `set` and `sync_now`.
    /// Outputs: The JSON answer. Side effects: `set` saves the settings; `set` turning the folder
    /// on and `sync_now` run a round.
    pub fn command(&self, request_json: &str, runner: &mut RoundRunner<'_>) -> Value {
        let Ok(request) = serde_json::from_str::<Value>(request_json) else {
            return failure("invalid_request", "the request is not JSON");
        };
        match request.get("command").and_then(Value::as_str) {
            Some("get") => self.status(),
            Some("set") => {
                let Some(enabled) = request.get("enabled").and_then(Value::as_bool) else {
                    return failure("invalid_request", "`enabled` must be true or false");
                };
                let mut settings = self.settings();
                match request.get("path") {
                    None | Some(Value::Null) => {}
                    Some(Value::String(path)) => match validate_root(Path::new(path.trim())) {
                        Ok(path) => settings.path = Some(path),
                        Err(error) => return failure("invalid_folder", &error.to_string()),
                    },
                    Some(_) => return failure("invalid_request", "`path` must be text"),
                }
                if enabled && settings.path.is_none() {
                    settings.path = self.suggested.clone();
                }
                if enabled && settings.path.is_none() {
                    return failure("invalid_folder", "choose a folder");
                }
                settings.enabled = enabled;
                if let Err(error) = settings.save(&self.settings_file) {
                    return failure("folder_failed", &error.to_string());
                }
                if enabled && let Some(Err(error)) = self.round(runner) {
                    return failure(error_code(&error), &error.to_string());
                }
                self.status()
            }
            Some("sync_now") => match self.round(runner) {
                Some(Err(error)) => failure(error_code(&error), &error.to_string()),
                _ => self.status(),
            },
            _ => failure("invalid_request", "unknown notes folder command"),
        }
    }

    /// The status JSON.
    fn status(&self) -> Value {
        let settings = self.settings();
        let last = self
            .last
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        json!({
            "ok": true,
            "enabled": settings.enabled,
            "path": settings.path.as_deref().map(|path| path.to_string_lossy()),
            "suggested_path": self.suggested.as_deref().map(|path| path.to_string_lossy()),
            "last_round_at": last.at.map(|at| at.to_rfc3339()),
            "last_error": last.error,
            "written": last.written,
            "imported": last.imported,
        })
    }

    /// Runs rounds on a thread of its own: at once, then every `ROUND_INTERVAL` or when woken.
    ///
    /// Inputs: The keeper, the wake-up, and `runner`, which takes the host's store for one round
    /// (and announces the meetings it changed). Side effects: Spawns the thread.
    pub fn spawn(
        self: Arc<Self>,
        wake: Arc<AutoSummaryWake>,
        mut runner: SendRoundRunner,
    ) -> std::io::Result<thread::JoinHandle<()>> {
        thread::Builder::new()
            .name("notes-folder".to_owned())
            .spawn(move || {
                loop {
                    let _ = self.round(runner.as_mut());
                    wake.wait(ROUND_INTERVAL);
                }
            })
    }
}

/// The stable code of a round's failure.
fn error_code(error: &NotesFolderError) -> &'static str {
    match error {
        NotesFolderError::NotAbsolute => "invalid_folder",
        NotesFolderError::Io(_) | NotesFolderError::Settings(_) => "folder_failed",
        NotesFolderError::Storage(_) => "storage_failed",
    }
}

/// A failure answer.
fn failure(error_code: &str, error: &str) -> Value {
    json!({"ok": false, "error_code": error_code, "error": error})
}
