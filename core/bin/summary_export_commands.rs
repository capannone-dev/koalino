//! Exports a meeting's stored summary as a PDF, plain text, Word or Markdown file (#130), for the
//! app (FFI, which saves where the owner chose) and the web API (which sends the bytes).
//!
//! The summary is read from the meeting document; rendering is done by
//! `adapters::export::summary_export`. Summary text is never logged.

use std::path::{Path, PathBuf};

use crate::{
    adapters::export::{
        pdf_fonts::PdfFontConfig,
        summary_export::{SummaryExportFormat, render_summary, summary_file_name},
    },
    domain::models::MeetingId,
    meeting_summaries::load_summary,
    ports::storage::NoteRepository,
};

/// A summary rendered as a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportedSummary {
    /// The suggested file name: `<meeting title> - summary.<ext>`.
    pub file_name: String,
    /// The file's bytes.
    pub bytes: Vec<u8>,
    /// The format.
    pub format: SummaryExportFormat,
}

/// Why a summary could not be exported.
#[derive(Debug, thiserror::Error)]
pub enum SummaryExportFailure {
    /// No meeting has this ID.
    #[error("the meeting does not exist")]
    MeetingNotFound,
    /// The meeting has no summary yet.
    #[error("the meeting has no summary yet")]
    NoSummary,
    /// The meeting could not be read.
    #[error("the meeting could not be read: {0}")]
    Storage(String),
    /// The file could not be made or saved.
    #[error("the summary could not be exported: {0}")]
    Export(String),
}

impl SummaryExportFailure {
    /// A stable code for clients: `meeting_not_found`, `no_summary`, `storage_error` or
    /// `export_failed`. Side effects: None.
    pub fn code(&self) -> &'static str {
        match self {
            Self::MeetingNotFound => "meeting_not_found",
            Self::NoSummary => "no_summary",
            Self::Storage(_) => "storage_error",
            Self::Export(_) => "export_failed",
        }
    }
}

/// Renders a meeting's summary.
///
/// Inputs: The repository, the meeting, the format, and where PDF fonts are found.
/// Outputs: The file name and bytes; `MeetingNotFound`, `NoSummary`, `Storage` or `Export`.
/// Side effects: Reads the meeting's metadata and document; the PDF may read font files.
pub fn export_summary<Repository>(
    repository: &Repository,
    meeting_id: &MeetingId,
    format: SummaryExportFormat,
    font_config: &PdfFontConfig,
) -> Result<ExportedSummary, SummaryExportFailure>
where
    Repository: NoteRepository + ?Sized,
{
    let metadata = repository
        .load_meeting_metadata(meeting_id)
        .map_err(|error| SummaryExportFailure::Storage(error.to_string()))?
        .ok_or(SummaryExportFailure::MeetingNotFound)?;
    let summary = load_summary(repository, meeting_id)
        .map_err(|error| SummaryExportFailure::Storage(error.to_string()))?
        .filter(|summary| !summary.text.trim().is_empty())
        .ok_or(SummaryExportFailure::NoSummary)?;
    let title = metadata.display_title();
    let bytes = render_summary(format, &title, &summary.text, font_config)
        .map_err(|error| SummaryExportFailure::Export(error.to_string()))?;
    Ok(ExportedSummary {
        file_name: summary_file_name(&title, format),
        bytes,
        format,
    })
}

/// Renders a meeting's summary and saves it at `output_path`.
///
/// Inputs: As `export_summary`, plus the file to write (its folder is created when missing; an
/// existing file is replaced).
/// Outputs: The saved path, or the failures of `export_summary` and `Export` when saving fails.
/// Side effects: Writes a temporary sibling file and renames it over `output_path`, so no
/// half-written file is left.
pub fn export_summary_to_file<Repository>(
    repository: &Repository,
    meeting_id: &MeetingId,
    format: SummaryExportFormat,
    output_path: &Path,
    font_config: &PdfFontConfig,
) -> Result<PathBuf, SummaryExportFailure>
where
    Repository: NoteRepository + ?Sized,
{
    let exported = export_summary(repository, meeting_id, format, font_config)?;
    let save_error = |error: std::io::Error| SummaryExportFailure::Export(error.to_string());
    let folder = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(folder).map_err(save_error)?;
    let partial_path = folder.join(format!(".summary-export-{}.partial", uuid::Uuid::new_v4()));
    std::fs::write(&partial_path, &exported.bytes).map_err(save_error)?;
    if let Err(error) = std::fs::rename(&partial_path, output_path) {
        let _ = std::fs::remove_file(&partial_path);
        return Err(save_error(error));
    }
    Ok(output_path.to_path_buf())
}
