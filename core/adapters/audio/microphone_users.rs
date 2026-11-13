//! Asks the system which apps capture the microphone (#111), for `domain::call_watch`.
//!
//! - Linux: `pactl -f json list source-outputs` and `pactl -f json list short sources`
//!   (PulseAudio, or PipeWire's pulse server). A paused (corked) stream and a stream on a monitor
//!   source (someone recording the speakers) are not microphone use.
//! - Windows: the active WASAPI capture sessions (`wasapi_capture_sessions`), named by the
//!   owning process's executable and carrying its process ID. Store apps (the new Teams) show as
//!   their process too. Only when the sessions cannot be read does it fall back to `reg query`
//!   of the microphone consent store (`HKCU\...\CapabilityAccessManager\ConsentStore\
//!   microphone`), where an app with a start time and no stop time is using it now. Windows 11
//!   26H2 no longer writes those times, and spawning `reg` every look is slow on a busy machine.
//! - Elsewhere: unavailable.
//!
//! On Linux the standard tool is run instead of linking an audio library; on Windows the `windows`
//! crate already built for UI Automation asks the audio service. No new crate is needed. Nothing
//! here logs which apps were found.

use std::{
    collections::HashSet,
    process::{Command, Stdio},
};

use serde_json::Value;

use crate::{adapters::child_process::reaped_elsewhere, domain::call_watch::MicrophoneUser};

/// The consent store's microphone key.
const CONSENT_STORE: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";

/// Why the microphone's users could not be read.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MicrophoneUsersError {
    /// This platform has no way to tell here.
    #[error("this system cannot tell which apps use the microphone")]
    Unavailable,
    /// The tool could not be run or failed.
    #[error("the system's audio tool failed: {0}")]
    Tool(String),
}

/// Reads the microphone's users from the running system.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemMicrophoneUsers;

impl SystemMicrophoneUsers {
    /// Lists the apps capturing the microphone now.
    ///
    /// Outputs: The users, or `Unavailable` / `Tool` (on Windows only when both the audio
    /// sessions and the consent store fail). Side effects: Runs `pactl` (Linux), or queries the
    /// audio session manager and, failing that, runs `reg` (Windows); call it off the UI thread.
    pub fn current(&self) -> Result<Vec<MicrophoneUser>, MicrophoneUsersError> {
        if cfg!(target_os = "linux") {
            let outputs = run("pactl", &["-f", "json", "list", "source-outputs"])?;
            let sources = run("pactl", &["-f", "json", "list", "short", "sources"])?;
            Ok(parse_pactl(&outputs, &sources))
        } else if cfg!(windows) {
            // The sessions answer in milliseconds; `reg` costs a process per look, so it is
            // asked only when they cannot be read.
            match active_capture_sessions() {
                Ok(sessions) => Ok(sessions),
                Err(error) => {
                    warn_sessions_unreadable_once(&error);
                    Ok(parse_consent_store(&run(
                        "reg",
                        &["query", CONSENT_STORE, "/s"],
                    )?))
                }
            }
        } else {
            Err(MicrophoneUsersError::Unavailable)
        }
    }
}

/// The active capture sessions on Windows.
#[cfg(windows)]
fn active_capture_sessions() -> Result<Vec<MicrophoneUser>, MicrophoneUsersError> {
    super::wasapi_capture_sessions::active_capture_sessions()
}

/// Logs, once per process, that the audio sessions could not be read (the registry still is).
fn warn_sessions_unreadable_once(error: &MicrophoneUsersError) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static WARNED: AtomicBool = AtomicBool::new(false);
    if !WARNED.swap(true, Ordering::Relaxed) {
        tracing::warn!(error = %error, "The audio sessions could not be read; calls are noticed from the registry only");
    }
}

/// No audio sessions outside Windows (only the Windows branch asks).
#[cfg(not(windows))]
fn active_capture_sessions() -> Result<Vec<MicrophoneUser>, MicrophoneUsersError> {
    Err(MicrophoneUsersError::Unavailable)
}

/// Runs a tool without a console window and returns its standard output.
///
/// When the Flutter engine collected the exit status first (`child_process`), the output, read to
/// its end, is still whole, so it is kept.
fn run(program: &str, arguments: &[&str]) -> Result<String, MicrophoneUsersError> {
    use std::io::Read as _;
    let mut command = Command::new(program);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        /// Keeps a console window from flashing up at every look.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn().map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => MicrophoneUsersError::Unavailable,
        kind => MicrophoneUsersError::Tool(kind.to_string()),
    })?;
    let mut output = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        stdout
            .read_to_string(&mut output)
            .map_err(|error| MicrophoneUsersError::Tool(error.kind().to_string()))?;
    }
    match child.wait() {
        Ok(status) if status.success() => Ok(output),
        Ok(status) => Err(MicrophoneUsersError::Tool(format!(
            "{program} exited with {status}"
        ))),
        Err(error) if reaped_elsewhere(&error) && !output.trim().is_empty() => Ok(output),
        Err(error) => Err(MicrophoneUsersError::Tool(error.kind().to_string())),
    }
}

/// Reads `pactl`'s JSON: the source outputs, and the sources to tell monitors apart.
///
/// Outputs: The apps capturing a real source and not paused; none for unreadable JSON.
/// Side effects: None.
pub fn parse_pactl(outputs_json: &str, sources_json: &str) -> Vec<MicrophoneUser> {
    let monitors: HashSet<u64> = serde_json::from_str::<Vec<Value>>(sources_json)
        .unwrap_or_default()
        .iter()
        .filter(|source| {
            source["name"]
                .as_str()
                .is_some_and(|name| name.ends_with(".monitor"))
        })
        .filter_map(|source| source["index"].as_u64())
        .collect();
    serde_json::from_str::<Vec<Value>>(outputs_json)
        .unwrap_or_default()
        .iter()
        .filter(|output| output["corked"] != true)
        .filter(|output| {
            output["source"]
                .as_u64()
                .is_none_or(|source| !monitors.contains(&source))
        })
        .map(|output| {
            let property = |name: &str| {
                output["properties"][name]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned()
            };
            MicrophoneUser {
                process: property("application.process.binary"),
                name: property("application.name"),
                pid: property("application.process.id").parse().ok(),
            }
        })
        .collect()
}

/// Reads `reg query <consent store> /s`.
///
/// Outputs: The apps with a start time and a zero stop time, named by their package or their
/// executable's file name. Side effects: None.
pub fn parse_consent_store(reg_output: &str) -> Vec<MicrophoneUser> {
    let mut users = Vec::new();
    let mut entry: Option<(String, bool, bool)> = None;
    let mut finish = |entry: Option<(String, bool, bool)>| {
        if let Some((process, started, stopped)) = entry
            && started
            && !stopped
        {
            users.push(MicrophoneUser {
                process,
                name: String::new(),
                pid: None,
            });
        }
    };
    for line in reg_output.lines() {
        let line = line.trim_end();
        if line.starts_with("HKEY_") {
            finish(entry.take());
            let last = line.rsplit('\\').next().unwrap_or_default();
            // A desktop app's key is its path with `#` for `\`.
            let process = last.rsplit('#').next().unwrap_or(last).to_owned();
            entry = Some((process, false, false));
            continue;
        }
        let mut fields = line.split_whitespace();
        let (Some(name), Some(_kind), Some(value)) = (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let nonzero = value != "0x0";
        if let Some((_, started, stopped)) = entry.as_mut() {
            match name {
                "LastUsedTimeStart" => *started = nonzero,
                "LastUsedTimeStop" => *stopped = nonzero,
                _ => {}
            }
        }
    }
    finish(entry);
    users
}

/// Names a process by its executable's file name, from a full image path.
///
/// Inputs: A Windows image path such as `C:\...\msedge.exe`, or a bare name.
/// Outputs: The text after the last `\` or `/` (`msedge.exe`); empty for an empty path.
/// Side effects: None.
pub fn process_name_from_image_path(image_path: &str) -> &str {
    image_path.rsplit(['\\', '/']).next().unwrap_or(image_path)
}

/// Joins two lists of users, once per app (several capture endpoints' sessions).
///
/// Inputs: `first` wins on a clash; `second` adds the apps it alone names. Apps match by process name, ignoring case.
/// Outputs: `first` in order, then `second`'s new apps in order. Side effects: None.
pub fn merge_users(first: Vec<MicrophoneUser>, second: Vec<MicrophoneUser>) -> Vec<MicrophoneUser> {
    let mut seen: HashSet<String> = HashSet::new();
    first
        .into_iter()
        .chain(second)
        .filter(|user| seen.insert(user.process.to_lowercase()))
        .collect()
}
