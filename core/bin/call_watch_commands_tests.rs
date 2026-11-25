//! Specifies the call watch's Settings and polls for the app (#111).

use serde_json::json;

use super::call_watch_commands::{
    CallWatchSettings, CallWatcher, browser_meeting_active, set_browser_meeting_active,
    toast_command,
};
use crate::{
    adapters::{
        audio::microphone_users::MicrophoneUsersError, notification::call_toast::CallToastAnswer,
    },
    domain::call_watch::MicrophoneUser,
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

#[test]
fn a_poll_can_carry_the_users_the_host_saw() {
    // On Android the system answer comes from Kotlin, so the poll carries it (#111).
    let folder = folder();
    let mut watcher = CallWatcher::new(CallWatchSettings::file_for(&folder.join("n.sqlite3")));
    let poll =
        r#"{"command":"poll","users":[{"process":"android:voice-communication","name":""}]}"#;
    let mut asked_the_system = false;
    let mut system = || {
        asked_the_system = true;
        Err(MicrophoneUsersError::Unavailable)
    };
    let first = watcher.command(poll, &mut system);
    assert_eq!(first["available"], true);
    assert_eq!(first["in_call"], false);
    assert!(!asked_the_system, "the host's users replace the system's");

    let empty = watcher.command(r#"{"command":"poll","users":[]}"#, &mut || {
        Err(MicrophoneUsersError::Unavailable)
    });
    assert_eq!(empty["available"], true, "no users is an answer too");

    let bad = watcher.command(r#"{"command":"poll","users":"teams"}"#, &mut || {
        Ok(Vec::new())
    });
    assert_eq!(bad["error_code"], "invalid_request");
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn the_toast_offers_for_an_app_and_withdraws() {
    let mut offered = Vec::new();
    let mut withdrawn = 0;
    let answer = toast_command(
        r#"{"command":"offer","app":"Teams"}"#,
        &mut |app| {
            offered.push(app.to_owned());
            CallToastAnswer::Record
        },
        &mut || withdrawn += 1,
    );
    assert_eq!(answer, json!({"ok": true, "answer": "record"}));
    assert_eq!(offered, ["Teams"]);

    let answer = toast_command(
        r#"{"command":"withdraw"}"#,
        &mut |_| CallToastAnswer::Record,
        &mut || withdrawn += 1,
    );
    assert_eq!(answer, json!({"ok": true}));
    assert_eq!(withdrawn, 1);

    for bad in [
        r#"{"command":"offer"}"#,
        r#"{"command":"offer","app":""}"#,
        "{",
    ] {
        let answer = toast_command(bad, &mut |_| CallToastAnswer::Record, &mut || {});
        assert_eq!(answer["error_code"], "invalid_request", "{bad}");
    }
}

#[test]
fn polls_say_whether_the_browser_extension_records_a_meeting() {
    let folder = folder();
    let mut watcher = CallWatcher::new(CallWatchSettings::file_for(&folder.join("n.sqlite3")));
    watcher.poll_at_with_browser(0, Ok(zoom()), false);
    let started = watcher.poll_at_with_browser(4_000, Ok(zoom()), true);
    // The call is still noticed; the app decides not to offer it again (#174).
    assert_eq!(started["event"], "started");
    assert_eq!(started["browser_recording"], true);
    let later = watcher.poll_at_with_browser(8_000, Ok(zoom()), false);
    assert_eq!(later["browser_recording"], false);
    let unavailable =
        watcher.poll_at_with_browser(9_000, Err(MicrophoneUsersError::Unavailable), true);
    assert_eq!(unavailable["browser_recording"], true);
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn the_bridge_marks_a_browser_meeting_for_the_polls() {
    // The only test that touches the process-wide mark, so parallel tests do not race on it.
    set_browser_meeting_active(true);
    assert!(browser_meeting_active());
    set_browser_meeting_active(false);
    assert!(!browser_meeting_active());
}
