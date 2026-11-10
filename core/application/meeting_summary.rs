//! Asks a language model for a meeting's summary (#95).
//!
//! The rendered template is the instructions; the owner's notes and the speaker-attributed
//! transcript go in the user message, with the meeting's photos and slides attached. Nothing is
//! cut to fit: when the transcript is longer than the connection's context, it is split into
//! parts, the model takes notes on each part, and the summary is written from those notes (up to
//! three rounds). A server that cannot read images is asked once more without them.

use chrono::{DateTime, Utc};

use crate::{
    domain::{
        errors::LlmError,
        llm::{
            LlmConnection, LlmImage, LlmRequest, LlmRole, LlmTurn, MeetingSummary, PromptTemplate,
            PromptValues,
        },
    },
    ports::ai::LlmEngine,
};

/// Characters per token assumed when sizing requests; low, so Italian and other languages fit.
const CHARACTERS_PER_TOKEN: usize = 3;
/// Tokens kept for the answer, at most.
const MAX_ANSWER_TOKENS: u32 = 4_096;
/// Tokens assumed for one image (Anthropic's for a 1568-pixel image).
const IMAGE_TOKENS: usize = 1_600;
/// Tokens kept for the request's own framing.
const FRAMING_TOKENS: usize = 256;
/// Rounds of note-taking allowed before giving up on a very long meeting.
const MAX_CONDENSE_ROUNDS: usize = 3;

/// One speaker's line of the transcript.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpokenLine {
    /// The speaker's name, or a label such as `Unknown speaker`.
    pub speaker: String,
    /// When it starts, in meeting milliseconds.
    pub start_ms: u64,
    /// What was said.
    pub text: String,
}

/// Everything a summary is written from.
#[derive(Clone, Debug, Default)]
pub struct SummaryMaterial {
    /// The values the template is filled with.
    pub values: PromptValues,
    /// The owner's notes, if any.
    pub notes: Option<String>,
    /// The transcript, in time order.
    pub lines: Vec<SpokenLine>,
    /// Photos and slides, already checked by `LlmImage::new`.
    pub images: Vec<LlmImage>,
}

/// A summary and how it was obtained.
#[derive(Clone, Debug)]
pub struct SummaryOutcome {
    /// The summary to store.
    pub summary: MeetingSummary,
    /// Requests made on parts of the transcript; 1 when it fitted in one request.
    pub parts: usize,
    /// Images the model was sent.
    pub images_sent: usize,
    /// Images left out: the server could not read them, or they did not fit.
    pub images_skipped: usize,
}

/// Why no summary was written.
#[derive(Debug, thiserror::Error)]
pub enum SummaryError {
    /// The meeting has neither a transcript nor notes.
    #[error("the meeting has no transcript or notes to summarize yet")]
    NothingToSummarize,
    /// The meeting is too long for this connection's context, even in parts.
    #[error("the meeting is too long for this connection; choose one with a larger context")]
    TooLong,
    /// The model gave no answer.
    #[error(transparent)]
    Llm(#[from] LlmError),
    /// The meeting could not be read or the summary could not be stored.
    #[error("the summary could not be stored: {0}")]
    Storage(String),
}

/// Writes the transcript as `[hh:mm:ss] Speaker: text` lines, one per change of speaker.
///
/// Inputs: Lines in time order. Outputs: The text. Side effects: None.
pub fn render_transcript(lines: &[SpokenLine]) -> String {
    merged_lines(lines).join("\n")
}

/// Joins a speaker's consecutive lines and formats each turn.
fn merged_lines(lines: &[SpokenLine]) -> Vec<String> {
    let mut turns: Vec<(String, u64, String)> = Vec::new();
    for line in lines.iter().filter(|line| !line.text.trim().is_empty()) {
        match turns.last_mut() {
            Some((speaker, _, text)) if *speaker == line.speaker => {
                text.push(' ');
                text.push_str(line.text.trim());
            }
            _ => turns.push((
                line.speaker.clone(),
                line.start_ms,
                line.text.trim().to_owned(),
            )),
        }
    }
    turns
        .into_iter()
        .map(|(speaker, start_ms, text)| {
            let seconds = start_ms / 1_000;
            format!(
                "[{:02}:{:02}:{:02}] {speaker}: {text}",
                seconds / 3_600,
                seconds / 60 % 60,
                seconds % 60
            )
        })
        .collect()
}

/// Asks the model for the meeting's summary.
///
/// Inputs: The engine for `connection`, the template, the material, and the time to record.
/// Outputs: The summary and how it was obtained; `NothingToSummarize` without transcript or
/// notes; `TooLong` when even condensed notes do not fit; or the model's error.
/// Side effects: Sends the meeting's text and images to the connection's server, one request
/// per part plus one for the summary. Blocks: run it on a background thread.
pub fn summarize_meeting(
    engine: &dyn LlmEngine,
    connection: &LlmConnection,
    template: &PromptTemplate,
    material: SummaryMaterial,
    now: DateTime<Utc>,
) -> Result<SummaryOutcome, SummaryError> {
    let notes = material
        .notes
        .as_deref()
        .map(str::trim)
        .filter(|notes| !notes.is_empty());
    let mut pieces = merged_lines(&material.lines);
    if pieces.is_empty() && notes.is_none() {
        return Err(SummaryError::NothingToSummarize);
    }
    let instructions = template.render(&material.values);
    let budget = Budget::for_connection(connection);
    let notes_section = notes.map_or_else(String::new, |notes| {
        format!("The owner's notes:\n{notes}\n\n")
    });

    let mut parts = 0;
    let mut condensed = false;
    for _round in 0..=MAX_CONDENSE_ROUNDS {
        let body = pieces.join("\n");
        let fixed = instructions.len() + notes_section.len() + 120;
        if fixed + body.len() <= budget.text_characters(0) {
            let heading = if condensed {
                format!("Notes taken on the transcript in {parts} parts, in order:\n\n")
            } else {
                String::from("Transcript, with each speaker and the time from the start:\n")
            };
            let user = format!("{notes_section}{heading}{body}");
            // An engine that cannot read images (Claude Code without tools, #105) gets none.
            let room_for_images = if engine.reads_images() {
                budget.images_that_fit(fixed + body.len(), material.images.len())
            } else {
                0
            };
            let mut images = material.images;
            let mut images_skipped = images.len().saturating_sub(room_for_images);
            images.truncate(room_for_images);
            let mut request = LlmRequest {
                system: instructions.clone(),
                turns: vec![LlmTurn {
                    role: LlmRole::User,
                    text: user,
                }],
                images,
                max_output_tokens: budget.answer_tokens,
            };
            let completion = match engine.complete(&request) {
                Err(LlmError::Rejected {
                    status: 400 | 415 | 422,
                    ..
                }) if !request.images.is_empty() => {
                    images_skipped += request.images.len();
                    request.images.clear();
                    engine.complete(&request)?
                }
                other => other?,
            };
            return Ok(SummaryOutcome {
                parts: parts.max(1),
                images_sent: request.images.len(),
                images_skipped,
                summary: MeetingSummary {
                    text: completion.text,
                    connection_label: connection.label().to_owned(),
                    model: connection.model().to_owned(),
                    created_at: now,
                    prompt: instructions,
                    edited: false,
                    truncated: completion.truncated,
                },
            });
        }
        if pieces.is_empty() {
            // The owner's notes alone are too long.
            return Err(SummaryError::TooLong);
        }
        let part_instructions_length = part_instructions(&material.values.title, 1, 1).len() + 40;
        let chunk_characters = budget
            .text_characters(0)
            .saturating_sub(part_instructions_length);
        let chunks = pack(&pieces, chunk_characters);
        let total = chunks.len();
        let mut notes_taken = Vec::with_capacity(total);
        for (index, chunk) in chunks.into_iter().enumerate() {
            let completion = engine.complete(&LlmRequest {
                system: part_instructions(&material.values.title, index + 1, total),
                turns: vec![LlmTurn {
                    role: LlmRole::User,
                    text: chunk,
                }],
                images: Vec::new(),
                max_output_tokens: budget.answer_tokens,
            })?;
            parts += 1;
            notes_taken.push(format!("Part {}:\n{}", index + 1, completion.text.trim()));
        }
        pieces = notes_taken;
        condensed = true;
    }
    Err(SummaryError::TooLong)
}

/// The instructions for taking notes on one part of a long transcript.
fn part_instructions(title: &str, part: usize, total: usize) -> String {
    format!(
        "You take notes on part {part} of {total} of the transcript of the meeting \"{title}\". \
Write concise notes in the transcript's language: the topics, decisions, action items with \
their owners and dates, figures and open questions. Keep names and numbers exactly as written. \
Write only the notes."
    )
}

/// Packs pieces into chunks of at most `limit` characters, splitting a piece only when it is
/// longer than the limit by itself, at a character boundary.
fn pack(pieces: &[String], limit: usize) -> Vec<String> {
    let limit = limit.max(200);
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    for piece in pieces {
        let mut rest = piece.as_str();
        while !rest.is_empty() {
            let room = if current.is_empty() {
                limit
            } else {
                limit.saturating_sub(current.len() + 1)
            };
            if rest.len() <= room {
                if !current.is_empty() {
                    current.push('\n');
                }
                current.push_str(rest);
                rest = "";
            } else if current.is_empty() {
                let mut cut = room;
                while !rest.is_char_boundary(cut) {
                    cut -= 1;
                }
                chunks.push(rest[..cut].to_owned());
                rest = &rest[cut..];
            } else {
                chunks.push(std::mem::take(&mut current));
            }
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// How much one request to a connection may hold.
struct Budget {
    /// The connection's context, in tokens.
    context_tokens: usize,
    /// Tokens kept for the answer.
    answer_tokens: u32,
}

impl Budget {
    fn for_connection(connection: &LlmConnection) -> Self {
        Self {
            context_tokens: connection.context_tokens() as usize,
            answer_tokens: MAX_ANSWER_TOKENS.min(connection.context_tokens() / 4),
        }
    }

    /// Characters of text that fit beside `images` images.
    fn text_characters(&self, images: usize) -> usize {
        self.context_tokens
            .saturating_sub(self.answer_tokens as usize + FRAMING_TOKENS + images * IMAGE_TOKENS)
            * CHARACTERS_PER_TOKEN
    }

    /// How many of `wanted` images fit beside `text_characters` characters of text.
    fn images_that_fit(&self, text_characters: usize, wanted: usize) -> usize {
        (0..=wanted)
            .rev()
            .find(|images| text_characters <= self.text_characters(*images))
            .unwrap_or(0)
    }
}
