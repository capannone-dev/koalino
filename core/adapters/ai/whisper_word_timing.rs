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

/// Recognizes a segment that is only one of whisper.cpp's non-speech markers (Issue #51).
///
/// Inputs: One segment's text.
/// Outputs: `true` for a lone bracketed, parenthesized or starred tag such as `[BLANK_AUDIO]`,
/// `(silence)` or `*applause*`, which would otherwise end up in the transcript; `false` for
/// anything with words outside such a tag, and for empty text.
/// Side effects: None.
pub fn is_non_speech_marker(segment_text: &str) -> bool {
    let trimmed = segment_text.trim();
    let mut characters = trimmed.chars();
    let (Some(opening), Some(closing)) = (characters.next(), characters.next_back()) else {
        return false;
    };
    let inner = characters.as_str();
    let expected_closing = match opening {
        '[' => ']',
        '(' => ')',
        '*' => '*',
        _ => return false,
    };
    closing == expected_closing
        && !inner.contains([opening, expected_closing])
        && !inner.trim().is_empty()
}

/// How many times in a row a run of words must repeat before it counts as a decoding loop.
///
/// People repeat themselves ("sì, sì, sì, sì"), but five or more identical repeats in a row are
/// Whisper looping on noise or silence: on a real meeting it wrote "sì, sì, sì" for 18 s.
const LOOP_MIN_REPEATS: usize = 5;
/// How many repeats of a loop are kept: enough to read like what was said.
const LOOP_KEPT_REPEATS: usize = 3;
/// The longest run of words looked at as one repeated unit.
const LOOP_MAX_UNIT_WORDS: usize = 4;

/// A word as compared for loops: lower case, without surrounding punctuation or quotes.
fn loop_key(word: &str) -> String {
    let trimmed = word.trim_matches(|character: char| !character.is_alphanumeric());
    if trimmed.is_empty() {
        word.to_owned()
    } else {
        trimmed.to_lowercase()
    }
}

/// Shortens Whisper decoding loops: a run of one to four words repeated five or more times in a
/// row keeps its first three repeats.
///
/// Inputs: The transcript text and its timed words (empty when there is no timing).
/// Outputs: The text with each loop shortened, and the timed words of what is kept (still matching
/// the text word for word when they matched it before; returned unchanged otherwise, since the
/// caller then ignores them). Natural repeats of up to four times are left alone.
/// Side effects: None.
pub fn collapse_repetition_loops(text: &str, words: Vec<TimedWord>) -> (String, Vec<TimedWord>) {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let keys: Vec<String> = tokens.iter().map(|token| loop_key(token)).collect();
    let mut kept = vec![true; tokens.len()];
    let mut index = 0;
    while index < tokens.len() {
        // The unit length whose repeats cover the most words from here.
        let mut best: Option<(usize, usize)> = None;
        for unit in 1..=LOOP_MAX_UNIT_WORDS {
            let mut repeats = 1;
            while index + (repeats + 1) * unit <= tokens.len()
                && keys[index + repeats * unit..index + (repeats + 1) * unit]
                    == keys[index..index + unit]
            {
                repeats += 1;
            }
            if repeats >= LOOP_MIN_REPEATS
                && best.is_none_or(|(best_unit, best_repeats)| {
                    repeats * unit > best_unit * best_repeats
                })
            {
                best = Some((unit, repeats));
            }
        }
        match best {
            Some((unit, repeats)) => {
                for dropped in &mut kept[index + LOOP_KEPT_REPEATS * unit..index + repeats * unit] {
                    *dropped = false;
                }
                index += repeats * unit;
            }
            None => index += 1,
        }
    }
    if kept.iter().all(|keep| *keep) {
        return (text.to_owned(), words);
    }
    let words = if words_match_text(&words, text) {
        // Each text word belongs to one timed word; a timed word stays only if all of it stays.
        let mut owners: Vec<usize> = Vec::with_capacity(tokens.len());
        for (word_index, word) in words.iter().enumerate() {
            owners.extend(word.text().split_whitespace().map(|_| word_index));
        }
        let mut word_kept = vec![true; words.len()];
        for (token_index, owner) in owners.iter().enumerate() {
            if !kept[token_index] {
                word_kept[*owner] = false;
            }
        }
        for (token_index, owner) in owners.iter().enumerate() {
            kept[token_index] = word_kept[*owner];
        }
        words
            .into_iter()
            .zip(word_kept)
            .filter_map(|(word, keep)| keep.then_some(word))
            .collect()
    } else {
        words
    };
    let text = tokens
        .iter()
        .zip(&kept)
        .filter_map(|(token, keep)| keep.then_some(*token))
        .collect::<Vec<_>>()
        .join(" ");
    (text, words)
}
