//! Specifies the owner's language model settings (#95, #101): the connections, the default one,
//! the summary template, automatic summaries, and where each connection's key is kept.

use std::path::PathBuf;

use super::llm_settings::{
    LlmKeyStore, SavedLlmSettings, llm_settings_file_for, load_llm_settings, save_llm_settings,
};
use crate::domain::llm::{DEFAULT_SUMMARY_TEMPLATE, LlmConnection, LlmKind};

fn folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("note-taker-llm-{name}-{}", std::process::id()));
    drop(std::fs::remove_dir_all(&folder));
    std::fs::create_dir_all(&folder).unwrap_or_else(|e| panic!("{e}"));
    folder
}

fn local(id: &str) -> LlmConnection {
    LlmConnection::with_id(
        id,
        "Office server",
        LlmKind::OpenAiCompatible,
        "http://192.168.1.20:11434/v1",
        "llama3.1:8b",
        32_768,
    )
    .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn the_settings_file_sits_beside_the_database() {
    assert_eq!(
        llm_settings_file_for(&PathBuf::from("/data/note_taker.sqlite3")),
        PathBuf::from("/data/note_taker.sqlite3.llm-settings.json")
    );
}

#[test]
fn nothing_saved_means_no_connection_the_default_template_and_automatic_summaries() {
    let settings = load_llm_settings(&folder("empty").join("missing.json"));
    assert!(settings.connections().is_empty());
    assert!(settings.default_connection().is_none());
    assert_eq!(settings.template().as_str(), DEFAULT_SUMMARY_TEMPLATE);
    assert!(
        settings.auto_summarize(),
        "automatic summaries are on until the owner opts out"
    );
}

#[test]
fn connections_round_trip_and_the_default_follows_edits_and_removals() {
    let file = folder("round-trip").join("s.json");
    let mut settings = SavedLlmSettings::default();
    settings.upsert_connection(local("a"));
    assert_eq!(
        settings.default_connection().map(LlmConnection::id),
        Some("a"),
        "a single connection is the default"
    );
    settings.upsert_connection(local("b"));
    assert_eq!(settings.set_default(Some("b")), Ok(()));
    assert!(settings.set_default(Some("missing")).is_err());
    settings
        .set_template(Some("Summarize {{title}} for {{project}}."))
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(settings.set_template(Some("{{secret}}")).is_err());
    settings.set_auto_summarize(false);
    save_llm_settings(&file, &settings).unwrap_or_else(|e| panic!("{e}"));

    let mut loaded = load_llm_settings(&file);
    assert_eq!(loaded, settings);
    assert_eq!(
        loaded.default_connection().map(LlmConnection::id),
        Some("b")
    );
    assert!(!loaded.auto_summarize());

    let edited = LlmConnection::with_id(
        "b",
        "Renamed",
        LlmKind::Anthropic,
        "https://api.anthropic.com",
        "claude-opus-5-5",
        200_000,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    loaded.upsert_connection(edited);
    assert_eq!(loaded.connections().len(), 2, "an edit keeps its place");
    assert_eq!(
        loaded.connection("b").map(LlmConnection::label),
        Some("Renamed")
    );
    assert!(loaded.remove_connection("b"));
    assert_eq!(
        loaded.default_connection().map(LlmConnection::id),
        Some("a"),
        "removing the default falls back to the one left"
    );
    assert!(!loaded.remove_connection("b"));
    loaded.set_template(None).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(loaded.template().as_str(), DEFAULT_SUMMARY_TEMPLATE);
}

#[test]
fn a_hand_edited_file_loses_only_what_is_no_longer_valid() {
    let file = folder("hand-edited").join("s.json");
    std::fs::write(
        &file,
        r#"{
            "connections": [
                {"id": "ok", "label": "Fine", "kind": "anthropic", "base_url": "https://api.anthropic.com", "model": "claude-opus-5-5", "context_tokens": 200000},
                {"id": "bad", "label": "Plain", "kind": "open_ai_compatible", "base_url": "http://example.com/v1", "model": "m", "context_tokens": 8192}
            ],
            "default_connection": "bad",
            "summary_template": "{{unclosed",
            "auto_summarize": true
        }"#,
    )
    .unwrap_or_else(|e| panic!("{e}"));

    let settings = load_llm_settings(&file);

    assert_eq!(
        settings.connections().len(),
        1,
        "plain HTTP to a public host is dropped"
    );
    assert_eq!(
        settings.default_connection().map(LlmConnection::id),
        Some("ok")
    );
    assert_eq!(settings.template().as_str(), DEFAULT_SUMMARY_TEMPLATE);
}

#[test]
fn each_connection_keeps_its_own_sealed_key() {
    let keys = folder("keys");
    let host_key = Some(vec![7_u8; 32]);
    let first = LlmKeyStore::open(&keys, host_key.clone(), "a");
    let second = LlmKeyStore::open(&keys, host_key.clone(), "b");

    assert_eq!(first.load().map(|key| key.is_none()), Ok(true));
    first.save("sk-first").unwrap_or_else(|e| panic!("{e}"));
    second.save("sk-second").unwrap_or_else(|e| panic!("{e}"));

    let reopened = LlmKeyStore::open(&keys, host_key, "a");
    assert_eq!(
        reopened
            .load()
            .ok()
            .flatten()
            .as_deref()
            .map(String::as_str),
        Some("sk-first")
    );
    let on_disk = std::fs::read(keys.join("llm-a.sealed")).unwrap_or_default();
    assert!(
        !on_disk.windows(8).any(|w| w == b"sk-first"),
        "the key is sealed"
    );
    reopened.remove().unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(reopened.load().map(|key| key.is_none()), Ok(true));
    assert_eq!(
        second.load().ok().flatten().as_deref().map(String::as_str),
        Some("sk-second")
    );
}
