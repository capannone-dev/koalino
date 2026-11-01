//! Keeps the owner's choice of Whisper model and transcription provider, and the OpenAI key.
//!
//! The choices live in `<db>.transcription-settings.json`. The key never does: it stays in the
//! platform keychain, or on Android in a file sealed under the Keystore-protected database key.
//! Saved values replace the environment's, and the daemon and CLI keep using the environment.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use zeroize::Zeroizing;

use crate::{
    adapters::keychain::{
        platform_keychain::PlatformKeychain, sealed_file_keychain::SealedFileKeychain,
    },
    domain::{errors::CryptoError, whisper_catalog::find_whisper_model},
    pipeline_composition::{PipelineConfig, TranscriptionProvider, WHISPER_MODEL_VARIABLE},
    ports::crypto::KeychainStore,
};

/// Keychain service of the OpenAI key, separate from the database key's.
const OPENAI_KEYCHAIN_SERVICE: &str = "note-taker-openai";
/// Longest accepted OpenAI key; real keys are far shorter.
const MAXIMUM_KEY_LENGTH: usize = 512;

/// The provider the owner chose in Settings.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SavedProvider {
    /// Transcribe on this device.
    Local,
    /// Send recordings to OpenAI, which the owner explicitly confirmed.
    Openai,
}

/// The stored choices; anything missing leaves the environment's value in place.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedTranscriptionSettings {
    /// The chosen provider.
    pub provider: Option<SavedProvider>,
    /// The chosen model's file name, from the catalog.
    pub whisper_model: Option<String>,
    /// The chosen OpenAI model, such as `whisper-1`.
    pub openai_model: Option<String>,
    /// True when local transcription should use efficient mode, on a phone GPU that supports it.
    #[serde(default)]
    pub efficient_gpu: bool,
}

/// Where the settings, downloaded models and sealed key live.
#[derive(Clone, Debug)]
pub struct TranscriptionPaths {
    /// The JSON file holding the choices.
    pub settings_file: PathBuf,
    /// `<data directory>/models`, where downloads go.
    pub models_directory: PathBuf,
    /// The sealed key file, used only on Android.
    pub sealed_key_file: PathBuf,
    /// The model the app ships or the environment names, which cannot be removed.
    pub shipped_model: Option<PathBuf>,
    /// Exists while a GPU attempt is unfinished; finding it at the start means a crash (#76).
    pub gpu_attempt_marker: PathBuf,
}

impl TranscriptionPaths {
    /// Derives the paths from the settings lookup.
    ///
    /// Inputs: The lookup for `NOTE_TAKER_DATABASE_PATH`, the key folder and the shipped model.
    /// Outputs: The paths. Side effects: None.
    pub fn from_lookup(setting: &dyn Fn(&str) -> Option<String>) -> Self {
        let database_path =
            setting("NOTE_TAKER_DATABASE_PATH").unwrap_or_else(|| "note_taker.sqlite3".to_owned());
        let database_path = PathBuf::from(database_path);
        let data_directory = database_path
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf);
        let mut gpu_attempt_marker = database_path.clone().into_os_string();
        gpu_attempt_marker.push(".gpu-attempt");
        let mut settings_file = database_path.into_os_string();
        settings_file.push(".transcription-settings.json");
        let key_directory = setting("NOTE_TAKER_ENCRYPTION_KEY_DIR")
            .map_or_else(|| data_directory.join("keys"), PathBuf::from);
        Self {
            settings_file: PathBuf::from(settings_file),
            models_directory: data_directory.join("models"),
            sealed_key_file: key_directory.join("openai_key.sealed"),
            shipped_model: setting(WHISPER_MODEL_VARIABLE).map(PathBuf::from),
            gpu_attempt_marker: PathBuf::from(gpu_attempt_marker),
        }
    }

    /// Returns the shipped model's file name, such as `ggml-base.bin`.
    pub fn shipped_file_name(&self) -> Option<&str> {
        self.shipped_model
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
    }

    /// Finds a model file on this device.
    ///
    /// Inputs: A file name. Outputs: Its path in the models folder, else the shipped copy, else
    /// `None`. Side effects: Checks files.
    pub fn locate_model(&self, file_name: &str) -> Option<PathBuf> {
        find_whisper_model(file_name)?;
        let own_copy = self.models_directory.join(file_name);
        if own_copy.is_file() {
            return Some(own_copy);
        }
        self.shipped_model
            .clone()
            .filter(|_| self.shipped_file_name() == Some(file_name))
            .filter(|shipped| shipped.is_file())
    }

    /// Reports whether a model file can be removed: only downloads can, not the shipped model.
    pub fn is_removable(&self, file_name: &str) -> bool {
        self.shipped_file_name() != Some(file_name)
            && find_whisper_model(file_name).is_some()
            && self.models_directory.join(file_name).is_file()
    }
}

/// Reads the saved choices.
///
/// Inputs: The JSON file's path. Outputs: The choices; a missing or damaged file counts as none,
/// so recordings can always start. Side effects: Reads the file.
pub fn load_settings(settings_file: &Path) -> SavedTranscriptionSettings {
    fs::read(settings_file)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Stores the choices.
///
/// Inputs: The JSON file's path and the choices. Outputs: Success or the I/O error.
/// Side effects: Replaces the file through a temporary one, so a crash cannot damage it.
pub fn save_settings(
    settings_file: &Path,
    settings: &SavedTranscriptionSettings,
) -> io::Result<()> {
    let bytes = serde_json::to_vec(settings)?;
    if let Some(parent) = settings_file.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let temporary = settings_file.with_extension("json.tmp");
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, settings_file)
}

/// Layers the saved choices over a configuration.
///
/// Inputs: The configuration from the environment, the choices, the paths, and the saved key.
/// Outputs: The configuration to run. A saved key alone selects Local, so saving a key never
/// sends audio anywhere. A chosen model that is missing keeps the previous model.
/// Side effects: Checks that the chosen model exists.
pub fn apply_saved_settings(
    config: PipelineConfig,
    saved: &SavedTranscriptionSettings,
    paths: &TranscriptionPaths,
    saved_key: Option<String>,
) -> PipelineConfig {
    let model_path = saved.whisper_model.as_deref().and_then(|file_name| {
        let located = paths.locate_model(file_name);
        if located.is_none() {
            tracing::warn!(
                file_name,
                "the chosen Whisper model is missing; keeping the previous one"
            );
        }
        located
    });
    let provider = match (saved.provider, saved_key.is_some()) {
        (Some(SavedProvider::Openai), _) => Some(TranscriptionProvider::OpenAi),
        (Some(SavedProvider::Local), _) | (None, true) => Some(TranscriptionProvider::Local),
        (None, false) => None,
    };
    config
        .with_saved_transcription(
            provider,
            model_path,
            saved_key,
            saved.openai_model.clone(),
            saved.provider == Some(SavedProvider::Openai),
        )
        .with_gpu_choices(saved.efficient_gpu, paths.gpu_attempt_marker.clone())
}

/// Checks a pasted key's form, without judging whether OpenAI accepts it.
///
/// Inputs: The text. Outputs: The trimmed key, or `None` when blank, too long or containing
/// whitespace or control characters. Side effects: None.
pub fn validated_key(text: &str) -> Option<&str> {
    let key = text.trim();
    (!key.is_empty()
        && key.len() <= MAXIMUM_KEY_LENGTH
        && !key.chars().any(|c| c.is_whitespace() || c.is_control()))
    .then_some(key)
}

/// The place the OpenAI key is kept.
pub enum OpenAiKeyStore {
    /// The operating system's credential store.
    Platform(PlatformKeychain),
    /// A file sealed under the Keystore-protected database key.
    Sealed(SealedFileKeychain),
}

impl OpenAiKeyStore {
    /// Opens the right store for this platform.
    ///
    /// Inputs: The paths and the host-provided database key (Android). Outputs: The store; one
    /// that does not exist on this machine reports `KeychainUnavailable` when used.
    /// Side effects: May initialize the platform's credential store.
    pub fn open(paths: &TranscriptionPaths, host_key: Option<Vec<u8>>) -> Self {
        if cfg!(target_os = "android") || host_key.is_some() {
            Self::Sealed(SealedFileKeychain::new(
                paths.sealed_key_file.clone(),
                host_key,
            ))
        } else {
            Self::Platform(PlatformKeychain::for_service(OPENAI_KEYCHAIN_SERVICE))
        }
    }

    /// Reads the key.
    ///
    /// Outputs: The key, `None` when none is saved, or the store's error; never logged.
    /// Side effects: Queries the store.
    pub fn load(&self) -> Result<Option<Zeroizing<String>>, CryptoError> {
        let stored = match self {
            Self::Platform(store) => store.load_key(),
            Self::Sealed(store) => store.load_key(),
        }?;
        Ok(stored.and_then(|bytes| String::from_utf8(bytes.to_vec()).ok().map(Zeroizing::new)))
    }

    /// Saves the key. Outputs: Success or the store's error. Side effects: Writes the store.
    pub fn save(&self, key: &str) -> Result<(), CryptoError> {
        match self {
            Self::Platform(store) => store.store_key(key.as_bytes()),
            Self::Sealed(store) => store.store_key(key.as_bytes()),
        }
    }

    /// Deletes the key. Outputs: Success or the store's error. Side effects: Writes the store.
    pub fn remove(&self) -> Result<(), CryptoError> {
        match self {
            Self::Platform(store) => store.delete_key(),
            Self::Sealed(store) => store.delete_key(),
        }
    }
}

/// What testing a key found.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyTestResult {
    /// OpenAI accepted the key.
    Valid,
    /// OpenAI refused the key.
    Invalid,
    /// The account has hit a limit or used up its quota.
    QuotaUsed,
    /// OpenAI could not be reached.
    NoNetwork,
    /// OpenAI answered with something else.
    Unexpected,
}

impl KeyTestResult {
    /// Maps an HTTP status from the models endpoint. Inputs: The status. Side effects: None.
    pub fn from_status(status: u16) -> Self {
        match status {
            200..=299 => Self::Valid,
            401 | 403 => Self::Invalid,
            402 | 429 => Self::QuotaUsed,
            _ => Self::Unexpected,
        }
    }

    /// Returns the stable label the app switches on. Side effects: None.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Valid => "valid",
            Self::Invalid => "invalid",
            Self::QuotaUsed => "quota_used",
            Self::NoNetwork => "no_network",
            Self::Unexpected => "unexpected",
        }
    }
}

/// Asks OpenAI whether a key works. This lists models and sends no audio.
///
/// Inputs: The key. Outputs: What the server said.
/// Side effects: One HTTPS request to `api.openai.com`; the key is never logged.
pub fn test_openai_key(key: &str) -> KeyTestResult {
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
    else {
        return KeyTestResult::Unexpected;
    };
    match client
        .get("https://api.openai.com/v1/models")
        .bearer_auth(key)
        .send()
    {
        Ok(response) => KeyTestResult::from_status(response.status().as_u16()),
        Err(_) => KeyTestResult::NoNetwork,
    }
}
