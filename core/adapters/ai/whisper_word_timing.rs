//! Builds timed words from whisper.cpp tokens, which split words and even multi-byte characters.

use crate::domain::models::TimedWord;

/// Length of one whisper.cpp timestamp unit, in milliseconds.
const MILLISECONDS_PER_TIMESTAMP_UNIT: u64 = 10;

/// One decoded token, independent of whisper.cpp types.
#[derive(Clone, Copy, Debug)]
pub struct WhisperTokenTiming<'a> {
    /// The token's raw bytes, which may hold only part of a UTF-8 character.
    pub bytes: &'a [u8],
    /// Start in whisper.cpp timestamp units (10 ms); negative when unknown.
    pub start_units: i64,
    /// End in whisper.cpp timestamp units (10 ms); negative when unknown.
    pub end_units: i64,
    /// How sure the decoder was of the token, in [0, 1].
    pub probability: f32,
}

/// Joins tokens into words: a token starting with whitespace starts a new word.
///
/// Inputs: Tokens in decoding order. Control tokens such as `[_BEG_]` or `<|it|>` are skipped.
/// Outputs: Words with times in milliseconds from the start of the audio, without empty words,
/// each as sure as its least sure token.
/// Invalid UTF-8 is replaced rather than dropped, so the word count stays right.
/// Side effects: None.
pub fn words_from_tokens<'a>(
    tokens: impl IntoIterator<Item = WhisperTokenTiming<'a>>,
) -> Vec<TimedWord> {
    let mut words = Vec::new();
    let mut pending: Option<PendingWord> = None;
    for token in tokens {
        if token.bytes.starts_with(b"[_") || token.bytes.starts_with(b"<|") {
            continue;
        }
        let start_ms = units_to_milliseconds(token.start_units);
        let end_ms = units_to_milliseconds(token.end_units);
        let starts_word = token.bytes.first().is_some_and(u8::is_ascii_whitespace);
        match &mut pending {
            Some(word) if !starts_word => {
                word.bytes.extend_from_slice(token.bytes);
                word.end_ms = end_ms.max(word.end_ms);
                word.probability = word.probability.min(token.probability);
            }
            _ => {
                finish_word(pending.take(), &mut words);
                pending = Some(PendingWord {
                    bytes: token.bytes.to_vec(),
                    start_ms,
                    end_ms,
                    probability: token.probability,
                });
            }
        }
    }
    finish_word(pending, &mut words);
    words
}

/// Checks that timed words spell the transcript, so timing can never change the text.
///
/// Inputs: The words and the transcript text.
/// Outputs: `true` when both hold the same words in the same order.
/// Side effects: None.
pub fn words_match_text(words: &[TimedWord], text: &str) -> bool {
    words
        .iter()
        .flat_map(|word| word.text().split_whitespace())
        .eq(text.split_whitespace())
}

/// Converts whisper.cpp timestamp units to milliseconds, treating unknown times as zero.
fn units_to_milliseconds(units: i64) -> u64 {
    u64::try_from(units).unwrap_or(0) * MILLISECONDS_PER_TIMESTAMP_UNIT
}

/// A word being joined from its tokens.
struct PendingWord {
    /// The bytes so far, possibly ending inside a character.
    bytes: Vec<u8>,
    /// Start in milliseconds.
    start_ms: u64,
    /// End in milliseconds.
    end_ms: u64,
    /// The least sure token's probability so far.
    probability: f32,
}

/// Adds a finished word unless it is blank.
fn finish_word(pending: Option<PendingWord>, words: &mut Vec<TimedWord>) {
    if let Some(word) = pending {
        let text = String::from_utf8_lossy(&word.bytes);
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            words.push(
                TimedWord::new(trimmed, word.start_ms, word.end_ms)
                    .with_probability(word.probability),
            );
        }
    }
}
