//! Specifies the default language: the owner's stored choice, else the environment, else AUTO.

use std::path::{Path, PathBuf};

use super::{default_language, save_default_language};
use crate::domain::transcription_language::TranscriptionLanguage;

/// A database path in a new temporary directory.
fn database_path() -> PathBuf {
    let directory = std::env::temp_dir().join(format!("ffi-language-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory)
        .unwrap_or_else(|error| panic!("directory must be created: {error}"));
    directory.join("note_taker.sqlite3")
}

/// Settings holding the database path and, optionally, the Whisper variable.
fn settings<'a>(path: &'a Path, whisper: Option<&'a str>) -> impl Fn(&str) -> Option<String> + 'a {
    move |name| match name {
        "NOTE_TAKER_DATABASE_PATH" => Some(path.display().to_string()),
        "NOTE_TAKER_WHISPER_LANGUAGE" => whisper.map(str::to_owned),
        _ => None,
    }
}

#[test]
fn without_a_choice_the_default_is_auto_or_the_environments() {
    let path = database_path();
    assert_eq!(
        default_language(&settings(&path, None)),
        TranscriptionLanguage::Auto
    );
    assert_eq!(
        default_language(&settings(&path, Some("de"))).as_str(),
        "de"
    );
    assert_eq!(
        default_language(&settings(&path, Some("nonsense"))),
        TranscriptionLanguage::Auto
    );
}

#[test]
fn a_stored_choice_survives_and_wins_over_the_environment() {
    let path = database_path();
    let italian = TranscriptionLanguage::parse("it").ok();
    let italian = italian.unwrap_or_else(|| panic!("it is valid"));

    save_default_language(&settings(&path, None), italian)
        .unwrap_or_else(|error| panic!("the choice must be stored: {error}"));
    assert_eq!(default_language(&settings(&path, Some("de"))), italian);

    save_default_language(&settings(&path, None), TranscriptionLanguage::Auto)
        .unwrap_or_else(|error| panic!("the choice must be stored: {error}"));
    assert_eq!(
        default_language(&settings(&path, Some("de"))),
        TranscriptionLanguage::Auto
    );
}

/// A damaged file must not stop recordings: it reads as no choice.
#[test]
fn a_damaged_file_reads_as_no_choice() {
    let path = database_path();
    let mut file = path.clone().into_os_string();
    file.push(".transcription-language");
    std::fs::write(&file, "garbage").unwrap_or_else(|error| panic!("file: {error}"));

    assert_eq!(
        default_language(&settings(&path, None)),
        TranscriptionLanguage::Auto
    );
}
