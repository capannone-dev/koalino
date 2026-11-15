//! Hands a meeting to the owner's own AI app, such as Claude or ChatGPT, and keeps the answer
//! (#114).
//!
//! The owner uses their own account in their own app; Koalino never holds its login. The
//! platform's share sheet carries a message and the meeting as a Markdown file (plus its photos
//! and files, which the caller adds). For a summary, the message is the owner's summary prompt;
//! the answer comes back through the clipboard or a share and is kept as the meeting's summary,
//! with Undo. For a chat, the conversation stays in the other app.

use std::cell::RefCell;

use chrono::{DateTime, Utc};

use super::{
    meeting_edits::{EditError, MAX_NOTES_CHARS, check_length, require_meeting},
    meeting_summary::{SummaryError, SummaryMaterial, render_transcript},
    meeting_summary_document::{read_summary, with_summary, without_summary},
};
use crate::{
    domain::{
        llm::{MeetingSummary, PromptTemplate},
        models::MeetingId,
    },
    ports::storage::NoteRepository,
};

/// What the other app is asked to do.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandoffPurpose {
    /// Write the summary, with the owner's summary prompt.
    Summary,
    /// Read the meeting and answer the owner's questions there.
    Chat,
}

/// How the meeting reaches the other app.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandoffDelivery {
    /// As a Markdown file the share attaches; the message names it (Android, #114).
    Attached,
    /// Inside the message, which the owner pastes into the website (computers, #126).
    Pasted,
}

/// What is handed to the other app.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Handoff {
    /// The text the share carries: the prompt for a summary, an introduction for a chat; when
    /// pasted, followed by the document.
    pub message: String,
    /// The attached file's name, such as `Budget review.md`.
    pub file_name: String,
    /// The attached file: the title, when, the owner's notes and the transcript, in Markdown.
    pub document: String,
    /// For a summary, the instructions as filled in, kept with the answer; empty for a chat.
    pub prompt: String,
}

/// Builds the hand-off of a meeting.
///
/// Inputs: The summary template (the meeting's own or the default), the meeting's material, the
/// purpose, and how the meeting travels. Images in the material are ignored; the caller attaches
/// the files itself.
/// Outputs: The message and document, or `NothingToSummarize` without words, notes or files'
/// text. When pasted, the document ends with the text of the meeting's files (#128).
/// Side effects: None.
pub fn prepare_handoff(
    template: &PromptTemplate,
    material: &SummaryMaterial,
    purpose: HandoffPurpose,
    delivery: HandoffDelivery,
) -> Result<Handoff, SummaryError> {
    let notes = material
        .notes
        .as_deref()
        .map(str::trim)
        .filter(|notes| !notes.is_empty());
    let transcript = render_transcript(&material.lines);
    let documents: Vec<_> = material
        .documents
        .iter()
        .filter(|document| !document.text.trim().is_empty())
        .collect();
    if transcript.is_empty() && notes.is_none() && documents.is_empty() {
        return Err(SummaryError::NothingToSummarize);
    }
    let values = &material.values;
    let file_name = handoff_file_name(&values.title);
    let mut document = format!(
        "# {}\n\n{} {}, {}\n",
        values.title, values.date, values.time, values.duration
    );
    if let Some(notes) = notes {
        document.push_str(&format!("\n## The owner's notes\n\n{notes}\n"));
    }
    if !transcript.is_empty() {
        document.push_str(&format!(
            "\n## Transcript\n\nAutomatic transcript, with each speaker and the time from the start. \
             Words may be misheard, especially names, technical terms and people talking at once.\n\n{transcript}\n"
        ));
    }
    // A pasted hand-off carries no files, so their text comes along (#128); an attached one
    // shares the files themselves.
    if delivery == HandoffDelivery::Pasted && !documents.is_empty() {
        document.push_str("\n## Attached files\n");
        for attached in documents {
            document.push_str(&format!(
                "\n### {}\n\n{}\n",
                attached.name,
                attached.text.trim()
            ));
        }
    }
    let (message, prompt) = match (purpose, delivery) {
        (HandoffPurpose::Summary, HandoffDelivery::Pasted) => {
            let prompt = template.render(values);
            (format!("{prompt}\n\n{document}"), prompt)
        }
        (HandoffPurpose::Chat, HandoffDelivery::Pasted) => (
            format!(
                "Below are the notes and transcript of the meeting \"{}\" ({}). \
                 Please read them; I will ask you questions about the meeting.\n\n{document}",
                values.title, values.date
            ),
            String::new(),
        ),
        (HandoffPurpose::Summary, HandoffDelivery::Attached) => {
            let prompt = template.render(values);
            (
                format!(
                    "{prompt}\n\nThe meeting's notes and transcript are in the attached file {file_name}."
                ),
                prompt,
            )
        }
        (HandoffPurpose::Chat, HandoffDelivery::Attached) => (
            format!(
                "The attached file {file_name} has the notes and transcript of the meeting \"{}\" ({}). \
                 Please read it; I will ask you questions about it.",
                values.title, values.date
            ),
            String::new(),
        ),
    };
    Ok(Handoff {
        message,
        file_name,
        document,
        prompt,
    })
}

/// Names the attached file after the meeting's title.
///
/// Inputs: The title. Outputs: The title with path and reserved characters replaced by `-`, at
/// most 80 characters, plus `.md`; `Meeting.md` for a blank title. Side effects: None.
pub fn handoff_file_name(title: &str) -> String {
    let cleaned: String = title
        .trim()
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            character if character.is_control() => '-',
            character => character,
        })
        .take(80)
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    if cleaned.is_empty() {
        String::from("Meeting.md")
    } else {
        format!("{cleaned}.md")
    }
}

/// An answer kept as the summary, and the one it replaced, for Undo.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeptAnswer {
    /// The summary as stored.
    pub summary: MeetingSummary,
    /// The summary before, or `None` when there was none.
    pub previous: Option<MeetingSummary>,
}

/// Keeps an answer from the owner's own app as the meeting's summary.
///
/// Inputs: The meeting; the answer's Markdown (not blank, at most `MAX_NOTES_CHARS`); where it
/// came from, such as `Claude app`; the prompt it answered (empty when unknown); the time.
/// Outputs: The summary and the one it replaced. It is marked `edited`, so automatic summaries
/// leave it alone. Side effects: Writes the meeting document, which syncs to paired devices.
pub fn store_external_summary<Repository>(
    repository: &Repository,
    meeting_id: &MeetingId,
    text: &str,
    source_label: &str,
    prompt: &str,
    now: DateTime<Utc>,
) -> Result<KeptAnswer, EditError>
where
    Repository: NoteRepository + ?Sized,
{
    let text = text.trim();
    if text.is_empty() {
        return Err(EditError::Empty);
    }
    check_length(text, MAX_NOTES_CHARS)?;
    require_meeting(repository, meeting_id)?;
    let summary = MeetingSummary {
        text: text.to_owned(),
        connection_label: source_label.to_owned(),
        model: String::new(),
        // The document keeps milliseconds; the answer matches what is stored.
        created_at: DateTime::from_timestamp_millis(now.timestamp_millis()).unwrap_or(now),
        prompt: prompt.to_owned(),
        edited: true,
        truncated: false,
    };
    let previous = RefCell::new(None);
    repository.edit_crdt_document(meeting_id, &|document| {
        previous.replace(read_summary(document)?);
        with_summary(document, &summary)
    })?;
    Ok(KeptAnswer {
        summary,
        previous: previous.into_inner(),
    })
}

/// Puts back the summary an answer replaced (Undo).
///
/// Inputs: The meeting and the earlier summary, or `None` to remove the summary.
/// Outputs: Success or why it failed. Side effects: Writes the meeting document, which syncs.
pub fn restore_summary<Repository>(
    repository: &Repository,
    meeting_id: &MeetingId,
    previous: Option<&MeetingSummary>,
) -> Result<(), EditError>
where
    Repository: NoteRepository + ?Sized,
{
    require_meeting(repository, meeting_id)?;
    repository.edit_crdt_document(meeting_id, &|document| match previous {
        Some(summary) => with_summary(document, summary),
        None => without_summary(document),
    })?;
    Ok(())
}
