//! Specifies which languages can be chosen and how a context picks the one a chunk uses.

use super::{
    models::TranscriptionContext,
    transcription_language::{TranscriptionLanguage, WHISPER_LANGUAGE_CODES},
};

fn parsed(text: &str) -> TranscriptionLanguage {
    TranscriptionLanguage::parse(text).unwrap_or_else(|error| panic!("{text} is valid: {error}"))
}

#[test]
fn auto_and_whisper_codes_are_accepted_and_normalized() {
    assert_eq!(parsed("auto"), TranscriptionLanguage::Auto);
    assert_eq!(parsed(" AUTO "), TranscriptionLanguage::Auto);
    assert_eq!(parsed("it").as_str(), "it");
    assert_eq!(parsed(" IT\n").as_str(), "it");
    assert_eq!(parsed("yue").as_str(), "yue");
}

/// whisper.cpp fails every chunk on a language it does not know, so these must be refused.
#[test]
fn anything_whisper_does_not_know_is_refused() {
    for text in ["", "xx", "italian", "it-IT", "../it", "i\0t"] {
        assert!(TranscriptionLanguage::parse(text).is_err(), "{text:?}");
    }
}

#[test]
fn the_code_list_has_no_duplicates() {
    let mut codes = WHISPER_LANGUAGE_CODES.to_vec();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), WHISPER_LANGUAGE_CODES.len());
}

#[test]
fn a_context_language_overrides_the_configured_one() {
    let plain = TranscriptionContext::new(None);
    assert_eq!(plain.language_or(Some("de")), Some("de"));
    assert_eq!(plain.language_or(None), None);

    let italian = TranscriptionContext::new(None).with_language(Some(parsed("it")));
    assert_eq!(italian.language_or(Some("de")), Some("it"));

    let auto = TranscriptionContext::new(None).with_language(Some(TranscriptionLanguage::Auto));
    assert_eq!(auto.language_or(Some("de")), Some("auto"));
}
