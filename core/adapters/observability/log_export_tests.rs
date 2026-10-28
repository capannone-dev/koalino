//! Specifies how the rolling log files are gathered into one file to share (Issue #46).

use std::{fs, path::PathBuf};

use uuid::Uuid;

use super::{LogExportError, MAX_EXPORTED_LOG_FILES, export_log_files};

/// Returns a fresh log directory and an output path outside it, plus their root for cleanup.
fn scratch_logs() -> (PathBuf, PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("note-taker-log-export-{}", Uuid::new_v4()));
    let log_directory = root.join("logs");
    fs::create_dir_all(&log_directory)
        .unwrap_or_else(|error| panic!("scratch log directory must be created: {error}"));
    (log_directory, root.join("exports").join("logs.jsonl"), root)
}

/// Writes one daily log file.
fn write_log(log_directory: &std::path::Path, name: &str, content: &str) {
    fs::write(log_directory.join(name), content)
        .unwrap_or_else(|error| panic!("log file must be written: {error}"));
}

/// Verifies the daily files are joined oldest first, so the export reads in time order.
#[test]
fn the_daily_files_are_joined_oldest_first() {
    let (log_directory, output_path, root) = scratch_logs();
    write_log(&log_directory, "app_log.2026-10-28", "{\"day\":28}\n");
    write_log(&log_directory, "app_log.2026-10-26", "{\"day\":26}\n");
    write_log(&log_directory, "app_log.2026-10-27", "{\"day\":27}\n");

    let export = export_log_files(&log_directory, &output_path)
        .unwrap_or_else(|error| panic!("logs must export: {error}"));

    let exported = fs::read_to_string(&output_path).unwrap_or_default();
    assert_eq!(exported, "{\"day\":26}\n{\"day\":27}\n{\"day\":28}\n");
    assert_eq!(export.path, output_path);
    assert_eq!(export.size_bytes, exported.len() as u64);
    assert_eq!(export.file_count, 3);
    let _ = fs::remove_dir_all(root);
}

/// Verifies only the newest days are exported, so a long-lived install shares a bounded file.
#[test]
fn only_the_newest_days_are_exported() {
    let (log_directory, output_path, root) = scratch_logs();
    let days = MAX_EXPORTED_LOG_FILES + 3;
    for day in 1..=days {
        write_log(
            &log_directory,
            &format!("app_log.2026-10-{day:02}"),
            &format!("{day}\n"),
        );
    }

    let export = export_log_files(&log_directory, &output_path)
        .unwrap_or_else(|error| panic!("logs must export: {error}"));

    let exported = fs::read_to_string(&output_path).unwrap_or_default();
    let first_kept_day = days - MAX_EXPORTED_LOG_FILES + 1;
    let expected: String = (first_kept_day..=days)
        .map(|day| format!("{day}\n"))
        .collect();
    assert_eq!(exported, expected);
    assert_eq!(export.file_count, MAX_EXPORTED_LOG_FILES);
    let _ = fs::remove_dir_all(root);
}

/// Verifies a file that does not end in a newline cannot glue its last line to the next day's.
#[test]
fn each_file_ends_its_last_line() {
    let (log_directory, output_path, root) = scratch_logs();
    write_log(&log_directory, "app_log.2026-10-27", "{\"a\":1}");
    write_log(&log_directory, "app_log.2026-10-28", "{\"b\":2}\n");

    export_log_files(&log_directory, &output_path)
        .unwrap_or_else(|error| panic!("logs must export: {error}"));

    assert_eq!(
        fs::read_to_string(&output_path).unwrap_or_default(),
        "{\"a\":1}\n{\"b\":2}\n"
    );
    let _ = fs::remove_dir_all(root);
}

/// Verifies files other than the rolling logs (and directories) are left out.
#[test]
fn other_files_are_left_out() {
    let (log_directory, output_path, root) = scratch_logs();
    write_log(&log_directory, "app_log.2026-10-28", "log\n");
    write_log(&log_directory, "notes.txt", "private\n");
    fs::create_dir_all(log_directory.join("app_log.directory"))
        .unwrap_or_else(|error| panic!("directory must be created: {error}"));

    let export = export_log_files(&log_directory, &output_path)
        .unwrap_or_else(|error| panic!("logs must export: {error}"));

    assert_eq!(
        fs::read_to_string(&output_path).unwrap_or_default(),
        "log\n"
    );
    assert_eq!(export.file_count, 1);
    let _ = fs::remove_dir_all(root);
}

/// Verifies an export replaces an earlier file at the same path instead of appending to it.
#[test]
fn an_earlier_export_is_replaced() {
    let (log_directory, output_path, root) = scratch_logs();
    write_log(&log_directory, "app_log.2026-10-28", "new\n");
    fs::create_dir_all(output_path.parent().unwrap_or(&root))
        .unwrap_or_else(|error| panic!("output directory must be created: {error}"));
    fs::write(&output_path, "old export\n")
        .unwrap_or_else(|error| panic!("old export must be written: {error}"));

    export_log_files(&log_directory, &output_path)
        .unwrap_or_else(|error| panic!("logs must export: {error}"));

    assert_eq!(
        fs::read_to_string(&output_path).unwrap_or_default(),
        "new\n"
    );
    let _ = fs::remove_dir_all(root);
}

/// Verifies no log files (or no log directory) is reported as such, and writes nothing.
#[test]
fn no_logs_is_its_own_error() {
    let (log_directory, output_path, root) = scratch_logs();

    assert!(matches!(
        export_log_files(&log_directory, &output_path),
        Err(LogExportError::NoLogs)
    ));
    assert!(matches!(
        export_log_files(&root.join("missing"), &output_path),
        Err(LogExportError::NoLogs)
    ));
    assert!(!output_path.exists());
    let _ = fs::remove_dir_all(root);
}
