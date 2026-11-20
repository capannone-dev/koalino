//! Asks a language model about the owner's notes (#97): one meeting, one project, or everything.
//!
//! The host gathers the material as `ContextItem`s in priority order (for a project, newest
//! meetings first; for everything, the meetings and files that best match the question first).
//! Each item has its full text and, for a meeting, a short version (its summary and notes).
//! `fit_context` keeps what fits the connection's context: everything when it fits; otherwise
//! short versions, dropping the last items, then full versions again while they fit. The reply
//! says what was included and what was left out. The material goes in the instructions; the
//! chat's earlier turns (the most recent that fit) and the question follow. Nothing here logs
//! questions, answers or material.

use crate::{
    application::search::fold,
    domain::{
        chat::{
            ChatMessage, ChatRole, ChatSource, ChatTarget, CitedMaterial, MAX_CHAT_QUESTION_CHARS,
            parse_citations,
        },
        errors::LlmError,
        llm::{LlmConnection, LlmImage, LlmRequest, LlmRole, LlmTurn},
        shared_prompts::{ChatPrompt, DEFAULT_CHAT_PROMPT},
    },
    ports::ai::LlmEngine,
};

/// Characters per token assumed when sizing requests, as for summaries.
const CHARACTERS_PER_TOKEN: usize = 3;
/// Tokens kept for the answer, at most.
const MAX_ANSWER_TOKENS: u32 = 2_048;
/// Tokens assumed for one image.
const IMAGE_TOKENS: usize = 1_600;
/// Tokens kept for the instructions' own wording.
const FRAMING_TOKENS: usize = 400;
/// How the model must cite the material; the app turns `[n]` into links, so the owner's chat
/// prompt (#145) does not replace it.
const CITATION_RULE: &str = "Each piece of material has a number; cite where each statement \
     comes from right after it, as [n], or as [n, hh:mm:ss] for a moment in a transcript.";

/// One piece of material the model may be given.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContextItem {
    /// What it is, such as `Meeting "Budget review", 2026-11-09 10:00 UTC` or `File plan.md`.
    pub label: String,
    /// Its whole text.
    pub full: String,
    /// A shorter version (a meeting's summary and notes), used when the whole does not fit.
    pub short: Option<String>,
    /// What a citation of it opens (#110); `None` when there is nothing to open.
    pub target: Option<ChatTarget>,
}

/// How much of an item the model was given.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Inclusion {
    /// All of it.
    Full,
    /// Its short version.
    Short,
    /// The beginning of it: a single item larger than the whole context.
    Cut,
    /// Nothing.
    Omitted,
}

/// What a chat is about, as gathered by the host.
#[derive(Clone, Debug, Default)]
pub struct ChatMaterial {
    /// What the material covers, such as `the meeting "Budget review"`.
    pub description: String,
    /// The material, most important first.
    pub items: Vec<ContextItem>,
    /// Photos and slides (one meeting's), already checked.
    pub images: Vec<LlmImage>,
}

/// The model's answer and what it was given.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatReply {
    pub text: String,
    /// Items given, with `(summary only)` or `(beginning only)` when not whole.
    pub included: Vec<String>,
    /// Items left out for length.
    pub omitted: Vec<String>,
    /// The answer stopped at the length limit.
    pub truncated: bool,
    /// Images the model was sent.
    pub images_sent: usize,
    /// The places the answer cites, in order of first mention (#110).
    pub sources: Vec<ChatSource>,
}

/// Why there is no answer.
#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    /// The question is blank.
    #[error("type a question")]
    EmptyQuestion,
    /// The question is longer than `MAX_CHAT_QUESTION_CHARS`.
    #[error("the question is longer than {MAX_CHAT_QUESTION_CHARS} characters")]
    QuestionTooLong,
    /// There is no transcript, summary, note or file to talk about.
    #[error("there is nothing to talk about yet: no transcript, summary, notes or files")]
    NothingToChatAbout,
    /// The connection's context is too small for even a short question.
    #[error("this connection's context is too small; choose one with a larger context")]
    ContextTooSmall,
    /// The model gave no answer.
    #[error(transparent)]
    Llm(#[from] LlmError),
}

/// Chooses how much of each item fits in `budget_chars`.
///
/// Inputs: The items, most important first, and the room in characters. Outputs: One inclusion
/// per item: all full when they fit; otherwise short versions (full where there is none) in
/// order while they fit, then full versions again in order while they fit. When nothing fits,
/// the first item is cut. Side effects: None.
pub fn fit_context(items: &[ContextItem], budget_chars: usize) -> Vec<Inclusion> {
    let size = |text: &str| text.chars().count() + 64;
    let full_sizes: Vec<usize> = items.iter().map(|item| size(&item.full)).collect();
    if full_sizes.iter().sum::<usize>() <= budget_chars {
        return vec![Inclusion::Full; items.len()];
    }
    let mut chosen = vec![Inclusion::Omitted; items.len()];
    let mut used = 0;
    for (index, item) in items.iter().enumerate() {
        let (inclusion, cost) = match &item.short {
            Some(short) => (Inclusion::Short, size(short)),
            None => (Inclusion::Full, full_sizes[index]),
        };
        if used + cost <= budget_chars {
            chosen[index] = inclusion;
            used += cost;
        }
    }
    for (index, item) in items.iter().enumerate() {
        if chosen[index] != Inclusion::Short {
            continue;
        }
        let short = item.short.as_deref().map_or(0, size);
        let extra = full_sizes[index].saturating_sub(short);
        if used + extra <= budget_chars {
            chosen[index] = Inclusion::Full;
            used += extra;
        }
    }
    if !items.is_empty()
        && chosen
            .iter()
            .all(|inclusion| *inclusion == Inclusion::Omitted)
    {
        chosen[0] = Inclusion::Cut;
    }
    chosen
}

/// Scores how well a text matches a question: the number of the question's words (of four
/// letters or more, case and accents ignored) found in it, each counted once.
///
/// Side effects: None.
pub fn relevance(question: &str, text: &str) -> usize {
    let haystack = fold(text);
    let mut words: Vec<String> = fold(question)
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| word.chars().count() >= 4)
        .map(str::to_owned)
        .collect();
    words.sort();
    words.dedup();
    words
        .iter()
        .filter(|word| haystack.contains(word.as_str()))
        .count()
}

/// The text given for an item at an inclusion, cut to `cut_chars` when cut.
fn text_for(item: &ContextItem, inclusion: Inclusion, cut_chars: usize) -> Option<String> {
    match inclusion {
        Inclusion::Full => Some(item.full.clone()),
        Inclusion::Short => item.short.clone(),
        Inclusion::Cut => Some(item.full.chars().take(cut_chars).collect()),
        Inclusion::Omitted => None,
    }
}

/// The label shown for an included item.
fn included_label(item: &ContextItem, inclusion: Inclusion) -> String {
    match inclusion {
        Inclusion::Short => format!("{} (summary only)", item.label),
        Inclusion::Cut => format!("{} (beginning only)", item.label),
        _ => item.label.clone(),
    }
}

/// The most recent turns of the history that fit `budget_chars`, starting with a question.
fn recent_turns(history: &[ChatMessage], budget_chars: usize) -> Vec<LlmTurn> {
    let mut kept: Vec<&ChatMessage> = Vec::new();
    let mut used = 0;
    for message in history.iter().rev() {
        let cost = message.text.chars().count();
        if used + cost > budget_chars {
            break;
        }
        used += cost;
        kept.push(message);
    }
    kept.reverse();
    // The conversation must start with the owner's turn and alternate.
    while kept
        .first()
        .is_some_and(|message| message.role != ChatRole::User)
    {
        kept.remove(0);
    }
    let mut turns: Vec<LlmTurn> = Vec::new();
    for message in kept {
        let role = match message.role {
            ChatRole::User => LlmRole::User,
            ChatRole::Assistant => LlmRole::Assistant,
        };
        match turns.last_mut() {
            Some(last) if last.role == role => {
                last.text.push_str("\n\n");
                last.text.push_str(&message.text);
            }
            _ => turns.push(LlmTurn {
                role,
                text: message.text.clone(),
            }),
        }
    }
    // A question left unanswered (a failed request) is replaced by the new one.
    if turns.last().is_some_and(|turn| turn.role == LlmRole::User) {
        turns.pop();
    }
    turns
}

/// Asks the model a question about the material, framed by the built-in chat prompt.
///
/// Inputs, outputs and side effects: As `ask_with_prompt`.
pub fn ask(
    engine: &dyn LlmEngine,
    connection: &LlmConnection,
    material: ChatMaterial,
    history: &[ChatMessage],
    question: &str,
) -> Result<ChatReply, ChatError> {
    ask_with_prompt(
        engine,
        connection,
        &ChatPrompt::default(),
        material,
        history,
        question,
    )
}

/// Asks the model a question about the material, framed by the owner's chat prompt (#145).
///
/// Inputs: The engine for `connection`, the chat prompt (filled in with what is asked about;
/// the rule to cite the material by number always follows it), the material, the chat so far,
/// and the question.
/// Outputs: The answer with what was included, or why there is none. Side effects: Sends the
/// question, the recent turns and the material that fits to the connection's server; a server
/// that cannot read images is asked once more without them. Blocks: run it on a background
/// thread.
pub fn ask_with_prompt(
    engine: &dyn LlmEngine,
    connection: &LlmConnection,
    prompt: &ChatPrompt,
    material: ChatMaterial,
    history: &[ChatMessage],
    question: &str,
) -> Result<ChatReply, ChatError> {
    let question = question.trim();
    if question.is_empty() {
        return Err(ChatError::EmptyQuestion);
    }
    if question.chars().count() > MAX_CHAT_QUESTION_CHARS {
        return Err(ChatError::QuestionTooLong);
    }
    if material
        .items
        .iter()
        .all(|item| item.full.trim().is_empty())
        && material.images.is_empty()
    {
        return Err(ChatError::NothingToChatAbout);
    }
    let context_tokens = connection.context_tokens() as usize;
    let answer_tokens = MAX_ANSWER_TOKENS
        .min(connection.context_tokens() / 4)
        .max(256);
    let mut images = material.images;
    if !engine.reads_images() {
        // Claude Code runs without tools and cannot read images (#105).
        images.clear();
    }
    let image_tokens = images.len() * IMAGE_TOKENS;
    let mut room_tokens = context_tokens
        .saturating_sub(answer_tokens as usize + FRAMING_TOKENS)
        .saturating_sub(question.chars().count() / CHARACTERS_PER_TOKEN)
        // The owner's chat prompt beyond what the framing allowance covers (#145).
        .saturating_sub(
            prompt
                .as_str()
                .chars()
                .count()
                .saturating_sub(DEFAULT_CHAT_PROMPT.chars().count())
                / CHARACTERS_PER_TOKEN,
        );
    if image_tokens > room_tokens / 2 {
        // Images never take more than half the room.
        let keep = (room_tokens / 2) / IMAGE_TOKENS;
        images.truncate(keep);
    }
    room_tokens = room_tokens.saturating_sub(images.len() * IMAGE_TOKENS);
    let room_chars = room_tokens * CHARACTERS_PER_TOKEN;
    if room_chars < 400 {
        return Err(ChatError::ContextTooSmall);
    }
    let mut turns = recent_turns(history, room_chars / 4);
    let history_chars: usize = turns.iter().map(|turn| turn.text.chars().count()).sum();
    let material_chars = room_chars.saturating_sub(history_chars);

    let inclusions = fit_context(&material.items, material_chars);
    let mut included = Vec::new();
    let mut omitted = Vec::new();
    let mut sections = String::new();
    // What each number in the material is, `[1]` first.
    let mut numbered: Vec<CitedMaterial> = Vec::new();
    for (item, inclusion) in material.items.iter().zip(&inclusions) {
        match text_for(item, *inclusion, material_chars.saturating_sub(64)) {
            Some(text) if !text.trim().is_empty() => {
                numbered.push(CitedMaterial {
                    label: item.label.clone(),
                    target: item.target.clone(),
                });
                sections.push_str(&format!(
                    "\n\n## [{}] {}\n\n{}",
                    numbered.len(),
                    item.label,
                    text.trim()
                ));
                included.push(included_label(item, *inclusion));
            }
            Some(_) => {}
            None => omitted.push(item.label.clone()),
        }
    }
    // The owner's framing, then the citation rule the app relies on to link sources.
    let mut system = format!(
        "{}\n\n{CITATION_RULE}",
        prompt.render(&material.description).trim()
    );
    if !omitted.is_empty() {
        system.push_str(&format!(
            "\n\nLeft out for length (say so if the answer may be there): {}.",
            omitted.join("; ")
        ));
    }
    system.push_str("\n\n# Material");
    system.push_str(&sections);
    turns.push(LlmTurn {
        role: LlmRole::User,
        text: question.to_owned(),
    });
    let mut request = LlmRequest {
        system,
        turns,
        images,
        max_output_tokens: answer_tokens,
    };
    let completion = match engine.complete(&request) {
        Err(LlmError::Rejected {
            status: 400 | 415 | 422,
            ..
        }) if !request.images.is_empty() => {
            request.images.clear();
            engine.complete(&request)?
        }
        other => other?,
    };
    let sources = parse_citations(&completion.text, &numbered);
    Ok(ChatReply {
        sources,
        text: completion.text,
        included,
        omitted,
        truncated: completion.truncated,
        images_sent: request.images.len(),
    })
}
