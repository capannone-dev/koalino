//! Specifies the owner's language model settings (#95, #101): the connections, the default one,
//! the summary template, automatic summaries, and dropping what an earlier version kept (#117).

use std::path::PathBuf;

use super::llm_settings::{
    LlmSettingsError, SavedLlmSettings, forget_sealed_api_keys, llm_settings_file_for,
    load_llm_settings, save_llm_settings,
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
        LlmKind::OpenAiCompatible,
        "https://llm.example/v1",
        "qwen3:32b",
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
                {"id": "ok", "label": "Fine", "kind": "open_ai_compatible", "base_url": "https://llm.example/v1", "model": "m", "context_tokens": 200000},
                {"id": "gone", "label": "Keyed", "kind": "anthropic", "base_url": "https://api.anthropic.com", "model": "claude-opus-5-5", "context_tokens": 200000},
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
        "plain HTTP to a public host and a removed kind (Anthropic) are dropped"
    );
    assert_eq!(
        settings.default_connection().map(LlmConnection::id),
        Some("ok")
    );
    assert_eq!(settings.template().as_str(), DEFAULT_SUMMARY_TEMPLATE);
}

#[test]
fn the_old_openai_transcription_key_is_deleted_but_connection_keys_stay() {
    let keys = folder("old-keys");
    for name in [
        "llm-a.sealed",
        "llm-b.sealed",
        "openai_key.sealed",
        "database.key",
    ] {
        std::fs::write(keys.join(name), b"sealed").unwrap_or_else(|e| panic!("{e}"));
    }

    assert_eq!(forget_sealed_api_keys(&keys), 1);

    assert!(
        keys.join("llm-a.sealed").exists(),
        "connection keys are kept again (#138)"
    );
    assert!(
        !keys.join("openai_key.sealed").exists(),
        "the OpenAI transcription key too"
    );
    assert!(keys.join("database.key").exists(), "other keys stay");
    assert_eq!(forget_sealed_api_keys(&folder("missing").join("none")), 0);
}

#[test]
fn answering_other_devices_is_off_until_a_connection_is_chosen_and_follows_removals() {
    let mut settings = SavedLlmSettings::default();
    assert_eq!(settings.paired_summary_connection(), None, "off by default");
    assert_eq!(
        settings.set_paired_summary_connection(Some("missing")),
        Err(LlmSettingsError::UnknownConnection)
    );
    settings.upsert_connection(local("office"));
    settings.upsert_connection(local("other"));
    assert_eq!(
        settings.set_paired_summary_connection(Some("office")),
        Ok(())
    );
    assert_eq!(
        settings.paired_summary_connection().map(LlmConnection::id),
        Some("office")
    );

    let file = folder("paired").join("settings.json");
    save_llm_settings(&file, &settings).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(load_llm_settings(&file), settings, "it is saved");

    assert!(settings.remove_connection("office"));
    assert_eq!(
        settings.paired_summary_connection(),
        None,
        "removing the connection turns answering off"
    );
    assert_eq!(
        settings.set_paired_summary_connection(Some("other")),
        Ok(())
    );
    assert_eq!(settings.set_paired_summary_connection(None), Ok(()));
    assert_eq!(settings.paired_summary_connection(), None);
}

#[test]
fn with_the_keychain_turned_off_api_keys_never_reach_the_platform_keychain() {
    use super::llm_settings::{LlmKeyring, PlatformLlmKeyring};
    let directory = folder("llm-keychain-off");
    let keyring = PlatformLlmKeyring::new(directory.clone(), None).with_keychain_enabled(false);

    // The platform keychain would prompt or store; the switched-off keyring refuses instead.
    assert!(keyring.save("online", "sk-test-0123456789").is_err());
    assert!(matches!(keyring.load("online"), Ok(None) | Err(_)));
    assert!(
        !directory.join("llm-online.sealed").exists(),
        "no key is written without a sealing key"
    );
}

/// The chat prompt (#145) is the built-in one until the owner writes one; each prompt notes
/// when it last changed, a reset to the built-in prompt included, so paired devices can tell the
/// newest edit.
#[test]
fn the_chat_prompt_and_both_prompts_change_times_are_kept() {
    let file = folder("chat-prompt").join("settings.json");
    let mut settings = load_llm_settings(&file);
    assert_eq!(
        settings.chat_prompt().as_str(),
        crate::domain::shared_prompts::DEFAULT_CHAT_PROMPT
    );
    assert!(!settings.has_custom_chat_prompt());
    assert_eq!(settings.shared_prompts().chat.changed_at_ms, 0);

    let refused = settings.set_chat_prompt(Some("About {{title}}"));
    assert!(refused.is_err());
    settings
        .set_chat_prompt(Some("Brief answers about {{scope}}."))
        .unwrap_or_else(|e| panic!("{e}"));
    settings
        .set_template(Some("Only decisions."))
        .unwrap_or_else(|e| panic!("{e}"));
    save_llm_settings(&file, &settings).unwrap_or_else(|e| panic!("{e}"));

    let mut reloaded = load_llm_settings(&file);
    assert_eq!(
        reloaded.chat_prompt().as_str(),
        "Brief answers about {{scope}}."
    );
    let prompts = reloaded.shared_prompts();
    assert!(prompts.chat.changed_at_ms > 0);
    assert!(prompts.summary.changed_at_ms > 0);
    assert_eq!(prompts.summary.text.as_deref(), Some("Only decisions."));

    reloaded
        .set_chat_prompt(None)
        .unwrap_or_else(|e| panic!("{e}"));
    let reset = reloaded.shared_prompts().chat;
    assert_eq!(reset.text, None);
    assert!(reset.changed_at_ms >= prompts.chat.changed_at_ms);
}

/// Another device's newer prompts are written to the file, and the rest of the settings stay; a
/// damaged file is not overwritten, so its connections are not lost.
#[test]
fn a_paired_devices_prompts_merge_into_the_settings_file() {
    use super::llm_settings::FileSharedPrompts;
    use crate::{
        domain::shared_prompts::{PromptVersion, SharedPrompts},
        ports::shared_prompts::SharedPromptsSync,
    };

    let file = folder("merge-prompts").join("settings.json");
    let mut settings = SavedLlmSettings::default();
    settings.upsert_connection(local("office"));
    save_llm_settings(&file, &settings).unwrap_or_else(|e| panic!("{e}"));
    let store = FileSharedPrompts::new(file.clone());
    let incoming = SharedPrompts {
        summary: PromptVersion {
            text: Some("Decisions of {{title}}.".to_owned()),
            changed_at_ms: 5,
        },
        chat: PromptVersion {
            text: Some("One line about {{scope}}.".to_owned()),
            changed_at_ms: 6,
        },
    };

    assert_eq!(store.merge_shared_prompts(&incoming).ok(), Some(true));
    assert_eq!(store.merge_shared_prompts(&incoming).ok(), Some(false));
    let merged = load_llm_settings(&file);
    assert_eq!(merged.template().as_str(), "Decisions of {{title}}.");
    assert_eq!(merged.chat_prompt().as_str(), "One line about {{scope}}.");
    assert_eq!(merged.connections().len(), 1);
    assert_eq!(store.shared_prompts().ok(), Some(incoming.clone()));

    std::fs::write(&file, b"{ damaged").unwrap_or_else(|e| panic!("{e}"));
    assert!(store.merge_shared_prompts(&incoming).is_err());
    assert_eq!(
        std::fs::read(&file).unwrap_or_default(),
        b"{ damaged".to_vec()
    );
}
