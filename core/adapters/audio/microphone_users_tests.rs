//! Specifies reading the microphone's users from PulseAudio/PipeWire and from Windows' consent
//! store (#111), on captured command output.

use super::microphone_users::{parse_consent_store, parse_pactl};
use crate::domain::call_watch::MicrophoneUser;

const SOURCES: &str = r#"[{"index":55,"name":"alsa_output.pci-0000_00_1b.0.analog-stereo.monitor"},
{"index":56,"name":"alsa_input.pci-0000_00_1b.0.analog-stereo"}]"#;

fn output(index: u32, source: u32, corked: bool, binary: &str, name: &str, pid: &str) -> String {
    format!(
        r#"{{"index":{index},"source":{source},"corked":{corked},"properties":{{
            "application.name":"{name}","application.process.binary":"{binary}",
            "application.process.id":"{pid}","media.name":"capture"}}}}"#
    )
}

#[test]
fn pactl_capture_streams_on_real_sources_are_users() {
    let outputs = format!(
        "[{},{},{}]",
        output(1, 56, false, "zoom", "ZOOM VoiceEngine", "4242"),
        output(2, 56, true, "firefox", "Firefox", "77"),
        output(3, 55, false, "obs", "OBS", "88"),
    );
    assert_eq!(
        parse_pactl(&outputs, SOURCES),
        vec![MicrophoneUser {
            process: String::from("zoom"),
            name: String::from("ZOOM VoiceEngine"),
            pid: Some(4242),
        }],
        "a paused (corked) stream and a stream on a monitor are not microphone use"
    );
}

#[test]
fn unreadable_pactl_output_gives_no_users() {
    assert!(parse_pactl("not json", SOURCES).is_empty());
    assert!(parse_pactl("[]", "[]").is_empty());
}

const STORE: &str = r"
HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone
    Value    REG_SZ    Allow

HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone\MSTeams_8wekyb3d8bbwe
    Value    REG_SZ    Allow
    LastUsedTimeStart    REG_QWORD    0x1da1c8e3a5b7e2f
    LastUsedTimeStop    REG_QWORD    0x0

HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone\NonPackaged
    Value    REG_SZ    Allow

HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone\NonPackaged\C:#Program Files#Zoom#bin#Zoom.exe
    LastUsedTimeStart    REG_QWORD    0x1da1c8e3a5b7e2f
    LastUsedTimeStop    REG_QWORD    0x1da1c8f0000000a

HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone\NonPackaged\C:#Users#utente#app#note_taker_ui.exe
    LastUsedTimeStart    REG_QWORD    0x1da1c8e3a5b7e2f
    LastUsedTimeStop    REG_QWORD    0x0

HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone\NonPackaged\C:#Program Files#Never#never.exe
    LastUsedTimeStart    REG_QWORD    0x0
    LastUsedTimeStop    REG_QWORD    0x0

End of search: 12 match(es) found.
";

#[test]
fn consent_store_entries_started_and_not_stopped_are_users() {
    let users = parse_consent_store(STORE);
    let processes: Vec<&str> = users.iter().map(|user| user.process.as_str()).collect();
    assert_eq!(processes, ["MSTeams_8wekyb3d8bbwe", "note_taker_ui.exe"]);
    assert!(users.iter().all(|user| user.pid.is_none()));
}

#[test]
fn windows_line_endings_are_read_too() {
    let users = parse_consent_store(&STORE.replace('\n', "\r\n"));
    assert_eq!(users.len(), 2);
}
