//! Specifies how composition roots resolve the acquisition device ID stamped on every note.

use std::{
    fs,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use super::{device_id_file_path, resolve_device_id};

/// A unique scratch directory, removed with its contents when dropped.
struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    /// Creates a fresh directory under the system temp directory.
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("note-taker-device-id-{}", Uuid::new_v4()));
        fs::create_dir_all(&directory)
            .unwrap_or_else(|error| panic!("scratch directory must be created: {error}"));
        Self(directory)
    }

    /// Returns the directory path.
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDirectory {
    /// Removes the directory; a failure only leaves a temp directory behind.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Creates a scratch directory holding a not-yet-created database path.
fn scratch_database() -> (ScratchDirectory, PathBuf) {
    let scratch_directory = ScratchDirectory::new();
    let database_path = scratch_directory.path().join("note_taker.sqlite3");
    (scratch_directory, database_path)
}

/// Verifies a configured `NOTE_TAKER_DEVICE_ID` wins and is trimmed, without writing a file.
#[test]
fn a_configured_device_id_is_used_as_is() {
    let (_scratch_directory, database_path) = scratch_database();

    let device_id = resolve_device_id(Some("  laptop-rome ".to_owned()), &database_path)
        .unwrap_or_else(|error| panic!("a configured ID must resolve: {error:#}"));

    assert_eq!(device_id.as_str(), "laptop-rome");
    assert!(!device_id_file_path(&database_path).exists());
}

/// Verifies an unconfigured device gets a generated ID that is persisted and reused.
#[test]
fn an_unconfigured_device_generates_and_reuses_a_persisted_id() {
    let (_scratch_directory, database_path) = scratch_database();

    let first_device_id = resolve_device_id(None, &database_path)
        .unwrap_or_else(|error| panic!("a generated ID must resolve: {error:#}"));
    let second_device_id = resolve_device_id(Some("   ".to_owned()), &database_path)
        .unwrap_or_else(|error| panic!("a persisted ID must resolve: {error:#}"));

    assert!(first_device_id.as_str().starts_with("device-"));
    assert_eq!(first_device_id.as_str().len(), "device-".len() + 36);
    assert_eq!(first_device_id, second_device_id);
    assert_eq!(
        fs::read_to_string(device_id_file_path(&database_path))
            .unwrap_or_else(|error| panic!("the ID file must exist: {error}"))
            .trim(),
        first_device_id.as_str()
    );
}

/// Verifies two databases (two installations) get different generated IDs.
#[test]
fn separate_installations_get_distinct_ids() {
    let (_first_directory, first_database) = scratch_database();
    let (_second_directory, second_database) = scratch_database();

    let first_device_id = resolve_device_id(None, &first_database)
        .unwrap_or_else(|error| panic!("first ID must resolve: {error:#}"));
    let second_device_id = resolve_device_id(None, &second_database)
        .unwrap_or_else(|error| panic!("second ID must resolve: {error:#}"));

    assert_ne!(first_device_id, second_device_id);
}

/// Verifies the ID file sits beside the database, so each database keeps its own identity.
#[test]
fn the_id_file_sits_beside_the_database() {
    let (scratch_directory, database_path) = scratch_database();

    assert_eq!(
        device_id_file_path(&database_path),
        scratch_directory
            .path()
            .join("note_taker.sqlite3.device-id")
    );
}

/// Verifies an empty or corrupted ID file is rejected instead of silently replaced, because a new
/// ID would split this device's notes across two identities.
#[test]
fn an_unusable_id_file_is_an_error_naming_the_file() {
    let (_scratch_directory, database_path) = scratch_database();
    for unusable_content in ["", "  \n", "bad\u{0}id"] {
        fs::write(device_id_file_path(&database_path), unusable_content)
            .unwrap_or_else(|error| panic!("fixture must be written: {error}"));

        let error = resolve_device_id(None, &database_path)
            .err()
            .unwrap_or_else(|| panic!("{unusable_content:?} must be rejected"));

        assert!(format!("{error:#}").contains("note_taker.sqlite3.device-id"));
    }
}

/// Verifies a missing parent directory is reported instead of panicking.
#[test]
fn an_unwritable_location_is_an_error() {
    let (scratch_directory, _database_path) = scratch_database();
    let database_path = scratch_directory
        .path()
        .join("missing-directory")
        .join("note_taker.sqlite3");

    assert!(resolve_device_id(None, &database_path).is_err());
}
