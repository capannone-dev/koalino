//! Orchestrates meeting exports: extracts data through storage and voice ports, stages attachment
//! copies, chooses output paths, and delegates rendering exclusively through `MeetingExporter`.
//!
//! The work is split in two so drivers can keep heavy rendering off async runtimes:
//! - `ExportManager` (read side) borrows the repositories and builds `ExportDocument` values.
//! - `ExportWriter` (write side) owns the exporters and output directory, is `Send`, and performs
//!   all filesystem work. The manager only borrows it, so a driver can drop the manager (and its
//!   repository locks) after extraction and move the writer onto a blocking thread.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    domain::{
        errors::ExportError,
        models::{
            ExportAttachment, ExportDocument, ExportFormat, ExportOptions, ExportResult,
            ExportTranscriptSegment, MeetingId, SpeakerIdentity,
        },
    },
    ports::{
        export::MeetingExporter,
        storage::{AttachmentRepository, NoteRepository},
        voice::VoiceVectorDB,
    },
};

/// Speaker label for segments diarization could not attribute.
pub const UNATTRIBUTED_SPEAKER_LABEL: &str = "Unattributed";
/// Speaker label for segments whose identity no longer exists in the voice store.
pub const UNKNOWN_SPEAKER_LABEL: &str = "Unknown speaker";

/// Extracts offline meeting data and delegates artifact creation to an `ExportWriter`.
pub struct ExportManager<'repository, NotePort: ?Sized, AttachmentPort: ?Sized, VoicePort: ?Sized> {
    note_repository: &'repository NotePort,
    attachment_repository: &'repository AttachmentPort,
    voice_database: &'repository VoicePort,
    writer: &'repository ExportWriter,
}

impl<'repository, NotePort, AttachmentPort, VoicePort>
    ExportManager<'repository, NotePort, AttachmentPort, VoicePort>
where
    NotePort: NoteRepository + ?Sized,
    AttachmentPort: AttachmentRepository + ?Sized,
    VoicePort: VoiceVectorDB + ?Sized,
{
    /// Creates an export orchestrator from injected ports and a configured writer.
    ///
    /// Inputs: Borrowed meeting, attachment, and voice-identity ports; borrowed `writer` holding
    /// exporters and the default output directory.
    /// Outputs: A manager ready to export.
    /// Side effects: None.
    pub fn new(
        note_repository: &'repository NotePort,
        attachment_repository: &'repository AttachmentPort,
        voice_database: &'repository VoicePort,
        writer: &'repository ExportWriter,
    ) -> Self {
        Self {
            note_repository,
            attachment_repository,
            voice_database,
            writer,
        }
    }

    /// Extracts and writes one meeting.
    ///
    /// Inputs: `meeting_id` to export, `options`, and an optional explicit `destination` file
    /// (otherwise a unique name is generated in the writer's output directory).
    /// Outputs: The artifact summary, or an error wrapping `ExportError` (`UnsupportedFormat`,
    /// `MeetingNotFound`, `Io`, `Rendering`) or a storage failure.
    /// Side effects: Reads repositories; copies attachments; writes the export file.
    pub fn export_meeting(
        &self,
        meeting_id: &MeetingId,
        options: &ExportOptions,
        destination: Option<&Path>,
    ) -> Result<ExportResult> {
        self.writer.ensure_format_supported(options.format())?;
        let document = self.extract_meeting(meeting_id, options)?;
        self.writer.write(vec![document], options, destination)
    }

    /// Extracts and writes several meetings into one batch artifact.
    ///
    /// Inputs: Non-empty `meeting_ids`, `options`, and an optional explicit `destination`.
    /// Outputs: The artifact summary or an error as for `export_meeting`, plus
    /// `ExportError::NoMeetingsSelected` for an empty list.
    /// Side effects: Reads repositories; copies attachments; writes the export file.
    pub fn export_meetings(
        &self,
        meeting_ids: &[MeetingId],
        options: &ExportOptions,
        destination: Option<&Path>,
    ) -> Result<ExportResult> {
        self.writer.ensure_format_supported(options.format())?;
        let documents = self.extract_meetings(meeting_ids, options)?;
        self.writer.write(documents, options, destination)
    }

    /// Exports every local meeting created inside the options' date range (all meetings if none).
    ///
    /// Inputs: `options` (its `date_range` filters by creation time) and an optional destination.
    /// Outputs: The artifact summary, or `ExportError::NoMeetingsSelected` when nothing matched.
    /// Side effects: Reads repositories; copies attachments; writes the export file.
    pub fn export_all(
        &self,
        options: &ExportOptions,
        destination: Option<&Path>,
    ) -> Result<ExportResult> {
        self.writer.ensure_format_supported(options.format())?;
        let meeting_ids = self.select_meetings(options)?;
        self.export_meetings(&meeting_ids, options, destination)
    }

    /// Lists meetings matching the options' date range, in repository order.
    ///
    /// Inputs: `options`; only `date_range` is consulted.
    /// Outputs: Matching meeting IDs, or `ExportError::NoMeetingsSelected` when none match.
    /// Side effects: Reads meeting metadata only.
    pub fn select_meetings(&self, options: &ExportOptions) -> Result<Vec<MeetingId>> {
        let mut selected_meeting_ids = Vec::new();
        for meeting_id in self
            .note_repository
            .list_meeting_ids()
            .context("unable to list meetings for export")?
        {
            let Some(meeting_metadata) = self
                .note_repository
                .load_meeting_metadata(&meeting_id)
                .context("unable to load meeting metadata for export")?
            else {
                continue;
            };
            if options
                .date_range()
                .is_none_or(|date_range| date_range.contains(meeting_metadata.created_at()))
            {
                selected_meeting_ids.push(meeting_id);
            }
        }
        if selected_meeting_ids.is_empty() {
            return Err(ExportError::NoMeetingsSelected.into());
        }
        Ok(selected_meeting_ids)
    }

    /// Extracts several meetings, failing on the first missing one.
    ///
    /// Inputs: `meeting_ids` (must be non-empty) and `options`.
    /// Outputs: Documents in request order, or the first extraction error.
    /// Side effects: Reads repositories.
    pub fn extract_meetings(
        &self,
        meeting_ids: &[MeetingId],
        options: &ExportOptions,
    ) -> Result<Vec<ExportDocument>> {
        if meeting_ids.is_empty() {
            return Err(ExportError::NoMeetingsSelected.into());
        }
        meeting_ids
            .iter()
            .map(|meeting_id| self.extract_meeting(meeting_id, options))
            .collect()
    }

    /// Builds the format-neutral export document for one meeting.
    ///
    /// Inputs: `meeting_id` and `options` (attachments are loaded only when included).
    /// Outputs: The document, `ExportError::MeetingNotFound`, or a storage failure. Stored
    /// segments are preferred; a flat transcript becomes one unattributed segment; a meeting
    /// with neither exports an empty transcript.
    /// Side effects: Reads meeting, segment, transcript, attachment, and identity metadata.
    pub fn extract_meeting(
        &self,
        meeting_id: &MeetingId,
        options: &ExportOptions,
    ) -> Result<ExportDocument> {
        let meeting_metadata = self
            .note_repository
            .load_meeting_metadata(meeting_id)
            .context("unable to load meeting metadata for export")?
            .ok_or_else(|| ExportError::MeetingNotFound {
                meeting_id: String::from(meeting_id.as_str()),
            })?;
        let known_identities: HashMap<Uuid, SpeakerIdentity> = self
            .voice_database
            .list_speaker_identities()
            .context("unable to load speaker identities for export")?
            .into_iter()
            .map(|identity| (identity.identity_id(), identity))
            .collect();

        let mut transcript_segments = self
            .note_repository
            .load_transcript_segments(meeting_id)
            .context("unable to load transcript segments for export")?
            .into_iter()
            .map(|segment| {
                let speaker_label = match segment.speaker_identity_id() {
                    Some(identity_id) => known_identities
                        .get(&identity_id)
                        .map_or(UNKNOWN_SPEAKER_LABEL, SpeakerIdentity::display_name),
                    None => UNATTRIBUTED_SPEAKER_LABEL,
                };
                ExportTranscriptSegment::new(
                    segment.start_time_ms(),
                    segment.end_time_ms(),
                    segment.speaker_identity_id(),
                    speaker_label,
                    segment.text(),
                )
            })
            .collect::<Vec<_>>();
        if transcript_segments.is_empty()
            && let Some(flat_transcript) = self
                .note_repository
                .load_transcript(meeting_id)
                .context("unable to load transcript for export")?
            && !flat_transcript.content().trim().is_empty()
        {
            transcript_segments.push(ExportTranscriptSegment::new(
                0,
                0,
                None,
                UNATTRIBUTED_SPEAKER_LABEL,
                flat_transcript.content(),
            ));
        }

        let mut referenced_identities: Vec<SpeakerIdentity> = Vec::new();
        for segment in &transcript_segments {
            if let Some(identity) = segment
                .speaker_identity_id()
                .and_then(|identity_id| known_identities.get(&identity_id))
                && !referenced_identities.contains(identity)
            {
                referenced_identities.push(identity.clone());
            }
        }

        let attachments = if options.include_attachments() {
            self.attachment_repository
                .get_attachments_for_meeting(meeting_id)
                .context("unable to load attachments for export")?
                .into_iter()
                .map(|attachment| {
                    ExportAttachment::new(
                        attachment.attachment_id(),
                        attachment.file_path(),
                        attachment.mime_type(),
                        attachment.original_timestamp(),
                    )
                })
                .collect()
        } else {
            Vec::new()
        };

        Ok(ExportDocument::new(
            meeting_id.clone(),
            meeting_metadata.display_title(),
            meeting_metadata.created_at(),
            transcript_segments,
            attachments,
            referenced_identities,
        )
        .with_stored_title(meeting_metadata.title().is_some()))
    }
}

/// Owns the format exporters and output directory and performs all export filesystem work.
pub struct ExportWriter {
    exporters: Vec<Box<dyn MeetingExporter>>,
    output_directory: PathBuf,
}

impl ExportWriter {
    /// Creates a writer from exporters and the default output directory.
    ///
    /// Inputs: One exporter per supported format; `output_directory` for generated filenames.
    /// Outputs: The writer.
    /// Side effects: None; directories are created lazily when writing.
    pub fn new(
        exporters: Vec<Box<dyn MeetingExporter>>,
        output_directory: impl AsRef<Path>,
    ) -> Self {
        Self {
            exporters,
            output_directory: output_directory.as_ref().to_path_buf(),
        }
    }

    /// Borrows the directory used for generated filenames.
    pub fn output_directory(&self) -> &Path {
        &self.output_directory
    }

    /// Rejects formats without a configured exporter (including reserved SRT).
    ///
    /// Inputs: The requested `format`.
    /// Outputs: Success or `ExportError::UnsupportedFormat`.
    /// Side effects: None.
    pub fn ensure_format_supported(&self, format: ExportFormat) -> Result<(), ExportError> {
        self.exporter_for(format).map(|_| ())
    }

    /// Stages attachments, resolves the output path, and delegates to the matching exporter.
    ///
    /// Inputs: Extracted `documents`, `options`, and an optional explicit `destination` file.
    /// Outputs: The artifact summary or an error wrapping `ExportError`.
    /// Side effects: Markdown copies attachments into `<file stem>_attachments/` beside the
    /// export; PDF copies them into a temporary directory that is always removed afterwards;
    /// JSON copies nothing. Missing source files are reported as unavailable, not as failures.
    /// Writes the export file; source attachment files are never modified.
    pub fn write(
        &self,
        documents: Vec<ExportDocument>,
        options: &ExportOptions,
        destination: Option<&Path>,
    ) -> Result<ExportResult> {
        let exporter = self.exporter_for(options.format())?;
        let output_path = match destination {
            Some(explicit_destination) => explicit_destination.to_path_buf(),
            None => unique_output_path(&self.output_directory, &documents, options.format()),
        };
        let has_attachments = documents
            .iter()
            .any(|document| !document.attachments().is_empty());
        let staging_directory = match options.format() {
            ExportFormat::Markdown if has_attachments => {
                Some(StagingDirectory::beside(&output_path))
            }
            ExportFormat::Pdf if has_attachments => Some(StagingDirectory::temporary()),
            _ => None,
        };
        let documents = match &staging_directory {
            Some(staging) => stage_attachments(documents, staging.path())?,
            None => documents,
        };
        let export_outcome = exporter.export_meetings(&documents, options, &output_path);
        if let Some(staging) = staging_directory {
            staging.finish(export_outcome.is_ok());
        }
        Ok(export_outcome?)
    }

    /// Finds the exporter registered for `format`.
    fn exporter_for(&self, format: ExportFormat) -> Result<&dyn MeetingExporter, ExportError> {
        self.exporters
            .iter()
            .find(|exporter| exporter.format() == format)
            .map(AsRef::as_ref)
            .ok_or(ExportError::UnsupportedFormat { format })
    }
}

/// A directory holding staged attachment copies and its cleanup policy.
struct StagingDirectory {
    path: PathBuf,
    /// `true` for temporary PDF staging, which is removed even after success.
    is_temporary: bool,
}

impl StagingDirectory {
    /// Stages beside a Markdown file as `<stem>_attachments/` so relative links survive moves.
    fn beside(output_path: &Path) -> Self {
        let file_stem = output_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("export");
        let parent_directory = output_path.parent().unwrap_or_else(|| Path::new(""));
        Self {
            path: parent_directory.join(format!("{file_stem}_attachments")),
            is_temporary: false,
        }
    }

    /// Stages in a unique OS temp directory for renderers that embed file bytes.
    fn temporary() -> Self {
        Self {
            path: std::env::temp_dir()
                .join(format!("note-taker-export-staging-{}", Uuid::new_v4())),
            is_temporary: true,
        }
    }

    /// Borrows the staging path.
    fn path(&self) -> &Path {
        &self.path
    }

    /// Removes temporary staging always, and persistent staging only when the export failed.
    fn finish(self, export_succeeded: bool) {
        if self.is_temporary || !export_succeeded {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

/// Copies each readable attachment into `staging_directory` and records the copy's path.
///
/// Inputs: Extracted `documents` and the staging directory.
/// Outputs: Documents whose attachments carry `staged_path` when copying succeeded, or
/// `ExportError::Io` when the staging directory cannot be created.
/// Side effects: Creates the staging directory and copies files. Unreadable sources are logged by
/// attachment ID only and left unstaged.
fn stage_attachments(
    documents: Vec<ExportDocument>,
    staging_directory: &Path,
) -> Result<Vec<ExportDocument>> {
    fs::create_dir_all(staging_directory).map_err(|error| ExportError::Io {
        reason: format!("unable to create attachment staging directory: {error}"),
    })?;
    let mut used_file_names: Vec<String> = Vec::new();
    Ok(documents
        .into_iter()
        .map(|document| {
            let staged_attachments = document
                .attachments()
                .iter()
                .map(|attachment| {
                    let mut staged_file_name = safe_file_name(attachment.display_name());
                    if used_file_names.contains(&staged_file_name) {
                        staged_file_name = format!(
                            "{}_{staged_file_name}",
                            &attachment.attachment_id().simple().to_string()[..8]
                        );
                    }
                    let staged_path = staging_directory.join(&staged_file_name);
                    match fs::copy(attachment.file_path(), &staged_path) {
                        Ok(_) => {
                            used_file_names.push(staged_file_name);
                            attachment
                                .clone()
                                .with_staged_path(staged_path.to_string_lossy())
                        }
                        Err(_) => {
                            tracing::warn!(attachment_id = %attachment.attachment_id(), "export could not read attachment source; listing it as unavailable");
                            attachment.clone()
                        }
                    }
                })
                .collect();
            document.with_attachments(staged_attachments)
        })
        .collect())
}

/// Generates `{date}_{label}_{timestamp}.{ext}` (single) or
/// `meetings-export_{count}_{timestamp}.{ext}` (batch), appending `-N` until the path is unused.
/// `label` is the sanitized stored title, or the meeting ID for untitled meetings (see
/// `single_meeting_file_label`).
///
/// Inputs: The output directory, the documents being exported, and the format.
/// Outputs: A path that does not currently exist.
/// Side effects: Checks the filesystem for existing files.
fn unique_output_path(
    output_directory: &Path,
    documents: &[ExportDocument],
    format: ExportFormat,
) -> PathBuf {
    let export_timestamp = Utc::now().format("%Y%m%dT%H%M%S%3fZ");
    let file_stem = match documents {
        [single_document] => format!(
            "{}_{}_{export_timestamp}",
            single_document.started_at().format("%Y-%m-%d"),
            single_meeting_file_label(single_document)
        ),
        _ => format!("meetings-export_{}_{export_timestamp}", documents.len()),
    };
    let extension = format.file_extension();
    let mut candidate_path = output_directory.join(format!("{file_stem}.{extension}"));
    let mut collision_counter = 1_u32;
    while candidate_path.exists() {
        candidate_path =
            output_directory.join(format!("{file_stem}-{collision_counter}.{extension}"));
        collision_counter += 1;
    }
    candidate_path
}

/// Longest title-derived filename label, in characters, so paths stay well under OS limits.
const MAX_TITLE_FILE_LABEL_CHARACTERS: usize = 80;

/// Chooses the filename label of a single-meeting export.
///
/// Inputs: The export document.
/// Outputs: The sanitized stored title (truncated to `MAX_TITLE_FILE_LABEL_CHARACTERS`) when the
/// meeting has one with at least one ASCII letter or digit; otherwise the sanitized meeting ID.
/// The dated fallback title is not used because the filename already starts with the date and
/// the ID is what tells two untitled meetings apart.
/// Side effects: None.
fn single_meeting_file_label(document: &ExportDocument) -> String {
    if document.has_stored_title() {
        let title_label: String = safe_file_name(document.title())
            .chars()
            .take(MAX_TITLE_FILE_LABEL_CHARACTERS)
            .collect();
        if title_label
            .chars()
            .any(|character| character.is_ascii_alphanumeric())
        {
            return title_label;
        }
    }
    safe_file_name(document.meeting_id().as_str())
}

/// Replaces characters outside `[A-Za-z0-9._-]` with `_` and strips leading dots.
///
/// Inputs: An identifier or file name.
/// Outputs: A non-empty name safe on every supported filesystem.
/// Side effects: None.
fn safe_file_name(raw_name: &str) -> String {
    let sanitized_name: String = raw_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect();
    let trimmed_name = sanitized_name.trim_start_matches('.');
    if trimmed_name.is_empty() {
        String::from("export")
    } else {
        String::from(trimmed_name)
    }
}
