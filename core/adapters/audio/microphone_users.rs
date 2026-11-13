//! Asks the system which apps capture the microphone (#111), for `domain::call_watch`.
//!
//! - Linux: `pactl -f json list source-outputs` and `pactl -f json list short sources`
//!   (PulseAudio, or PipeWire's pulse server). A paused (corked) stream and a stream on a monitor
//!   source (someone recording the speakers) are not microphone use.
//! - Windows: `reg query` of the microphone consent store (`HKCU\...\CapabilityAccessManager\
//!   ConsentStore\microphone`): an app with a start time and no stop time is using it now.
//!   Store apps (the new Teams) are named by package, desktop apps by executable.
//! - Elsewhere: unavailable.
//!
//! The standard tools are run instead of linking an audio or registry library: one look every
//! few seconds is cheap, and no new crate is needed. Nothing here logs which apps were found.

use std::{collections::HashSet, process::Command};

use serde_json::Value;

use crate::domain::call_watch::MicrophoneUser;

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
    /// Outputs: The users, or `Unavailable` / `Tool`. Side effects: Runs `pactl` (Linux) or
    /// `reg` (Windows), which takes a few milliseconds; call it off the UI thread.
    pub fn current(&self) -> Result<Vec<MicrophoneUser>, MicrophoneUsersError> {
        if cfg!(target_os = "linux") {
            let outputs = run("pactl", &["-f", "json", "list", "source-outputs"])?;
            let sources = run("pactl", &["-f", "json", "list", "short", "sources"])?;
            Ok(parse_pactl(&outputs, &sources))
        } else if cfg!(windows) {
            Ok(parse_consent_store(&run(
                "reg",
                &["query", CONSENT_STORE, "/s"],
            )?))
        } else {
            Err(MicrophoneUsersError::Unavailable)
        }
    }
}

/// Runs a tool without a console window and returns its standard output.
fn run(program: &str, arguments: &[&str]) -> Result<String, MicrophoneUsersError> {
    let mut command = Command::new(program);
    command.args(arguments);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        /// Keeps a console window from flashing up at every look.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output().map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => MicrophoneUsersError::Unavailable,
        kind => MicrophoneUsersError::Tool(kind.to_string()),
    })?;
    if !output.status.success() {
        return Err(MicrophoneUsersError::Tool(format!(
            "{program} exited with {}",
            output.status
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
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
