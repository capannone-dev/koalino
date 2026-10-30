//! Holds the settings the Flutter app passes to `note_taker_configure`, layered over the
//! environment (Issue #43).
//!
//! A desktop process inherits its environment, but an Android app has none: its working directory
//! is read-only, so the core could not open its database. The app instead passes its private data
//! directory and options as JSON once, before any other call. They are stored under the variable
//! names the composition roots already read (`NOTE_TAKER_DATABASE_PATH`, `NOTE_TAKER_WHISPER_MODEL`,
//! ...), and every FFI read goes through `app_setting`, which falls back to the environment. The
//! process environment is never modified (`set_var` is unsafe with threads running).

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use serde::Deserialize;

/// Why a configuration was refused.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    /// The JSON or one of its fields is invalid; the message names the field, never its value.
    #[error("invalid configuration: {0}")]
    Invalid(String),
    /// The settings were already configured, or the runtime already started without them.
    #[error("the app is already configured")]
    AlreadyConfigured,
}

/// The JSON accepted by `note_taker_configure`; only `data_directory` is required.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AppConfiguration {
    /// Absolute app-private directory for the database, keys, logs, clips, and exports.
    data_directory: PathBuf,
    /// Name shown to other devices when pairing.
    device_name: Option<String>,
    /// Absolute path of the GGML Whisper model for on-device transcription.
    whisper_model_path: Option<PathBuf>,
    /// Whether to keep the encryption key in the platform keychain (unavailable on Android).
    keychain_enabled: Option<bool>,
    /// Whether to run P2P sync; on by default.
    p2p_enabled: Option<bool>,
    /// `.../p2p/<peer id>` addresses of the owner's devices that mDNS cannot find.
    p2p_peers: Option<Vec<String>>,
    /// `.../p2p/<peer id>` address of the owner's relay, for reaching devices across NATs.
    p2p_relay: Option<String>,
    /// Absolute directory for exports; defaults to `exports` in the data directory.
    export_directory: Option<PathBuf>,
}

/// Validated settings, keyed by the variable names the composition roots read.
pub struct ConfiguredSettings {
    values: HashMap<&'static str, String>,
}

impl ConfiguredSettings {
    /// Parses and validates the app's configuration JSON.
    ///
    /// Inputs: JSON matching `AppConfiguration`; paths must be absolute, a device name must not be
    /// blank, and a peer address must not contain a comma (the list is stored comma-separated).
    /// Outputs: The settings, or `Invalid` naming the offending field.
    /// Side effects: None; directories are created when the runtime opens its stores.
    pub fn from_json(json: &str) -> Result<Self, SettingsError> {
        let configuration: AppConfiguration = serde_json::from_str(json)
            .map_err(|error| SettingsError::Invalid(format!("unreadable JSON: {error}")))?;
        let data_directory = absolute(&configuration.data_directory, "data_directory")?;
        let mut values: HashMap<&'static str, String> = HashMap::new();
        for (name, file) in [
            ("NOTE_TAKER_DATABASE_PATH", "note_taker.sqlite3"),
            ("NOTE_TAKER_ENCRYPTION_KEY_DIR", "keys"),
            ("NOTE_TAKER_LOG_DIR", "logs"),
            ("NOTE_TAKER_REVIEW_CLIP_DIR", "voice_review_clips"),
            ("NOTE_TAKER_ATTACHMENT_DIR", "attachments"),
            ("NOTE_TAKER_EXPORT_DIR", "exports"),
        ] {
            values.insert(name, data_directory.join(file).display().to_string());
        }
        if let Some(export_directory) = &configuration.export_directory {
            let export_directory = absolute(export_directory, "export_directory")?;
            values.insert(
                "NOTE_TAKER_EXPORT_DIR",
                export_directory.display().to_string(),
            );
        }
        if let Some(device_name) = configuration.device_name {
            if device_name.trim().is_empty() {
                return Err(SettingsError::Invalid("device_name is blank".to_owned()));
            }
            values.insert("NOTE_TAKER_DEVICE_NAME", device_name);
        }
        if let Some(model_path) = &configuration.whisper_model_path {
            let model_path = absolute(model_path, "whisper_model_path")?;
            values.insert("NOTE_TAKER_WHISPER_MODEL", model_path.display().to_string());
        }
        if let Some(keychain_enabled) = configuration.keychain_enabled {
            values.insert("NOTE_TAKER_KEYCHAIN_ENABLED", keychain_enabled.to_string());
        }
        if let Some(p2p_enabled) = configuration.p2p_enabled {
            values.insert("NOTE_TAKER_P2P_ENABLED", p2p_enabled.to_string());
        }
        if let Some(peers) = configuration.p2p_peers {
            if peers.iter().any(|peer| peer.contains(',')) {
                return Err(SettingsError::Invalid(
                    "a p2p_peers entry contains a comma".to_owned(),
                ));
            }
            values.insert("NOTE_TAKER_P2P_PEERS", peers.join(","));
        }
        if let Some(relay) = configuration.p2p_relay {
            values.insert("NOTE_TAKER_P2P_RELAY", relay);
        }
        Ok(Self { values })
    }

    /// Returns the configured value of `name`, else the fallback's.
    ///
    /// Inputs: A variable name and the fallback (the environment in production).
    /// Outputs: The value, or `None` when neither has it.
    /// Side effects: Those of `fallback`.
    pub fn lookup(&self, name: &str, fallback: impl Fn(&str) -> Option<String>) -> Option<String> {
        self.values.get(name).cloned().or_else(|| fallback(name))
    }
}

/// Requires `path` to be non-empty and absolute.
fn absolute<'path>(path: &'path Path, field: &str) -> Result<&'path Path, SettingsError> {
    if path.as_os_str().is_empty() || !path.is_absolute() {
        return Err(SettingsError::Invalid(format!(
            "{field} must be an absolute path"
        )));
    }
    Ok(path)
}

/// Holds at most one configuration.
pub struct SettingsCell {
    settings: OnceLock<ConfiguredSettings>,
}

impl SettingsCell {
    /// Creates an empty cell.
    pub const fn new() -> Self {
        Self {
            settings: OnceLock::new(),
        }
    }

    /// Stores `settings` unless the cell already holds some.
    ///
    /// Outputs: Success, or `AlreadyConfigured`, leaving the first configuration in place.
    /// Side effects: Fills the cell.
    pub fn configure(&self, settings: ConfiguredSettings) -> Result<(), SettingsError> {
        self.settings
            .set(settings)
            .map_err(|_| SettingsError::AlreadyConfigured)
    }

    /// Returns the configured value of `name`, else the fallback's.
    pub fn lookup_with(
        &self,
        name: &str,
        fallback: impl Fn(&str) -> Option<String>,
    ) -> Option<String> {
        match self.settings.get() {
            Some(settings) => settings.lookup(name, fallback),
            None => fallback(name),
        }
    }
}

impl Default for SettingsCell {
    fn default() -> Self {
        Self::new()
    }
}

/// The process's settings, filled by `note_taker_configure`.
pub static APP_SETTINGS: SettingsCell = SettingsCell::new();

/// Reads a setting: the app's configured value, else the process environment.
///
/// Inputs: A variable name such as `NOTE_TAKER_DATABASE_PATH`.
/// Outputs: Its value, or `None`.
/// Side effects: Reads the environment when the app did not configure `name`.
pub fn app_setting(name: &str) -> Option<String> {
    APP_SETTINGS.lookup_with(name, |name| std::env::var(name).ok())
}

#[cfg(test)]
#[path = "ffi_settings_tests.rs"]
mod tests;
