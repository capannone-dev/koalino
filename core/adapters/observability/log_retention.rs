//! Removes old daily log files so local logs cannot fill the disk (#147).
//!
//! The rolling appender writes one `app_log.<date>` file per day and never deleted any. At the
//! `trace` level a day can reach several gigabytes. At start-up the newest `max_files` days are
//! kept (the same days the log export shares) and, while they exceed `max_total_bytes`, the oldest
//! of them are removed too. The newest file, the one being written, is never removed.

use std::{fs, io, path::Path};

/// How many daily files, and how many bytes in all, the log directory keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogRetention {
    /// The most daily files kept.
    pub max_files: usize,
    /// The most bytes all kept files may hold; the newest file is kept even above it.
    pub max_total_bytes: u64,
}

impl LogRetention {
    /// A week of logs, up to 2 GiB.
    pub const DEFAULT: Self = Self {
        max_files: super::MAX_EXPORTED_LOG_FILES,
        max_total_bytes: 2 * 1024 * 1024 * 1024,
    };
}

/// Removes the daily log files beyond `retention`.
///
/// Inputs: The log directory and the limits. Outputs: How many files were removed; 0 when the
/// directory does not exist; an I/O error when it cannot be listed.
/// Side effects: Deletes `app_log*` files, oldest first; other files are left alone. A file that
/// cannot be removed is skipped.
pub fn prune_log_files(log_directory: &Path, retention: LogRetention) -> io::Result<usize> {
    let entries = match fs::read_dir(log_directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    // (name, path, size); the date suffix sorts chronologically.
    let mut log_files = Vec::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let is_log_name = file_name
            .to_str()
            .is_some_and(|name| name.starts_with(super::log_export::LOG_FILE_PREFIX));
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if is_log_name && metadata.is_file() {
            log_files.push((file_name, entry.path(), metadata.len()));
        }
    }
    log_files.sort();

    let mut removed = 0;
    let surplus = log_files.len().saturating_sub(retention.max_files.max(1));
    let mut total: u64 = log_files
        .iter()
        .skip(surplus)
        .map(|(_, _, size)| size)
        .sum();
    let newest = log_files.len().saturating_sub(1);
    for (position, (_, path, size)) in log_files.iter().enumerate() {
        let too_many = position < surplus;
        let too_large = total > retention.max_total_bytes && position < newest;
        if !too_many && !too_large {
            break;
        }
        // A file that cannot be removed still counts; the next one may bring the total down.
        if fs::remove_file(path).is_ok() {
            removed += 1;
            if !too_many {
                total = total.saturating_sub(*size);
            }
        }
    }
    Ok(removed)
}

#[cfg(test)]
#[path = "log_retention_tests.rs"]
mod tests;
