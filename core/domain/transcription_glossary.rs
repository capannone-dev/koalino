//! Defines the glossary of names and terms that steers transcription: people's names, project
//! names, acronyms and jargon that Whisper would otherwise misspell.
//!
//! Whisper takes an optional prompt that it reads as text spoken just before the audio, so words in
//! it are spelled as written. The glossary becomes that prompt as a plain list, `Marco Rossi,
//! Kubernetes, OKR.`, with no words of any language around it, so it does not push language
//! detection towards English. Whisper's prompt holds about 220 tokens, so the list is bounded by
//! `MAX_PROMPT_CHARACTERS`. A known failure of prompting is that Whisper writes the prompt back
//! over quiet or unclear audio; `leading_echo_words` and `is_echo` recognise that text so it can be
//! dropped. The glossary is user content: it is never logged.

/// Most characters in one term.
pub const MAX_TERM_CHARACTERS: usize = 60;
/// Most characters in the prompt the terms make.
pub const MAX_PROMPT_CHARACTERS: usize = 500;

/// Why a glossary was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum GlossaryError {
    /// One term is longer than `MAX_TERM_CHARACTERS`.
    #[error("a glossary term is longer than {MAX_TERM_CHARACTERS} characters")]
    TermTooLong,
    /// A term holds a line break or another control character.
    #[error("a glossary term holds a control character")]
    ControlCharacter,
    /// The terms together are longer than Whisper's prompt allows.
    #[error("the glossary is longer than {MAX_PROMPT_CHARACTERS} characters")]
    TooLong,
}

/// A validated list of terms, in the order the owner gave them, each kept once.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TranscriptionGlossary {
    terms: Vec<String>,
}

impl TranscriptionGlossary {
    /// Builds a glossary from the owner's terms.
    ///
    /// Inputs: Terms in any form; each is trimmed, blank ones are dropped, and a term repeated in
    /// another case is kept once (its first spelling).
    /// Outputs: The glossary, or why it was refused (a term too long or holding a control
    /// character, or all of them too long for the prompt).
    /// Side effects: None.
    pub fn parse<'term>(
        terms: impl IntoIterator<Item = &'term str>,
    ) -> Result<Self, GlossaryError> {
        let mut kept: Vec<String> = Vec::new();
        for term in terms {
            let term = term.trim();
            if term.is_empty() {
                continue;
            }
            if term.chars().any(char::is_control) {
                return Err(GlossaryError::ControlCharacter);
            }
            if term.chars().count() > MAX_TERM_CHARACTERS {
                return Err(GlossaryError::TermTooLong);
            }
            if !kept
                .iter()
                .any(|existing| existing.to_lowercase() == term.to_lowercase())
            {
                kept.push(term.to_owned());
            }
        }
        let glossary = Self { terms: kept };
        if glossary
            .prompt()
            .is_some_and(|prompt| prompt.chars().count() > MAX_PROMPT_CHARACTERS)
        {
            return Err(GlossaryError::TooLong);
        }
        Ok(glossary)
    }

    /// Borrows the terms in order. Side effects: None.
    pub fn terms(&self) -> &[String] {
        &self.terms
    }

    /// Reports whether the glossary has no terms. Side effects: None.
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Returns the prompt Whisper receives: the terms separated by commas, ending in a full stop;
    /// `None` without terms. Side effects: None.
    pub fn prompt(&self) -> Option<String> {
        (!self.terms.is_empty()).then(|| format!("{}.", self.terms.join(", ")))
    }

    /// Counts the words at the start of `text` that only repeat glossary terms, when they repeat at
    /// least two of them: Whisper writing its prompt back before the real speech.
    ///
    /// Inputs: A transcript, split into words at whitespace.
    /// Outputs: How many of its leading words are the echo, or 0 (also when one term alone opens
    /// the text, which is ordinary speech). Side effects: None.
    pub fn leading_echo_words(&self, text: &str) -> usize {
        let words: Vec<String> = text.split_whitespace().map(normalized).collect();
        let terms: Vec<Vec<String>> = self
            .terms
            .iter()
            .map(|term| {
                term.split_whitespace()
                    .map(normalized)
                    .filter(|word| !word.is_empty())
                    .collect::<Vec<_>>()
            })
            .filter(|term| !term.is_empty())
            .collect();
        let (mut position, mut matched_terms) = (0, 0);
        while position < words.len() {
            if words[position].is_empty() {
                position += 1;
                continue;
            }
            let longest_match = terms
                .iter()
                .filter(|term| words[position..].starts_with(term))
                .map(Vec::len)
                .max();
            let Some(length) = longest_match else {
                break;
            };
            position += length;
            matched_terms += 1;
        }
        if matched_terms >= 2 { position } else { 0 }
    }

    /// Reports whether `text` is nothing but an echo of the glossary (see `leading_echo_words`).
    ///
    /// Side effects: None.
    pub fn is_echo(&self, text: &str) -> bool {
        let word_count = text.split_whitespace().count();
        word_count > 0 && self.leading_echo_words(text) == word_count
    }
}

impl TranscriptionGlossary {
    /// Removes an echo of the glossary from the start of a transcript (see `leading_echo_words`).
    ///
    /// Inputs: A transcript made with this glossary as its prompt.
    /// Outputs: The transcript without the echoed words, and without their word times; when the
    /// times do not match the words one for one, they are dropped and the text alone is kept. A
    /// transcript that is only an echo comes back empty. Side effects: None.
    pub fn without_echo(
        &self,
        transcript: crate::domain::models::Transcript,
    ) -> crate::domain::models::Transcript {
        let echo_words = self.leading_echo_words(transcript.content());
        if echo_words == 0 {
            return transcript;
        }
        let words: Vec<&str> = transcript.content().split_whitespace().collect();
        let kept_text = words[echo_words.min(words.len())..].join(" ");
        let timed_words = transcript.timed_words();
        let kept_times = if timed_words.len() == words.len() {
            timed_words[echo_words..].to_vec()
        } else {
            Vec::new()
        };
        crate::domain::models::Transcript::new(transcript.meeting_id().clone(), kept_text)
            .with_timed_words(kept_times)
    }
}

/// Lowercases a word and keeps only its letters and digits, so punctuation never decides a match.
fn normalized(word: &str) -> String {
    word.chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
