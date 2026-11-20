//! Specifies how old daily log files are removed, so logs cannot fill the disk (#147).

use std::{fs, path::PathBuf};

use uuid::Uuid;

use super::{LogRetention, prune_log_files};

/// Returns a fresh log directory.
fn scratch_logs() -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("note-taker-log-retention-{}", Uuid::new_v4()));
    fs::create_dir_all(&directory)
        .unwrap_or_else(|error| panic!("scratch log directory must be created: {error}"));
    directory
}

/// Writes a log file of `size` bytes.
fn write_log(directory: &std::path::Path, name: &str, size: usize) {
    fs::write(directory.join(name), vec![b'x'; size])
        .unwrap_or_else(|error| panic!("log file must be written: {error}"));
}

/// The names left in the directory, sorted.
fn names(directory: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn only_the_newest_days_are_kept() {
    let directory = scratch_logs();
    for day in 10..20 {
        write_log(&directory, &format!("app_log.2026-11-{day}"), 10);
    }
    write_log(&directory, "notes.txt", 10);

    let removed = prune_log_files(
        &directory,
        LogRetention {
            max_files: 3,
            max_total_bytes: 1000,
        },
    )
    .unwrap_or_else(|error| panic!("pruning must work: {error}"));

    assert_eq!(removed, 7);
    assert_eq!(
        names(&directory),
        [
            "app_log.2026-11-17",
            "app_log.2026-11-18",
            "app_log.2026-11-19",
            "notes.txt"
        ]
    );
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn the_oldest_days_go_when_the_logs_are_too_large_but_today_stays() {
    let directory = scratch_logs();
    write_log(&directory, "app_log.2026-11-17", 400);
    write_log(&directory, "app_log.2026-11-18", 400);
    write_log(&directory, "app_log.2026-11-19", 300);
    write_log(&directory, "app_log.2026-11-20", 2000);

    let removed = prune_log_files(
        &directory,
        LogRetention {
            max_files: 7,
            max_total_bytes: 1000,
        },
    )
    .unwrap_or_else(|error| panic!("pruning must work: {error}"));

    // Even alone the newest is over the limit; it is the file being written, so it stays.
    assert_eq!(removed, 3);
    assert_eq!(names(&directory), ["app_log.2026-11-20"]);
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn logs_within_the_limits_are_left_alone() {
    let directory = scratch_logs();
    write_log(&directory, "app_log.2026-11-19", 300);
    write_log(&directory, "app_log.2026-11-20", 300);

    let removed = prune_log_files(
        &directory,
        LogRetention {
            max_files: 7,
            max_total_bytes: 1000,
        },
    )
    .unwrap_or_else(|error| panic!("pruning must work: {error}"));

    assert_eq!(removed, 0);
    assert_eq!(names(&directory).len(), 2);
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn a_missing_directory_is_nothing_to_prune() {
    let directory = scratch_logs().join("absent");
    assert_eq!(
        prune_log_files(&directory, LogRetention::DEFAULT).ok(),
        Some(0)
    );
}
