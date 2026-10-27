//! Declares the application-facing boundary for writing format-specific meeting exports.
//!
//! Exporters receive fully extracted `ExportDocument` values. They never access repositories, so
//! the application layer owns data selection and adapters own only presentation and file layout.

use std::path::Path;

use crate::domain::{
    errors::ExportError,
    models::{ExportDocument, ExportFormat, ExportOptions, ExportResult},
};

/// Writes application-extracted meeting data in one concrete exchange format.
pub trait MeetingExporter: Send + Sync {
    /// Returns the single format produced by this renderer.
    ///
    /// Inputs: None.
    /// Outputs: The renderer's format; the orchestrator dispatches on it.
    /// Side effects: None.
    fn format(&self) -> ExportFormat;

    /// Exports one meeting document to `output_path`.
    ///
    /// Inputs: `document` is extracted meeting data (attachments already staged when required);
    /// `options` carries inclusion policy; `output_path` is the destination file.
    /// Outputs: File metadata for the completed export or an `ExportError`.
    /// Side effects: Creates or replaces `output_path` and creates its parent directory.
    fn export_meeting(
        &self,
        document: &ExportDocument,
        options: &ExportOptions,
        output_path: &Path,
    ) -> Result<ExportResult, ExportError> {
        self.export_meetings(std::slice::from_ref(document), options, output_path)
    }

    /// Exports several meeting documents as one format-specific batch artifact.
    ///
    /// Inputs: `documents` is non-empty and ordered as it should appear; `options` applies to all.
    /// Outputs: File metadata for the completed batch export or an `ExportError`.
    /// Side effects: Creates or replaces `output_path` and creates its parent directory.
    fn export_meetings(
        &self,
        documents: &[ExportDocument],
        options: &ExportOptions,
        output_path: &Path,
    ) -> Result<ExportResult, ExportError>;
}
