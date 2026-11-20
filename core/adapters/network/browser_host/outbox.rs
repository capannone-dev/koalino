//! The browser host's outbox (#135): meeting frames Koalino hasn't received yet, kept on disk so
//! a meeting is never lost because Koalino was closed, still starting, or broken.
//!
//! Each browser-host session writes the frames it can't deliver into its own folder,
//! `<root>/<uuid>/`, one sealed file per frame named by its sequence number, and keeps a
//! heartbeat file (`alive`) fresh while it runs. Frames are sealed with AES-256-GCM under a key
//! derived (HKDF-SHA256) from the Teams bridge secret, which both the host and Koalino hold; the
//! file name is the additional data, so files can't be reordered or swapped unnoticed. A frame is
//! deleted as soon as it was delivered.
//!
//! Only one program delivers at a time, the holder of `<root>/delivery.lock`: the bridge treats
//! audio as part of the last meeting announced, so two streams must never interleave. A session
//! whose heartbeat stopped (its host exited) is claimed by Koalino by renaming it to
//! `<uuid>.claimed`, then replayed in order; a claimed folder found later belongs to a reader that
//! crashed and is replayed again. Koalino holds `<root>/app-running.lock` while it runs, which
//! tells the host whether to start it.
//!
//! Frame contents and the secret are never logged.

use std::{
    collections::VecDeque,
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng, Payload},
};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::{
    adapters::storage::private_files::{create_private_directory, file_error, write_private_file},
    domain::errors::StorageError,
};

/// Overrides where the outbox lives (tests, unusual setups).
pub const OUTBOX_DIRECTORY_VARIABLE: &str = "NOTE_TAKER_BROWSER_OUTBOX_DIR";
/// Default cap on one session's waiting frames: 4 GiB, about 17 hours of both audio tracks.
pub const DEFAULT_OUTBOX_CAP_BYTES: u64 = 4 << 30;
/// A session whose heartbeat is older than this is taken to have no host any more.
pub const STALE_SESSION_AFTER: Duration = Duration::from_secs(60);
/// Suffix of a frame file.
const FRAME_SUFFIX: &str = "frame";
/// Suffix of a session folder claimed for replay.
const CLAIMED_SUFFIX: &str = "claimed";
/// The heartbeat file in a session folder.
const HEARTBEAT_FILE: &str = "alive";
/// Bytes of an AES-GCM nonce.
const NONCE_BYTES: usize = 12;
/// HKDF info naming this key's use.
const KEY_INFO: &[u8] = b"note_taker browser outbox v1";

/// Where the outbox lives: `NOTE_TAKER_BROWSER_OUTBOX_DIR`, else `%LOCALAPPDATA%\note_taker\
/// browser-outbox` on Windows, else `$XDG_STATE_HOME/note_taker/browser-outbox` or
/// `~/.local/state/note_taker/browser-outbox`.
///
/// Inputs: An environment lookup; blank values count as unset. Outputs: The folder, or `None`
/// without any of those variables. Side effects: None.
pub fn browser_outbox_root(lookup: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let value = |name: &str| lookup(name).filter(|value| !value.trim().is_empty());
    if let Some(configured) = value(OUTBOX_DIRECTORY_VARIABLE) {
        return Some(PathBuf::from(configured));
    }
    let state_folder = if cfg!(windows) {
        value("LOCALAPPDATA").map(PathBuf::from)
    } else {
        value("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| value("HOME").map(|home| PathBuf::from(home).join(".local/state")))
    }?;
    Some(state_folder.join("note_taker").join("browser-outbox"))
}

/// The key frames are sealed with.
#[derive(Clone)]
pub struct OutboxKey(Zeroizing<[u8; 32]>);

impl OutboxKey {
    /// Derives the key from the Teams bridge secret, which the host and Koalino share.
    ///
    /// Side effects: None; the secret is not kept.
    pub fn from_bridge_secret(secret: &str) -> Self {
        let mut key = Zeroizing::new([0_u8; 32]);
        // HKDF-SHA256 accepts any output length up to 8160 bytes, so 32 cannot fail.
        let _ = Hkdf::<Sha256>::new(None, secret.as_bytes()).expand(KEY_INFO, key.as_mut());
        Self(key)
    }

    /// The cipher. Side effects: None.
    fn cipher(&self) -> Option<Aes256Gcm> {
        Aes256Gcm::new_from_slice(self.0.as_ref()).ok()
    }
}

impl std::fmt::Debug for OutboxKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("OutboxKey(..)")
    }
}

/// A file lock held until dropped; the OS releases it when the process ends, even in a crash.
#[derive(Debug)]
pub struct ExclusiveLock {
    _file: File,
}

/// Takes the lock on `path` without waiting.
///
/// Outputs: The lock, or `None` when another process holds it or the file can't be opened.
/// Side effects: Creates the folder and the lock file when missing.
fn try_lock(path: &Path) -> Option<ExclusiveLock> {
    if let Some(folder) = path.parent() {
        create_private_directory(folder).ok()?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .ok()?;
    file.try_lock().ok()?;
    Some(ExclusiveLock { _file: file })
}

/// Takes the right to deliver frames to Koalino; one holder at a time (see the module notes).
///
/// Side effects: Creates `<root>/delivery.lock`.
pub fn try_delivery_lock(root: &Path) -> Option<ExclusiveLock> {
    try_lock(&root.join("delivery.lock"))
}

/// Marks Koalino as running for as long as the returned lock lives.
///
/// Outputs: The lock, or `None` when another Koalino holds it. Side effects: Creates the file.
pub fn hold_app_running(root: &Path) -> Option<ExclusiveLock> {
    try_lock(&root.join("app-running.lock"))
}

/// Whether a Koalino process holds the running lock.
///
/// Side effects: Briefly takes and releases the lock when nobody holds it.
pub fn app_is_running(root: &Path) -> bool {
    let path = root.join("app-running.lock");
    if !path.exists() {
        return false;
    }
    hold_app_running(root).is_none()
}

/// One session's waiting frames, oldest first.
pub struct OutboxSession {
    directory: PathBuf,
    key: OutboxKey,
    cap_bytes: u64,
    /// Sequence numbers of the frames on disk, oldest first, with their sealed sizes.
    waiting: VecDeque<(u64, u64)>,
    next_sequence: u64,
    used_bytes: u64,
    /// The oldest frame, already opened, so a failed send doesn't read it again.
    opened_front: Option<Vec<u8>>,
    dropped_audio_frames: usize,
}

impl OutboxSession {
    /// Starts a new session folder under `root`.
    ///
    /// Inputs: The outbox root, the key, and the most sealed bytes to keep waiting.
    /// Outputs: The empty session, or a storage error when the folder can't be made.
    /// Side effects: Creates `<root>/<uuid>/` (private) and its heartbeat.
    pub fn create(root: &Path, key: OutboxKey, cap_bytes: u64) -> Result<Self, StorageError> {
        let directory = root.join(uuid::Uuid::new_v4().to_string());
        create_private_directory(&directory)?;
        let session = Self {
            directory,
            key,
            cap_bytes,
            waiting: VecDeque::new(),
            next_sequence: 0,
            used_bytes: 0,
            opened_front: None,
            dropped_audio_frames: 0,
        };
        session.heartbeat();
        Ok(session)
    }

    /// Opens a session folder left by another process (one returned by `claim_stale_sessions`
    /// or `claimed_sessions`) to read its frames.
    ///
    /// Outputs: The session with its frames in order, or a storage error when it can't be listed.
    /// Side effects: Lists the folder.
    pub fn adopt(directory: PathBuf, key: OutboxKey) -> Result<Self, StorageError> {
        let entries = std::fs::read_dir(&directory)
            .map_err(|error| file_error("unable to list an outbox session", &error))?;
        let mut waiting: Vec<(u64, u64)> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension()? != FRAME_SUFFIX {
                    return None;
                }
                let sequence = path.file_stem()?.to_str()?.parse::<u64>().ok()?;
                Some((
                    sequence,
                    entry.metadata().map_or(0, |metadata| metadata.len()),
                ))
            })
            .collect();
        waiting.sort_unstable();
        let next_sequence = waiting.last().map_or(0, |(sequence, _)| sequence + 1);
        let used_bytes = waiting.iter().map(|(_, size)| size).sum();
        Ok(Self {
            directory,
            key,
            cap_bytes: u64::MAX,
            waiting: waiting.into(),
            next_sequence,
            used_bytes,
            opened_front: None,
            dropped_audio_frames: 0,
        })
    }

    /// The session's folder. Side effects: None.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Frames waiting. Side effects: None.
    pub fn len(&self) -> usize {
        self.waiting.len()
    }

    /// Whether nothing waits. Side effects: None.
    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }

    /// Audio frames refused because the outbox was full. Side effects: None.
    pub fn dropped_audio_frames(&self) -> usize {
        self.dropped_audio_frames
    }

    /// The file of frame `sequence`.
    fn frame_path(&self, sequence: u64) -> PathBuf {
        self.directory
            .join(format!("{sequence:020}.{FRAME_SUFFIX}"))
    }

    /// Keeps a frame until it is delivered.
    ///
    /// Inputs: The frame's bytes; whether it is audio. Past the cap, audio is refused (a meeting
    /// start, slide, document or end is always kept, they are small).
    /// Outputs: Whether the frame was kept.
    /// Side effects: Writes one sealed file; logs counts only when writing fails.
    pub fn push(&mut self, frame: &[u8], is_audio: bool) -> bool {
        let sealed_size = (frame.len() + NONCE_BYTES + 16) as u64;
        if is_audio && self.used_bytes.saturating_add(sealed_size) > self.cap_bytes {
            self.dropped_audio_frames += 1;
            return false;
        }
        let sequence = self.next_sequence;
        let path = self.frame_path(sequence);
        let Some(sealed) = self.seal(&path, frame) else {
            return false;
        };
        if let Err(error) = write_private_file(&path, &mut sealed.as_slice()) {
            tracing::warn!(error = %error, "A meeting frame could not be kept for Koalino");
            if is_audio {
                self.dropped_audio_frames += 1;
            }
            return false;
        }
        self.next_sequence += 1;
        self.used_bytes += sealed.len() as u64;
        self.waiting.push_back((sequence, sealed.len() as u64));
        true
    }

    /// The oldest waiting frame, opened.
    ///
    /// Outputs: Its bytes, or `None` when nothing waits. A file that can't be read or opened
    /// (another key, damage) is deleted and skipped.
    /// Side effects: Reads (and may delete) files.
    pub fn front(&mut self) -> Option<Vec<u8>> {
        while self.opened_front.is_none() {
            let (sequence, _) = *self.waiting.front()?;
            let path = self.frame_path(sequence);
            match std::fs::read(&path)
                .ok()
                .and_then(|sealed| self.open(&path, &sealed))
            {
                Some(frame) => self.opened_front = Some(frame),
                None => {
                    tracing::warn!("A kept meeting frame could not be opened; it was skipped");
                    self.remove_front();
                }
            }
        }
        self.opened_front.clone()
    }

    /// Forgets the oldest frame once it was delivered.
    ///
    /// Side effects: Deletes its file.
    pub fn remove_front(&mut self) {
        self.opened_front = None;
        if let Some((sequence, size)) = self.waiting.pop_front() {
            let _ = std::fs::remove_file(self.frame_path(sequence));
            self.used_bytes = self.used_bytes.saturating_sub(size);
        }
    }

    /// Shows this session's host is alive, so nobody claims it.
    ///
    /// Side effects: Rewrites the heartbeat file.
    pub fn heartbeat(&self) {
        let _ = std::fs::write(self.directory.join(HEARTBEAT_FILE), b"");
    }

    /// Ends the session: an empty folder is removed; waiting frames stay for Koalino.
    ///
    /// Side effects: May delete the folder.
    pub fn finish(self) {
        if self.waiting.is_empty() {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }

    /// Seals `frame` for `path`: a random nonce followed by the ciphertext.
    fn seal(&self, path: &Path, frame: &[u8]) -> Option<Vec<u8>> {
        let cipher = self.key.cipher()?;
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: frame,
                    aad: file_name_of(path).as_bytes(),
                },
            )
            .ok()?;
        let mut sealed = nonce.to_vec();
        sealed.extend(ciphertext);
        Some(sealed)
    }

    /// Opens a sealed frame read from `path`.
    fn open(&self, path: &Path, sealed: &[u8]) -> Option<Vec<u8>> {
        if sealed.len() < NONCE_BYTES {
            return None;
        }
        let (nonce, ciphertext) = sealed.split_at(NONCE_BYTES);
        self.key
            .cipher()?
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad: file_name_of(path).as_bytes(),
                },
            )
            .ok()
    }
}

/// A path's file name as text (empty when it has none).
fn file_name_of(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned()
}

/// When a session folder last showed life: its heartbeat, else the folder itself.
fn last_sign_of_life(directory: &Path) -> Option<SystemTime> {
    std::fs::metadata(directory.join(HEARTBEAT_FILE))
        .or_else(|_| std::fs::metadata(directory))
        .and_then(|metadata| metadata.modified())
        .ok()
}

/// Claims every session whose host stopped: its heartbeat is at least `stale_after` old.
///
/// Call it only while holding the delivery lock. Outputs: The claimed folders (renamed to
/// `<uuid>.claimed`, so they are not claimed twice), oldest first. Side effects: Renames folders.
pub fn claim_stale_sessions(root: &Path, stale_after: Duration) -> Vec<PathBuf> {
    let now = SystemTime::now();
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut stale: Vec<(SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|directory| directory.is_dir() && directory.extension().is_none())
        .filter_map(|directory| {
            let seen = last_sign_of_life(&directory)?;
            (now.duration_since(seen).unwrap_or_default() >= stale_after)
                .then_some((seen, directory))
        })
        .collect();
    // Oldest first, so meetings are replayed in the order they happened.
    stale.sort();
    let mut claimed = Vec::new();
    for (_, directory) in stale {
        let target = directory.with_extension(CLAIMED_SUFFIX);
        if std::fs::rename(&directory, &target).is_ok() {
            claimed.push(target);
        }
    }
    claimed
}

/// Sessions claimed earlier whose reader stopped before finishing (call it only while holding
/// the delivery lock, so no reader is at work).
///
/// Side effects: Lists the root.
pub fn claimed_sessions(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut claimed: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path
                    .extension()
                    .is_some_and(|extension| extension == CLAIMED_SUFFIX)
        })
        .collect();
    claimed.sort_by_key(|path| last_sign_of_life(path));
    claimed
}
