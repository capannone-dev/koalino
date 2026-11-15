//! Hands a project, or every note, to the owner's own AI app for a chat (#126 follow-up), as a
//! meeting's hand-off does (#114): the meetings as one Markdown document (notes, summary and
//! transcript of each, oldest first), and the projects' files and the meetings' photos and files
//! copied beside it as files, never turned into text. On a computer a project's message carries
//! the document for pasting into claude.ai or chatgpt.com; every note is too long to paste, so
//! that document always goes as a file. On Android the document is attached. Nothing is sent
//! anywhere by Koalino, and nothing here logs content.

use std::path::Path;

use crate::{
    application::{
        meeting_handoff::{HandoffDelivery, handoff_file_name},
        meeting_summary::{SummaryError, render_transcript},
    },
    domain::projects::ProjectId,
    meeting_summaries::{
        HandoffFile, HandoffSource, WrittenHandoff, copy_handoff_files, empty_folder, load_summary,
        prepare_summary,
    },
    notes_chat_commands::listed_meetings,
    ports::voice::VoiceVectorDB,
    project_commands::ProjectCommandRepository,
};

/// Which notes a chat hand-off carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotesHandoffScope {
    /// One project: its meetings and files.
    Project(ProjectId),
    /// Every meeting and every project's files ("Chat with all your notes").
    Everything,
}

/// Writes a project's, or every note's, chat hand-off into `directory`.
///
/// Inputs: The repository, the voice library (for speakers' names), the scope, how the document
/// travels, and a folder the hand-off owns.
/// Outputs: The message (an introduction for a chat, followed by a project's document when
/// pasted), no prompt, the files (the Markdown first, then the projects' files and each
/// meeting's photos and files), and `document_attached`; or `Storage` when the project is
/// missing or a read fails.
/// Side effects: Empties `directory`, then writes the Markdown and clear copies of the files:
/// personal data the caller shares at once and deletes soon.
pub fn write_notes_handoff<Repository, Voices>(
    repository: &Repository,
    voices: &Voices,
    scope: NotesHandoffScope,
    delivery: HandoffDelivery,
    directory: &Path,
) -> Result<WrittenHandoff, SummaryError>
where
    Repository: ProjectCommandRepository + ?Sized,
    Voices: VoiceVectorDB + ?Sized,
{
    let storage = |error: &dyn std::fmt::Display| SummaryError::Storage(error.to_string());
    let projects = repository
        .list_projects()
        .map_err(|error| storage(&error))?;
    let project_name = |project_id: ProjectId| {
        projects
            .iter()
            .find(|summary| summary.project.project_id() == project_id)
            .map(|summary| summary.project.name().as_str().to_owned())
    };
    // Meetings the list shows: not deleted, not folded into another (#88).
    let listed = listed_meetings(repository).map_err(|error| storage(&error))?;
    let (name, project_ids, meeting_ids) = match scope {
        NotesHandoffScope::Project(project_id) => (
            project_name(project_id)
                .ok_or_else(|| SummaryError::Storage(String::from("there is no such project")))?,
            vec![project_id],
            repository
                .project_meeting_ids(project_id)
                .map_err(|error| storage(&error))?,
        ),
        NotesHandoffScope::Everything => (
            String::from("All notes"),
            projects
                .iter()
                .map(|summary| summary.project.project_id())
                .collect(),
            listed.clone(),
        ),
    };
    let everything = scope == NotesHandoffScope::Everything;
    let meeting_projects = repository
        .meeting_projects()
        .map_err(|error| storage(&error))?;
    empty_folder(directory).map_err(|error| storage(&error))?;
    let document_name = handoff_file_name(&name);

    let mut resources = Vec::new();
    for project_id in project_ids {
        resources.extend(
            repository
                .project_resources(project_id)
                .map_err(|error| storage(&error))?,
        );
    }
    resources.sort_by_key(|resource| resource.original_timestamp());
    let mut sources: Vec<HandoffSource> = resources
        .iter()
        .map(|resource| HandoffSource {
            stored_path: resource.file_path().to_owned(),
            mime_type: resource.mime_type().to_owned(),
            name: resource.shown_file_name(),
        })
        .collect();

    let mut meetings = Vec::new();
    for meeting_id in meeting_ids {
        if !listed.contains(&meeting_id) {
            continue;
        }
        let prepared = prepare_summary(repository, voices, &meeting_id, &[], "", None)?;
        let summary = load_summary(repository, &meeting_id)
            .map_err(|error| storage(&error))?
            .map(|summary| summary.text);
        let mut attachments = repository
            .get_attachments_for_meeting(&meeting_id)
            .map_err(|error| storage(&error))?;
        attachments.sort_by_key(|attachment| attachment.original_timestamp());
        let started = format!(
            "{} {}",
            prepared.material.values.date, prepared.material.values.time
        );
        // Every note's document says which project each meeting belongs to.
        let in_project = meeting_projects
            .iter()
            .find(|(id, _)| everything && *id == meeting_id)
            .and_then(|(_, project_id)| project_name(*project_id))
            .map(|project| format!(", project \"{project}\""))
            .unwrap_or_default();
        meetings.push((started, in_project, prepared, summary, attachments));
    }
    meetings.sort_by(|left, right| left.0.cmp(&right.0));

    let mut document = if everything {
        format!(
            "# {name}\n\nEvery meeting ({}) and the files of {} projects.\n",
            meetings.len(),
            projects.len()
        )
    } else {
        format!("# {name}\n\nProject with {} meetings.\n", meetings.len())
    };
    for (started, in_project, prepared, summary, attachments) in &meetings {
        let material = &prepared.material;
        document.push_str(&format!(
            "\n## Meeting \"{}\", {started}, {}{in_project}\n",
            material.values.title, material.values.duration
        ));
        if let Some(summary) = summary.as_deref().filter(|text| !text.trim().is_empty()) {
            document.push_str(&format!("\n### Summary\n\n{}\n", summary.trim()));
        }
        if let Some(notes) = material
            .notes
            .as_deref()
            .filter(|text| !text.trim().is_empty())
        {
            document.push_str(&format!("\n### The owner's notes\n\n{}\n", notes.trim()));
        }
        let transcript = render_transcript(&material.lines);
        if !transcript.is_empty() {
            document.push_str(&format!(
                "\n### Transcript\n\nAutomatic transcript; words may be misheard.\n\n{transcript}\n"
            ));
        }
        sources.extend(attachments.iter().map(|attachment| HandoffSource {
            stored_path: attachment.file_path().to_owned(),
            mime_type: attachment.mime_type().to_owned(),
            name: attachment.shown_file_name(),
        }));
    }

    let mut used_names = vec![document_name.clone()];
    let (attached, skipped) = copy_handoff_files(repository, &sources, directory, &mut used_names)?;
    if used_names.len() > 1 {
        let whose = if everything {
            "The projects' and meetings'"
        } else {
            "The project's and its meetings'"
        };
        document.push_str(&format!(
            "\n## Attached files\n\n{whose} photos and files come with it as files: {}.\n",
            used_names[1..].join(", ")
        ));
    }
    // Every note is too long to paste into a website, so its document always goes as a file.
    let document_attached = everything || delivery == HandoffDelivery::Attached;
    let message = match (everything, document_attached) {
        (true, _) => format!(
            "The attached file {document_name} has all my notes: every meeting's summary, notes \
             and transcript, and which project it belongs to. Please read it; I will ask you \
             questions about my notes."
        ),
        (false, false) => format!(
            "Below are the meetings of the project \"{name}\": summaries, notes and transcripts. \
             Please read them; I will ask you questions about the project.\n\n{document}"
        ),
        (false, true) => format!(
            "The attached file {document_name} has the meetings of the project \"{name}\": \
             summaries, notes and transcripts. Please read it; I will ask you questions about the \
             project."
        ),
    };
    let document_path = directory.join(&document_name);
    std::fs::write(&document_path, document.as_bytes()).map_err(|error| storage(&error))?;
    let mut files = vec![HandoffFile {
        path: document_path,
        mime_type: String::from("text/markdown"),
    }];
    files.extend(attached);
    Ok(WrittenHandoff {
        message,
        prompt: String::new(),
        files,
        skipped,
        document_attached,
    })
}
