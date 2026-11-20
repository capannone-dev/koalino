//! Keeps the owner's language model settings (#95, #101): the connections, the default one, the
//! summary template, whether meetings are summarized automatically, and which connection answers
//! paired devices' summary requests (#107).
//!
//! The settings live per device in `<db>.llm-settings.json`, beside the transcription settings.
//! API keys never do (#138): an online connection's key is kept in the platform keychain under
//! `note-taker-llm-<id>`, or on Android in `<key folder>/llm-<id>.sealed`, sealed under the
//! Keystore-protected database key. Connections of a removed kind (Anthropic) are dropped when the
//! file is read.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use zeroize::Zeroizing;

use crate::{
    adapters::keychain::{
        platform_keychain::PlatformKeychain, sealed_file_keychain::SealedFileKeychain,
    },
    domain::{
        errors::CryptoError,
        llm::{LlmConnection, PromptTemplate, PromptTemplateError},
    },
    ports::crypto::KeychainStore,
};

/// Why a default could not be chosen.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LlmSettingsError {
    /// No connection has that ID.
    #[error("there is no such connection")]
    UnknownConnection,
}

/// The stored settings, as written to the JSON file.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedLlmSettings {
    /// The connections, in the order the owner added them.
    #[serde(default, deserialize_with = "known_connections")]
    connections: Vec<LlmConnection>,
    /// The connection used when none is named.
    #[serde(default)]
    default_connection: Option<String>,
    /// The owner's summary template; `None` uses the built-in one.
    #[serde(default)]
    summary_template: Option<String>,
    /// Summarize each meeting once its transcript is final (#101); the owner can opt out.
    #[serde(default = "automatic_summaries_by_default")]
    auto_summarize: bool,
    /// The connection that writes the summaries paired devices ask for (#107); `None`, the
    /// default, leaves their requests unanswered on this device.
    #[serde(default)]
    paired_summary_connection: Option<String>,
}

/// Reads the connections, skipping any the app can no longer read (a removed kind).
fn known_connections<'de, D>(deserializer: D) -> Result<Vec<LlmConnection>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = <Vec<serde_json::Value> as serde::Deserialize>::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect())
}

/// Automatic summaries are on until the owner opts out (#101).
fn automatic_summaries_by_default() -> bool {
    true
}

impl Default for SavedLlmSettings {
    fn default() -> Self {
        Self {
            connections: Vec::new(),
            default_connection: None,
            summary_template: None,
            auto_summarize: automatic_summaries_by_default(),
            paired_summary_connection: None,
        }
    }
}

impl SavedLlmSettings {
    /// Lists the connections. Side effects: None.
    pub fn connections(&self) -> &[LlmConnection] {
        &self.connections
    }

    /// Finds a connection by ID. Side effects: None.
    pub fn connection(&self, id: &str) -> Option<&LlmConnection> {
        self.connections
            .iter()
            .find(|connection| connection.id() == id)
    }

    /// Returns the connection to use when none is named.
    ///
    /// Outputs: The chosen default; else the only connection when there is exactly one; else
    /// `None`. Side effects: None.
    pub fn default_connection(&self) -> Option<&LlmConnection> {
        self.default_connection
            .as_deref()
            .and_then(|id| self.connection(id))
            .or(match self.connections.as_slice() {
                [only] => Some(only),
                _ => None,
            })
    }

    /// Returns the ID the owner chose as default, if it still exists. Side effects: None.
    pub fn chosen_default_id(&self) -> Option<&str> {
        self.default_connection
            .as_deref()
            .filter(|id| self.connection(id).is_some())
    }

    /// Adds a connection, or replaces the one with the same ID in its place.
    ///
    /// Inputs: The checked connection. Side effects: Changes these settings only.
    pub fn upsert_connection(&mut self, connection: LlmConnection) {
        match self
            .connections
            .iter_mut()
            .find(|existing| existing.id() == connection.id())
        {
            Some(existing) => *existing = connection,
            None => self.connections.push(connection),
        }
    }

    /// Removes a connection; a default pointing at it is cleared.
    ///
    /// Outputs: Whether it existed. Side effects: Changes these settings only.
    pub fn remove_connection(&mut self, id: &str) -> bool {
        let before = self.connections.len();
        self.connections.retain(|connection| connection.id() != id);
        if self.default_connection.as_deref() == Some(id) {
            self.default_connection = None;
        }
        if self.paired_summary_connection.as_deref() == Some(id) {
            self.paired_summary_connection = None;
        }
        self.connections.len() != before
    }

    /// Chooses the default connection, or clears the choice.
    ///
    /// Outputs: `UnknownConnection` when no connection has that ID. Side effects: Changes these
    /// settings only.
    pub fn set_default(&mut self, id: Option<&str>) -> Result<(), LlmSettingsError> {
        if let Some(id) = id
            && self.connection(id).is_none()
        {
            return Err(LlmSettingsError::UnknownConnection);
        }
        self.default_connection = id.map(str::to_owned);
        Ok(())
    }

    /// Returns the summary template: the owner's, or the built-in one. Side effects: None.
    pub fn template(&self) -> PromptTemplate {
        self.summary_template
            .as_deref()
            .and_then(|text| PromptTemplate::parse(text).ok())
            .unwrap_or_else(PromptTemplate::default_summary)
    }

    /// Reports whether the owner wrote their own template. Side effects: None.
    pub fn has_custom_template(&self) -> bool {
        self.summary_template.is_some()
    }

    /// Stores the owner's template, or goes back to the built-in one with `None`.
    ///
    /// Outputs: The template's problem, if any. Side effects: Changes these settings only.
    pub fn set_template(&mut self, text: Option<&str>) -> Result<(), PromptTemplateError> {
        self.summary_template = match text {
            Some(text) => Some(PromptTemplate::parse(text)?.as_str().to_owned()),
            None => None,
        };
        Ok(())
    }

    /// Reports whether meetings are summarized automatically. Side effects: None.
    pub fn auto_summarize(&self) -> bool {
        self.auto_summarize
    }

    /// Turns automatic summaries on or off. Side effects: Changes these settings only.
    pub fn set_auto_summarize(&mut self, enabled: bool) {
        self.auto_summarize = enabled;
    }

    /// Returns the connection that answers paired devices' summary requests (#107), if the
    /// owner chose one that still exists. Side effects: None.
    pub fn paired_summary_connection(&self) -> Option<&LlmConnection> {
        self.paired_summary_connection
            .as_deref()
            .and_then(|id| self.connection(id))
    }

    /// Chooses the connection that answers paired devices' summary requests, or `None` to stop
    /// answering them.
    ///
    /// Outputs: `UnknownConnection` when no connection has that ID. Side effects: Changes these
    /// settings only.
    pub fn set_paired_summary_connection(
        &mut self,
        id: Option<&str>,
    ) -> Result<(), LlmSettingsError> {
        if let Some(id) = id
            && self.connection(id).is_none()
        {
            return Err(LlmSettingsError::UnknownConnection);
        }
        self.paired_summary_connection = id.map(str::to_owned);
        Ok(())
    }

    /// Drops what is no longer valid, such as a hand-edited connection to a public plain-HTTP
    /// host or an unparsable template. Side effects: Logs counts only.
    fn revalidated(mut self) -> Self {
        let before = self.connections.len();
        self.connections = std::mem::take(&mut self.connections)
            .into_iter()
            .filter_map(|connection| connection.revalidated().ok())
            .collect();
        if self.connections.len() != before {
            tracing::warn!(
                dropped = before - self.connections.len(),
                "ignored LLM connections that are no longer valid"
            );
        }
        if self
            .summary_template
            .as_deref()
            .is_some_and(|text| PromptTemplate::parse(text).is_err())
        {
            tracing::warn!("ignored a summary template that is no longer valid");
            self.summary_template = None;
        }
        if self.chosen_default_id().is_none() {
            self.default_connection = None;
        }
        if self.paired_summary_connection().is_none() {
            self.paired_summary_connection = None;
        }
        self
    }
}

/// Returns the settings file kept beside a database: `<database>.llm-settings.json`.
///
/// Side effects: None.
pub fn llm_settings_file_for(database_path: &Path) -> PathBuf {
    let mut settings_file = database_path.as_os_str().to_owned();
    settings_file.push(".llm-settings.json");
    PathBuf::from(settings_file)
}

/// Reads the settings.
///
/// Inputs: The JSON file's path. Outputs: The settings; a missing or damaged file counts as
/// none, and invalid entries are dropped. Side effects: Reads the file.
pub fn load_llm_settings(settings_file: &Path) -> SavedLlmSettings {
    fs::read(settings_file)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<SavedLlmSettings>(&bytes).ok())
        .unwrap_or_default()
        .revalidated()
}

/// Stores the settings.
///
/// Inputs: The JSON file's path and the settings. Outputs: Success or the I/O error.
/// Side effects: Replaces the file through a temporary one, so a crash cannot damage it.
pub fn save_llm_settings(settings_file: &Path, settings: &SavedLlmSettings) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(settings)?;
    if let Some(parent) = settings_file.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let temporary = settings_file.with_extension("json.tmp");
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, settings_file)
}

/// Deletes the sealed OpenAI transcription key an earlier version kept on Android (#117).
/// Connection keys (`llm-<id>.sealed`) are kept again since #138.
///
/// Inputs: The folder for sealed keys. Outputs: How many files were deleted. Side effects: Removes
/// files; a folder that cannot be read deletes nothing.
pub fn forget_sealed_api_keys(key_directory: &Path) -> usize {
    let Ok(entries) = fs::read_dir(key_directory) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name == "openai_key.sealed"
        })
        .filter(|entry| fs::remove_file(entry.path()).is_ok())
        .count()
}

/// Prefix of each connection's keychain service (#138; the name the app used before #117).
const LLM_KEYCHAIN_SERVICE_PREFIX: &str = "note-taker-llm-";

/// Keeps the API key of each online connection (#138); tests use an in-memory one.
pub trait LlmKeyring: Send + Sync {
    /// Reads a connection's key. Outputs: The key, `None` when none is saved, or the store's
    /// error; never logged. Side effects: Queries the store.
    fn load(&self, connection_id: &str) -> Result<Option<Zeroizing<String>>, CryptoError>;
    /// Saves a connection's key. Side effects: Writes the store.
    fn save(&self, connection_id: &str, key: &str) -> Result<(), CryptoError>;
    /// Deletes a connection's key; a missing one is not an error. Side effects: Writes the store.
    fn remove(&self, connection_id: &str) -> Result<(), CryptoError>;
}

/// The platform keychain (`note-taker-llm-<id>`), or on Android and wherever the host provides
/// the database key, a file `llm-<id>.sealed` sealed under that key.
pub struct PlatformLlmKeyring {
    key_directory: PathBuf,
    host_key: Option<Zeroizing<Vec<u8>>>,
    /// False when the owner turned the platform keychain off (`NOTE_TAKER_KEYCHAIN_ENABLED`):
    /// keys then go only to a sealed file under a host key, or nowhere (#142 follow-up).
    keychain_enabled: bool,
}

impl PlatformLlmKeyring {
    /// Creates the keyring.
    ///
    /// Inputs: The folder for sealed keys and the host-provided database key (Android).
    /// Outputs: The keyring. Side effects: None.
    pub fn new(key_directory: PathBuf, host_key: Option<Vec<u8>>) -> Self {
        Self {
            key_directory,
            host_key: host_key.map(Zeroizing::new),
            keychain_enabled: true,
        }
    }

    /// Follows the owner's keychain switch: with it off and no host key, keys cannot be saved
    /// (`KeychainUnavailable`), and the platform keychain is never touched, so no prompt appears.
    ///
    /// Inputs: Whether the platform keychain may be used. Outputs: The keyring.
    /// Side effects: None.
    #[must_use]
    pub fn with_keychain_enabled(mut self, keychain_enabled: bool) -> Self {
        self.keychain_enabled = keychain_enabled;
        self
    }

    /// The store for one connection. Side effects: May initialize the platform store.
    fn store(&self, connection_id: &str) -> ConnectionKeyStore {
        if cfg!(target_os = "android") || self.host_key.is_some() || !self.keychain_enabled {
            ConnectionKeyStore::Sealed(SealedFileKeychain::new(
                self.key_directory
                    .join(format!("llm-{connection_id}.sealed")),
                self.host_key.as_ref().map(|key| key.to_vec()),
            ))
        } else {
            ConnectionKeyStore::Platform(PlatformKeychain::for_account(
                &format!("{LLM_KEYCHAIN_SERVICE_PREFIX}{connection_id}"),
                "api-key",
                "Koalino AI connection key",
            ))
        }
    }
}

/// Where one connection's key is kept.
enum ConnectionKeyStore {
    /// The operating system's credential store.
    Platform(PlatformKeychain),
    /// A file sealed under the host-provided database key.
    Sealed(SealedFileKeychain),
}

impl LlmKeyring for PlatformLlmKeyring {
    fn load(&self, connection_id: &str) -> Result<Option<Zeroizing<String>>, CryptoError> {
        let stored = match self.store(connection_id) {
            ConnectionKeyStore::Platform(store) => store.load_key(),
            ConnectionKeyStore::Sealed(store) => store.load_key(),
        }?;
        Ok(stored.and_then(|bytes| String::from_utf8(bytes.to_vec()).ok().map(Zeroizing::new)))
    }
    fn save(&self, connection_id: &str, key: &str) -> Result<(), CryptoError> {
        match self.store(connection_id) {
            ConnectionKeyStore::Platform(store) => store.store_key(key.as_bytes()),
            ConnectionKeyStore::Sealed(store) => store.store_key(key.as_bytes()),
        }
    }
    fn remove(&self, connection_id: &str) -> Result<(), CryptoError> {
        match self.store(connection_id) {
            ConnectionKeyStore::Platform(store) => store.delete_key(),
            ConnectionKeyStore::Sealed(store) => store.delete_key(),
        }
    }
}

/// Keys kept in memory, for tests.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct MemoryLlmKeyring(std::sync::Mutex<std::collections::HashMap<String, String>>);

#[cfg(test)]
impl LlmKeyring for MemoryLlmKeyring {
    fn load(&self, connection_id: &str) -> Result<Option<Zeroizing<String>>, CryptoError> {
        Ok(self
            .0
            .lock()
            .ok()
            .and_then(|keys| keys.get(connection_id).cloned())
            .map(Zeroizing::new))
    }
    fn save(&self, connection_id: &str, key: &str) -> Result<(), CryptoError> {
        if let Ok(mut keys) = self.0.lock() {
            keys.insert(connection_id.to_owned(), key.to_owned());
        }
        Ok(())
    }
    fn remove(&self, connection_id: &str) -> Result<(), CryptoError> {
        if let Ok(mut keys) = self.0.lock() {
            keys.remove(connection_id);
        }
        Ok(())
    }
}
