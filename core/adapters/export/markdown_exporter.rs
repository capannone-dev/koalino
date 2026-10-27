//! Renders portable meeting export documents as deterministic, human-readable Markdown.
//!
//! Layout per meeting: title heading, optional metadata list, speaker-attributed transcript,
//! attachment links relative to the export file, and summary statistics. Batches add a top-level
//! `# Meeting export` heading and separate meetings with horizontal rules.

use std::path::{Component, Path};

use crate::{
    domain::{
        errors::ExportError,
        models::{
            ExportAttachment, ExportDocument, ExportFormat, ExportOptions, ExportResult,
            format_export_offset,
        },
    },
    ports::export::MeetingExporter,
};

use super::{ensure_documents_present, write_export_artifact};

/// Produces CommonMark output while keeping all repository access in the application layer.
pub struct MarkdownMeetingExporter;

impl MarkdownMeetingExporter {
    /// Renders one or more meetings as a single Markdown document.
    ///
    /// Inputs: `documents` are extracted meetings (one renders as a standalone document, several
    /// as a batch); `options` controls the metadata section; `output_directory` is the directory
    /// the Markdown file will live in and anchors relative attachment links.
    /// Outputs: UTF-8 Markdown text.
    /// Side effects: None; it neither reads nor copies attachment files.
    pub fn render(
        documents: &[ExportDocument],
        options: &ExportOptions,
        output_directory: &Path,
    ) -> String {
        match documents {
            [single_document] => render_meeting(single_document, options, output_directory, 1),
            _ => {
                let rendered_meetings = documents
                    .iter()
                    .map(|document| render_meeting(document, options, output_directory, 2))
                    .collect::<Vec<_>>();
                format!(
                    "# Meeting export\n\n- Meetings: {}\n\n{}",
                    documents.len(),
                    rendered_meetings.join("\n---\n\n")
                )
            }
        }
    }
}

impl MeetingExporter for MarkdownMeetingExporter {
    fn format(&self) -> ExportFormat {
        ExportFormat::Markdown
    }

    /// Writes rendered Markdown atomically to `output_path`.
    ///
    /// Inputs: Non-empty `documents`, export `options`, and the destination path.
    /// Outputs: The written artifact summary or an `ExportError`.
    /// Side effects: Creates the destination directory and file.
    fn export_meetings(
        &self,
        documents: &[ExportDocument],
        options: &ExportOptions,
        output_path: &Path,
    ) -> Result<ExportResult, ExportError> {
        ensure_documents_present(documents.len())?;
        let output_directory = output_path.parent().unwrap_or_else(|| Path::new(""));
        let rendered_markdown = Self::render(documents, options, output_directory);
        write_export_artifact(
            output_path,
            rendered_markdown.as_bytes(),
            ExportFormat::Markdown,
            documents.len(),
        )
    }
}

/// Renders one meeting with headings starting at `heading_level`.
///
/// Inputs: The meeting `document`, export `options`, link anchor `output_directory`, and the
/// heading level of the meeting title (1 standalone, 2 inside a batch).
/// Outputs: Markdown for the meeting.
/// Side effects: None.
fn render_meeting(
    document: &ExportDocument,
    options: &ExportOptions,
    output_directory: &Path,
    heading_level: usize,
) -> String {
    let title_heading = "#".repeat(heading_level);
    let section_heading = "#".repeat(heading_level + 1);
    let mut markdown_output = format!("{title_heading} {}\n\n", escape_markdown(document.title()));

    if options.include_metadata() {
        let participants = document.participants();
        markdown_output.push_str(&format!(
            "{section_heading} Meeting metadata\n\n- Date: {}\n- Duration: {}\n- Participants: {}\n\n",
            document.started_at().to_rfc3339(),
            format_export_offset(document.duration_ms()),
            if participants.is_empty() {
                String::from("none")
            } else {
                escape_markdown(&participants.join(", "))
            },
        ));
    }

    markdown_output.push_str(&format!("{section_heading} Transcript\n\n"));
    if document.transcript_segments().is_empty() {
        markdown_output.push_str("_No transcript available._\n\n");
    }
    for transcript_segment in document.transcript_segments() {
        markdown_output.push_str(&format!(
            "**{}** — [{}] {}\n\n",
            escape_markdown(transcript_segment.speaker()),
            format_export_offset(transcript_segment.start_time_ms()),
            escape_markdown(transcript_segment.text()),
        ));
    }

    if !document.attachments().is_empty() {
        markdown_output.push_str(&format!("{section_heading} Attachments\n\n"));
        for attachment in document.attachments() {
            markdown_output.push_str(&render_attachment(attachment, output_directory));
        }
        markdown_output.push('\n');
    }

    markdown_output.push_str(&format!("{section_heading} Summary statistics\n\n"));
    markdown_output.push_str(&format!("- Word count: {}\n", document.word_count()));
    for (speaker, speaking_time_ms) in document.speaking_time_by_speaker() {
        markdown_output.push_str(&format!(
            "- Speaking time: {} — {}\n",
            escape_markdown(&speaker),
            format_export_offset(speaking_time_ms)
        ));
    }
    markdown_output
}

/// Renders one attachment list item, linking the staged copy relative to the export file.
///
/// Inputs: `attachment` metadata and the export file's `output_directory`.
/// Outputs: One Markdown list line ending in a newline. Images use `![…](…)` so they display
/// inline; attachments without a staged copy are listed as unavailable without a link.
/// Side effects: None.
fn render_attachment(attachment: &ExportAttachment, output_directory: &Path) -> String {
    let display_name = escape_markdown(attachment.display_name());
    let Some(staged_path) = attachment.staged_path() else {
        return format!(
            "- {display_name} ({}) — source file unavailable\n",
            attachment.mime_type()
        );
    };
    let link_target = relative_link_target(Path::new(staged_path), output_directory);
    if attachment.is_image() {
        format!("- ![{display_name}](<{link_target}>)\n")
    } else {
        format!(
            "- [{display_name}](<{link_target}>) ({})\n",
            attachment.mime_type()
        )
    }
}

/// Expresses `target_path` relative to `base_directory` using `/` separators.
///
/// Inputs: The staged attachment path and the directory containing the Markdown file.
/// Outputs: A relative path when `target_path` is inside `base_directory`, otherwise the
/// target path unchanged (still `/`-separated).
/// Side effects: None.
fn relative_link_target(target_path: &Path, base_directory: &Path) -> String {
    let relative_path = target_path
        .strip_prefix(base_directory)
        .unwrap_or(target_path);
    relative_path
        .components()
        .filter_map(|component| match component {
            Component::Normal(segment) => Some(segment.to_string_lossy().into_owned()),
            Component::RootDir => Some(String::new()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
        .replace('>', "%3E")
}

/// Escapes characters that CommonMark would interpret, and folds newlines into spaces.
///
/// Inputs: User- or model-supplied text.
/// Outputs: Text that renders literally inside a paragraph, heading, or list item.
/// Side effects: None.
fn escape_markdown(raw_text: &str) -> String {
    let mut escaped_text = String::with_capacity(raw_text.len());
    for character in raw_text.chars() {
        match character {
            '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '#' | '|' => {
                escaped_text.push('\\');
                escaped_text.push(character);
            }
            '\r' | '\n' => escaped_text.push(' '),
            _ => escaped_text.push(character),
        }
    }
    escaped_text
}
