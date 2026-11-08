//! Specifies the glossary of names and terms that steers transcription: how terms are cleaned and
//! bounded, the prompt Whisper receives, and how an echo of that prompt is recognised.

use super::transcription_glossary::{
    GlossaryError, MAX_PROMPT_CHARACTERS, MAX_TERM_CHARACTERS, TranscriptionGlossary,
};

fn glossary(terms: &[&str]) -> TranscriptionGlossary {
    TranscriptionGlossary::parse(terms.iter().copied()).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn terms_are_trimmed_blanks_dropped_and_repeats_kept_once() {
    let glossary = glossary(&["  Marco Rossi ", "", "Kubernetes", "kubernetes", "  "]);

    assert_eq!(glossary.terms(), ["Marco Rossi", "Kubernetes"]);
}

#[test]
fn the_prompt_lists_the_terms_without_words_of_any_language() {
    let glossary = glossary(&["Marco Rossi", "Kubernetes", "OKR"]);

    assert_eq!(
        glossary.prompt().as_deref(),
        Some("Marco Rossi, Kubernetes, OKR.")
    );
    assert_eq!(TranscriptionGlossary::default().prompt(), None);
}

#[test]
fn overlong_or_control_character_terms_and_too_many_terms_are_refused() {
    let long_term = "x".repeat(MAX_TERM_CHARACTERS + 1);
    assert_eq!(
        TranscriptionGlossary::parse([long_term.as_str()]),
        Err(GlossaryError::TermTooLong)
    );
    assert_eq!(
        TranscriptionGlossary::parse(["line\nbreak"]),
        Err(GlossaryError::ControlCharacter)
    );
    let many: Vec<String> = (0..MAX_PROMPT_CHARACTERS / 5)
        .map(|index| format!("term{index:04}"))
        .collect();
    assert_eq!(
        TranscriptionGlossary::parse(many.iter().map(String::as_str)),
        Err(GlossaryError::TooLong)
    );
}

#[test]
fn a_transcript_that_only_repeats_glossary_terms_is_an_echo() {
    let glossary = glossary(&["Marco Rossi", "Kubernetes", "OKR"]);

    assert!(glossary.is_echo("Marco Rossi, Kubernetes, OKR."));
    assert!(glossary.is_echo(" kubernetes okr "));
    assert!(
        !glossary.is_echo("Marco."),
        "one name alone can be a real answer"
    );
    assert!(!glossary.is_echo("Marco, can you check Kubernetes?"));
    assert!(!TranscriptionGlossary::default().is_echo("anything"));
}

#[test]
fn a_leading_echo_is_counted_in_words_so_the_speech_after_it_survives() {
    let glossary = glossary(&["Marco Rossi", "Kubernetes", "OKR"]);

    assert_eq!(
        glossary.leading_echo_words("Marco Rossi, Kubernetes, OKR. So we agreed on Friday."),
        4
    );
    assert_eq!(
        glossary.leading_echo_words("Marco, can you check Kubernetes?"),
        0
    );
    assert_eq!(glossary.leading_echo_words("Kubernetes is down"), 0);
}

#[test]
fn a_transcription_request_prompts_with_any_earlier_text_then_the_glossary() {
    use crate::domain::models::TranscriptionContext;
    let terms = glossary(&["Marco Rossi", "OKR"]);

    let both =
        TranscriptionContext::new(Some("we said".to_owned())).with_glossary(Some(terms.clone()));
    let glossary_only = TranscriptionContext::new(None).with_glossary(Some(terms.clone()));
    let empty_glossary =
        TranscriptionContext::new(None).with_glossary(Some(TranscriptionGlossary::default()));

    assert_eq!(both.prompt().as_deref(), Some("we said Marco Rossi, OKR."));
    assert_eq!(glossary_only.prompt().as_deref(), Some("Marco Rossi, OKR."));
    assert_eq!(glossary_only.glossary(), Some(&terms));
    assert_eq!(empty_glossary.prompt(), None);
    assert_eq!(
        empty_glossary.glossary(),
        None,
        "an empty glossary is no glossary"
    );
    assert_eq!(
        TranscriptionContext::new(Some("we said".to_owned()))
            .prompt()
            .as_deref(),
        Some("we said")
    );
}

#[test]
fn an_echo_is_removed_from_a_transcript_with_its_word_times() {
    use crate::domain::models::{MeetingId, TimedWord, Transcript};
    let glossary = glossary(&["Marco Rossi", "OKR"]);
    let words = ["Marco", "Rossi,", "OKR.", "We", "agreed."];
    let transcript = Transcript::new(MeetingId::new("m"), words.join(" ")).with_timed_words(
        words
            .iter()
            .enumerate()
            .map(|(index, word)| TimedWord::new(*word, index as u64 * 100, index as u64 * 100 + 90))
            .collect(),
    );

    let cleaned = glossary.without_echo(transcript);

    assert_eq!(cleaned.content(), "We agreed.");
    let kept: Vec<&str> = cleaned.timed_words().iter().map(TimedWord::text).collect();
    assert_eq!(kept, ["We", "agreed."]);
    assert_eq!(cleaned.timed_words()[0].start_ms(), 300);

    let only_echo =
        glossary.without_echo(Transcript::new(MeetingId::new("m"), "Marco Rossi, OKR."));
    assert_eq!(only_echo.content(), "");
    let speech = glossary.without_echo(Transcript::new(MeetingId::new("m"), "OKR review today"));
    assert_eq!(speech.content(), "OKR review today");
}
