//! Specifies when and how the browser host starts Koalino (#135).

use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

use super::{
    app_launcher::{AppLauncher, should_start, sibling_app},
    outbox::hold_app_running,
};

fn folder() -> PathBuf {
    let folder = std::env::temp_dir().join(format!("launcher-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("{error}"));
    folder
}

#[test]
fn koalino_is_started_unless_it_runs_or_was_just_started() {
    let now = SystemTime::now();
    assert!(should_start(false, None, now));
    assert!(!should_start(true, None, now), "already running");
    assert!(
        !should_start(false, Some(now - Duration::from_secs(30)), now),
        "still starting"
    );
    assert!(should_start(
        false,
        Some(now - Duration::from_secs(600)),
        now
    ));
}

#[test]
fn the_app_is_the_one_next_to_the_host() {
    let bundle = folder();
    let host = bundle.join(if cfg!(windows) {
        "browser-host.exe"
    } else {
        "browser-host"
    });
    assert_eq!(sibling_app(&host), None, "no app installed beside it");
    let app = bundle.join(if cfg!(windows) {
        "note_taker_ui.exe"
    } else {
        "note_taker_ui"
    });
    std::fs::write(&app, b"").unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(sibling_app(&host), Some(app));
    let _ = std::fs::remove_dir_all(&bundle);
}

#[cfg(unix)]
#[test]
fn the_app_is_started_detached_once() {
    use std::os::unix::fs::PermissionsExt;

    let bundle = folder();
    let root = bundle.join("outbox");
    let marker = bundle.join("started");
    let app = bundle.join("note_taker_ui");
    std::fs::write(
        &app,
        format!("#!/bin/sh\necho started >> '{}'\n", marker.display()),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    std::fs::set_permissions(&app, std::fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|error| panic!("{error}"));
    let launcher = AppLauncher::new(app, root.clone());

    assert!(launcher.start_if_needed());
    assert!(!launcher.start_if_needed(), "not again while it starts");
    for _attempt in 0..100 {
        if marker.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        std::fs::read_to_string(&marker).unwrap_or_default(),
        "started\n"
    );

    let other_bundle = folder();
    let running = hold_app_running(&other_bundle);
    let launcher = AppLauncher::new(bundle.join("note_taker_ui"), other_bundle.clone());
    assert!(!launcher.start_if_needed(), "Koalino already runs");
    drop(running);
    let _ = std::fs::remove_dir_all(&bundle);
    let _ = std::fs::remove_dir_all(&other_bundle);
}
