//! Remembers a GPU attempt, so a GPU driver that kills the process costs one crash and no more.
//!
//! A driver can abort or exit the process during setup or inference, where Rust can't catch it
//! (#76). A marker file written before the attempt survives that, and holds how many attempts in a
//! row never finished. One unfinished attempt may only be the app being closed or killed (#133),
//! so the GPU is tried again; after `MAX_UNFINISHED_ATTEMPTS` transcription uses the CPU until the
//! owner tries again. A finished attempt removes the marker.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Unfinished attempts in a row after which the GPU is no longer tried.
pub const MAX_UNFINISHED_ATTEMPTS: u32 = 2;

/// An attempt in progress. Dropping it ends the attempt without blaming the GPU; `confirm` also
/// forgives the earlier unfinished attempts.
#[derive(Debug)]
pub struct GpuAttemptGuard {
    marker_file: PathBuf,
    /// Unfinished attempts the marker held before this one, restored by an orderly end.
    earlier_unfinished: u32,
    /// Whether the GPU finished real work during this attempt.
    proven: bool,
}

impl GpuAttemptGuard {
    /// Starts an attempt.
    ///
    /// Inputs: The marker file's path.
    /// Outputs: The guard, or `None` when `MAX_UNFINISHED_ATTEMPTS` attempts in a row never
    /// finished or the marker can't be written, because then a crash could not be noticed.
    /// Side effects: Creates the marker file, or counts one more attempt in it.
    pub fn begin(marker_file: &Path) -> Option<Self> {
        let unfinished = unfinished_attempts(marker_file);
        if unfinished >= MAX_UNFINISHED_ATTEMPTS {
            return None;
        }
        fs::write(marker_file, (unfinished + 1).to_string()).ok()?;
        Some(Self {
            marker_file: marker_file.to_path_buf(),
            earlier_unfinished: unfinished,
            proven: false,
        })
    }

    /// Records that the GPU finished real work, which clears every earlier unfinished attempt.
    ///
    /// Side effects: Removes the marker file, through `Drop`.
    pub fn confirm(mut self) {
        self.proven = true;
    }
}

impl Drop for GpuAttemptGuard {
    /// Ends the attempt. This doesn't run when the process dies, which is what leaves the count.
    ///
    /// Side effects: Removes the marker file after proof or when no earlier attempt is unfinished;
    /// otherwise writes back the earlier count, so closing the app between a crash and the next
    /// proof keeps that crash counted.
    fn drop(&mut self) {
        if self.proven || self.earlier_unfinished == 0 {
            let _ = fs::remove_file(&self.marker_file);
        } else {
            let _ = fs::write(&self.marker_file, self.earlier_unfinished.to_string());
        }
    }
}

/// Reports whether too many attempts in a row never finished.
///
/// Inputs: The marker file's path. Outputs: True when the marker counts `MAX_UNFINISHED_ATTEMPTS`
/// or more. Side effects: Reads the file.
pub fn gpu_is_blocked(marker_file: &Path) -> bool {
    unfinished_attempts(marker_file) >= MAX_UNFINISHED_ATTEMPTS
}

/// Counts the unfinished attempts the marker records.
///
/// Outputs: 0 without a marker; the number it holds; 1 for an empty marker (written by versions
/// before #133) and `MAX_UNFINISHED_ATTEMPTS` for one that can't be read, so a marker that exists
/// always counts. Side effects: Reads the file.
fn unfinished_attempts(marker_file: &Path) -> u32 {
    match fs::read_to_string(marker_file) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
        Err(_) => MAX_UNFINISHED_ATTEMPTS,
        Ok(text) if text.trim().is_empty() => 1,
        Ok(text) => text.trim().parse().unwrap_or(MAX_UNFINISHED_ATTEMPTS),
    }
}

/// Lets the GPU be tried again.
///
/// Inputs: The marker file's path. Outputs: Success, including when there was no marker.
/// Side effects: Removes the file.
pub fn clear_gpu_block(marker_file: &Path) -> io::Result<()> {
    match fs::remove_file(marker_file) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}
