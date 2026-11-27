//! Settings → Developer (#187): app variables an expert or developer can override from the app,
//! where a phone has no environment to set them in.
//!
//! The section is hidden until the owner taps the version in Settings seven times (`unlock`).
//! Overrides are saved in `developer-settings.json` in the app's data folder and applied at the
//! next start, ahead of the app's own configuration and the environment (`ffi_settings`): the
//! running pipeline, sync and bridge read their settings once, so changing them live would leave
//! half the app on old values.
//!
//! Only `DEVELOPER_SETTINGS` can be overridden. Secrets (the encryption passphrase, the bridge
//! secret), storage paths and the device identity are never on it: a wrong value there could lock
//! the owner out of their data or break pairing. Test-only variables stay out too. The file holds
//! no secrets, and values are never logged, only names.

use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The file, in the app's data folder, holding the section's state and overrides.
pub const DEVELOPER_SETTINGS_FILE: &str = "developer-settings.json";

/// What values a setting takes.
#[derive(Clone, Copy, Debug)]
pub enum SettingKind {
    /// `true` or `false` (also accepts 1/0, yes/no, on/off).
    Flag,
    /// A whole number in `min..=max`.
    Integer {
        /// The smallest accepted value.
        min: i64,
        /// The largest accepted value.
        max: i64,
    },
    /// One of a few words.
    Choice(&'static [&'static str]),
    /// `auto` or a 2–3 letter Whisper language code.
    Language,
    /// A libp2p multiaddress such as `/ip4/0.0.0.0/tcp/4001`.
    Multiaddress,
}

/// One overridable app variable.
#[derive(Clone, Copy, Debug)]
pub struct DeveloperSetting {
    name: &'static str,
    kind: SettingKind,
    default: &'static str,
    description: &'static str,
}

impl DeveloperSetting {
    /// The variable name, such as `NOTE_TAKER_VAD_ENABLED`.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// What values it takes.
    pub fn kind(&self) -> SettingKind {
        self.kind
    }

    /// What the app uses when nothing sets it, as shown to the developer.
    pub fn default_value(&self) -> &'static str {
        self.default
    }

    /// What it does, in English (these are technical variable names, not translated).
    pub fn description(&self) -> &'static str {
        self.description
    }
}

/// The variables Settings → Developer offers. Nothing else can be overridden.
pub const DEVELOPER_SETTINGS: &[DeveloperSetting] = &[
    DeveloperSetting {
        name: "NOTE_TAKER_WHISPER_GPU",
        kind: SettingKind::Choice(&["on", "off"]),
        default: "on",
        description: "Lets local transcription try the GPU. Off rules a faulty GPU driver out.",
    },
    DeveloperSetting {
        name: "NOTE_TAKER_VAD_ENABLED",
        kind: SettingKind::Flag,
        default: "true",
        description: "Cuts the audio at pauses in speech. Off cuts about every 15 seconds.",
    },
    DeveloperSetting {
        name: "NOTE_TAKER_ENABLE_DIARIZATION",
        kind: SettingKind::Flag,
        default: "true when the models are present",
        description: "Tells speakers apart. On works only when the diarization models are present.",
    },
    DeveloperSetting {
        name: "NOTE_TAKER_TRANSCRIPT_PASS",
        kind: SettingKind::Flag,
        default: "true",
        description: "Transcribes the meeting again after it ends. A choice saved in Settings wins.",
    },
    DeveloperSetting {
        name: "NOTE_TAKER_WHISPER_LANGUAGE",
        kind: SettingKind::Language,
        default: "auto",
        description: "The transcription language when none is chosen. The meeting's language wins.",
    },
    DeveloperSetting {
        name: "NOTE_TAKER_TEAMS_BRIDGE_ENABLED",
        kind: SettingKind::Flag,
        default: "true",
        description: "The local bridge the browser extension and the Teams bot send call audio to.",
    },
    DeveloperSetting {
        name: "NOTE_TAKER_TEAMS_BRIDGE_PORT",
        kind: SettingKind::Integer {
            min: 1024,
            max: 65_535,
        },
        default: "8765",
        description: "The bridge's port on 127.0.0.1. The browser extension must use the same one.",
    },
    DeveloperSetting {
        name: "NOTE_TAKER_TEAMS_BRIDGE_BUFFER_SECONDS",
        kind: SettingKind::Integer { min: 1, max: 300 },
        default: "10",
        description: "Seconds of call audio the bridge holds while transcription catches up.",
    },
    DeveloperSetting {
        name: "NOTE_TAKER_P2P_LISTEN",
        kind: SettingKind::Multiaddress,
        default: "/ip4/0.0.0.0/tcp/0",
        description: "Where sync listens. A fixed port helps a firewall or port forward.",
    },
];

/// The saved state of the section.
#[derive(Debug, Default, Deserialize, Serialize)]
struct DeveloperFile {
    /// Whether the owner unlocked the section.
    #[serde(default)]
    unlocked: bool,
    /// Variable name → saved value, as written (validated again on load).
    #[serde(default)]
    overrides: BTreeMap<String, String>,
}

/// Why a request was refused.
#[derive(Debug, thiserror::Error)]
enum DeveloperError {
    #[error("the request must be a JSON object with a known command")]
    InvalidArgument,
    #[error("the developer settings are locked")]
    Locked,
    #[error("this variable cannot be changed here")]
    UnknownSetting,
    #[error("{0}")]
    InvalidValue(String),
    #[error("the developer settings could not be saved")]
    StorageFailed(#[from] std::io::Error),
}

impl DeveloperError {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidArgument => "invalid_argument",
            Self::Locked => "locked",
            Self::UnknownSetting => "unknown_setting",
            Self::InvalidValue(_) => "invalid_value",
            Self::StorageFailed(_) => "storage_failed",
        }
    }
}

/// The path of the file in `data_directory`.
fn file_path(data_directory: &Path) -> PathBuf {
    data_directory.join(DEVELOPER_SETTINGS_FILE)
}

/// Reads the saved state; a missing or unreadable file is an empty, locked one.
///
/// Side effects: Reads the file; logs (without values) when it cannot be parsed.
fn read_file(data_directory: &Path) -> DeveloperFile {
    let Ok(text) = std::fs::read_to_string(file_path(data_directory)) else {
        return DeveloperFile::default();
    };
    serde_json::from_str(&text).unwrap_or_else(|_| {
        tracing::warn!("developer settings file is unreadable; ignoring it");
        DeveloperFile::default()
    })
}

/// Writes the state through a temporary file, so a crash never leaves half a file.
fn write_file(data_directory: &Path, state: &DeveloperFile) -> Result<(), DeveloperError> {
    std::fs::create_dir_all(data_directory)?;
    let text = serde_json::to_string_pretty(state).map_err(std::io::Error::other)?;
    let temporary = data_directory.join(format!("{DEVELOPER_SETTINGS_FILE}.tmp"));
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, file_path(data_directory))?;
    Ok(())
}

/// Finds an allowed setting by name.
fn find(name: &str) -> Option<&'static DeveloperSetting> {
    DEVELOPER_SETTINGS
        .iter()
        .find(|setting| setting.name == name)
}

/// Checks `raw` for `setting` and returns it in the form the app reads.
///
/// Outputs: The normalized value (flags become `true`/`false`, numbers lose padding), or a
/// message saying what the setting accepts.
fn normalize(setting: &DeveloperSetting, raw: &str) -> Result<String, DeveloperError> {
    let value = raw.trim();
    let invalid = |accepts: String| DeveloperError::InvalidValue(accepts);
    match setting.kind {
        SettingKind::Flag => match value.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok("true".to_owned()),
            "0" | "false" | "no" | "off" => Ok("false".to_owned()),
            _ => Err(invalid("use true or false".to_owned())),
        },
        SettingKind::Integer { min, max } => value
            .parse::<i64>()
            .ok()
            .filter(|number| (min..=max).contains(number))
            .map(|number| number.to_string())
            .ok_or_else(|| invalid(format!("use a whole number from {min} to {max}"))),
        SettingKind::Choice(options) => {
            let lowered = value.to_ascii_lowercase();
            options
                .iter()
                .find(|option| **option == lowered)
                .map(|option| (*option).to_owned())
                .ok_or_else(|| invalid(format!("use one of: {}", options.join(", "))))
        }
        SettingKind::Language => {
            let lowered = value.to_ascii_lowercase();
            let is_code = (2..=3).contains(&lowered.len())
                && lowered
                    .chars()
                    .all(|character| character.is_ascii_lowercase());
            if lowered == "auto" || is_code {
                Ok(lowered)
            } else {
                Err(invalid(
                    "use auto or a language code such as it or en".to_owned(),
                ))
            }
        }
        SettingKind::Multiaddress => value
            .parse::<libp2p::Multiaddr>()
            .ok()
            .filter(|_| !value.is_empty())
            .map(|address| address.to_string())
            .ok_or_else(|| invalid("use an address such as /ip4/0.0.0.0/tcp/4001".to_owned())),
    }
}

/// The saved overrides that are allowed and valid, for `ffi_settings` to apply at start.
///
/// Inputs: The app's data folder.
/// Outputs: Variable name → normalized value; empty when there is no file, it is unreadable, or
/// the section is locked. Unknown names and invalid values are dropped.
/// Side effects: Reads the file; logs the names (never the values) of dropped and applied entries.
pub fn load_overrides(data_directory: &Path) -> HashMap<&'static str, String> {
    let state = read_file(data_directory);
    if !state.unlocked {
        return HashMap::new();
    }
    let mut overrides = HashMap::new();
    for (name, raw) in &state.overrides {
        match find(name).map(|setting| (setting, normalize(setting, raw))) {
            Some((setting, Ok(value))) => {
                tracing::info!(setting = setting.name, "developer override applied");
                overrides.insert(setting.name, value);
            }
            _ => tracing::warn!(setting = %name, "developer override ignored"),
        }
    }
    overrides
}

/// The request accepted by `run_developer_command`.
#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Get,
    Unlock,
    Lock,
    Reset,
    Set { name: String, value: Option<String> },
}

/// Runs one Settings → Developer request and answers in JSON.
///
/// Inputs: The app's data folder; `in_effect`, the value the running app uses for a variable
/// (`app_setting`); `at_start`, the overrides loaded when the app started; and the request:
/// `{"command":"get"|"unlock"|"lock"|"reset"}` or `{"command":"set","name","value":str|null}`.
/// Outputs: `{"ok":true,"unlocked","restart_needed","settings":[{"name","kind","description",
/// "default","options"?,"min"?,"max"?,"saved","in_effect"}]}`, or `{"ok":false,"error_code",
/// "error"}` with `invalid_argument`, `locked`, `unknown_setting`, `invalid_value` (the message
/// says what is accepted) or `storage_failed`.
/// Side effects: `unlock` shows the section; `set` saves or clears one override (only while
/// unlocked); `reset` clears every override; `lock` clears them and hides the section. Each
/// writes the file in the data folder. Nothing changes in the running app until it restarts.
pub fn run_developer_command(
    data_directory: &Path,
    in_effect: &dyn Fn(&str) -> Option<String>,
    at_start: &HashMap<&'static str, String>,
    request_json: &str,
) -> String {
    match handle(data_directory, request_json) {
        Ok(state) => listing(&state, in_effect, at_start).to_string(),
        Err(error) => {
            json!({"ok": false, "error_code": error.code(), "error": error.to_string()}).to_string()
        }
    }
}

/// Applies the request and returns the state to list.
fn handle(data_directory: &Path, request_json: &str) -> Result<DeveloperFile, DeveloperError> {
    let request: Request =
        serde_json::from_str(request_json).map_err(|_| DeveloperError::InvalidArgument)?;
    let mut state = read_file(data_directory);
    match request {
        Request::Get => return Ok(state),
        Request::Unlock => state.unlocked = true,
        Request::Lock => {
            state.unlocked = false;
            state.overrides.clear();
        }
        Request::Reset => state.overrides.clear(),
        Request::Set { name, value } => {
            if !state.unlocked {
                return Err(DeveloperError::Locked);
            }
            let setting = find(&name).ok_or(DeveloperError::UnknownSetting)?;
            match value {
                Some(raw) => {
                    let value = normalize(setting, &raw)?;
                    state.overrides.insert(setting.name.to_owned(), value);
                }
                None => {
                    state.overrides.remove(setting.name);
                }
            }
            tracing::info!(setting = setting.name, "developer override saved");
        }
    }
    write_file(data_directory, &state)?;
    Ok(state)
}

/// The `ok` answer for `state`.
fn listing(
    state: &DeveloperFile,
    in_effect: &dyn Fn(&str) -> Option<String>,
    at_start: &HashMap<&'static str, String>,
) -> Value {
    let saved = |name: &str| {
        find(name).and_then(|setting| {
            state
                .overrides
                .get(name)
                .and_then(|raw| normalize(setting, raw).ok())
        })
    };
    let settings: Vec<Value> = DEVELOPER_SETTINGS
        .iter()
        .map(|setting| {
            let mut entry = json!({
                "name": setting.name,
                "description": setting.description,
                "default": setting.default,
                "saved": saved(setting.name),
                "in_effect": in_effect(setting.name),
            });
            let kind = match setting.kind {
                SettingKind::Flag => "flag",
                SettingKind::Integer { min, max } => {
                    entry["min"] = json!(min);
                    entry["max"] = json!(max);
                    "integer"
                }
                SettingKind::Choice(options) => {
                    entry["options"] = json!(options);
                    "choice"
                }
                SettingKind::Language => "language",
                SettingKind::Multiaddress => "multiaddress",
            };
            entry["kind"] = json!(kind);
            entry
        })
        .collect();
    let now: HashMap<&'static str, String> = if state.unlocked {
        DEVELOPER_SETTINGS
            .iter()
            .filter_map(|setting| saved(setting.name).map(|value| (setting.name, value)))
            .collect()
    } else {
        HashMap::new()
    };
    json!({
        "ok": true,
        "unlocked": state.unlocked,
        "restart_needed": &now != at_start,
        "settings": settings,
    })
}

/// Runs one Settings → Developer request for the app (#187).
///
/// Inputs: A NUL-terminated UTF-8 JSON request (see `run_developer_command`).
/// Outputs: Caller-freed JSON as `run_developer_command` describes, or `{"ok":false,
/// "error_code":"invalid_argument"|"not_configured","error"}` for a null request or a core the
/// app has not configured. Returns null only if the answer cannot be allocated.
/// Side effects: Those of the command; it never touches the running pipeline, sync or bridge.
///
/// # Safety
/// `request_json` must be null or a valid NUL-terminated string that stays alive for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_developer_settings(
    request_json: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    let failure = |code: &str, message: &str| {
        json!({"ok": false, "error_code": code, "error": message}).to_string()
    };
    let answer = match (
        unsafe { crate::ffi::ffi_string(request_json) },
        crate::ffi_settings::APP_SETTINGS.configured(),
    ) {
        (None, _) => failure("invalid_argument", "the request must be a UTF-8 C string"),
        (_, None) => failure("not_configured", "the app has not configured the core"),
        (Some(request_json), Some(settings)) => run_developer_command(
            settings.data_directory(),
            &crate::ffi_settings::app_setting,
            settings.developer_overrides(),
            request_json,
        ),
    };
    std::ffi::CString::new(answer)
        .map(std::ffi::CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

#[cfg(test)]
#[path = "ffi_developer_tests.rs"]
mod tests;
