//! Tests the Whisper model catalog.

use super::whisper_catalog::{WHISPER_MODELS, find_whisper_model};

/// Verifies the hashes and names cannot silently be malformed.
#[test]
fn every_entry_is_well_formed_and_unique() {
    let mut names = std::collections::HashSet::new();
    for model in WHISPER_MODELS {
        assert!(names.insert(model.file_name()));
        assert_eq!(model.sha256().len(), 64);
        assert!(model.sha256().bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(model.size_bytes() > 0);
    }
    assert_eq!(WHISPER_MODELS.len(), 33);
}

/// Verifies groups and language flags come from the name.
#[test]
fn names_split_into_family_and_language() {
    let turbo = find_whisper_model("ggml-large-v3-turbo-q5_0.bin");
    assert_eq!(turbo.map(|m| m.family()), Some("large-v3-turbo"));
    assert_eq!(turbo.map(|m| m.is_english_only()), Some(false));
    let english = find_whisper_model("ggml-small.en-q8_0.bin");
    assert_eq!(english.map(|m| m.family()), Some("small"));
    assert_eq!(english.map(|m| m.is_english_only()), Some(true));
    assert_eq!(english.map(|m| m.name()), Some("small.en-q8_0"));
}

/// Verifies an unknown name or a path never matches.
#[test]
fn unknown_files_and_paths_are_not_found() {
    assert!(find_whisper_model("../ggml-base.bin").is_none());
    assert!(find_whisper_model("ggml-small.en-tdrz.bin").is_none());
}
