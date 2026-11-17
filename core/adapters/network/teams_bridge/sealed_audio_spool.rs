//! Keeps Teams audio the meeting pump hasn't reached yet in sealed files (#133).
//!
//! When transcription runs slower than real time (a computer without a usable GPU, or a busy
//! one), the bridge buffer moves each newly arrived chunk's audio here instead of dropping older
//! audio, and takes it back when the pump reaches it. Each chunk is one file, `<uuid>.spool`, in a
//! private folder, sealed with the database key like attachments (AES-256-GCM, `SealedFiles`).
//! A file is deleted as soon as its audio is taken back, and every file left by an earlier run
//! (a crash or a quit mid-meeting) is deleted when the spool opens, so raw audio never outlives
//! its processing. The total kept is capped; beyond it the buffer drops audio as before.

use std::{
    io::Cursor,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
};

use crate::{
    adapters::{
        network::teams_bridge::bridge_buffer::{AudioSpool, SpoolTicket},
        storage::{private_files::create_private_directory, sealed_files::SealedFiles},
    },
    domain::errors::StorageError,
    ports::crypto::EmbeddingCipher,
};

/// File-name suffix of a spooled chunk.
const SPOOL_FILE_SUFFIX: &str = "spool";
/// Default cap on spooled audio: 1 GiB, about 9 hours of one 16 kHz track.
pub const DEFAULT_SPOOL_CAP_BYTES: u64 = 1 << 30;

/// A folder of sealed chunk files.
pub struct SealedAudioSpool {
    directory: PathBuf,
    sealed_files: SealedFiles,
    cap_bytes: u64,
    /// Clear bytes currently spooled.
    used_bytes: Mutex<u64>,
}

impl SealedAudioSpool {
    /// Opens the spool, deleting audio left by an earlier run.
    ///
    /// Inputs: The folder (created private when missing), the database cipher, and the most clear
    /// bytes to keep at once.
    /// Outputs: The spool, or a storage error when the folder can't be created.
    /// Side effects: Creates the folder; deletes every `.spool` file in it.
    pub fn open(
        directory: &Path,
        cipher: Arc<dyn EmbeddingCipher>,
        cap_bytes: u64,
    ) -> Result<Self, StorageError> {
        create_private_directory(directory)?;
        let mut removed = 0_usize;
        if let Ok(entries) = std::fs::read_dir(directory) {
            for path in entries.filter_map(|entry| entry.ok().map(|entry| entry.path())) {
                let is_spool_file = path
                    .extension()
                    .is_some_and(|extension| extension == SPOOL_FILE_SUFFIX);
                if is_spool_file && std::fs::remove_file(&path).is_ok() {
                    removed += 1;
                }
            }
        }
        if removed > 0 {
            tracing::warn!(
                removed_chunks = removed,
                "Deleted Teams audio an earlier run left untranscribed"
            );
        }
        Ok(Self {
            directory: directory.to_path_buf(),
            sealed_files: SealedFiles::new(cipher),
            cap_bytes,
            used_bytes: Mutex::new(0),
        })
    }

    /// Locks the spooled byte count, recovering from poisoning (it is a plain number).
    fn lock_used(&self) -> std::sync::MutexGuard<'_, u64> {
        self.used_bytes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Gives back room for `length` clear bytes.
    fn release(&self, length: u64) {
        let mut used = self.lock_used();
        *used = used.saturating_sub(length);
    }

    /// The file holding a ticket's audio.
    fn path_for(&self, ticket: &SpoolTicket) -> PathBuf {
        self.directory
            .join(format!("{}.{SPOOL_FILE_SUFFIX}", ticket.id()))
    }
}

impl AudioSpool for SealedAudioSpool {
    /// Seals the audio into a new file, unless it would pass the cap.
    ///
    /// Side effects: Writes and syncs one sealed file; logs counts only on failure.
    fn store(&self, audio_bytes: &[u8]) -> Option<SpoolTicket> {
        let length = u64::try_from(audio_bytes.len()).ok()?;
        {
            let mut used = self.lock_used();
            *used = used
                .checked_add(length)
                .filter(|total| *total <= self.cap_bytes)?;
        }
        let ticket = SpoolTicket::new();
        match self
            .sealed_files
            .write_new(&self.path_for(&ticket), &mut Cursor::new(audio_bytes))
        {
            Ok(_) => Some(ticket),
            Err(error) => {
                self.release(length);
                tracing::warn!(error = %error, "Teams audio could not be spooled");
                None
            }
        }
    }

    /// Reads the audio back and deletes its file.
    ///
    /// Side effects: Reads and deletes one file.
    fn take(&self, ticket: &SpoolTicket) -> Option<Vec<u8>> {
        let path = self.path_for(ticket);
        let audio = self.sealed_files.read_all(&path).ok();
        let _ = std::fs::remove_file(&path);
        let audio = audio?;
        let length = u64::try_from(audio.len()).unwrap_or(u64::MAX);
        self.release(length);
        Some(audio.to_vec())
    }
}
