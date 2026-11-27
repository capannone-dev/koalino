//! Specifies Settings → Developer (#187): a short list of app variables an expert can override,
//! validated, saved in the data folder, and applied at the next start ahead of the app's own
//! configuration and the environment. Secrets and storage paths are never on the list.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde_json::{Value, json};
use uuid::Uuid;

use crate::ffi_developer::{DEVELOPER_SETTINGS, load_overrides, run_developer_command};

/// A data folder removed when the test ends.
struct DataFolder(PathBuf);

impl DataFolder {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("developer-settings-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap_or_else(|error| panic!("{error}"));
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for DataFolder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs a command with nothing in effect and nothing loaded at start.
fn run(folder: &DataFolder, request: Value) -> Value {
    run_with(folder, request, &HashMap::new())
}

fn run_with(
    folder: &DataFolder,
    request: Value,
    at_start: &HashMap<&'static str, String>,
) -> Value {
    let answer = run_developer_command(folder.path(), &|_| None, at_start, &request.to_string());
    serde_json::from_str(&answer).unwrap_or_else(|error| panic!("{error}: {answer}"))
}

fn setting<'a>(answer: &'a Value, name: &str) -> &'a Value {
    answer["settings"]
        .as_array()
        .and_then(|settings| settings.iter().find(|setting| setting["name"] == name))
        .unwrap_or_else(|| panic!("{name} missing from {answer}"))
}

#[test]
fn the_list_never_offers_secrets_storage_paths_or_test_only_variables() {
    for setting in DEVELOPER_SETTINGS {
        let name = setting.name();
        for forbidden in [
            "PASSPHRASE",
            "SECRET",
            "_DIR",
            "DATABASE",
            "KEY",
            "DEVICE_ID",
            "CHECK_",
            "MEASURE_",
            "REPLAY_",
            "OPUS_",
            "TEST_",
        ] {
            assert!(!name.contains(forbidden), "{name} must not be overridable");
        }
        assert!(name.starts_with("NOTE_TAKER_"), "{name}");
        assert!(
            !setting.description().is_empty(),
            "{name} needs a description"
        );
    }
}

#[test]
fn the_section_starts_locked_and_unlocking_is_remembered() {
    let folder = DataFolder::new();
    let first = run(&folder, json!({"command": "get"}));
    assert_eq!(first["ok"], true);
    assert_eq!(first["unlocked"], false);
    assert_eq!(run(&folder, json!({"command": "unlock"}))["unlocked"], true);
    assert_eq!(run(&folder, json!({"command": "get"}))["unlocked"], true);
}

#[test]
fn a_value_is_validated_normalized_saved_and_loaded_at_the_next_start() {
    let folder = DataFolder::new();
    let locked = run(
        &folder,
        json!({"command": "set", "name": "NOTE_TAKER_VAD_ENABLED", "value": "off"}),
    );
    assert_eq!(locked["error_code"], "locked");

    run(&folder, json!({"command": "unlock"}));
    let saved = run(
        &folder,
        json!({"command": "set", "name": "NOTE_TAKER_VAD_ENABLED", "value": " OFF "}),
    );
    assert_eq!(saved["ok"], true, "{saved}");
    assert_eq!(setting(&saved, "NOTE_TAKER_VAD_ENABLED")["saved"], "false");
    assert_eq!(saved["restart_needed"], true);

    let loaded = load_overrides(folder.path());
    assert_eq!(
        loaded.get("NOTE_TAKER_VAD_ENABLED").map(String::as_str),
        Some("false")
    );

    // After that restart, the saved value is the one in effect: nothing left to restart for.
    let after_restart = run_with(&folder, json!({"command": "get"}), &loaded);
    assert_eq!(after_restart["restart_needed"], false);
}

#[test]
fn bad_values_and_unknown_names_are_refused_without_writing() {
    let folder = DataFolder::new();
    run(&folder, json!({"command": "unlock"}));
    for (name, value, code) in [
        ("NOTE_TAKER_VAD_ENABLED", "maybe", "invalid_value"),
        ("NOTE_TAKER_TEAMS_BRIDGE_PORT", "70000", "invalid_value"),
        ("NOTE_TAKER_TEAMS_BRIDGE_PORT", "80", "invalid_value"),
        (
            "NOTE_TAKER_TEAMS_BRIDGE_BUFFER_SECONDS",
            "0",
            "invalid_value",
        ),
        ("NOTE_TAKER_WHISPER_GPU", "maybe", "invalid_value"),
        ("NOTE_TAKER_WHISPER_LANGUAGE", "italian", "invalid_value"),
        ("NOTE_TAKER_P2P_LISTEN", "0.0.0.0:4001", "invalid_value"),
        ("NOTE_TAKER_ENCRYPTION_PASSPHRASE", "x", "unknown_setting"),
        ("NOTE_TAKER_DATABASE_PATH", "/tmp/x", "unknown_setting"),
    ] {
        let answer = run(
            &folder,
            json!({"command": "set", "name": name, "value": value}),
        );
        assert_eq!(answer["error_code"], code, "{name}={value}: {answer}");
    }
    assert!(load_overrides(folder.path()).is_empty());
    let good = run(
        &folder,
        json!({"command": "set", "name": "NOTE_TAKER_P2P_LISTEN", "value": "/ip4/0.0.0.0/tcp/4001"}),
    );
    assert_eq!(good["ok"], true, "{good}");
}

#[test]
fn clearing_one_reset_and_lock_remove_overrides() {
    let folder = DataFolder::new();
    run(&folder, json!({"command": "unlock"}));
    for (name, value) in [
        ("NOTE_TAKER_VAD_ENABLED", "false"),
        ("NOTE_TAKER_TEAMS_BRIDGE_PORT", "9000"),
        ("NOTE_TAKER_WHISPER_GPU", "off"),
    ] {
        run(
            &folder,
            json!({"command": "set", "name": name, "value": value}),
        );
    }
    let cleared = run(
        &folder,
        json!({"command": "set", "name": "NOTE_TAKER_VAD_ENABLED", "value": null}),
    );
    assert_eq!(
        setting(&cleared, "NOTE_TAKER_VAD_ENABLED")["saved"],
        Value::Null
    );
    assert_eq!(load_overrides(folder.path()).len(), 2);

    let reset = run(&folder, json!({"command": "reset"}));
    assert_eq!(reset["unlocked"], true);
    assert!(load_overrides(folder.path()).is_empty());

    run(
        &folder,
        json!({"command": "set", "name": "NOTE_TAKER_WHISPER_GPU", "value": "off"}),
    );
    let locked = run(&folder, json!({"command": "lock"}));
    assert_eq!(locked["unlocked"], false);
    assert!(load_overrides(folder.path()).is_empty());
}

#[test]
fn a_hand_edited_file_keeps_only_valid_allowed_values() {
    let folder = DataFolder::new();
    std::fs::write(
        folder.path().join("developer-settings.json"),
        r#"{"unlocked": true, "overrides": {
            "NOTE_TAKER_VAD_ENABLED": "no",
            "NOTE_TAKER_TEAMS_BRIDGE_PORT": "not a port",
            "NOTE_TAKER_ENCRYPTION_PASSPHRASE": "sneaky"
        }}"#,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let loaded = load_overrides(folder.path());
    assert_eq!(loaded.len(), 1, "{loaded:?}");
    assert_eq!(
        loaded.get("NOTE_TAKER_VAD_ENABLED").map(String::as_str),
        Some("false")
    );

    std::fs::write(folder.path().join("developer-settings.json"), "not json")
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(load_overrides(folder.path()).is_empty());
    assert_eq!(run(&folder, json!({"command": "get"}))["ok"], true);
}

#[test]
fn the_list_shows_each_value_in_effect() {
    let folder = DataFolder::new();
    let answer = run_developer_command(
        folder.path(),
        &|name| (name == "NOTE_TAKER_TEAMS_BRIDGE_PORT").then(|| "8800".to_owned()),
        &HashMap::new(),
        r#"{"command":"get"}"#,
    );
    let answer: Value = serde_json::from_str(&answer).unwrap_or_else(|error| panic!("{error}"));
    let port = setting(&answer, "NOTE_TAKER_TEAMS_BRIDGE_PORT");
    assert_eq!(port["in_effect"], "8800");
    assert_eq!(port["kind"], "integer");
    assert_eq!(port["default"], "8765");
    assert_eq!(
        setting(&answer, "NOTE_TAKER_VAD_ENABLED")["in_effect"],
        Value::Null
    );
}

#[test]
fn a_malformed_request_is_an_invalid_argument() {
    let folder = DataFolder::new();
    for request in ["", "[]", r#"{"command":"explode"}"#, r#"{"command":"set"}"#] {
        let answer = run_developer_command(folder.path(), &|_| None, &HashMap::new(), request);
        let answer: Value = serde_json::from_str(&answer).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(answer["error_code"], "invalid_argument", "{request}");
    }
}
