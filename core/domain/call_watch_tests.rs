//! Specifies which apps count as calls and when a call starts and ends (#111).

use super::call_watch::{CallEvent, CallWatch, MicrophoneUser, call_app};

fn user(process: &str, name: &str, pid: u32) -> MicrophoneUser {
    MicrophoneUser {
        process: process.to_owned(),
        name: name.to_owned(),
        pid: Some(pid),
    }
}

#[test]
fn call_apps_and_browsers_are_named_and_others_are_not() {
    assert_eq!(call_app("zoom", "ZOOM VoiceEngine"), Some("Zoom"));
    assert_eq!(call_app("ms-teams.exe", ""), Some("Teams"));
    assert_eq!(call_app("teams-for-linux", ""), Some("Teams"));
    assert_eq!(call_app("MSTeams_8wekyb3d8bbwe", ""), Some("Teams"));
    assert_eq!(call_app("firefox-esr", "Firefox"), Some("Firefox"));
    assert_eq!(call_app("chrome.exe", ""), Some("Chrome"));
    assert_eq!(call_app("Discord", ""), Some("Discord"));
    assert_eq!(call_app("slack", ""), Some("Slack"));
    assert_eq!(call_app("webex", ""), Some("Webex"));
    assert_eq!(call_app("pacat", "parecord"), None);
    assert_eq!(call_app("audacity", ""), None);
    assert_eq!(call_app("note_taker_ui", ""), None);
}

#[test]
fn a_call_starts_after_it_holds_the_microphone_for_a_moment() {
    let mut watch = CallWatch::new();
    let zoom = [user("zoom", "ZOOM VoiceEngine", 10)];
    assert_eq!(watch.observe(0, &zoom, None), None);
    assert_eq!(watch.in_call(), None);
    assert_eq!(
        watch.observe(3_000, &zoom, None),
        Some(CallEvent::Started { app: "Zoom" })
    );
    assert_eq!(watch.in_call(), Some("Zoom"));
    assert_eq!(watch.observe(6_000, &zoom, None), None, "started only once");
}

#[test]
fn a_blip_of_the_microphone_is_not_a_call() {
    let mut watch = CallWatch::new();
    assert_eq!(watch.observe(0, &[user("chrome", "", 4)], None), None);
    assert_eq!(watch.observe(2_000, &[], None), None);
    assert_eq!(watch.observe(4_000, &[user("chrome", "", 4)], None), None);
    assert_eq!(watch.in_call(), None);
}

#[test]
fn a_call_ends_after_the_microphone_stays_free_for_the_grace_period() {
    let mut watch = CallWatch::new();
    let teams = [user("ms-teams", "", 7)];
    watch.observe(0, &teams, None);
    watch.observe(3_000, &teams, None);
    // A short gap (muting, switching devices) does not end it.
    assert_eq!(watch.observe(6_000, &[], None), None);
    assert_eq!(watch.observe(10_000, &teams, None), None);
    assert_eq!(watch.observe(13_000, &[], None), None);
    assert_eq!(watch.observe(29_000, &[], None), None);
    assert_eq!(
        watch.observe(30_000, &[], None),
        Some(CallEvent::Ended { app: "Teams" })
    );
    assert_eq!(watch.in_call(), None);
}

#[test]
fn this_app_never_counts_even_when_named_like_a_browser() {
    let mut watch = CallWatch::new();
    let ourselves = [user("note_taker_ui", "", 99), user("chrome", "", 99)];
    watch.observe(0, &ourselves, Some(99));
    assert_eq!(watch.observe(5_000, &ourselves, Some(99)), None);
}

#[test]
fn another_call_app_keeps_the_call_going() {
    let mut watch = CallWatch::new();
    watch.observe(0, &[user("zoom", "", 1)], None);
    watch.observe(3_000, &[user("zoom", "", 1)], None);
    assert_eq!(watch.observe(10_000, &[user("firefox", "", 2)], None), None);
    assert_eq!(watch.observe(40_000, &[user("firefox", "", 2)], None), None);
    assert_eq!(watch.in_call(), Some("Zoom"));
}
