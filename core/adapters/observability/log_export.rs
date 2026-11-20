//! Gathers the rolling log files into one file to share (REQUIREMENTS 6.2, Issue #46).
//!
//! `initialize_logging` writes one JSON-lines file per day (`app_log.<YYYY-MM-DD>`). An export
//! joins the newest `MAX_EXPORTED_LOG_FILES` of them, oldest first, into a single `.jsonl` file
//! that the app shares or saves and the daemon serves for download. The entries were already
//! redacted when they were written, so the export adds no user content.

use std::{
    ffi::OsString,
    fs::{self, File},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
};

/// The name prefix `initialize_logging` gives the daily files.
pub(super) const LOG_FILE_PREFIX: &str = "app_log";

/// How many daily files an export holds, so a long-lived install shares a bounded file.
pub const MAX_EXPORTED_LOG_FILES: usize = 7;

/// A written log export.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogExport {
    /// Where the export was written.
    pub path: PathBuf,
    /// Its size.
    pub size_bytes: u64,
    /// How many daily files it holds.
    pub file_count: usize,
}

/// Why logs could not be exported.
#[derive(Debug, thiserror::Error)]
pub enum LogExportError {
    /// The log directory holds no log files (or does not exist yet).
    #[error("there are no log files to export")]
    NoLogs,
    /// A log file could not be read or the export could not be written.
    #[error("unable to export the logs: {0}")]
    Io(#[from] io::Error),
}

/// Joins the newest daily log files into `output_path`.
///
/// Inputs: `log_directory` is the directory given to `initialize_logging`; `output_path` is the
/// file to write, outside that directory. Its parent is created when missing.
/// Outputs: The export's path, size, and file count; `NoLogs` when there is nothing to export, or
/// `Io` when a file cannot be read or written.
/// Side effects: Reads the log files and writes `output_path` through a `.partial` file renamed
/// over it, replacing an earlier export. Nothing is left behind on failure.
pub fn export_log_files(
    log_directory: &Path,
    output_path: &Path,
) -> Result<LogExport, LogExportError> {
    let log_files = newest_log_files(log_directory)?;
    if log_files.is_empty() {
        return Err(LogExportError::NoLogs);
    }
    if let Some(output_directory) = output_path.parent()
        && !output_directory.as_os_str().is_empty()
    {
        fs::create_dir_all(output_directory)?;
    }
    let mut partial_path = output_path.as_os_str().to_owned();
    partial_path.push(".partial");
    let partial_path = PathBuf::from(partial_path);
    let written = write_joined(&log_files, &partial_path)
        .and_then(|size_bytes| fs::rename(&partial_path, output_path).map(|()| size_bytes));
    match written {
        Ok(size_bytes) => Ok(LogExport {
            path: output_path.to_path_buf(),
            size_bytes,
            file_count: log_files.len(),
        }),
        Err(error) => {
            let _ = fs::remove_file(&partial_path);
            Err(error.into())
        }
    }
}

/// Lists the newest daily log files, oldest first.
///
/// Inputs: The log directory. Outputs: Up to `MAX_EXPORTED_LOG_FILES` regular files named
/// `app_log*`, sorted by name (the date suffix sorts chronologically); empty when the directory
/// does not exist. Side effects: Reads the directory.
fn newest_log_files(log_directory: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(log_directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut log_files: Vec<(OsString, PathBuf)> = Vec::new();
    for entry in entries {
        let entry = entry?;
        let file_name = entry.file_name();
        let is_log_name = file_name
            .to_str()
            .is_some_and(|name| name.starts_with(LOG_FILE_PREFIX));
        if is_log_name && entry.file_type()?.is_file() {
            log_files.push((file_name, entry.path()));
        }
    }
    log_files.sort();
    let skipped = log_files.len().saturating_sub(MAX_EXPORTED_LOG_FILES);
    Ok(log_files
        .into_iter()
        .skip(skipped)
        .map(|(_, path)| path)
        .collect())
}

/// Writes `log_files` one after another into `destination`, ending each with a newline.
///
/// Inputs: The files, in order, and the file to create. Outputs: The bytes written.
/// Side effects: Creates or truncates `destination`; streams each file, so a large day is never
/// held in memory.
fn write_joined(log_files: &[PathBuf], destination: &Path) -> io::Result<u64> {
    let mut writer = LastByteWriter::new(BufWriter::new(File::create(destination)?));
    for log_file in log_files {
        io::copy(&mut File::open(log_file)?, &mut writer)?;
        // A day cut off mid-line (a crash) must not glue its last entry to the next day's first.
        if writer.last_byte.is_some_and(|byte| byte != b'\n') {
            writer.write_all(b"\n")?;
        }
    }
    writer.inner.flush()?;
    writer.inner.get_ref().sync_all()?;
    Ok(writer.written)
}

/// Counts the bytes passed through and remembers the last one.
struct LastByteWriter<W> {
    inner: W,
    written: u64,
    last_byte: Option<u8>,
}

impl<W> LastByteWriter<W> {
    /// Wraps `inner` with nothing written yet.
    fn new(inner: W) -> Self {
        Self {
            inner,
            written: 0,
            last_byte: None,
        }
    }
}

impl<W: Write> Write for LastByteWriter<W> {
    /// Writes to the inner writer and records what it accepted.
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let accepted = self.inner.write(buffer)?;
        if let Some(byte) = buffer[..accepted].last() {
            self.last_byte = Some(*byte);
        }
        self.written += accepted as u64;
        Ok(accepted)
    }

    /// Flushes the inner writer.
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
#[path = "log_export_tests.rs"]
mod tests;
