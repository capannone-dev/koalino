//! Prepares, writes and keeps meeting summaries (#95), shared by the app (FFI), the web API and
//! the CLI.
//!
//! The work is split so callers hold the repository only briefly: `prepare_summary` reads what
//! the summary is written from, `run_summary` asks the model (which can take minutes, so it runs
//! with no lock held, on a background thread), and `store_summary` writes the result into the
//! meeting document in one transaction.

use std::{collections::HashMap, path::Path};

use chrono::{DateTime, Utc};

use crate::{
    application::{
        export_manager::{ExportManager, ExportWriter},
        meeting_crdt::MeetingCrdtService,
        meeting_summary::{
            SpokenLine, SummaryError, SummaryMaterial, SummaryOutcome, summarize_meeting,
        },
        meeting_summary_document::{
            read_summary, read_summary_prompt, with_summary, with_summary_prompt,
        },
    },
    domain::{
        llm::{
            LlmConnection, LlmImage, MAX_LLM_IMAGE_BYTES, MAX_LLM_IMAGES, MeetingSummary,
            PromptTemplate, PromptTemplateError, PromptValues,
        },
        models::{ExportFormat, ExportOptions, MeetingId},
    },
    ports::{
        ai::LlmEngine,
        projects::ProjectRepository,
        storage::{AttachmentRepository, NoteRepository},
        voice::VoiceVectorDB,
    },
};

/// What a summary is written from, read from storage.
#[derive(Clone, Debug)]
pub struct PreparedSummary {
    /// The transcript, notes, photos and template values.
    pub material: SummaryMaterial,
    /// The meeting's own template text, when it has one.
    pub meeting_prompt: Option<String>,
}

/// Reads everything a meeting's summary is written from.
///
/// Inputs: The repositories, the meeting, the glossary terms and the recording device's name.
/// Outputs: The material; `Storage` when the meeting cannot be read. Images are the meeting's
/// JPEG, PNG, WebP and GIF attachments under the size limit, oldest first, at most
/// `MAX_LLM_IMAGES`. Side effects: Reads the database and the image files.
pub fn prepare_summary<Repository, Voices>(
    repository: &Repository,
    voices: &Voices,
    meeting_id: &MeetingId,
    glossary: &[String],
    device_name: &str,
) -> Result<PreparedSummary, SummaryError>
where
    Repository: NoteRepository + AttachmentRepository + ProjectRepository + ?Sized,
    Voices: VoiceVectorDB + ?Sized,
{
    let storage = |error: &dyn std::fmt::Display| SummaryError::Storage(error.to_string());
    let writer = ExportWriter::new(Vec::new(), std::env::temp_dir());
    let document = ExportManager::new(repository, repository, voices, &writer)
        .extract_meeting(
            meeting_id,
            &ExportOptions::new(ExportFormat::Markdown, true, true, None),
        )
        .map_err(|error| storage(&error))?;
    let notes = MeetingCrdtService::new(repository)
        .load_note_text(meeting_id)
        .map_err(|error| storage(&error))?;
    let project = project_name(repository, meeting_id).map_err(|error| storage(&error))?;
    let language = repository
        .load_meeting_metadata(meeting_id)
        .map_err(|error| storage(&error))?
        .and_then(|metadata| {
            metadata
                .transcription_languages()
                .iter()
                .map(|language| language.as_str())
                .find(|code| *code != "auto")
                .map(str::to_owned)
        })
        .unwrap_or_else(|| String::from("the language spoken in the transcript"));
    let meeting_prompt = read_summary_prompt(
        &repository
            .load_crdt_document(meeting_id)
            .map_err(|error| storage(&error))?,
    )
    .map_err(|error| storage(&error))?;

    let lines: Vec<SpokenLine> = document
        .transcript_segments()
        .iter()
        .map(|segment| SpokenLine {
            speaker: segment.speaker().to_owned(),
            start_ms: segment.start_time_ms(),
            text: segment.text().to_owned(),
        })
        .collect();
    let mut speakers: Vec<String> = Vec::new();
    for line in &lines {
        if !speakers.contains(&line.speaker) {
            speakers.push(line.speaker.clone());
        }
    }
    let mut attachments: Vec<_> = document
        .attachments()
        .iter()
        .filter(|attachment| attachment.is_image())
        .collect();
    attachments.sort_by_key(|attachment| attachment.original_timestamp());
    let images = attachments
        .into_iter()
        .filter_map(|attachment| {
            read_image(Path::new(attachment.file_path()), attachment.mime_type())
        })
        .take(MAX_LLM_IMAGES)
        .collect();
    let started_at = document.started_at();
    let minutes = document.duration_ms().div_ceil(60_000);
    Ok(PreparedSummary {
        material: SummaryMaterial {
            values: PromptValues {
                title: document.title().to_owned(),
                date: started_at.format("%Y-%m-%d").to_string(),
                time: started_at.format("%H:%M UTC").to_string(),
                duration: format!("{minutes} min"),
                device: device_name.to_owned(),
                project,
                speakers,
                glossary: glossary.to_vec(),
                language,
            },
            notes,
            lines,
            images,
        },
        meeting_prompt,
    })
}

/// Reads an image file the model can take, skipping one too large or unreadable.
fn read_image(path: &Path, mime_type: &str) -> Option<LlmImage> {
    let size = std::fs::metadata(path).ok()?.len();
    if size == 0 || size > MAX_LLM_IMAGE_BYTES as u64 {
        return None;
    }
    LlmImage::new(mime_type, std::fs::read(path).ok()?)
}

/// Finds the name of the meeting's project.
fn project_name<Repository>(
    repository: &Repository,
    meeting_id: &MeetingId,
) -> Result<Option<String>, crate::domain::errors::StorageError>
where
    Repository: ProjectRepository + ?Sized,
{
    let projects: HashMap<_, _> = repository.meeting_projects()?.into_iter().collect();
    let Some(project_id) = projects.get(meeting_id) else {
        return Ok(None);
    };
    Ok(repository
        .load_project(*project_id)?
        .map(|project| project.name().as_str().to_owned()))
}

/// Chooses the template: the meeting's own when it is still valid, else the default.
///
/// Side effects: None.
pub fn summary_template_for(
    prepared: &PreparedSummary,
    default: &PromptTemplate,
) -> PromptTemplate {
    prepared
        .meeting_prompt
        .as_deref()
        .and_then(|text| PromptTemplate::parse(text).ok())
        .unwrap_or_else(|| default.clone())
}

/// Asks the model for the summary.
///
/// Inputs: The engine for `connection`, the owner's default template, the prepared material and
/// the time to record. Outputs: The summary and how it was obtained, or why there is none.
/// Side effects: Sends the meeting's text and photos to the connection's server. Blocks.
pub fn run_summary(
    engine: &dyn LlmEngine,
    connection: &LlmConnection,
    default_template: &PromptTemplate,
    prepared: PreparedSummary,
    now: DateTime<Utc>,
) -> Result<SummaryOutcome, SummaryError> {
    let template = summary_template_for(&prepared, default_template);
    summarize_meeting(engine, connection, &template, prepared.material, now)
}

/// Stores a summary in the meeting document, replacing the earlier one.
///
/// Side effects: Writes the document in one transaction; it syncs to paired devices.
pub fn store_summary<Repository>(
    repository: &Repository,
    meeting_id: &MeetingId,
    summary: &MeetingSummary,
) -> Result<(), SummaryError>
where
    Repository: NoteRepository + ?Sized,
{
    repository
        .edit_crdt_document(meeting_id, &|document| with_summary(document, summary))
        .map_err(|error| SummaryError::Storage(error.to_string()))
}

/// Reads the meeting's summary.
///
/// Outputs: The summary, `None` when there is none, or a storage error. Side effects: Reads the
/// document.
pub fn load_summary<Repository>(
    repository: &Repository,
    meeting_id: &MeetingId,
) -> Result<Option<MeetingSummary>, SummaryError>
where
    Repository: NoteRepository + ?Sized,
{
    repository
        .load_crdt_document(meeting_id)
        .and_then(|document| read_summary(&document))
        .map_err(|error| SummaryError::Storage(error.to_string()))
}

/// Why the meeting's own prompt was not saved.
#[derive(Debug, thiserror::Error)]
pub enum MeetingPromptError {
    /// The template is not valid.
    #[error(transparent)]
    Template(#[from] PromptTemplateError),
    /// The document could not be written.
    #[error("the prompt could not be saved: {0}")]
    Storage(String),
}

/// Sets the meeting's own summary prompt, or goes back to the default with `None`.
///
/// Outputs: Success, the template's problem, or a storage error. Side effects: Writes the
/// document; it syncs to paired devices.
pub fn set_meeting_prompt<Repository>(
    repository: &Repository,
    meeting_id: &MeetingId,
    prompt: Option<&str>,
) -> Result<(), MeetingPromptError>
where
    Repository: NoteRepository + ?Sized,
{
    let checked = prompt.map(PromptTemplate::parse).transpose()?;
    repository
        .edit_crdt_document(meeting_id, &|document| {
            with_summary_prompt(document, checked.as_ref().map(PromptTemplate::as_str))
        })
        .map_err(|error| MeetingPromptError::Storage(error.to_string()))
}
