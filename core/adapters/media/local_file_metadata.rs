//! Extracts a privacy-safe attachment timestamp from local filesystem metadata.

use std::{fs, path::Path};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};

use crate::ports::media::MetadataExtractor;

/// Reads a local file modification timestamp when format-specific capture metadata is unavailable.
pub struct LocalFileMetadataExtractor;

impl MetadataExtractor for LocalFileMetadataExtractor {
    /// Obtains the selected local file's modification time as a UTC timestamp.
    ///
    /// Inputs: `path` must identify a readable local file selected by the user.
    /// Outputs: Its filesystem modification time converted to UTC, or a contextual I/O error.
    /// Side effects: Reads filesystem metadata only; never reads or logs file contents or paths.
    fn extract_timestamp(&self, path: &str) -> Result<DateTime<Utc>> {
        let modification_time = fs::metadata(Path::new(path))
            .context("unable to read attachment filesystem metadata")?
            .modified()
            .context("attachment filesystem metadata has no modification time")?;
        Ok(DateTime::<Utc>::from(modification_time))
    }
}
