//! The owner's prompts that every paired device shares (#145): the summary prompt and the chat
//! prompt.
//!
//! Each prompt is a `PromptVersion`: the owner's text (`None` for the built-in prompt) and when
//! it was last changed, in milliseconds since the Unix epoch. Devices exchange both and keep the
//! newest version of each (`SharedPrompts::merge`); going back to the built-in prompt is a change
//! too, so it reaches the other devices. Equal times are settled by the text, so every device
//! ends with the same prompts whatever order it hears of them in.
//!
//! The chat prompt is the first paragraph of the instructions a notes chat sends (#97), with
//! `{{scope}}` for what is asked about. The rule to cite the material as `[n]` follows it and is
//! not the owner's to change, since the app links answers to their sources by those numbers.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use super::llm::{MAX_TEMPLATE_CHARACTERS, PromptTemplateError};

/// The placeholder for what a chat asks about: a meeting, a project, or all the notes.
pub const CHAT_SCOPE_PLACEHOLDER: &str = "scope";

/// The built-in chat prompt.
pub const DEFAULT_CHAT_PROMPT: &str = "You answer the owner's questions about {{scope}}, using \
     only the material below from their note-taking app: transcripts ([hh:mm:ss] Speaker: text), \
     summaries, notes and files. When the material does not hold the answer, say so. Answer in \
     the language of the question, in Markdown.";

/// A checked chat prompt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatPrompt(String);

impl Default for ChatPrompt {
    fn default() -> Self {
        Self(DEFAULT_CHAT_PROMPT.to_owned())
    }
}

impl ChatPrompt {
    /// Checks a chat prompt.
    ///
    /// Inputs: The text. Outputs: The prompt, or why it was refused: empty, longer than
    /// `MAX_TEMPLATE_CHARACTERS`, an unclosed `{{`, or a placeholder other than `{{scope}}`.
    /// Side effects: None.
    pub fn parse(text: &str) -> Result<Self, PromptTemplateError> {
        if text.trim().is_empty() {
            return Err(PromptTemplateError::Empty);
        }
        if text.chars().count() > MAX_TEMPLATE_CHARACTERS {
            return Err(PromptTemplateError::TooLong);
        }
        let mut rest = text;
        while let Some(start) = rest.find("{{") {
            let after = &rest[start + 2..];
            let end = after.find("}}").ok_or(PromptTemplateError::Unclosed)?;
            let name = after[..end].trim();
            if name != CHAT_SCOPE_PLACEHOLDER {
                return Err(PromptTemplateError::UnknownPlaceholder(name.to_owned()));
            }
            rest = &after[end + 2..];
        }
        Ok(Self(text.to_owned()))
    }

    /// The prompt's text, placeholders unfilled.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Fills `{{scope}}` with what is asked about.
    ///
    /// Inputs: The scope, such as "the meeting \"Budget\"". Outputs: The paragraph to send.
    /// Side effects: None.
    pub fn render(&self, scope: &str) -> String {
        let mut text = String::with_capacity(self.0.len() + scope.len());
        let mut rest = self.0.as_str();
        while let Some(start) = rest.find("{{") {
            text.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            match after.find("}}") {
                Some(end) => {
                    text.push_str(scope);
                    rest = &after[end + 2..];
                }
                None => {
                    rest = &rest[start..];
                    break;
                }
            }
        }
        text.push_str(rest);
        text
    }
}

/// One prompt as a device knows it.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PromptVersion {
    /// The owner's text; `None` for the built-in prompt.
    pub text: Option<String>,
    /// When it was last changed, in milliseconds since the Unix epoch; 0 when never.
    pub changed_at_ms: u64,
}

impl PromptVersion {
    /// Orders two versions: the later change first, then (for equal times) the larger text, the
    /// built-in prompt being the smallest. Side effects: None.
    fn order(&self, other: &Self) -> Ordering {
        self.changed_at_ms
            .cmp(&other.changed_at_ms)
            .then_with(|| self.text.cmp(&other.text))
    }

    /// Replaces this version with `incoming` when that one is newer.
    ///
    /// Outputs: Whether it changed. Side effects: None.
    fn merge(&mut self, incoming: &Self) -> bool {
        if incoming.order(self) == Ordering::Greater {
            *self = incoming.clone();
            true
        } else {
            false
        }
    }
}

/// The prompts paired devices share.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SharedPrompts {
    /// The summary prompt template (see `PromptTemplate`).
    pub summary: PromptVersion,
    /// The chat prompt (see `ChatPrompt`).
    pub chat: PromptVersion,
}

impl SharedPrompts {
    /// Keeps the newer version of each prompt.
    ///
    /// Inputs: Another device's prompts. Outputs: Whether any prompt changed here.
    /// Side effects: None.
    pub fn merge(&mut self, incoming: &SharedPrompts) -> bool {
        let summary = self.summary.merge(&incoming.summary);
        let chat = self.chat.merge(&incoming.chat);
        summary || chat
    }
}
