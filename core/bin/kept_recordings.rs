//! The owner's kept meeting recordings (#140): a per-device switch, playback and "wipe all".
//!
//! Raw audio is normally deleted once the after-meeting pass and combining are done (CONTRIBUTING.md
//! §7). When the owner turns "Keep meeting recordings" on, the worker moves each meeting's sealed
//! speech into `<data folder>/recordings` instead (`RetainedAudioStore::keep_in`): the same
//! AES-256-GCM records under the database key, bound to their meeting. Only speech is kept (the
//! recorder keeps no silent chunks), so playback puts each chunk at its place in the meeting and
//! fills the gaps with silence. A recording goes when its meeting is deleted, or when the owner
//! wipes them all. The switch is per device, like the other retention choices, and is saved as
//! `keep_recordings` in `<db>.transcription-settings.json`.
//!
//! The app (FFI) and the web API answer the same JSON through `RecordingsServices`. Playback
//! writes a decrypted WAV to one file in the playback folder, replaced by the next playback and
//! removed by `release`; the web streams the same WAV without writing it.

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use serde_json::{Value, json};

use crate::{
    adapters::audio::retained_audio_store::{RetainedAudioError, RetainedAudioStore},
    domain::models::MeetingId,
    ports::{crypto::EmbeddingCipher, storage::NoteRepository},
    refinement_worker::KeepRecordingsSource,
    transcription_settings::{load_settings, save_settings, settings_file_for},
};

/// Samples per second of kept speech.
const SAMPLE_RATE: u32 = 16_000;
/// Samples per millisecond.
const SAMPLES_PER_MS: u64 = 16;
/// The longest recording played back, so a damaged span can't ask for an endless file.
const MAX_PLAYBACK_MS: u64 = 12 * 60 * 60 * 1000;
/// The one decrypted file playback uses, in the playback folder.
const PLAYBACK_FILE: &str = "meeting-recording.wav";

/// Returns where a database's kept recordings live: `<data folder>/recordings`.
pub fn kept_recordings_directory(database_path: &Path) -> PathBuf {
    database_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("recordings")
}

/// Reads whether the owner keeps recordings on this device.
///
/// Inputs: The settings file. Outputs: The saved switch; off when unset. Side effects: Reads it.
pub fn keeps_recordings(settings_file: &Path) -> bool {
    load_settings(settings_file).keep_recordings
}

/// Saves the switch, keeping every other saved choice.
///
/// Side effects: Rewrites the settings file atomically. Recordings already kept stay until
/// wiped; turning it off only stops keeping new ones.
pub fn save_keeps_recordings(settings_file: &Path, keep: bool) -> io::Result<()> {
    let mut saved = load_settings(settings_file);
    saved.keep_recordings = keep;
    save_settings(settings_file, &saved)
}

/// Builds the source the after-meeting worker reads the switch from, at each use.
pub fn keep_recordings_source(settings_file: PathBuf) -> KeepRecordingsSource {
    Arc::new(move || keeps_recordings(&settings_file))
}

/// Why a recording could not be played.
#[derive(Debug, thiserror::Error)]
pub enum PlaybackError {
    /// No recording is kept for the meeting.
    #[error("this meeting has no kept recording")]
    NotFound,
    /// The recording is damaged, sealed under another key, or longer than playback allows.
    #[error("the recording is unreadable")]
    Unreadable,
    /// The playback file could not be written.
    #[error("the recording could not be written for playback: {0}")]
    Storage(#[from] io::Error),
}

/// Where a recording sits on the meeting's timeline: its own meeting, and the shift onto the
/// meeting played (non-zero for a recording folded into it, #88).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordingSource {
    /// The meeting the recording was kept under.
    pub meeting_id: MeetingId,
    /// Milliseconds added to its times.
    pub offset_ms: i64,
}

/// Writes kept speech as a 16 kHz mono 16-bit WAV, each chunk at its time, silence between.
///
/// Inputs: The chunks' start times (ms, already shifted) with their samples, in any order, and
/// the writer. Outputs: The length in milliseconds, or `Unreadable` when it exceeds
/// `MAX_PLAYBACK_MS`. Overlapping speech keeps the earlier chunk's samples.
/// Side effects: Writes to `writer`.
pub fn write_wav(
    chunks: &mut [(u64, &[i16])],
    writer: &mut dyn Write,
) -> Result<u64, PlaybackError> {
    chunks.sort_by_key(|(start_ms, _)| *start_ms);
    let total_samples = chunks
        .iter()
        .map(|(start_ms, samples)| start_ms * SAMPLES_PER_MS + samples.len() as u64)
        .max()
        .unwrap_or(0);
    if total_samples > MAX_PLAYBACK_MS * SAMPLES_PER_MS {
        return Err(PlaybackError::Unreadable);
    }
    let data_bytes = u32::try_from(total_samples * 2).map_err(|_| PlaybackError::Unreadable)?;
    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    header.extend_from_slice(b"WAVEfmt ");
    header.extend_from_slice(&16_u32.to_le_bytes());
    header.extend_from_slice(&1_u16.to_le_bytes()); // PCM
    header.extend_from_slice(&1_u16.to_le_bytes()); // mono
    header.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    header.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    header.extend_from_slice(&2_u16.to_le_bytes());
    header.extend_from_slice(&16_u16.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_bytes.to_le_bytes());
    writer.write_all(&header)?;
    let silence = [0_u8; 8192];
    let mut written: u64 = 0;
    for (start_ms, samples) in chunks.iter() {
        let start = start_ms * SAMPLES_PER_MS;
        let mut gap = start.saturating_sub(written) * 2;
        while gap > 0 {
            let step = gap.min(silence.len() as u64);
            writer.write_all(&silence[..step as usize])?;
            gap -= step;
        }
        let skip = usize::try_from(written.saturating_sub(start)).unwrap_or(usize::MAX);
        let fresh = samples.get(skip..).unwrap_or_default();
        let mut bytes = zeroize::Zeroizing::new(Vec::with_capacity(fresh.len() * 2));
        for sample in fresh {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        writer.write_all(&bytes)?;
        written = written.max(start) + fresh.len() as u64;
    }
    Ok(total_samples / SAMPLES_PER_MS)
}

/// The kept recordings of one database, the switch, and playback.
pub struct RecordingsServices {
    settings_file: PathBuf,
    store: RetainedAudioStore,
    playback_directory: PathBuf,
}

impl RecordingsServices {
    /// Creates the services for a database.
    ///
    /// Inputs: The database (its folder holds `recordings`, its settings file the switch), the
    /// database key's cipher, and the folder for the decrypted playback file.
    /// Outputs: The services. Side effects: None.
    pub fn for_database(
        database_path: &Path,
        cipher: Arc<dyn EmbeddingCipher>,
        playback_directory: PathBuf,
    ) -> Self {
        Self {
            settings_file: settings_file_for(database_path),
            store: RetainedAudioStore::new(kept_recordings_directory(database_path), cipher),
            playback_directory,
        }
    }

    /// The switch and what is kept: `{"keep","count","bytes"}`.
    ///
    /// Side effects: Reads the settings file and the folder.
    pub fn status_json(&self) -> Value {
        let (count, bytes) = self.store.usage();
        json!({
            "keep": keeps_recordings(&self.settings_file),
            "count": count,
            "bytes": bytes,
        })
    }

    /// Turns keeping recordings on or off. Outputs: The status. Side effects: Writes settings.
    pub fn set_keep(&self, keep: bool) -> io::Result<Value> {
        save_keeps_recordings(&self.settings_file, keep)?;
        tracing::info!(keep, "Keeping meeting recordings was changed");
        Ok(self.status_json())
    }

    /// Deletes every kept recording and any playback file.
    ///
    /// Outputs: `{"deleted","bytes"}`: how many were deleted and the space freed.
    /// Side effects: Deletes files; a file that can't be deleted is logged and counted out.
    pub fn wipe(&self) -> Value {
        self.release();
        let (mut deleted, mut bytes) = (0_usize, 0_u64);
        for meeting_id in self.store.pending_meetings() {
            let size = self.store.usage_of(&meeting_id);
            match self.store.delete(&meeting_id) {
                Ok(_) => {
                    deleted += 1;
                    bytes += size;
                }
                Err(error) => {
                    tracing::warn!(error = %error, "a kept recording could not be wiped");
                }
            }
        }
        tracing::info!(deleted, "Kept meeting recordings were wiped");
        json!({"deleted": deleted, "bytes": bytes})
    }

    /// Deletes one meeting's kept recording, as deleting the meeting does.
    ///
    /// Side effects: Deletes the file.
    pub fn delete(&self, meeting_id: &MeetingId) -> Result<bool, RetainedAudioError> {
        self.store.delete(meeting_id)
    }

    /// The recordings to play for a meeting: its own, then those folded into it (#88), each
    /// with its shift onto the meeting's timeline. Only kept ones are listed.
    ///
    /// Inputs: The meeting and, when available, the repository that knows folded recordings.
    /// Side effects: Reads the folder and the repository.
    pub fn sources(
        &self,
        meeting_id: &MeetingId,
        repository: Option<&dyn NoteRepository>,
    ) -> Vec<RecordingSource> {
        let mut sources = vec![RecordingSource {
            meeting_id: meeting_id.clone(),
            offset_ms: 0,
        }];
        if let Some(recordings) =
            repository.and_then(|repository| repository.meeting_recordings().ok())
        {
            sources.extend(recordings.into_iter().filter_map(|recording| {
                let combination = recording.combined_into?;
                (combination.into == *meeting_id).then_some(RecordingSource {
                    meeting_id: recording.meeting_id,
                    offset_ms: combination.offset_ms,
                })
            }));
        }
        sources.retain(|source| self.store.holds(&source.meeting_id));
        sources
    }

    /// Writes the first kept recording among `sources` as WAV.
    ///
    /// Outputs: The length in milliseconds; `NotFound` with no source, `Unreadable` for a damaged
    /// one. Side effects: Decrypts and decodes the speech into memory; writes to `writer`.
    pub fn write_recording(
        &self,
        sources: &[RecordingSource],
        writer: &mut dyn Write,
    ) -> Result<u64, PlaybackError> {
        let source = sources.first().ok_or(PlaybackError::NotFound)?;
        let chunks = self
            .store
            .load(&source.meeting_id)
            .map_err(|_| PlaybackError::Unreadable)?;
        if chunks.is_empty() {
            return Err(PlaybackError::NotFound);
        }
        let mut placed: Vec<(u64, &[i16])> = chunks
            .iter()
            .map(|chunk| {
                let start = i64::try_from(chunk.span().start_ms)
                    .unwrap_or(i64::MAX)
                    .saturating_add(source.offset_ms)
                    .max(0);
                (u64::try_from(start).unwrap_or(0), chunk.samples())
            })
            .collect();
        write_wav(&mut placed, writer)
    }

    /// Writes a meeting's recording to the playback file for the app's player.
    ///
    /// Outputs: `{"path","duration_ms"}`. Side effects: Replaces the playback file with
    /// decrypted audio, readable by this user only, until `release` or the next playback.
    pub fn prepare_playback(&self, sources: &[RecordingSource]) -> Result<Value, PlaybackError> {
        let path = self.playback_directory.join(PLAYBACK_FILE);
        let duration_ms = self.write_wav_file(sources, &path)?;
        let path = fs::canonicalize(&path).unwrap_or(path);
        Ok(json!({"path": path.to_string_lossy(), "duration_ms": duration_ms}))
    }

    /// Writes a meeting's recording as a WAV at `path`, for the owner to share or save (#199).
    ///
    /// Inputs: The meeting's sources and the target file, whose folder is created if missing.
    /// Outputs: `{"path","duration_ms"}`, or `NotFound` when nothing is kept.
    /// Side effects: Writes decrypted audio, readable by this user only, replacing any file
    /// there; the caller owns it from then on. The player's file is not touched.
    pub fn export_recording(
        &self,
        sources: &[RecordingSource],
        path: &Path,
    ) -> Result<Value, PlaybackError> {
        let duration_ms = self.write_wav_file(sources, path)?;
        Ok(json!({"path": path.to_string_lossy(), "duration_ms": duration_ms}))
    }

    /// Writes the recording to `path` through a `.partial` file renamed when complete, so a
    /// failure leaves nothing behind.
    ///
    /// Outputs: The length in milliseconds. Side effects: Creates the folder and the file (mode
    /// 0600 on Unix); removes the partial file on failure.
    fn write_wav_file(
        &self,
        sources: &[RecordingSource],
        path: &Path,
    ) -> Result<u64, PlaybackError> {
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)?;
        }
        let mut partial_name = path.as_os_str().to_owned();
        partial_name.push(".partial");
        let partial = PathBuf::from(partial_name);
        let file = private_file(&partial)?;
        let mut writer = io::BufWriter::new(file);
        let written = self.write_recording(sources, &mut writer);
        let finished = writer.flush().map_err(PlaybackError::from);
        drop(writer);
        match written.and_then(|duration| finished.map(|()| duration)) {
            Ok(duration) => {
                fs::rename(&partial, path)?;
                Ok(duration)
            }
            Err(error) => {
                let _ = fs::remove_file(&partial);
                Err(error)
            }
        }
    }

    /// Deletes the decrypted playback file. Side effects: Removes it; a missing file is fine.
    pub fn release(&self) {
        for name in [PLAYBACK_FILE.to_owned(), format!("{PLAYBACK_FILE}.partial")] {
            if let Err(error) = fs::remove_file(self.playback_directory.join(name))
                && error.kind() != io::ErrorKind::NotFound
            {
                tracing::warn!("the playback file could not be removed");
            }
        }
    }
}

/// Creates a file readable by this user only, replacing any.
fn private_file(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

/// Answers one recordings command from the app: `status`, `set_keep` (`keep`), `wipe`,
/// `has` and `prepare` (`meeting_id`), `export` (`meeting_id`, `path`; #199) and `release`.
///
/// Inputs: The services, the repository when one is open (for folded recordings) and the JSON
/// request. Outputs: `{"ok":true,...}` or `{"ok":false,"error_code","error"}` with
/// `invalid_request`, `not_found`, `unreadable` or `storage_failed`.
/// Side effects: As each command.
pub fn run_recordings_command(
    services: &RecordingsServices,
    repository: Option<&dyn NoteRepository>,
    request_json: &str,
) -> Value {
    let request: Value = match serde_json::from_str(request_json) {
        Ok(request) => request,
        Err(_) => return failure("invalid_request", "the request is not JSON"),
    };
    let answer = match request.get("command").and_then(Value::as_str) {
        Some("status") => Ok(services.status_json()),
        Some("set_keep") => match request.get("keep").and_then(Value::as_bool) {
            Some(keep) => services
                .set_keep(keep)
                .map_err(|_| ("storage_failed", "the choice could not be saved".to_owned())),
            None => Err(("invalid_request", "`keep` is true or false".to_owned())),
        },
        Some("wipe") => Ok(services.wipe()),
        Some("has") => match request.get("meeting_id").and_then(Value::as_str) {
            Some(meeting_id) => Ok(json!({
                "available": !services.sources(&MeetingId::new(meeting_id), repository).is_empty(),
            })),
            None => Err(("invalid_request", "`meeting_id` is required".to_owned())),
        },
        Some("release") => {
            services.release();
            Ok(json!({}))
        }
        Some("prepare") => match request.get("meeting_id").and_then(Value::as_str) {
            Some(meeting_id) => {
                let sources = services.sources(&MeetingId::new(meeting_id), repository);
                services
                    .prepare_playback(&sources)
                    .map_err(|error| (playback_code(&error), error.to_string()))
            }
            None => Err(("invalid_request", "`meeting_id` is required".to_owned())),
        },
        Some("export") => match (
            request.get("meeting_id").and_then(Value::as_str),
            request.get("path").and_then(Value::as_str),
        ) {
            (Some(meeting_id), Some(path)) if !path.is_empty() => {
                let sources = services.sources(&MeetingId::new(meeting_id), repository);
                services
                    .export_recording(&sources, Path::new(path))
                    .map_err(|error| (playback_code(&error), error.to_string()))
            }
            _ => Err((
                "invalid_request",
                "`meeting_id` and `path` are required".to_owned(),
            )),
        },
        _ => Err((
            "invalid_request",
            "`command` is status, set_keep, wipe, has, prepare, export or release".to_owned(),
        )),
    };
    match answer {
        Ok(Value::Object(mut fields)) => {
            fields.insert("ok".to_owned(), Value::Bool(true));
            Value::Object(fields)
        }
        Ok(_) => json!({"ok": true}),
        Err((code, message)) => failure(code, &message),
    }
}

/// The error code of a playback failure.
pub fn playback_code(error: &PlaybackError) -> &'static str {
    match error {
        PlaybackError::NotFound => "not_found",
        PlaybackError::Unreadable => "unreadable",
        PlaybackError::Storage(_) => "storage_failed",
    }
}

/// A failure answer.
fn failure(code: &str, message: &str) -> Value {
    json!({"ok": false, "error_code": code, "error": message})
}
