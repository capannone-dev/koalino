//! Declares concrete meeting-export format adapters, their shared file-writing helpers, and tests.
//!
//! Every exporter implements `ports::export::MeetingExporter` and receives fully extracted
//! `ExportDocument` values from `application::export_manager`; none of them touches a repository.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use crate::{
    domain::{
        errors::ExportError,
        models::{ExportFormat, ExportResult},
    },
    ports::export::MeetingExporter,
};

/// Renders versioned JSON meeting documents.
pub mod json_exporter;
/// Renders human-readable Markdown meeting documents.
pub mod markdown_exporter;
/// Renders paginated meeting PDFs and embeds attachments.
pub mod pdf_exporter;
/// Selects, loads, and measures the Unicode fonts embedded in meeting PDFs.
pub mod pdf_fonts;
/// Exports a meeting's summary alone as PDF, plain text, Word or Markdown (#130).
pub mod summary_export;

#[cfg(test)]
mod pdf_unicode_tests;
#[cfg(test)]
mod summary_export_tests;
#[cfg(test)]
mod tests;

/// Version identifier written into every export artifact's result metadata and JSON payload.
pub const EXPORT_SCHEMA_VERSION: &str = "1.0";

/// Returns one boxed exporter for every implemented format (Markdown, PDF, JSON).
///
/// Inputs: None.
/// Outputs: Exporters ready to inject into `ExportManager`/`ExportWriter`.
/// Side effects: None.
pub fn default_exporters() -> Vec<Box<dyn MeetingExporter>> {
    vec![
        Box::new(markdown_exporter::MarkdownMeetingExporter),
        Box::new(pdf_exporter::PdfMeetingExporter),
        Box::new(json_exporter::JsonMeetingExporter),
    ]
}

/// Resolves the default directory for locally written exports.
///
/// Inputs: Reads `NOTE_TAKER_EXPORT_DIR`, then `HOME` (or `USERPROFILE` on Windows).
/// Outputs: `NOTE_TAKER_EXPORT_DIR` when set, otherwise `~/Documents/NoteTaker/exports`, or a
/// relative `exports` directory when no home directory is known.
/// Side effects: Reads environment variables; it does not create the directory.
pub fn default_export_directory() -> PathBuf {
    if let Ok(configured_directory) = env::var("NOTE_TAKER_EXPORT_DIR")
        && !configured_directory.trim().is_empty()
    {
        return PathBuf::from(configured_directory);
    }
    env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .map(|home_directory| {
            PathBuf::from(home_directory)
                .join("Documents")
                .join("NoteTaker")
                .join("exports")
        })
        .unwrap_or_else(|_| PathBuf::from("exports"))
}

/// Writes rendered bytes atomically and summarizes the resulting artifact.
///
/// Inputs: `output_path` is the destination; `rendered_bytes` the complete artifact; `format` and
/// `meeting_count` describe it.
/// Outputs: An `ExportResult` with the observed size, or an `ExportError::Io`.
/// Side effects: Creates the parent directory, writes a temporary sibling file, and renames it
/// over `output_path` so readers never observe a partially written export.
pub(crate) fn write_export_artifact(
    output_path: &Path,
    rendered_bytes: &[u8],
    format: ExportFormat,
    meeting_count: usize,
) -> Result<ExportResult, ExportError> {
    let parent_directory = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent_directory)
        .map_err(|error| io_error("unable to create export directory", &error))?;
    let partial_file_name = format!(
        ".{}.partial-{}",
        output_path
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .unwrap_or("export"),
        Uuid::new_v4()
    );
    let partial_path = parent_directory.join(partial_file_name);
    fs::write(&partial_path, rendered_bytes)
        .map_err(|error| io_error("unable to write export file", &error))?;
    if let Err(error) = fs::rename(&partial_path, output_path) {
        let _ = fs::remove_file(&partial_path);
        return Err(io_error("unable to finalize export file", &error));
    }
    let size_bytes = fs::metadata(output_path)
        .map_err(|error| io_error("unable to inspect export file", &error))?
        .len();
    tracing::info!(
        format = %format,
        meeting_count,
        size_bytes,
        "meeting export written"
    );
    Ok(ExportResult::new(
        output_path.to_string_lossy(),
        format,
        size_bytes,
        vec![
            (
                String::from("schema_version"),
                String::from(EXPORT_SCHEMA_VERSION),
            ),
            (String::from("meeting_count"), meeting_count.to_string()),
        ],
    ))
}

/// Rejects an empty batch before a renderer produces a meaningless artifact.
///
/// Inputs: `meeting_count` is the number of documents supplied to an exporter.
/// Outputs: Success for non-empty batches, otherwise `ExportError::NoMeetingsSelected`.
/// Side effects: None.
pub(crate) fn ensure_documents_present(meeting_count: usize) -> Result<(), ExportError> {
    if meeting_count == 0 {
        Err(ExportError::NoMeetingsSelected)
    } else {
        Ok(())
    }
}

/// Converts an I/O failure into a portable export error without leaking file contents.
///
/// Inputs: `operation` names the failed step; `error` is the OS error.
/// Outputs: `ExportError::Io`.
/// Side effects: None.
pub(crate) fn io_error(operation: &str, error: &std::io::Error) -> ExportError {
    ExportError::Io {
        reason: format!("{operation}: {error}"),
    }
}
