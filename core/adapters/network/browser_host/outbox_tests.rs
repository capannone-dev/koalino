//! Specifies the browser host's outbox (#135): frames Koalino hasn't received yet, kept sealed
//! on disk in order until they are delivered, whoever delivers them.

use std::{path::PathBuf, time::Duration};

use super::outbox::{
    OutboxKey, OutboxSession, app_is_running, browser_outbox_root, claim_stale_sessions,
    hold_app_running, try_delivery_lock,
};

/// A fresh empty folder.
fn folder() -> PathBuf {
    let folder = std::env::temp_dir().join(format!("outbox-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
    folder
}

fn key() -> OutboxKey {
    OutboxKey::from_bridge_secret("bridge-secret-for-tests")
}

/// All the bytes of all files under `folder`.
fn everything_on_disk(folder: &std::path::Path) -> Vec<u8> {
    let mut bytes = Vec::new();
    for entry in walk(folder) {
        bytes.extend(std::fs::read(entry).unwrap_or_default());
    }
    bytes
}

fn walk(folder: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(walk(&path));
        } else {
            files.push(path);
        }
    }
    files
}

#[test]
fn frames_come_back_in_order_and_are_sealed_on_disk() {
    let root = folder();
    let mut session =
        OutboxSession::create(&root, key(), 1 << 20).unwrap_or_else(|error| panic!("{error}"));
    assert!(session.push(b"meeting-frame", false));
    assert!(session.push(b"audio-one", true));
    assert!(session.push(b"audio-two", true));
    assert_eq!(session.len(), 3);

    let on_disk = everything_on_disk(&root);
    for plain in [&b"meeting-frame"[..], b"audio-one", b"audio-two"] {
        assert!(
            !on_disk.windows(plain.len()).any(|window| window == plain),
            "frames are encrypted at rest"
        );
    }

    let mut received = Vec::new();
    while let Some(frame) = session.front() {
        received.push(frame);
        session.remove_front();
    }
    assert_eq!(
        received,
        vec![
            b"meeting-frame".to_vec(),
            b"audio-one".to_vec(),
            b"audio-two".to_vec()
        ]
    );
    assert!(session.is_empty());
    let directory = session.directory().to_path_buf();
    session.finish();
    assert!(
        !directory.exists(),
        "an emptied session leaves nothing behind"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_full_outbox_drops_new_audio_but_keeps_meeting_frames() {
    let root = folder();
    let mut session =
        OutboxSession::create(&root, key(), 50).unwrap_or_else(|error| panic!("{error}"));
    assert!(session.push(b"0123456789", true));
    assert!(!session.push(b"0123456789abc", true), "past the cap");
    assert!(session.push(b"meeting ended", false), "never dropped");
    assert_eq!(session.len(), 2);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_session_left_behind_is_claimed_once_and_read_in_order() {
    let root = folder();
    let mut session =
        OutboxSession::create(&root, key(), 1 << 20).unwrap_or_else(|error| panic!("{error}"));
    for frame in [&b"a"[..], b"b", b"c"] {
        session.push(frame, true);
    }
    session.remove_front();
    drop(session); // the host exited without delivering "b" and "c"

    assert!(
        claim_stale_sessions(&root, Duration::from_secs(3600)).is_empty(),
        "a recent session may still belong to a running host"
    );
    let claimed = claim_stale_sessions(&root, Duration::ZERO);
    assert_eq!(claimed.len(), 1);
    assert!(
        claim_stale_sessions(&root, Duration::ZERO).is_empty(),
        "claimed sessions are not claimed again while their reader works"
    );

    let mut adopted =
        OutboxSession::adopt(claimed[0].clone(), key()).unwrap_or_else(|error| panic!("{error}"));
    let mut received = Vec::new();
    while let Some(frame) = adopted.front() {
        received.push(frame);
        adopted.remove_front();
    }
    assert_eq!(received, vec![b"b".to_vec(), b"c".to_vec()]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_frame_sealed_with_another_key_is_skipped_and_deleted() {
    let root = folder();
    let mut session =
        OutboxSession::create(&root, key(), 1 << 20).unwrap_or_else(|error| panic!("{error}"));
    session.push(b"secret", true);
    let directory = session.directory().to_path_buf();
    drop(session);
    let mut adopted =
        OutboxSession::adopt(directory, OutboxKey::from_bridge_secret("another secret"))
            .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(adopted.front(), None);
    assert!(adopted.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn one_deliverer_at_a_time_and_koalino_is_seen_running() {
    let root = folder();
    let first = try_delivery_lock(&root);
    assert!(first.is_some());
    assert!(
        try_delivery_lock(&root).is_none(),
        "a second deliverer waits"
    );
    drop(first);
    assert!(try_delivery_lock(&root).is_some());

    assert!(!app_is_running(&root));
    let running = hold_app_running(&root);
    assert!(running.is_some());
    assert!(app_is_running(&root));
    drop(running);
    assert!(!app_is_running(&root));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_outbox_lives_in_the_users_own_state_folder() {
    let lookup = |pairs: &'static [(&'static str, &'static str)]| {
        move |name: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    };
    assert_eq!(
        browser_outbox_root(lookup(&[("NOTE_TAKER_BROWSER_OUTBOX_DIR", "/x/outbox")])),
        Some(PathBuf::from("/x/outbox"))
    );
    if cfg!(windows) {
        assert_eq!(
            browser_outbox_root(lookup(&[("LOCALAPPDATA", r"C:\Users\u\AppData\Local")])),
            Some(
                PathBuf::from(r"C:\Users\u\AppData\Local")
                    .join("note_taker")
                    .join("browser-outbox")
            )
        );
    } else {
        assert_eq!(
            browser_outbox_root(lookup(&[("XDG_STATE_HOME", "/s"), ("HOME", "/h")])),
            Some(PathBuf::from("/s/note_taker/browser-outbox"))
        );
        assert_eq!(
            browser_outbox_root(lookup(&[("HOME", "/h")])),
            Some(PathBuf::from("/h/.local/state/note_taker/browser-outbox"))
        );
    }
    assert_eq!(browser_outbox_root(lookup(&[])), None);
}
