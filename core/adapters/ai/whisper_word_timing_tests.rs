//! Tests turning whisper.cpp tokens into timed words.

use super::whisper_word_timing::{
    WhisperTokenTiming, collapse_repetition_loops, words_from_tokens, words_match_text,
};
use crate::domain::models::TimedWord;

fn token(bytes: &[u8], start_units: i64, end_units: i64) -> WhisperTokenTiming<'_> {
    WhisperTokenTiming {
        bytes,
        start_units,
        end_units,
        probability: 0.9,
    }
}

/// Verifies pieces of a word join and keep the whole word's time.
#[test]
fn tokens_without_leading_space_continue_the_word() {
    let words = words_from_tokens([
        token(b" buon", 0, 20),
        token(b"gior", 20, 30),
        token(b"no", 30, 50),
        token(b" a", 60, 70),
    ]);
    assert_eq!(words.len(), 2);
    assert_eq!(words[0].text(), "buongiorno");
    assert_eq!(words[0].midpoint_ms(), 250);
    assert_eq!(words[1].text(), "a");
    assert_eq!(words[1].midpoint_ms(), 650);
}

/// Verifies an accented letter split across two tokens survives.
#[test]
fn a_character_split_between_tokens_is_rejoined() {
    let accented = "perché".as_bytes();
    let (first, second) = accented.split_at(accented.len() - 1);
    let mut leading = b" ".to_vec();
    leading.extend_from_slice(first);
    let words = words_from_tokens([token(&leading, 0, 10), token(second, 10, 20)]);
    assert_eq!(words[0].text(), "perché");
}

/// Verifies control tokens are left out and do not join words.
#[test]
fn control_tokens_are_skipped() {
    let words = words_from_tokens([
        token(b"[_BEG_]", 0, 0),
        token(b" ciao", 0, 10),
        token(b"<|it|>", 10, 10),
        token(b"[_TT_50]", 10, 10),
    ]);
    assert_eq!(words.len(), 1);
    assert_eq!(words[0].text(), "ciao");
}

/// Verifies unknown times do not panic or wrap.
#[test]
fn negative_times_count_as_zero() {
    let words = words_from_tokens([token(b" ciao", -1, -1)]);
    assert_eq!(words[0].midpoint_ms(), 0);
}

/// Verifies the guard that protects the transcript's text.
#[test]
fn words_must_spell_the_text() {
    let words = words_from_tokens([token(b" un", 0, 1), token(b" due", 1, 2)]);
    assert!(words_match_text(&words, " un  due "));
    assert!(!words_match_text(&words, "un tre"));
    assert!(!words_match_text(&words, "un"));
}

/// Verifies a word is as sure as its least sure token (#91).
#[test]
fn a_word_takes_the_probability_of_its_least_sure_token() {
    let words = words_from_tokens([
        WhisperTokenTiming {
            probability: 0.95,
            ..token(b" Kuber", 0, 10)
        },
        WhisperTokenTiming {
            probability: 0.31,
            ..token(b"netes", 10, 20)
        },
        WhisperTokenTiming {
            probability: 0.88,
            ..token(b" now", 20, 30)
        },
    ]);
    let probabilities: Vec<Option<u16>> = words.iter().map(|w| w.probability_permille()).collect();
    assert_eq!(probabilities, [Some(310), Some(880)]);
}

/// Timed words for `text`, one per space-separated word, 100 ms each.
fn timed(text: &str) -> Vec<TimedWord> {
    text.split_whitespace()
        .enumerate()
        .map(|(index, word)| TimedWord::new(word, index as u64 * 100, index as u64 * 100 + 100))
        .collect()
}

/// Verifies a decoding loop ("sì, sì, sì, …" for seconds) keeps only a few repeats, in the text
/// and in the timed words alike (seen on a real meeting, 2026-11-24).
#[test]
fn a_repetition_loop_keeps_only_a_few_repeats() {
    let looping = format!("Però basta. {}e poi", "Sì, sì, sì. ".repeat(12));
    let (text, words) = collapse_repetition_loops(&looping, timed(&looping));
    assert_eq!(text, "Però basta. Sì, sì, sì. e poi");
    assert!(words_match_text(&words, &text));
    assert_eq!(words.last().map(TimedWord::text), Some("poi"));
}

/// Verifies a loop of one word is shortened too, whatever its punctuation or case.
#[test]
fn a_one_word_loop_is_shortened() {
    let looping = format!("quindi {}fine", "si ".repeat(40));
    let (text, words) = collapse_repetition_loops(&looping, timed(&looping));
    assert_eq!(text, "quindi si si si fine");
    assert!(words_match_text(&words, &text));
}

/// Verifies ordinary speech, and a few natural repeats, are left as they are.
#[test]
fn natural_repeats_are_kept() {
    for spoken in [
        "no no no, non è così",
        "sì, sì, sì, sì, certo",
        "molto molto bene, grazie grazie",
        "va bene. Va bene. Va bene.",
    ] {
        let (text, words) = collapse_repetition_loops(spoken, timed(spoken));
        assert_eq!(text, spoken);
        assert_eq!(words.len(), spoken.split_whitespace().count());
    }
}

/// Verifies a loop is shortened even without timed words, which stay empty.
#[test]
fn a_loop_without_timing_is_shortened() {
    let looping = "ok ".repeat(30);
    let (text, words) = collapse_repetition_loops(looping.trim(), Vec::new());
    assert_eq!(text, "ok ok ok");
    assert!(words.is_empty());
}
