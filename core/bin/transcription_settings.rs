//! Keeps the owner's transcription choices (Whisper model, glossary, how long speech waits to be
//! combined, and the rest of the transcription Settings).
//!
//! The choices live in `<db>.transcription-settings.json`. Transcription always runs on a device
//! the owner controls: the app keeps no API keys, so there is no cloud provider to choose. The
//! `provider` and `openai_model` an earlier version saved are ignored when read and dropped when
//! the file is next written. Saved values replace the environment's, and the daemon and CLI keep
//! using the environment.

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::{
    adapters::ai::npu_whisper::install::NpuInstall,
    application::{
        combined_meetings::{
            KEEP_FOR_COMBINING, MAX_KEEP_SPEECH_DAYS, MIN_KEEP_SPEECH_DAYS, keep_for_days,
        },
        pipeline::pipeline_factory::GlossarySource,
    },
    domain::{
        transcription_glossary::TranscriptionGlossary, whisper_catalog::find_whisper_model,
        whisper_suggestion::AutomaticModel,
    },
    pipeline_composition::{PipelineConfig, WHISPER_MODEL_VARIABLE},
};

/// Reads how long speech may wait to be combined; called once per worker round or status view.
pub type KeepForSource = std::sync::Arc<dyn Fn() -> Duration + Send + Sync>;

/// The stored choices; anything missing leaves the environment's value in place.
#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedTranscriptionSettings {
    /// The chosen model's file name, from the catalog.
    pub whisper_model: Option<String>,
    /// True when local transcription should use efficient mode, on a phone GPU that supports it.
    #[serde(default)]
    pub efficient_gpu: bool,
    /// Models whose proposal the owner dismissed, which are not proposed again.
    #[serde(default)]
    pub dismissed_proposals: Vec<String>,
    /// Whether each meeting is re-transcribed after Stop (#87); `None` keeps the default (on).
    #[serde(default)]
    pub transcript_pass: Option<bool>,
    /// Whether Automatic's model is downloaded by itself when it isn't on the device (#189); `None`
    /// keeps the default (on). The app still waits for an unmetered network on phones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_download: Option<bool>,
    /// The catalog file name of the model for that pass; `None` uses the live model.
    #[serde(default)]
    pub transcript_pass_model: Option<String>,
    /// Names and terms Whisper is prompted with, in the owner's order (user content, never logged).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub glossary: Vec<String>,
    /// Days a recording's speech may wait to be combined with another device's (#93); `None`
    /// keeps three weeks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep_speech_days: Option<u16>,
    /// Whether online-meeting participants' voices may be kept in the voice gallery (#103). Voice
    /// prints are biometric, so this is off until the owner turns it on.
    #[serde(default)]
    pub keep_meeting_voices: bool,
    /// Whether recordings also capture what the computer plays (#108); `None` keeps the default
    /// (on where the platform can).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub computer_sound: Option<bool>,
    /// Whether Whisper runs on the phone's NPU once its files are downloaded (#75); off until the
    /// owner turns it on.
    #[serde(default)]
    pub npu_whisper: bool,
    /// Whether meeting recordings are kept after processing (#140), for playback; off until the
    /// owner turns it on.
    #[serde(default)]
    pub keep_recordings: bool,
}

/// Where the settings, downloaded models and sealed key live.
#[derive(Clone, Debug)]
pub struct TranscriptionPaths {
    /// The JSON file holding the choices.
    pub settings_file: PathBuf,
    /// `<data directory>/models`, where downloads go.
    pub models_directory: PathBuf,
    /// The folder of sealed keys (the database key on Android).
    pub key_directory: PathBuf,
    /// The model the app ships or the environment names, which cannot be removed.
    pub shipped_model: Option<PathBuf>,
    /// Exists while a GPU attempt is unfinished; finding it at the start means a crash (#76).
    pub gpu_attempt_marker: PathBuf,
    /// The results of timing the clip on this device (#109), written by the after-meeting worker.
    pub speed_check_file: PathBuf,
    /// `<data directory>/npu`, where the NPU model and QNN libraries are downloaded (#75).
    pub npu_directory: PathBuf,
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
        let settings_file = settings_file_for(&database_path);
        let key_directory = setting("NOTE_TAKER_ENCRYPTION_KEY_DIR")
            .map_or_else(|| data_directory.join("keys"), PathBuf::from);
        Self {
            settings_file,
            models_directory: data_directory.join("models"),
            key_directory,
            shipped_model: setting(WHISPER_MODEL_VARIABLE).map(PathBuf::from),
            gpu_attempt_marker: PathBuf::from(gpu_attempt_marker),
            speed_check_file: crate::speed_check::record_file_for(&database_path),
            npu_directory: data_directory.join("npu"),
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

/// Returns the settings file kept beside a database: `<database>.transcription-settings.json`.
///
/// Side effects: None.
pub fn settings_file_for(database_path: &Path) -> PathBuf {
    let mut settings_file = database_path.as_os_str().to_owned();
    settings_file.push(".transcription-settings.json");
    PathBuf::from(settings_file)
}

/// Reads the saved glossary.
///
/// Outputs: The glossary, or `None` when there is none or the file holds one that is not valid
/// (edited by hand, for example), which is logged without its terms. Side effects: None.
pub fn saved_glossary(saved: &SavedTranscriptionSettings) -> Option<TranscriptionGlossary> {
    if saved.glossary.is_empty() {
        return None;
    }
    match TranscriptionGlossary::parse(saved.glossary.iter().map(String::as_str)) {
        Ok(glossary) => Some(glossary).filter(|glossary| !glossary.is_empty()),
        Err(error) => {
            tracing::warn!(error = %error, "the saved glossary is not valid; transcribing without it");
            None
        }
    }
}

/// Builds the source recordings and the after-meeting pass read the glossary from.
///
/// Inputs: The settings file. Outputs: A source that reads the file on each call, so a glossary
/// saved by any app, the CLI or the web applies to the next recording. Side effects: None until
/// called; each call reads the file.
pub fn glossary_source(settings_file: PathBuf) -> GlossarySource {
    std::sync::Arc::new(move || saved_glossary(&load_settings(&settings_file)))
}

/// Saves the glossary, keeping every other saved choice.
///
/// Inputs: The settings file and the glossary (empty clears it).
/// Outputs: `Ok(())` or the write error. Side effects: Rewrites the file atomically.
pub fn save_glossary(settings_file: &Path, glossary: &TranscriptionGlossary) -> io::Result<()> {
    let mut saved = load_settings(settings_file);
    saved.glossary = glossary.terms().to_vec();
    save_settings(settings_file, &saved)
}

/// Reads how long speech may wait to be combined on this device (#93).
///
/// Inputs: The saved choices. Outputs: The chosen window, or `KEEP_FOR_COMBINING` when none was
/// chosen or the file holds one out of range (edited by hand), which is logged. Side effects: None.
pub fn saved_keep_for(saved: &SavedTranscriptionSettings) -> Duration {
    match saved.keep_speech_days {
        None => KEEP_FOR_COMBINING,
        Some(days) => keep_for_days(days).unwrap_or_else(|| {
            tracing::warn!(
                days,
                "the saved speech retention window is out of range; using three weeks"
            );
            KEEP_FOR_COMBINING
        }),
    }
}

/// Expresses a window in whole days, as Settings shows it.
///
/// Side effects: None.
pub fn keep_speech_days(window: Duration) -> u16 {
    u16::try_from(window.as_secs() / (24 * 60 * 60)).unwrap_or(u16::MAX)
}

/// Reads the window from a settings file, if the process has one.
///
/// Inputs: The settings file, or `None` (the window is then `KEEP_FOR_COMBINING`).
/// Outputs: The window. Side effects: Reads the file.
pub fn keep_for_in(settings_file: Option<&Path>) -> Duration {
    settings_file.map_or(KEEP_FOR_COMBINING, |settings_file| {
        saved_keep_for(&load_settings(settings_file))
    })
}

/// Builds the source the after-meeting worker and the status views read the window from.
///
/// Inputs: The settings file. Outputs: A source that reads the file on each call, so a window
/// saved by any app, the CLI or the web applies from the worker's next round.
/// Side effects: None until called; each call reads the file.
pub fn keep_for_source(settings_file: PathBuf) -> KeepForSource {
    std::sync::Arc::new(move || saved_keep_for(&load_settings(&settings_file)))
}

/// Saves how many days speech may wait to be combined, keeping every other saved choice.
///
/// Inputs: The settings file and the days, `MIN_KEEP_SPEECH_DAYS..=MAX_KEEP_SPEECH_DAYS`.
/// Outputs: `Ok(())`, an `InvalidInput` error for days out of range (nothing is written), or the
/// write error. Side effects: Rewrites the file atomically. Speech already older than a shorter
/// window is deleted at the worker's next round.
pub fn save_keep_speech_days(settings_file: &Path, days: u16) -> io::Result<()> {
    if keep_for_days(days).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("choose between {MIN_KEEP_SPEECH_DAYS} and {MAX_KEEP_SPEECH_DAYS} days"),
        ));
    }
    let mut saved = load_settings(settings_file);
    saved.keep_speech_days = Some(days);
    save_settings(settings_file, &saved)
}

/// Whether meetings are re-transcribed after Stop (#87), as Settings shows it.
///
/// Inputs: The saved choices. Outputs: The saved switch; `true` when unset, the default on every
/// platform. `NOTE_TAKER_TRANSCRIPT_PASS` can still turn it off when nothing is saved.
/// Side effects: None.
pub fn transcript_pass_enabled(saved: &SavedTranscriptionSettings) -> bool {
    saved.transcript_pass.unwrap_or(true)
}

/// Saves whether meetings are re-transcribed after Stop, keeping every other saved choice.
///
/// Inputs: The settings file and the choice. Outputs: `Ok(())` or the write error.
/// Side effects: Rewrites the file atomically. It applies from the next recording; speech already
/// kept is still processed and deleted by the pass worker.
pub fn save_transcript_pass(settings_file: &Path, enabled: bool) -> io::Result<()> {
    let mut saved = load_settings(settings_file);
    saved.transcript_pass = Some(enabled);
    save_settings(settings_file, &saved)
}

/// Whether Automatic's model is downloaded by itself (#189).
///
/// Inputs: The saved settings. Outputs: The choice; on when unset. Side effects: None.
pub fn auto_download_enabled(saved: &SavedTranscriptionSettings) -> bool {
    saved.auto_download.unwrap_or(true)
}

/// Saves whether Automatic's model is downloaded by itself, keeping every other saved choice.
///
/// Inputs: The settings file and the choice. Outputs: `Ok(())` or the write error.
/// Side effects: Rewrites the file atomically; the app reads it at its next check.
pub fn save_auto_download(settings_file: &Path, enabled: bool) -> io::Result<()> {
    let mut saved = load_settings(settings_file);
    saved.auto_download = Some(enabled);
    save_settings(settings_file, &saved)
}

/// Saves whether Whisper runs on the NPU, keeping every other saved choice (#75).
///
/// Inputs: The settings file and the choice. Outputs: `Ok(())` or the write error.
/// Side effects: Rewrites the file atomically; it applies when the engine is next composed.
pub fn save_npu_whisper(settings_file: &Path, enabled: bool) -> io::Result<()> {
    let mut saved = load_settings(settings_file);
    saved.npu_whisper = enabled;
    save_settings(settings_file, &saved)
}

/// Whether online-meeting participants' voices may be kept in the gallery (#103).
///
/// Inputs: The settings file. Outputs: The saved choice; `false` when unset or unreadable.
/// Side effects: Reads the file, so a change applies at the next offer without a restart.
pub fn keeps_meeting_voices(settings_file: &Path) -> bool {
    load_settings(settings_file).keep_meeting_voices
}

/// Saves whether online-meeting participants' voices may be kept, keeping every other choice.
///
/// Inputs: The settings file and the choice. Outputs: `Ok(())` or the write error.
/// Side effects: Rewrites the file atomically. Voices kept earlier stay in the gallery; the
/// owner removes them there.
pub fn save_keep_meeting_voices(settings_file: &Path, keep: bool) -> io::Result<()> {
    let mut saved = load_settings(settings_file);
    saved.keep_meeting_voices = keep;
    save_settings(settings_file, &saved)
}

/// Whether recordings also capture what the computer plays (#108), as Settings shows it.
///
/// Inputs: The saved choices. Outputs: The saved switch; `true` when unset.
/// Side effects: None.
pub fn computer_sound_enabled(saved: &SavedTranscriptionSettings) -> bool {
    saved.computer_sound.unwrap_or(true)
}

/// Saves whether recordings capture the computer's sound, keeping every other choice.
///
/// Inputs: The settings file and the choice. Outputs: `Ok(())` or the write error.
/// Side effects: Rewrites the file atomically; it applies from the next start or resume.
pub fn save_computer_sound(settings_file: &Path, enabled: bool) -> io::Result<()> {
    let mut saved = load_settings(settings_file);
    saved.computer_sound = Some(enabled);
    save_settings(settings_file, &saved)
}

/// Applies "Automatic" (#106): with no model chosen by hand, the automatic model and its
/// efficient mode stand in for the saved ones.
///
/// Inputs: The saved choices and what Automatic resolved to on this device, if anything.
/// Outputs: The choices to run with; unchanged when a model was chosen by hand or Automatic found
/// nothing. Side effects: None; nothing is saved, so Automatic follows the device each time.
pub fn with_automatic_model(
    saved: &SavedTranscriptionSettings,
    automatic: Option<AutomaticModel<'_>>,
) -> SavedTranscriptionSettings {
    let mut effective = saved.clone();
    if saved.whisper_model.is_none()
        && let Some(automatic) = automatic
    {
        effective.whisper_model = Some(automatic.file_name.to_owned());
        effective.efficient_gpu = automatic.efficient;
    }
    effective
}

/// Layers the saved choices over a configuration.
///
/// Inputs: The configuration from the environment, the choices and the paths.
/// Outputs: The configuration to run. A chosen model that is missing keeps the previous model.
/// Side effects: Checks that the chosen model exists.
pub fn apply_saved_settings(
    config: PipelineConfig,
    saved: &SavedTranscriptionSettings,
    paths: &TranscriptionPaths,
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
    config
        .with_saved_transcription(model_path)
        .with_gpu_choices(saved.efficient_gpu, paths.gpu_attempt_marker.clone())
        .with_transcript_pass(
            saved.transcript_pass,
            saved
                .transcript_pass_model
                .as_deref()
                .and_then(|file_name| paths.locate_model(file_name)),
        )
        .with_npu_whisper(npu_choice(saved, paths))
}

/// Returns the NPU folder and the vocabulary model when the NPU engine should be tried (#75).
///
/// Inputs: The saved choices and the paths. Outputs: `Some` when the owner turned the NPU on,
/// every downloaded file is present, and the shipped multilingual model exists to supply the mel
/// filters and vocabulary. Side effects: Checks files.
fn npu_choice(
    saved: &SavedTranscriptionSettings,
    paths: &TranscriptionPaths,
) -> Option<(PathBuf, PathBuf)> {
    if !saved.npu_whisper {
        return None;
    }
    if !NpuInstall::new(&paths.npu_directory).is_installed() {
        tracing::warn!("the NPU is turned on but its files are not all downloaded");
        return None;
    }
    let vocabulary_model = paths
        .shipped_model
        .clone()
        .filter(|model| model.is_file())?;
    Some((paths.npu_directory.clone(), vocabulary_model))
}
