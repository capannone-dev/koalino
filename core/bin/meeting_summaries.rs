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
    adapters::documents::text_extraction::{extract_text, has_text},
    application::{
        combined_meetings::{MeetingListing, name_recording_devices, recording_devices_label},
        export_manager::{ExportManager, ExportWriter},
        meeting_crdt::MeetingCrdtService,
        meeting_handoff::{HandoffDelivery, HandoffPurpose, handoff_file_name, prepare_handoff},
        meeting_summary::{
            AttachedDocument, SpokenLine, SummaryError, SummaryMaterial, SummaryOutcome,
            summarize_meeting,
        },
        meeting_summary_document::{
            read_summary, read_summary_prompt, read_summary_request, with_summary,
            with_summary_prompt, with_summary_request, without_summary_request,
        },
        p2p_service::P2pStatus,
    },
    domain::{
        llm::{
            LlmConnection, LlmImage, MAX_LLM_IMAGE_BYTES, MAX_LLM_IMAGES, MeetingSummary,
            PromptTemplate, PromptTemplateError, PromptValues, SummaryRequest,
        },
        models::{ExportAttachment, ExportFormat, ExportOptions, MeetingId},
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
/// Inputs: The repositories, the meeting, the glossary terms, this device's name and the P2P
/// status (`None` without P2P). `{{device}}` names the devices that recorded the meeting, as
/// paired devices are named here, or this device when there is no P2P (#119).
/// Outputs: The material; `Storage` when the meeting cannot be read. Images are the meeting's
/// JPEG, PNG, WebP and GIF attachments under the size limit, oldest first, at most
/// `MAX_LLM_IMAGES`. Side effects: Reads the database and the image files.
pub fn prepare_summary<Repository, Voices>(
    repository: &Repository,
    voices: &Voices,
    meeting_id: &MeetingId,
    glossary: &[String],
    device_name: &str,
    p2p: Option<&P2pStatus>,
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
            read_image(
                repository,
                Path::new(attachment.file_path()),
                attachment.mime_type(),
            )
        })
        .take(MAX_LLM_IMAGES)
        .collect();
    let documents = read_documents(repository, document.attachments());
    let device = match p2p {
        Some(status) => {
            let recordings = repository
                .meeting_recordings()
                .map_err(|error| storage(&error))?;
            recording_devices_label(
                &name_recording_devices(
                    &MeetingListing::of(&recordings).recorded_on(meeting_id),
                    Some(status),
                ),
                device_name,
            )
        }
        None => device_name.to_owned(),
    };
    let started_at = document.started_at();
    let minutes = rounded_minutes(document.duration_ms());
    Ok(PreparedSummary {
        material: SummaryMaterial {
            values: PromptValues {
                title: document.title().to_owned(),
                date: started_at.format("%Y-%m-%d").to_string(),
                time: started_at.format("%H:%M UTC").to_string(),
                duration: format!("{minutes} min"),
                device,
                project,
                speakers,
                glossary: glossary.to_vec(),
                language,
            },
            notes,
            lines,
            images,
            documents,
        },
        meeting_prompt,
    })
}

/// Rounds a meeting's length to whole minutes for `{{duration}}`: to the nearest minute, half a
/// minute up, and at least 1 so a short meeting never reads "0 min" (#119).
fn rounded_minutes(duration_ms: u64) -> u64 {
    (duration_ms.saturating_add(30_000) / 60_000).max(1)
}

/// Most files a hand-off attaches besides the meeting itself (#114).
const MAX_HANDOFF_FILES: usize = 20;
/// Largest single attached file, in bytes.
const MAX_HANDOFF_FILE_BYTES: u64 = 50 * 1024 * 1024;
/// Most bytes of attached files in one hand-off.
const MAX_HANDOFF_TOTAL_BYTES: u64 = 100 * 1024 * 1024;

/// One file of a hand-off.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandoffFile {
    /// Where it was written.
    pub path: std::path::PathBuf,
    /// Its MIME type, for the share.
    pub mime_type: String,
}

/// A hand-off written to disk, ready for the platform's share sheet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WrittenHandoff {
    /// The text the share carries.
    pub message: String,
    /// For a summary, the instructions as filled in; empty for a chat.
    pub prompt: String,
    /// The meeting's Markdown file first, then its photos and files, oldest first.
    pub files: Vec<HandoffFile>,
    /// Files left out: unreadable, or over the size or count limits.
    pub skipped: usize,
}

/// Writes a meeting's hand-off to the owner's own AI app (#114) into `directory`.
///
/// Inputs: The repositories, the meeting, the glossary, device name and P2P status (for the
/// prompt's `{{device}}`, see `prepare_summary`), the
/// owner's default template (the meeting's own wins), the purpose, how the meeting travels
/// (attached, or pasted with the message, #126), and a folder the hand-off owns. Outputs: The message and the files, or why there is nothing to hand off.
/// Side effects: Empties `directory` of an earlier hand-off's files, then writes the meeting as
/// Markdown and plain copies of its attachments (unsealed, #99). They are personal data in
/// clear: the caller shares them at once and empties the folder again soon.
#[allow(clippy::too_many_arguments)]
pub fn write_handoff<Repository, Voices>(
    repository: &Repository,
    voices: &Voices,
    meeting_id: &MeetingId,
    glossary: &[String],
    device_name: &str,
    p2p: Option<&P2pStatus>,
    default_template: &PromptTemplate,
    purpose: HandoffPurpose,
    delivery: HandoffDelivery,
    directory: &Path,
) -> Result<WrittenHandoff, SummaryError>
where
    Repository: NoteRepository + AttachmentRepository + ProjectRepository + ?Sized,
    Voices: VoiceVectorDB + ?Sized,
{
    let storage = |error: &dyn std::fmt::Display| SummaryError::Storage(error.to_string());
    let prepared = prepare_summary(repository, voices, meeting_id, glossary, device_name, p2p)?;
    let template = summary_template_for(&prepared, default_template);
    empty_folder(directory).map_err(|error| storage(&error))?;
    let document_name = handoff_file_name(&prepared.material.values.title);

    // The photos and files go as they are, never as their text (#128).
    let writer = ExportWriter::new(Vec::new(), std::env::temp_dir());
    let document = ExportManager::new(repository, repository, voices, &writer)
        .extract_meeting(
            meeting_id,
            &ExportOptions::new(ExportFormat::Markdown, true, true, None),
        )
        .map_err(|error| storage(&error))?;
    let mut attachments: Vec<_> = document.attachments().iter().collect();
    attachments.sort_by_key(|attachment| attachment.original_timestamp());
    let mut used_names = vec![document_name.clone()];
    let sources: Vec<HandoffSource> = attachments
        .into_iter()
        .map(|attachment| {
            let stored_name = attachment.display_name();
            HandoffSource {
                stored_path: attachment.file_path().to_owned(),
                mime_type: attachment.mime_type().to_owned(),
                name: stored_name
                    .strip_prefix(&format!("{}-", attachment.attachment_id()))
                    .unwrap_or(stored_name)
                    .to_owned(),
            }
        })
        .collect();
    let (attached, skipped) = copy_handoff_files(repository, &sources, directory, &mut used_names)?;
    let handoff = prepare_handoff(
        &template,
        &prepared.material,
        purpose,
        delivery,
        &used_names[1..],
    )?;
    let document_path = directory.join(&handoff.file_name);
    std::fs::write(&document_path, handoff.document.as_bytes()).map_err(|error| storage(&error))?;
    let mut files = vec![HandoffFile {
        path: document_path,
        mime_type: String::from("text/markdown"),
    }];
    files.extend(attached);
    Ok(WrittenHandoff {
        message: handoff.message,
        prompt: handoff.prompt,
        files,
        skipped,
    })
}

/// One stored file a hand-off copies.
#[derive(Clone, Debug)]
pub(crate) struct HandoffSource {
    /// Where the sealed file is stored.
    pub stored_path: String,
    /// Its MIME type.
    pub mime_type: String,
    /// Its shown name.
    pub name: String,
}

/// Copies stored files into a hand-off folder as plain files, in the given order (#114, #128).
///
/// Inputs: The repository, the files, the folder, and the names already used there (extended).
/// Outputs: The copies, and how many were left out (unreadable, or over
/// `MAX_HANDOFF_FILES`, `MAX_HANDOFF_FILE_BYTES` or `MAX_HANDOFF_TOTAL_BYTES`).
/// Side effects: Decrypts the files and writes clear copies into `directory`: personal data the
/// caller shares at once and deletes soon.
pub(crate) fn copy_handoff_files<Repository: AttachmentRepository + ?Sized>(
    repository: &Repository,
    sources: &[HandoffSource],
    directory: &Path,
    used_names: &mut Vec<String>,
) -> Result<(Vec<HandoffFile>, usize), SummaryError> {
    let storage = |error: &dyn std::fmt::Display| SummaryError::Storage(error.to_string());
    let mut files = Vec::new();
    let (mut skipped, mut total_bytes) = (0, 0_u64);
    for source in sources {
        if files.len() >= MAX_HANDOFF_FILES {
            skipped += 1;
            continue;
        }
        let Some(contents) = repository
            .read_attachment_file(Path::new(&source.stored_path), MAX_HANDOFF_FILE_BYTES)
            .filter(|contents| total_bytes + contents.len() as u64 <= MAX_HANDOFF_TOTAL_BYTES)
        else {
            skipped += 1;
            continue;
        };
        let name = unique_name(&handoff_file_stem(&source.name), used_names);
        let path = directory.join(&name);
        std::fs::write(&path, contents.as_slice()).map_err(|error| storage(&error))?;
        total_bytes += contents.len() as u64;
        used_names.push(name);
        files.push(HandoffFile {
            path,
            mime_type: source.mime_type.clone(),
        });
    }
    Ok((files, skipped))
}

/// Creates `directory` if needed and deletes the files in it (not folders).
///
/// Side effects: Deletes files. Outputs: Success or the I/O error.
pub fn empty_folder(directory: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(directory)?;
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

/// Keeps a file name's last path component, without characters other apps refuse.
fn handoff_file_stem(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = last
        .chars()
        .map(|character| match character {
            ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            character if character.is_control() => '-',
            character => character,
        })
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.');
    if cleaned.is_empty() {
        String::from("attachment")
    } else {
        cleaned.to_owned()
    }
}

/// Returns `name`, or `2 name`, `3 name`… when it is taken.
fn unique_name(name: &str, used: &[String]) -> String {
    (1..)
        .map(|number| {
            if number == 1 {
                name.to_owned()
            } else {
                format!("{number} {name}")
            }
        })
        .find(|candidate| !used.contains(candidate))
        .unwrap_or_else(|| name.to_owned())
}

/// Reads (and unseals, #99) an image file the model can take, skipping one too large or
/// unreadable.
pub(crate) fn read_image<Repository: AttachmentRepository + ?Sized>(
    repository: &Repository,
    path: &Path,
    mime_type: &str,
) -> Option<LlmImage> {
    let contents = repository.read_attachment_file(path, MAX_LLM_IMAGE_BYTES as u64)?;
    LlmImage::new(mime_type, contents.to_vec())
}

/// Reads the text of a meeting's files that have some (#128), oldest first.
///
/// Inputs: The repository and the meeting's attachments as exported. Outputs: Up to
/// `MAX_LLM_DOCUMENTS` files' text; pictures, unreadable files and files over
/// `MAX_LLM_DOCUMENT_BYTES` are left out. Side effects: Reads and decrypts the files.
pub(crate) fn read_documents<Repository: AttachmentRepository + ?Sized>(
    repository: &Repository,
    attachments: &[ExportAttachment],
) -> Vec<AttachedDocument> {
    let mut readable: Vec<_> = attachments
        .iter()
        .filter(|attachment| !attachment.is_image())
        .collect();
    readable.sort_by_key(|attachment| attachment.original_timestamp());
    readable
        .into_iter()
        .filter_map(|attachment| {
            let stored_name = attachment.display_name();
            let name = stored_name
                .strip_prefix(&format!("{}-", attachment.attachment_id()))
                .unwrap_or(stored_name);
            read_document(
                repository,
                Path::new(attachment.file_path()),
                attachment.mime_type(),
                name,
            )
        })
        .take(MAX_LLM_DOCUMENTS)
        .collect()
}

/// Reads one stored file's text, when its kind has some (#128).
///
/// Inputs: The repository, the stored path, the MIME type and the shown name (for the
/// extension). Outputs: The named text, or `None` for a picture, an unreadable or empty file, or
/// one over `MAX_LLM_DOCUMENT_BYTES`. Side effects: Reads and decrypts the file.
pub(crate) fn read_document<Repository: AttachmentRepository + ?Sized>(
    repository: &Repository,
    stored_path: &Path,
    mime_type: &str,
    name: &str,
) -> Option<AttachedDocument> {
    if !has_text(mime_type, name) {
        return None;
    }
    let contents = repository.read_attachment_file(stored_path, MAX_LLM_DOCUMENT_BYTES)?;
    let text = extract_text(mime_type, name, &contents)?;
    Some(AttachedDocument {
        name: name.to_owned(),
        text,
    })
}

/// Most files whose text goes with a summary or chat (#128).
pub(crate) const MAX_LLM_DOCUMENTS: usize = 20;
/// Largest file read for its text, in bytes.
pub(crate) const MAX_LLM_DOCUMENT_BYTES: u64 = 50 * 1024 * 1024;

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

/// Reads a meeting's summary request from a paired device (#107).
///
/// Outputs: The request, `None` when there is none, or a storage error. Side effects: Reads the
/// document.
pub fn load_summary_request<Repository>(
    repository: &Repository,
    meeting_id: &MeetingId,
) -> Result<Option<SummaryRequest>, SummaryError>
where
    Repository: NoteRepository + ?Sized,
{
    repository
        .load_crdt_document(meeting_id)
        .and_then(|document| read_summary_request(&document))
        .map_err(|error| SummaryError::Storage(error.to_string()))
}

/// Stores a meeting's summary request, or removes it with `None` (answered elsewhere or
/// cancelled).
///
/// Outputs: Success or a storage error. Side effects: Writes the document, which syncs to paired
/// devices.
pub fn store_summary_request<Repository>(
    repository: &Repository,
    meeting_id: &MeetingId,
    request: Option<&SummaryRequest>,
) -> Result<(), SummaryError>
where
    Repository: NoteRepository + ?Sized,
{
    repository
        .edit_crdt_document(meeting_id, &|document| match request {
            Some(request) => with_summary_request(document, request),
            None => without_summary_request(document),
        })
        .map_err(|error| SummaryError::Storage(error.to_string()))
}

/// Stores the summary written for a paired device's request and removes the request, in one
/// write.
///
/// Outputs: Success or a storage error. Side effects: Writes the document, which syncs to paired
/// devices.
pub fn store_answered_request<Repository>(
    repository: &Repository,
    meeting_id: &MeetingId,
    summary: &MeetingSummary,
) -> Result<(), SummaryError>
where
    Repository: NoteRepository + ?Sized,
{
    repository
        .edit_crdt_document(meeting_id, &|document| {
            with_summary(document, summary).and_then(|document| without_summary_request(&document))
        })
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
