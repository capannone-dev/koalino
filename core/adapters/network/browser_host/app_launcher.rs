//! Starts Koalino from the browser host when a meeting needs it and it isn't running (#135).
//!
//! The app is the `note_taker_ui` program installed next to `browser-host` (the host's manifest
//! points into the app's folder). It is started detached, so it keeps running when the browser
//! closes: in its own process group on Linux; on Windows as a detached process that breaks away
//! from the browser's job object (Firefox kills the job's processes when it closes the host).
//! It is not started while it holds its running lock (see `outbox::hold_app_running`), nor
//! within `RESTART_GUARD` of an earlier start, since a starting app takes the lock only once
//! its core opens. `NOTE_TAKER_BROWSER_HOST_START_APP=false` turns this off.

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime},
};

use super::outbox::app_is_running;

/// Turns starting Koalino off when set to `false`.
pub const START_APP_VARIABLE: &str = "NOTE_TAKER_BROWSER_HOST_START_APP";
/// How long after a start Koalino is given to come up before it may be started again.
pub const RESTART_GUARD: Duration = Duration::from_secs(120);
/// The file recording the last start, in the outbox root.
const LAST_START_FILE: &str = "last-app-start";

/// The app's program name in its folder.
fn app_file_name() -> &'static str {
    if cfg!(windows) {
        "note_taker_ui.exe"
    } else {
        "note_taker_ui"
    }
}

/// The Koalino app installed next to the host, if there is one.
///
/// Side effects: Checks that the file exists.
pub fn sibling_app(host_path: &Path) -> Option<PathBuf> {
    let app = host_path.parent()?.join(app_file_name());
    app.is_file().then_some(app)
}

/// Whether to start Koalino: it isn't running, and no start happened within `RESTART_GUARD`.
///
/// Side effects: None.
pub fn should_start(app_running: bool, last_start: Option<SystemTime>, now: SystemTime) -> bool {
    !app_running
        && last_start
            .is_none_or(|started| now.duration_since(started).unwrap_or_default() >= RESTART_GUARD)
}

/// Starts one installed Koalino.
#[derive(Clone, Debug)]
pub struct AppLauncher {
    app: PathBuf,
    outbox_root: PathBuf,
}

impl AppLauncher {
    /// A launcher for `app`, using `outbox_root` for the running lock and the last start.
    /// Side effects: None.
    pub fn new(app: PathBuf, outbox_root: PathBuf) -> Self {
        Self { app, outbox_root }
    }

    /// Starts Koalino unless it runs or was just started.
    ///
    /// Outputs: Whether it was started. Side effects: May spawn the app detached; writes the
    /// last-start file; logs the outcome (never meeting content).
    pub fn start_if_needed(&self) -> bool {
        let last_start_file = self.outbox_root.join(LAST_START_FILE);
        let last_start = std::fs::metadata(&last_start_file)
            .and_then(|metadata| metadata.modified())
            .ok();
        if !should_start(
            app_is_running(&self.outbox_root),
            last_start,
            SystemTime::now(),
        ) {
            return false;
        }
        let _ = std::fs::create_dir_all(&self.outbox_root);
        let _ = std::fs::write(&last_start_file, b"");
        match spawn_detached(&self.app) {
            Ok(()) => {
                tracing::info!("Koalino was not running; started it for the meeting");
                true
            }
            Err(error) => {
                tracing::warn!(error = %error, "Koalino could not be started; the meeting is kept for it");
                false
            }
        }
    }
}

/// Spawns `app` so that it outlives the host and the browser.
///
/// Side effects: Starts a process with no standard streams, in the app's folder.
fn spawn_detached(app: &Path) -> std::io::Result<()> {
    let mut command = Command::new(app);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(folder) = app.parent() {
        command.current_dir(folder);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
        command.spawn().map(drop)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
        command.creation_flags(
            DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB,
        );
        match command.spawn() {
            Ok(_) => Ok(()),
            // A job that forbids breaking away: start it inside the job rather than not at all.
            Err(_) => command
                .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
                .spawn()
                .map(drop),
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        command.spawn().map(drop)
    }
}
