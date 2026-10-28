//! Specifies the app's log level and log export over FFI (Issue #46).
//!
//! The tests drive `AppLogSettings` from a fixed lookup over a scratch directory, so they never
//! touch the process-wide tracing subscriber or the app's configured settings.

use std::{collections::HashMap, fs, path::PathBuf};

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use super::{AppLogSettings, DEFAULT_LOG_LEVEL};

/// A scratch data directory and the settings that point into it.
struct Fixture {
    root: PathBuf,
    settings: AppLogSettings,
}

impl Fixture {
    /// Creates a data directory with the layout `note_taker_configure` sets up.
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("note-taker-ffi-logging-{}", Uuid::new_v4()));
        fs::create_dir_all(&root)
            .unwrap_or_else(|error| panic!("scratch directory must be created: {error}"));
        let values: HashMap<&str, String> = [
            ("NOTE_TAKER_DATABASE_PATH", "note_taker.sqlite3"),
            ("NOTE_TAKER_LOG_DIR", "logs"),
            ("NOTE_TAKER_EXPORT_DIR", "exports"),
        ]
        .into_iter()
        .map(|(name, file)| (name, root.join(file).display().to_string()))
        .collect();
        let settings = AppLogSettings::from_lookup(&|name| values.get(name).cloned());
        Self { root, settings }
    }

    /// Writes one daily log file.
    fn write_log(&self, name: &str, content: &str) {
        let log_directory = self.root.join("logs");
        fs::create_dir_all(&log_directory)
            .unwrap_or_else(|error| panic!("log directory must be created: {error}"));
        fs::write(log_directory.join(name), content)
            .unwrap_or_else(|error| panic!("log file must be written: {error}"));
    }

    /// Exports with the given output path and parses the answer.
    fn export(&self, output_path: Option<&str>) -> serde_json::Value {
        let now = Utc
            .with_ymd_and_hms(2026, 10, 28, 9, 5, 7)
            .single()
            .unwrap_or_else(|| panic!("the fixed time must be valid"));
        serde_json::from_str(&self.settings.export_json(output_path, now))
            .unwrap_or_else(|error| panic!("the answer must be JSON: {error}"))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Verifies a fresh install logs at INFO, the documented default.
#[test]
fn a_fresh_install_logs_at_info() {
    let fixture = Fixture::new();

    assert_eq!(DEFAULT_LOG_LEVEL, "info");
    assert_eq!(fixture.settings.current_level(), "info");
}

/// Verifies a chosen level is stored beside the database and read back on the next start.
#[test]
fn a_chosen_level_survives_a_restart() {
    let fixture = Fixture::new();

    let applied = fixture
        .settings
        .store_level(" debug ")
        .unwrap_or_else(|error| panic!("debug must be accepted: {error}"));

    assert_eq!(applied, "debug");
    assert_eq!(fixture.settings.current_level(), "debug");
    assert_eq!(
        fs::read_to_string(fixture.root.join("note_taker.sqlite3.log-level")).unwrap_or_default(),
        "debug"
    );
}

/// Verifies an invalid directive is refused and leaves the stored level as it was.
#[test]
fn an_invalid_level_is_refused_and_not_stored() {
    let fixture = Fixture::new();
    fixture
        .settings
        .store_level("warn")
        .unwrap_or_else(|error| panic!("warn must be accepted: {error}"));

    for invalid in ["", "   ", "info\nerror", "[not a directive"] {
        assert!(
            fixture.settings.store_level(invalid).is_err(),
            "{invalid:?}"
        );
    }
    assert_eq!(fixture.settings.current_level(), "warn");
}

/// Verifies a damaged stored level falls back to INFO instead of leaving the app without logs.
#[test]
fn a_damaged_stored_level_falls_back_to_info() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("note_taker.sqlite3.log-level"), "[broken")
        .unwrap_or_else(|error| panic!("control file must be written: {error}"));

    assert_eq!(fixture.settings.current_level(), "info");
}

/// Verifies an export goes to the path the owner chose and reports it.
#[test]
fn logs_export_to_the_chosen_path() {
    let fixture = Fixture::new();
    fixture.write_log("app_log.2026-10-28", "{\"level\":\"INFO\"}\n");
    let chosen = fixture.root.join("Desktop").join("logs.jsonl");

    let answer = fixture.export(Some(&chosen.display().to_string()));

    assert_eq!(answer["ok"], true);
    assert_eq!(answer["file_path"], chosen.display().to_string());
    assert_eq!(answer["size_bytes"], 17);
    assert_eq!(answer["file_count"], 1);
    assert_eq!(
        fs::read_to_string(&chosen).unwrap_or_default(),
        "{\"level\":\"INFO\"}\n"
    );
}

/// Verifies that without a path the export lands in the export folder under a dated name.
#[test]
fn without_a_path_logs_export_to_the_export_folder() {
    let fixture = Fixture::new();
    fixture.write_log("app_log.2026-10-28", "line\n");
    let expected = fixture
        .root
        .join("exports")
        .join("note_taker_logs_20261028-090507.jsonl");

    for output_path in [None, Some(""), Some("  ")] {
        let answer = fixture.export(output_path);

        assert_eq!(answer["ok"], true, "{output_path:?}");
        assert_eq!(answer["file_path"], expected.display().to_string());
    }
    assert!(expected.is_file());
}

/// Verifies a relative path is refused, since it would depend on the process's directory.
#[test]
fn a_relative_output_path_is_refused() {
    let fixture = Fixture::new();
    fixture.write_log("app_log.2026-10-28", "line\n");

    let answer = fixture.export(Some("logs.jsonl"));

    assert_eq!(answer["ok"], false);
    assert_eq!(answer["error_code"], "invalid_argument");
}

/// Verifies an install with no logs yet says so with its own error code.
#[test]
fn no_logs_yet_has_its_own_error_code() {
    let fixture = Fixture::new();

    let answer = fixture.export(None);

    assert_eq!(answer["ok"], false);
    assert_eq!(answer["error_code"], "no_logs");
}

/// Verifies the C entry point refuses a null or invalid level before storing or applying it.
#[test]
fn the_c_entry_point_refuses_null_and_invalid_levels() {
    use super::{FFI_INVALID_ARGUMENT, note_taker_set_log_level};

    assert_eq!(
        unsafe { note_taker_set_log_level(std::ptr::null()) },
        FFI_INVALID_ARGUMENT
    );
    let invalid_level = std::ffi::CString::new("[broken")
        .unwrap_or_else(|error| panic!("the level must be a C string: {error}"));
    assert_eq!(
        unsafe { note_taker_set_log_level(invalid_level.as_ptr()) },
        FFI_INVALID_ARGUMENT
    );
}
