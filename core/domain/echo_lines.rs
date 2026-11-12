//! Finds microphone lines that only repeat what the computer's sound said (#108).
//!
//! Without headphones, the microphone hears the other side's voice from the speakers, so the same
//! words land on both tracks. The computer's own sound is the clean copy, so the microphone's copy is
//! the one dropped. Nothing is dropped unless echo was heard in the meeting's audio
//! (`echo_detect`): with headphones, a short reply that repeats the other side's words is the
//! owner's own.
//!
//! Ported from Yogurt's `app/src/main/asr/echo.ts` (FrigadeHQ/yogurt, MIT License, Copyright (c)
//! 2026 Frigade; see `THIRD_PARTY_NOTICES.md`). Differences: the per-line audio check (`over`) is
//! left out, so a line is judged on the words and on whether the microphone echoed in the window
//! holding it; and Italian short replies and fillers join the English ones.

use std::collections::HashMap;

/// One transcript line on a track: its span in meeting time and its text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EchoLine<'text> {
    /// Start, in milliseconds from the meeting's start.
    pub start_ms: u64,
    /// End, in milliseconds from the meeting's start.
    pub end_ms: u64,
    /// The line's words; never logged.
    pub text: &'text str,
}

/// Lines of theirs within this of a microphone line may be what it copies.
const WINDOW_MS: u64 = 1_500;
/// The speakers' tail: a microphone line starting later than this after their line ended is the
/// owner's own (a read-back).
const TAIL_MS: u64 = 500;
/// A long line (8+ words) is an echo at this share of its words found in theirs.
const THRESHOLD: f32 = 0.6;
const LONG_TOKENS: usize = 8;
/// Six or seven words need more: a few shared words come easily.
const MID_THRESHOLD: f32 = 0.8;
const MID_TOKENS: usize = 6;
/// Under six words: nearly whole.
const SHORT_THRESHOLD: f32 = 0.95;
/// Against several of their lines at once, more of the words must be there.
const JOINED_THRESHOLD: f32 = 0.75;
/// A one or two word reply is too short to compare by words.
const MIN_TOKENS: usize = 3;
/// A scrap is checked against their lines within this of it.
const SCRAP_SLACK_MS: u64 = 300;

/// Sounds that are not words; a microphone line of only these says nothing worth keeping.
const FILLERS: &[&str] = &[
    "um", "umm", "uh", "uhh", "hm", "hmm", "mm", "mmm", "mhm", "erm", "ah", "oh", "eh", "huh",
    "ehm", "mh", "boh",
];
/// The owner's own short replies, English and Italian: never dropped, however their words fit.
const ACKNOWLEDGEMENTS: &[&str] = &[
    "yeah",
    "yes",
    "yep",
    "yup",
    "no",
    "nope",
    "right",
    "okay",
    "ok",
    "sure",
    "exactly",
    "true",
    "agreed",
    "fine",
    "great",
    "good",
    "thanks",
    "thank",
    "you",
    "please",
    "sorry",
    "absolutely",
    "definitely",
    "indeed",
    "correct",
    "cool",
    "perfect",
    "hi",
    "hello",
    "bye",
    "alright",
    "well",
    "so",
    "and",
    "but",
    "i",
    "me",
    "too",
    "also",
    "sì",
    "si",
    "va",
    "bene",
    "certo",
    "esatto",
    "giusto",
    "grazie",
    "perfetto",
    "ciao",
    "allora",
    "quindi",
    "ma",
    "e",
    "io",
    "anche",
    "d'accordo",
    "vero",
    "ottimo",
    "prego",
    "scusa",
    "scusate",
    "sicuro",
    "assolutamente",
];

/// Splits text into lowercased words without punctuation; text written without spaces (Chinese,
/// Japanese) is compared character by character.
///
/// Side effects: None.
pub fn tokens(text: &str) -> Vec<String> {
    let lowered = text.to_lowercase();
    let mut words = Vec::new();
    let mut current = String::new();
    for character in lowered.chars() {
        if character.is_alphanumeric() {
            if is_cjk(character) {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
                words.push(character.to_string());
            } else {
                current.push(character);
            }
        } else if !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// Han, Hiragana and Katakana characters, which carry no spaces between words.
fn is_cjk(character: char) -> bool {
    matches!(character as u32,
        0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF)
}

/// How many of `a`'s words `b` has too, each counted as often as it appears in both.
fn shared_count(a: &[String], b: &[String]) -> usize {
    let mut left: HashMap<&str, usize> = HashMap::new();
    for word in b {
        *left.entry(word.as_str()).or_default() += 1;
    }
    let mut shared = 0;
    for word in a {
        if let Some(count) = left.get_mut(word.as_str())
            && *count > 0
        {
            *count -= 1;
            shared += 1;
        }
    }
    shared
}

/// The share of the shorter text's words that the other has too.
///
/// Outputs: 0 to 1; 0 when either text has no words. Side effects: None.
pub fn token_overlap(a: &str, b: &str) -> f32 {
    let (first, second) = (tokens(a), tokens(b));
    if first.is_empty() || second.is_empty() {
        return 0.0;
    }
    shared_count(&first, &second) as f32 / first.len().min(second.len()) as f32
}

/// Whether a microphone line is a copy of what they said around the same time.
///
/// Inputs: The microphone line, their lines, and whether the microphone echoed in the window
/// holding a moment. Outputs: true for a copy. Side effects: None.
fn is_echo(line: &EchoLine<'_>, theirs: &[EchoLine<'_>], echoed_at: &dyn Fn(u64) -> bool) -> bool {
    let middle = line.start_ms + line.end_ms.saturating_sub(line.start_ms) / 2;
    if !(echoed_at(line.start_ms) || echoed_at(middle) || echoed_at(line.end_ms)) {
        return false;
    }
    let mine = tokens(line.text);
    // Their lines this one could copy: near in time, and begun before their line (and the
    // speakers' tail) ended.
    let near: Vec<&EchoLine<'_>> = theirs
        .iter()
        .filter(|other| {
            line.start_ms <= other.end_ms + WINDOW_MS
                && line.end_ms + WINDOW_MS >= other.start_ms
                && line.start_ms < other.end_ms + TAIL_MS
        })
        .collect();
    if mine.len() < MIN_TOKENS {
        if !mine.is_empty() && mine.iter().all(|word| FILLERS.contains(&word.as_str())) {
            return !near.is_empty();
        }
        if mine.is_empty()
            || mine
                .iter()
                .all(|word| ACKNOWLEDGEMENTS.contains(&word.as_str()))
        {
            return false;
        }
        // A scrap of their line said at that very moment ("role is" inside "my role is to design").
        return theirs
            .iter()
            .filter(|other| {
                line.start_ms < other.end_ms + SCRAP_SLACK_MS
                    && line.end_ms + SCRAP_SLACK_MS > other.start_ms
            })
            .any(|other| {
                let words = tokens(other.text);
                mine.iter().all(|word| words.contains(word))
            });
    }
    let long_enough = |other: &&&EchoLine<'_>| tokens(other.text).len() >= MIN_TOKENS;
    if mine.len() < MID_TOKENS {
        return near
            .iter()
            .filter(long_enough)
            .any(|other| token_overlap(line.text, other.text) >= SHORT_THRESHOLD);
    }
    let needed = if mine.len() < LONG_TOKENS {
        MID_THRESHOLD
    } else {
        THRESHOLD
    };
    let best = near
        .iter()
        .filter(long_enough)
        .map(|other| token_overlap(line.text, other.text))
        .fold(0.0_f32, f32::max);
    if best >= needed {
        return true;
    }
    // Whisper cuts the two tracks at different pauses, so one sentence's echo can straddle two
    // of their lines.
    let joined: Vec<&str> = near.iter().map(|other| other.text).collect();
    mine.len() >= LONG_TOKENS
        && near.len() > 1
        && token_overlap(line.text, &joined.join(" ")) >= JOINED_THRESHOLD
}

/// Finds the microphone lines that copy their lines.
///
/// Inputs: The microphone's lines; the computer sound's lines; whether echo was heard in the
/// meeting (`EchoDetector::present`); and whether the microphone echoed in the window holding a
/// moment (`EchoDetector::correlated_at`, or always when the audio is gone).
/// Outputs: The positions in `mine` of the copies, in order; none without echo heard.
/// Side effects: None.
pub fn echo_positions(
    mine: &[EchoLine<'_>],
    theirs: &[EchoLine<'_>],
    echo_heard: bool,
    echoed_at: &dyn Fn(u64) -> bool,
) -> Vec<usize> {
    if !echo_heard || theirs.is_empty() {
        return Vec::new();
    }
    mine.iter()
        .enumerate()
        .filter(|(_, line)| is_echo(line, theirs, echoed_at))
        .map(|(position, _)| position)
        .collect()
}
