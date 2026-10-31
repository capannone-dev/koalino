//! Tests turning whisper.cpp tokens into timed words.

use super::whisper_word_timing::{WhisperTokenTiming, words_from_tokens, words_match_text};

fn token(bytes: &[u8], start_units: i64, end_units: i64) -> WhisperTokenTiming<'_> {
    WhisperTokenTiming {
        bytes,
        start_units,
        end_units,
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
