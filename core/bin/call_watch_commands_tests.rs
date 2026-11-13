//! Specifies the call watch's Settings and polls for the app (#111).

use serde_json::json;

use super::call_watch_commands::{CallWatchSettings, CallWatcher};
use crate::{
    adapters::audio::microphone_users::MicrophoneUsersError, domain::call_watch::MicrophoneUser,
};

fn zoom() -> Vec<MicrophoneUser> {
    vec![MicrophoneUser {
        process: String::from("zoom"),
        name: String::new(),
        pid: Some(5),
    }]
}

fn folder() -> std::path::PathBuf {
    let folder = std::env::temp_dir().join(format!("call-watch-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
    folder
}

#[test]
fn settings_are_on_by_default_and_saved_beside_the_database() {
    let folder = folder();
    let file = CallWatchSettings::file_for(&folder.join("n.sqlite3"));
    assert!(
        file.to_string_lossy()
            .ends_with("n.sqlite3.call-watch.json")
    );
    assert_eq!(
        CallWatchSettings::load(&file),
        CallWatchSettings {
            offer: true,
            auto_stop: true,
            read_names: true,
        }
    );
    let mut watcher = CallWatcher::new(file.clone());
    let answer = watcher.command(r#"{"command":"set","auto_stop":false}"#, &mut || Ok(zoom()));
    assert_eq!(answer["ok"], true);
    assert_eq!(answer["offer"], true);
    assert_eq!(answer["auto_stop"], false);
    assert!(!CallWatchSettings::load(&file).auto_stop);
    let answer = watcher.command(
        r#"{"command":"set","read_names":false}"#,
        &mut || Ok(zoom()),
    );
    assert_eq!(answer["read_names"], false);
    assert_eq!(answer["auto_stop"], false);
    assert!(!CallWatchSettings::load(&file).read_names);
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn polls_report_the_start_and_the_end_of_a_call_once() {
    let folder = folder();
    let mut watcher = CallWatcher::new(CallWatchSettings::file_for(&folder.join("n.sqlite3")));
    let poll = r#"{"command":"poll"}"#;
    let first = watcher.poll_at(0, Ok(zoom()));
    assert_eq!(first["available"], true);
    assert_eq!(first["in_call"], false);
    assert_eq!(first["event"], serde_json::Value::Null);
    let started = watcher.poll_at(4_000, Ok(zoom()));
    assert_eq!(started["event"], "started");
    assert_eq!(started["app"], "Zoom");
    assert_eq!(started["in_call"], true);
    assert_eq!(
        watcher.poll_at(8_000, Ok(zoom()))["event"],
        serde_json::Value::Null
    );
    let ended = watcher.poll_at(40_000, Ok(Vec::new()));
    assert_eq!(ended["event"], "ended");
    assert_eq!(ended["app"], "Zoom");
    assert_eq!(ended["in_call"], false);
    // The command form polls the real clock; it answers in the same shape.
    let answer = watcher.command(poll, &mut || Ok(Vec::new()));
    assert_eq!(answer["ok"], true);
    assert_eq!(answer["in_call"], false);
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn a_failed_look_in_a_call_does_not_end_it() {
    let folder = folder();
    let mut watcher = CallWatcher::new(CallWatchSettings::file_for(&folder.join("n.sqlite3")));
    watcher.poll_at(0, Ok(zoom()));
    assert_eq!(watcher.poll_at(4_000, Ok(zoom()))["event"], "started");
    for at in [30_000, 60_000] {
        watcher.poll_at(at, Err(MicrophoneUsersError::Tool(String::from("busy"))));
    }
    assert_eq!(watcher.poll_at(62_000, Ok(zoom()))["in_call"], true);
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn an_unavailable_system_says_so() {
    let folder = folder();
    let mut watcher = CallWatcher::new(CallWatchSettings::file_for(&folder.join("n.sqlite3")));
    let answer = watcher.poll_at(0, Err(MicrophoneUsersError::Unavailable));
    assert_eq!(answer["ok"], true);
    assert_eq!(answer["available"], false);
    let failed = watcher.poll_at(0, Err(MicrophoneUsersError::Tool(String::from("x"))));
    assert_eq!(
        failed["ok"], false,
        "a failed look is not an unavailable system"
    );
    assert_eq!(failed["error_code"], "probe_failed");
    let bad = watcher.command("{", &mut || Ok(Vec::new()));
    assert_eq!(
        bad,
        json!({"ok": false, "error_code": "invalid_request",
                            "error": "the command must be poll, get or set"})
    );
    let _ = std::fs::remove_dir_all(&folder);
}
