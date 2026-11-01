//! Remembers a GPU attempt, so a GPU driver that kills the process costs one crash and no more.
//!
//! A driver can abort or exit the process during setup or inference, where Rust can't catch it
//! (#76). A marker file written before the attempt survives that. Finding it at the next start means
//! the last attempt never finished, so transcription uses the CPU until the owner tries again.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// An attempt in progress. Dropping it ends the attempt without blaming the GPU.
#[derive(Debug)]
pub struct GpuAttemptGuard {
    marker_file: PathBuf,
}

impl GpuAttemptGuard {
    /// Starts an attempt.
    ///
    /// Inputs: The marker file's path.
    /// Outputs: The guard, or `None` when a previous attempt never finished or the marker can't be
    /// written, because then a crash could not be noticed.
    /// Side effects: Creates the marker file.
    pub fn begin(marker_file: &Path) -> Option<Self> {
        if gpu_is_blocked(marker_file) {
            return None;
        }
        fs::write(marker_file, b"").ok()?;
        Some(Self {
            marker_file: marker_file.to_path_buf(),
        })
    }

    /// Records that the GPU finished real work.
    ///
    /// Side effects: Removes the marker file, through `Drop`.
    pub fn confirm(self) {}
}

impl Drop for GpuAttemptGuard {
    /// Ends the attempt. This doesn't run when the process dies, which is what leaves the marker.
    ///
    /// Side effects: Removes the marker file.
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.marker_file);
    }
}

/// Reports whether a previous attempt never finished.
///
/// Inputs: The marker file's path. Outputs: True when it exists. Side effects: Checks the file.
pub fn gpu_is_blocked(marker_file: &Path) -> bool {
    marker_file.exists()
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
