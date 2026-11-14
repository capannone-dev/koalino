//! Specifies the language model commands shared by the app (FFI) and the web API (#95): reading
//! and changing the settings, keeping no API key anywhere (#117), testing a connection, and
//! writing a meeting's summary with a fake model.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde_json::{Value, json};

use super::{
    llm_commands::{
        LlmServices, new_summary_request, run_llm_command, summarize_with, summary_request_json,
    },
    meeting_summaries::PreparedSummary,
};
use crate::{
    application::meeting_summary::{SpokenLine, SummaryMaterial},
    domain::{
        errors::LlmError,
        llm::{
            LlmCompletion, LlmConnection, LlmRequest, MeetingSummary, SummaryRequest,
            SummaryRequestState,
        },
    },
    ports::ai::LlmEngine,
};

/// Answers every request with a fixed text.
struct EchoEngine(String);

impl LlmEngine for EchoEngine {
    fn complete(&self, _request: &LlmRequest) -> Result<LlmCompletion, LlmError> {
        Ok(LlmCompletion {
            text: self.0.clone(),
            truncated: false,
        })
    }
}

fn services(name: &str) -> (LlmServices, Arc<Mutex<Vec<String>>>) {
    let folder = std::env::temp_dir().join(format!("llm-commands-{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|e| panic!("{e}"));
    let built: Arc<Mutex<Vec<String>>> = Arc::default();
    let seen = Arc::clone(&built);
    let services = LlmServices::new(folder.join("db.sqlite3.llm-settings.json"), "Laptop")
        .with_transcription_settings(PathBuf::from("/nonexistent"))
        .with_engine_factory(Arc::new(move |connection: &LlmConnection| {
            if let Ok(mut seen) = seen.lock() {
                seen.push(connection.id().to_owned());
            }
            Ok(Box::new(EchoEngine("OK".to_owned())) as Box<dyn LlmEngine>)
        }));
    (services, built)
}

fn run(services: &LlmServices, request: Value) -> Value {
    serde_json::from_str(&run_llm_command(services, &request.to_string()))
        .unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn connections_are_saved_listed_and_no_api_key_is_kept() {
    let (services, built) = services("save");
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
        json!({"command": "save_connection", "label": "Office", "kind": "open_ai_compatible",
               "base_url": "http://192.168.1.20:11434/v1", "model": "llama3.1:8b",
               "context_tokens": 32768, "api_key": "sk-ant-secret"}),
    );
    assert_eq!(saved["ok"], true, "{saved}");
    let id = saved["connection"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(saved["connection"].get("has_key").is_none());
    assert!(!saved.to_string().contains("sk-ant-secret"));

    let anthropic = run(
        &services,
        json!({"command": "save_connection", "label": "Claude", "kind": "anthropic",
               "base_url": "https://api.anthropic.com", "model": "claude-opus-5-5",
               "context_tokens": 200000}),
    );
    assert_eq!(
        anthropic["error_code"], "invalid_request",
        "the keyed Anthropic kind is gone: {anthropic}"
    );

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
    let on_disk = std::fs::read_to_string(services_settings_file(&services)).unwrap_or_default();
    assert!(
        !on_disk.contains("sk-ant-secret"),
        "a sent key is not stored"
    );

    let tested = run(&services, json!({"command": "test_connection", "id": id}));
    assert_eq!(tested["ok"], true, "{tested}");
    assert_eq!(tested["reply"], "OK");
    assert_eq!(
        built.lock().map(|seen| seen.clone()).unwrap_or_default(),
        vec![id.clone()]
    );

    let edited = run(
        &services,
        json!({"command": "save_connection", "id": id, "label": "Office work", "kind": "open_ai_compatible",
               "base_url": "http://192.168.1.20:11434/v1", "model": "qwen3:8b",
               "context_tokens": 32768}),
    );
    assert_eq!(edited["connection"]["label"], "Office work", "{edited}");

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
fn the_connection_that_answers_paired_devices_is_chosen_and_cleared() {
    let (services, _) = services("paired");
    assert_eq!(
        run(&services, json!({"command": "settings"}))["paired_summary_connection"],
        Value::Null,
        "off by default"
    );
    let saved = run(
        &services,
        json!({"command": "save_connection", "label": "Claude", "kind": "claude_code"}),
    );
    let id = saved["connection"]["id"].clone();
    let chosen = run(
        &services,
        json!({"command": "update", "paired_summary_connection": id}),
    );
    assert_eq!(chosen["paired_summary_connection"], id, "{chosen}");
    let untouched = run(&services, json!({"command": "update"}));
    assert_eq!(untouched["paired_summary_connection"], id);
    let unknown = run(
        &services,
        json!({"command": "update", "paired_summary_connection": "missing"}),
    );
    assert_eq!(unknown["error_code"], "connection_not_found");
    let off = run(
        &services,
        json!({"command": "update", "paired_summary_connection": null}),
    );
    assert_eq!(off["paired_summary_connection"], Value::Null);
}

#[test]
fn a_summary_uses_the_default_connection_and_is_stored() {
    let (services, built) = services("summarize");
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
    assert_eq!(built.lock().map(|seen| seen.len()).unwrap_or_default(), 1);
}

/// Rejects every request the way Anthropic does when the account has no credit left.
struct RejectingEngine;

impl LlmEngine for RejectingEngine {
    fn complete(&self, _request: &LlmRequest) -> Result<LlmCompletion, LlmError> {
        Err(LlmError::Rejected {
            status: 400,
            provider_reason: Some(
                "invalid_request_error: Your credit balance is too low".to_owned(),
            ),
        })
    }
}

#[test]
fn a_rejected_summary_tells_the_owner_the_servers_reason_apart_from_the_error_text() {
    let (services, _) = services("rejected");
    let services = services.with_engine_factory(Arc::new(|_connection: &LlmConnection| {
        Ok(Box::new(RejectingEngine) as Box<dyn LlmEngine>)
    }));
    run(
        &services,
        json!({"command": "save_connection", "label": "LAN", "kind": "open_ai_compatible",
               "base_url": "http://127.0.0.1:8080/v1", "model": "llama", "context_tokens": 8192}),
    );

    let answer = summarize_with(
        &services,
        None,
        |_, _| {
            Ok(PreparedSummary {
                material: SummaryMaterial {
                    lines: vec![SpokenLine {
                        speaker: "Anna".to_owned(),
                        start_ms: 0,
                        text: "We approve it.".to_owned(),
                    }],
                    ..SummaryMaterial::default()
                },
                meeting_prompt: None,
            })
        },
        |_| Ok(()),
    );

    assert_eq!(answer["error_code"], "llm_rejected", "{answer}");
    assert_eq!(
        answer["error_detail"],
        "invalid_request_error: Your credit balance is too low"
    );
    assert!(
        !answer["error"]
            .as_str()
            .unwrap_or_default()
            .contains("credit")
    );
}

#[test]
fn a_claude_code_connection_needs_no_address_or_key_and_says_whether_the_tool_is_installed() {
    let (services, _) = services("claude-code");
    let saved = run(
        &services,
        json!({"command": "save_connection", "label": "Claude Pro", "kind": "claude_code",
               "context_tokens": 200000, "api_key": "sk-ignored"}),
    );
    let connection = &saved["connection"];
    assert_eq!(connection["kind"], "claude_code", "{saved}");
    assert_eq!(connection["base_url"], "");
    assert_eq!(connection["model"], "");
    assert!(connection.get("has_key").is_none());
    assert!(connection["tool_installed"].is_boolean());
}

/// Fails the way Claude Code does when it is signed out.
struct SignedOutTool;

impl LlmEngine for SignedOutTool {
    fn complete(&self, _request: &LlmRequest) -> Result<LlmCompletion, LlmError> {
        Err(LlmError::ToolFailed {
            reason: Some("Not logged in · Please run /login".to_owned()),
        })
    }
}

#[test]
fn a_tool_failure_has_its_own_code_and_what_the_tool_said() {
    let (services, _) = services("tool-failed");
    let services = services.with_engine_factory(Arc::new(|_connection: &LlmConnection| {
        Ok(Box::new(SignedOutTool) as Box<dyn LlmEngine>)
    }));
    let saved = run(
        &services,
        json!({"command": "save_connection", "label": "Claude Pro", "kind": "claude_code",
               "context_tokens": 200000}),
    );
    let id = saved["connection"]["id"].clone();

    let answer = run(&services, json!({"command": "test_connection", "id": id}));

    assert_eq!(answer["error_code"], "llm_tool_failed", "{answer}");
    assert_eq!(answer["error_detail"], "Not logged in · Please run /login");
}

/// Saves Claude Code and Codex connections and tests them through the real tools on this
/// computer, as the app's Test button does. Ignored: it needs both tools signed in and uses
/// their plans. Run it by hand with `cargo test --lib real_tool_connections -- --ignored`.
#[test]
#[ignore = "runs the installed Claude Code and Codex"]
fn real_tool_connections_answer_the_test_request() {
    let folder = std::env::temp_dir().join(format!("llm-real-tools-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|e| panic!("{e}"));
    let services = LlmServices::new(folder.join("db.sqlite3.llm-settings.json"), "Laptop");
    for kind in ["claude_code", "codex"] {
        let saved = run(
            &services,
            json!({"command": "save_connection", "label": kind, "kind": kind,
                   "context_tokens": 200000}),
        );
        assert_eq!(saved["connection"]["tool_installed"], true, "{saved}");
        let id = saved["connection"]["id"].clone();
        let answer = run(&services, json!({"command": "test_connection", "id": id}));
        println!("{kind}: {answer}");
        assert_eq!(answer["ok"], true, "{kind}: {answer}");
    }
    let _ = std::fs::remove_dir_all(folder);
}

/// Where the services keep their settings file.
fn services_settings_file(services: &LlmServices) -> PathBuf {
    services.settings_file().to_path_buf()
}

#[test]
fn a_request_for_the_computer_carries_this_devices_template_only_when_the_owner_wrote_one() {
    let (services, _) = services("request");
    let at = chrono::DateTime::parse_from_rfc3339("2026-11-14T10:00:00Z")
        .map(|time| time.with_timezone(&chrono::Utc))
        .unwrap_or_default();
    let request = new_summary_request(&services, "12D3KooWPhone", at);
    assert_eq!(
        request,
        SummaryRequest {
            requested_by: "12D3KooWPhone".to_owned(),
            requested_at: at,
            template: None,
            state: SummaryRequestState::Waiting,
        },
        "the built-in template: the computer uses its own"
    );
    run(
        &services,
        json!({"command": "update", "template": "Only decisions of {{title}}."}),
    );
    assert_eq!(
        new_summary_request(&services, "12D3KooWPhone", at)
            .template
            .as_deref(),
        Some("Only decisions of {{title}}.")
    );
}

#[test]
fn a_request_is_described_for_the_app_by_its_state() {
    let at = chrono::DateTime::parse_from_rfc3339("2026-11-14T10:00:00Z")
        .map(|time| time.with_timezone(&chrono::Utc))
        .unwrap_or_default();
    let request = |state| SummaryRequest {
        requested_by: "phone".to_owned(),
        requested_at: at,
        template: Some("secret template text".to_owned()),
        state,
    };
    assert_eq!(
        summary_request_json(&request(SummaryRequestState::Waiting)),
        json!({"state": "waiting", "requested_at": "2026-11-14T10:00:00+00:00"})
    );
    assert_eq!(
        summary_request_json(&request(SummaryRequestState::Working {
            by: "laptop".to_owned(),
            since: at,
        })),
        json!({"state": "working", "requested_at": "2026-11-14T10:00:00+00:00",
               "by": "laptop", "since": "2026-11-14T10:00:00+00:00"})
    );
    assert_eq!(
        summary_request_json(&request(SummaryRequestState::Failed {
            by: "laptop".to_owned(),
            error_code: "llm_tool_failed".to_owned(),
            at,
        })),
        json!({"state": "failed", "requested_at": "2026-11-14T10:00:00+00:00",
               "by": "laptop", "error_code": "llm_tool_failed",
               "failed_at": "2026-11-14T10:00:00+00:00"})
    );
}
