//! Declares attachment metadata extraction without coupling application logic to file formats.

use anyhow::Result;
use chrono::{DateTime, Utc};

/// Extracts a capture timestamp from an attachment through adapter-specific metadata readers.
pub trait MetadataExtractor {
    /// Reads an attachment timestamp without exposing EXIF, PDF, or filesystem APIs to callers.
    ///
    /// Inputs: `path` identifies an attachment readable by the concrete adapter.
    /// Outputs: A UTC capture timestamp or an error when metadata is absent or invalid.
    /// Side effects: An adapter may read local file metadata; it must not log file contents.
    fn extract_timestamp(&self, path: &str) -> Result<DateTime<Utc>>;
}
