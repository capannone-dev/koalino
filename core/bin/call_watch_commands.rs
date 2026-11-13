//! The call watch (#111) for the desktop app: its Settings and the polls that notice calls.
//!
//! A request is `{"command":"poll"}`, `{"command":"get"}` or `{"command":"set","offer"?,
//! "auto_stop"?}`. A poll looks at the microphone's users once and answers
//! `{"ok":true,"available","in_call","app","event"}`, where `event` is `started` or `ended` on the
//! poll that noticed it, else null; a look whose tool failed this once answers `probe_failed`, and
//! the next may work. `get` and `set` answer `{"ok":true,"offer","auto_stop"}`.
//! `offer` (on by default) is whether the app offers to record when a call starts; `auto_stop`
//! (on by default) is whether a recording started from that offer stops when the call ends. Both
//! live in `<db>.call-watch.json`. Nothing here logs the apps seen.

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::Instant,
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    adapters::audio::microphone_users::MicrophoneUsersError,
    domain::call_watch::{CallEvent, CallWatch, MicrophoneUser},
};

/// The owner's choices.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CallWatchSettings {
    /// Offer to record when a call starts.
    #[serde(default = "on")]
    pub offer: bool,
    /// Stop a recording started from the offer when the call ends.
    #[serde(default = "on")]
    pub auto_stop: bool,
    /// Read who the call window shows as speaking during recordings, to suggest names for
    /// unknown voices (#113).
    #[serde(default = "on")]
    pub read_names: bool,
}

fn on() -> bool {
    true
}

impl Default for CallWatchSettings {
    fn default() -> Self {
        Self {
            offer: true,
            auto_stop: true,
            read_names: true,
        }
    }
}

impl CallWatchSettings {
    /// The settings file beside a database: `<db>.call-watch.json`. Side effects: None.
    pub fn file_for(database_path: &Path) -> PathBuf {
        let mut name = database_path.as_os_str().to_owned();
        name.push(".call-watch.json");
        PathBuf::from(name)
    }

    /// Reads the settings; a missing or unreadable file gives the defaults.
    ///
    /// Side effects: Reads the file.
    pub fn load(file: &Path) -> Self {
        fs::read(file)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Saves the settings. Side effects: Replaces the file through a temporary one.
    pub fn save(&self, file: &Path) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        let mut temporary = file.as_os_str().to_owned();
        temporary.push(".tmp");
        fs::write(&temporary, bytes)?;
        fs::rename(&temporary, file)
    }
}

/// The app's call watch: the settings file and the watch's state between polls.
pub struct CallWatcher {
    settings_file: PathBuf,
    watch: CallWatch,
    /// The clock polls are timed by.
    started: Instant,
}

impl CallWatcher {
    /// Creates a watcher with no call. Side effects: None.
    pub fn new(settings_file: PathBuf) -> Self {
        Self {
            settings_file,
            watch: CallWatch::new(),
            started: Instant::now(),
        }
    }

    /// Runs one request.
    ///
    /// Inputs: The JSON request, and how to read the microphone's users (called by `poll`).
    /// Outputs: The answer the module documentation describes, or `invalid_request` /
    /// `storage_failed`. Side effects: `poll` reads the users; `set` saves the settings.
    pub fn command(
        &mut self,
        request_json: &str,
        users: &mut dyn FnMut() -> Result<Vec<MicrophoneUser>, MicrophoneUsersError>,
    ) -> Value {
        let request: Value = serde_json::from_str(request_json).unwrap_or(Value::Null);
        match request.get("command").and_then(Value::as_str) {
            Some("poll") => {
                let now_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
                self.poll_at(now_ms, users())
            }
            Some("get") => self.settings_json(CallWatchSettings::load(&self.settings_file)),
            Some("set") => {
                let mut settings = CallWatchSettings::load(&self.settings_file);
                if let Some(offer) = request.get("offer").and_then(Value::as_bool) {
                    settings.offer = offer;
                }
                if let Some(auto_stop) = request.get("auto_stop").and_then(Value::as_bool) {
                    settings.auto_stop = auto_stop;
                }
                if let Some(read_names) = request.get("read_names").and_then(Value::as_bool) {
                    settings.read_names = read_names;
                }
                if settings.save(&self.settings_file).is_err() {
                    return json!({"ok": false, "error_code": "storage_failed",
                                  "error": "the setting could not be saved"});
                }
                self.settings_json(settings)
            }
            _ => json!({"ok": false, "error_code": "invalid_request",
                        "error": "the command must be poll, get or set"}),
        }
    }

    /// Takes one look at a given time.
    ///
    /// Inputs: The time on the watcher's clock and the users read (or why they could not be).
    /// Outputs: The poll answer; `available` is false where the system cannot tell, and a tool
    /// that failed this once answers `probe_failed`. The watch is then left as it was. Side effects: None beyond the watch's state.
    pub fn poll_at(
        &mut self,
        now_ms: u64,
        users: Result<Vec<MicrophoneUser>, MicrophoneUsersError>,
    ) -> Value {
        let users = match users {
            Ok(users) => users,
            Err(MicrophoneUsersError::Unavailable) => {
                return json!({"ok": true, "available": false, "in_call": false, "app": null,
                              "event": null});
            }
            // A look that failed once (the tool busy, its status collected elsewhere) changes
            // nothing; the next look may work.
            Err(MicrophoneUsersError::Tool(_)) => {
                return json!({"ok": false, "error_code": "probe_failed",
                              "error": "the system's audio tool failed this time"});
            }
        };
        let event = self.watch.observe(now_ms, &users, Some(std::process::id()));
        let (event_name, event_app) = match event {
            Some(CallEvent::Started { app }) => {
                tracing::info!(app, "A call app took the microphone");
                (Some("started"), Some(app))
            }
            Some(CallEvent::Ended { app }) => {
                tracing::info!(app, "The call app left the microphone");
                (Some("ended"), Some(app))
            }
            None => (None, None),
        };
        json!({
            "ok": true,
            "available": true,
            "in_call": self.watch.in_call().is_some(),
            "app": self.watch.in_call().or(event_app),
            "event": event_name,
        })
    }

    fn settings_json(&self, settings: CallWatchSettings) -> Value {
        json!({"ok": true, "offer": settings.offer, "auto_stop": settings.auto_stop,
               "read_names": settings.read_names})
    }
}
