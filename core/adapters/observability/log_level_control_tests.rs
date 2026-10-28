//! Specifies the file through which `cli log-level` changes a running daemon's verbosity.

use std::{fs, path::PathBuf};

use uuid::Uuid;

use super::{LogLevelControlFile, LogLevelWatcher};

/// Returns a database path in a fresh temp directory, plus that directory for cleanup.
fn scratch_database() -> (PathBuf, PathBuf) {
    let directory = std::env::temp_dir().join(format!("note-taker-log-level-{}", Uuid::new_v4()));
    fs::create_dir_all(&directory)
        .unwrap_or_else(|error| panic!("scratch directory must be created: {error}"));
    (directory.join("note_taker.sqlite3"), directory)
}

/// Verifies the control file sits beside the database, like the recording control files.
#[test]
fn the_control_file_sits_beside_the_database() {
    let (database_path, directory) = scratch_database();

    assert_eq!(
        LogLevelControlFile::for_database(&database_path).path(),
        directory.join("note_taker.sqlite3.log-level")
    );
    let _ = fs::remove_dir_all(directory);
}

/// Verifies no request reads as `None`, and a request round-trips trimmed.
#[test]
fn a_request_is_read_back() {
    let (database_path, directory) = scratch_database();
    let control_file = LogLevelControlFile::for_database(&database_path);

    assert_eq!(control_file.read().ok(), Some(None));
    control_file
        .request("  debug ")
        .unwrap_or_else(|error| panic!("a valid level must be accepted: {error:#}"));
    assert_eq!(control_file.read().ok(), Some(Some("debug".to_owned())));
    control_file
        .request("note_taker_core=trace,info")
        .unwrap_or_else(|error| panic!("a valid directive must be accepted: {error:#}"));
    assert_eq!(
        control_file.read().ok(),
        Some(Some("note_taker_core=trace,info".to_owned()))
    );
    let _ = fs::remove_dir_all(directory);
}

/// Verifies invalid directives are rejected before anything is written.
#[test]
fn invalid_directives_are_rejected_without_writing() {
    let (database_path, directory) = scratch_database();
    let control_file = LogLevelControlFile::for_database(&database_path);

    for invalid_directive in ["", "   ", "note_taker_core=loud", "[unclosed", "lo\u{0}g"] {
        assert!(
            control_file.request(invalid_directive).is_err(),
            "{invalid_directive:?} must be rejected"
        );
    }
    assert!(!control_file.path().exists());
    let _ = fs::remove_dir_all(directory);
}

/// Verifies a corrupted control file is reported rather than applied.
#[test]
fn an_invalid_stored_directive_is_an_error() {
    let (database_path, directory) = scratch_database();
    let control_file = LogLevelControlFile::for_database(&database_path);
    fs::write(control_file.path(), "note_taker_core=loud")
        .unwrap_or_else(|error| panic!("fixture must be written: {error}"));

    assert!(control_file.read().is_err());
    let _ = fs::remove_dir_all(directory);
}

/// Verifies the watcher ignores a request left from before start-up and reports later changes once.
#[test]
fn the_watcher_reports_each_new_request_once() {
    let (database_path, directory) = scratch_database();
    let control_file = LogLevelControlFile::for_database(&database_path);
    control_file
        .request("trace")
        .unwrap_or_else(|error| panic!("fixture request must be written: {error:#}"));
    let mut watcher = LogLevelWatcher::start(control_file.clone());

    assert_eq!(
        watcher.poll().ok(),
        Some(None),
        "a stale request must not apply"
    );
    control_file
        .request("debug")
        .unwrap_or_else(|error| panic!("request must be written: {error:#}"));
    assert_eq!(watcher.poll().ok(), Some(Some("debug".to_owned())));
    assert_eq!(
        watcher.poll().ok(),
        Some(None),
        "an applied request must not repeat"
    );
    control_file
        .request("trace")
        .unwrap_or_else(|error| panic!("request must be written: {error:#}"));
    assert_eq!(watcher.poll().ok(), Some(Some("trace".to_owned())));
    let _ = fs::remove_dir_all(directory);
}

/// Verifies a corrupted request is reported once, then the watcher keeps working.
#[test]
fn the_watcher_reports_an_invalid_request_and_recovers() {
    let (database_path, directory) = scratch_database();
    let control_file = LogLevelControlFile::for_database(&database_path);
    let mut watcher = LogLevelWatcher::start(control_file.clone());
    fs::write(control_file.path(), "note_taker_core=loud")
        .unwrap_or_else(|error| panic!("fixture must be written: {error}"));

    assert!(watcher.poll().is_err());
    assert_eq!(
        watcher.poll().ok(),
        Some(None),
        "the same bad file must not repeat"
    );
    control_file
        .request("warn")
        .unwrap_or_else(|error| panic!("request must be written: {error:#}"));
    assert_eq!(watcher.poll().ok(), Some(Some("warn".to_owned())));
    let _ = fs::remove_dir_all(directory);
}
