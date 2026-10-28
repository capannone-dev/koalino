//! Specifies the daemon's live log level behind the web Settings page (Issue #46).

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::bail;

use super::LiveLogControl;
use crate::application::log_control::{LogControl, LogControlError};

/// A control whose reloads are recorded, and fail when `fails` is set.
fn control(fails: bool) -> (LiveLogControl, Arc<Mutex<Vec<String>>>) {
    let reloads = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&reloads);
    let control = LiveLogControl::new(
        "info",
        PathBuf::from("/var/log/note_taker"),
        Box::new(move |directive: &str| {
            if fails {
                bail!("the subscriber is gone");
            }
            recorded
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(directive.to_owned());
            Ok(())
        }),
    );
    (control, reloads)
}

/// The directives passed to the reloader.
fn reloaded(reloads: &Mutex<Vec<String>>) -> Vec<String> {
    reloads
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// Verifies the start level and the log directory are reported.
#[test]
fn reports_the_start_level_and_directory() {
    let (control, _) = control(false);

    assert_eq!(control.current_level(), "info");
    assert_eq!(
        control.log_directory(),
        PathBuf::from("/var/log/note_taker")
    );
}

/// Verifies a valid level is applied trimmed and then reported.
#[test]
fn a_valid_level_is_applied() {
    let (control, reloads) = control(false);

    let applied = control
        .set_level(" debug ")
        .unwrap_or_else(|error| panic!("debug must be accepted: {error}"));

    assert_eq!(applied, "debug");
    assert_eq!(control.current_level(), "debug");
    assert_eq!(reloaded(&reloads), vec!["debug".to_owned()]);
}

/// Verifies an invalid level is refused before reaching the subscriber.
#[test]
fn an_invalid_level_is_refused() {
    let (control, reloads) = control(false);

    for invalid in ["", "info\nerror", "[broken"] {
        assert!(matches!(
            control.set_level(invalid),
            Err(LogControlError::InvalidLevel(_))
        ));
    }
    assert!(reloaded(&reloads).is_empty());
    assert_eq!(control.current_level(), "info");
}

/// Verifies a failed reload keeps the old level.
#[test]
fn a_failed_reload_keeps_the_old_level() {
    let (control, _) = control(true);

    assert!(matches!(
        control.set_level("trace"),
        Err(LogControlError::Unavailable(_))
    ));
    assert_eq!(control.current_level(), "info");
}
