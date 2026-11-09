//! Specifies the language model commands shared by the app (FFI) and the web API (#95): reading
//! and changing the settings, keeping keys out of every answer, testing a connection, and
//! writing a meeting's summary with a fake model.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde_json::{Value, json};
use zeroize::Zeroizing;

use super::{
    llm_commands::{LlmServices, run_llm_command, summarize_with},
    llm_settings::MemoryLlmKeyring,
    meeting_summaries::PreparedSummary,
};
use crate::{
    application::meeting_summary::{SpokenLine, SummaryMaterial},
    domain::{
        errors::LlmError,
        llm::{LlmCompletion, LlmConnection, LlmRequest, MeetingSummary},
    },
    ports::ai::LlmEngine,
};

/// Answers every request with a fixed text and keeps the keys it was built with.
struct EchoEngine(String);

impl LlmEngine for EchoEngine {
    fn complete(&self, _request: &LlmRequest) -> Result<LlmCompletion, LlmError> {
        Ok(LlmCompletion {
            text: self.0.clone(),
            truncated: false,
        })
    }
}

fn services(name: &str) -> (LlmServices, Arc<Mutex<Vec<Option<String>>>>) {
    let folder = std::env::temp_dir().join(format!("llm-commands-{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|e| panic!("{e}"));
    let keys_seen: Arc<Mutex<Vec<Option<String>>>> = Arc::default();
    let seen = Arc::clone(&keys_seen);
    let services = LlmServices::new(
        folder.join("db.sqlite3.llm-settings.json"),
        Arc::new(MemoryLlmKeyring::default()),
        "Laptop",
    )
    .with_transcription_settings(PathBuf::from("/nonexistent"))
    .with_engine_factory(Arc::new(move |_connection: &LlmConnection, key| {
        if let Ok(mut seen) = seen.lock() {
            seen.push(key.map(|key: Zeroizing<String>| key.as_str().to_owned()));
        }
        Ok(Box::new(EchoEngine("OK".to_owned())) as Box<dyn LlmEngine>)
    }));
    (services, keys_seen)
}

fn run(services: &LlmServices, request: Value) -> Value {
    serde_json::from_str(&run_llm_command(services, &request.to_string()))
        .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn connections_are_saved_listed_and_never_reveal_their_key() {
    let (services, keys_seen) = services("save");
    let empty = run(&services, json!({"command": "settings"}));
    assert_eq!(empty["ok"], true, "{empty}");
    assert_eq!(empty["connections"], json!([]));
    assert_eq!(empty["auto_summarize"], true);
    assert_eq!(empty["custom_template"], false);
    assert!(
        empty["placeholders"]
            .as_array()
            .is_some_and(|list| list.len() == 9)
    );

    let saved = run(
        &services,
        json!({"command": "save_connection", "label": "Claude", "kind": "anthropic",
               "base_url": "https://api.anthropic.com", "model": "claude-opus-5-5",
               "context_tokens": 200000, "api_key": "sk-ant-secret"}),
    );
    assert_eq!(saved["ok"], true, "{saved}");
    let id = saved["connection"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(saved["connection"]["has_key"], true);
    assert!(!saved.to_string().contains("sk-ant-secret"));

    let refused = run(
        &services,
        json!({"command": "save_connection", "label": "Bad", "kind": "open_ai_compatible",
               "base_url": "http://example.com/v1", "model": "m", "context_tokens": 8192}),
    );
    assert_eq!(refused["error_code"], "invalid_connection", "{refused}");

    let settings = run(&services, json!({"command": "settings"}));
    assert_eq!(settings["connections"][0]["id"], id.as_str());
    assert_eq!(settings["connections"][0]["is_default"], true);
    assert!(!settings.to_string().contains("sk-ant-secret"));

    let tested = run(&services, json!({"command": "test_connection", "id": id}));
    assert_eq!(tested["ok"], true, "{tested}");
    assert_eq!(tested["reply"], "OK");
    assert_eq!(
        keys_seen
            .lock()
            .map(|seen| seen.clone())
            .unwrap_or_default(),
        vec![Some("sk-ant-secret".to_owned())]
    );

    let edited = run(
        &services,
        json!({"command": "save_connection", "id": id, "label": "Claude work", "kind": "anthropic",
               "base_url": "https://api.anthropic.com", "model": "claude-sonnet-5-5",
               "context_tokens": 200000}),
    );
    assert_eq!(
        edited["connection"]["has_key"], true,
        "an edit keeps the key"
    );
    let cleared = run(
        &services,
        json!({"command": "save_connection", "id": id, "label": "Claude work", "kind": "anthropic",
               "base_url": "https://api.anthropic.com", "model": "claude-sonnet-5-5",
               "context_tokens": 200000, "clear_key": true}),
    );
    assert_eq!(cleared["connection"]["has_key"], false);

    let deleted = run(&services, json!({"command": "delete_connection", "id": id}));
    assert_eq!(deleted["deleted"], true);
    assert_eq!(
        run(&services, json!({"command": "delete_connection", "id": id}))["error_code"],
        "connection_not_found"
    );
}

#[test]
fn the_default_template_and_automatic_summaries_are_updated_only_when_named() {
    let (services, _) = services("update");
    let local = run(
        &services,
        json!({"command": "save_connection", "label": "LAN", "kind": "open_ai_compatible",
               "base_url": "http://192.168.1.5:11434/v1", "model": "llama", "context_tokens": 8192}),
    );
    let id = local["connection"]["id"].clone();
    assert_eq!(local["connection"]["plain_http"], true);

    let updated = run(
        &services,
        json!({"command": "update", "default_connection": id, "template": "Notes on {{title}}.",
               "auto_summarize": false}),
    );
    assert_eq!(updated["ok"], true, "{updated}");
    assert_eq!(updated["template"], "Notes on {{title}}.");
    assert_eq!(updated["custom_template"], true);
    assert_eq!(updated["auto_summarize"], false);

    let untouched = run(&services, json!({"command": "update"}));
    assert_eq!(untouched["template"], "Notes on {{title}}.");
    assert_eq!(untouched["default_connection"], id);

    let reset = run(&services, json!({"command": "update", "template": null}));
    assert_eq!(reset["custom_template"], false);

    let bad = run(
        &services,
        json!({"command": "update", "template": "{{nope}}"}),
    );
    assert_eq!(bad["error_code"], "invalid_template");
    let unknown = run(
        &services,
        json!({"command": "update", "default_connection": "missing"}),
    );
    assert_eq!(unknown["error_code"], "connection_not_found");
    assert_eq!(
        run(&services, json!({"command": "fly"}))["error_code"],
        "invalid_request"
    );
}

#[test]
fn a_summary_uses_the_default_connection_and_is_stored() {
    let (services, keys_seen) = services("summarize");
    let prepared = || PreparedSummary {
        material: SummaryMaterial {
            lines: vec![SpokenLine {
                speaker: "Anna".to_owned(),
                start_ms: 0,
                text: "We approve it.".to_owned(),
            }],
            ..SummaryMaterial::default()
        },
        meeting_prompt: None,
    };
    let none = summarize_with(&services, None, |_, _| Ok(prepared()), |_| Ok(()));
    assert_eq!(none["error_code"], "no_connection", "{none}");

    run(
        &services,
        json!({"command": "save_connection", "label": "LAN", "kind": "open_ai_compatible",
               "base_url": "http://127.0.0.1:8080/v1", "model": "llama", "context_tokens": 8192}),
    );
    let stored: Mutex<Option<MeetingSummary>> = Mutex::new(None);
    let answer = summarize_with(
        &services,
        None,
        |glossary, device| {
            assert!(glossary.is_empty());
            assert_eq!(device, "Laptop");
            Ok(prepared())
        },
        |summary| {
            if let Ok(mut slot) = stored.lock() {
                *slot = Some(summary.clone());
            }
            Ok(())
        },
    );

    assert_eq!(answer["ok"], true, "{answer}");
    assert_eq!(answer["summary"]["text"], "OK");
    assert_eq!(answer["summary"]["model"], "llama");
    assert_eq!(answer["parts"], 1);
    assert_eq!(
        stored
            .lock()
            .ok()
            .and_then(|slot| slot.clone())
            .map(|summary| summary.text),
        Some("OK".to_owned())
    );
    assert_eq!(
        keys_seen
            .lock()
            .map(|seen| seen.clone())
            .unwrap_or_default(),
        vec![None]
    );
}
