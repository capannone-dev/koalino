//! Specifies the backup commands behind the Android app's Settings (#167): a backup written to
//! a file restores the same key and files, and each failure reaches Kotlin as its own status.

use std::{fs, fs::File, path::PathBuf};

use uuid::Uuid;

use crate::{
    adapters::storage::backup_archive::BackupError,
    backup_commands::{
        BACKUP_DAMAGED, BACKUP_EMPTY_PASSPHRASE, BACKUP_IO_FAILED, BACKUP_KEY_MISMATCH,
        BACKUP_NOT_A_BACKUP, BACKUP_STAGING_NOT_EMPTY, BACKUP_UNSUPPORTED_VERSION,
        BACKUP_WRONG_PASSPHRASE, backup_status, create_backup, restore_backup,
    },
    ffi::{FFI_INVALID_ARGUMENT, FFI_SUCCESS},
};

/// A temporary directory deleted when dropped.
struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("backup-commands-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A data folder with a fresh database (no key check yet) and one attachment.
fn data_folder() -> TemporaryDirectory {
    let folder = TemporaryDirectory::new();
    rusqlite::Connection::open(folder.0.join("note_taker.sqlite3"))
        .unwrap()
        .execute_batch("CREATE TABLE meetings (id TEXT); INSERT INTO meetings VALUES ('m1');")
        .unwrap();
    fs::create_dir_all(folder.0.join("attachments/m1")).unwrap();
    fs::write(folder.0.join("attachments/m1/notes.txt"), b"agenda").unwrap();
    folder
}

#[test]
fn a_backup_file_restores_the_same_key_and_files() {
    let data = data_folder();
    let backup_path = data
        .0
        .join("../")
        .join(format!("{}.koalino-backup", Uuid::new_v4()));
    let key = [5u8; 32];

    let (status, summary) = create_backup(
        &data.0,
        "note_taker.sqlite3",
        File::create(&backup_path).unwrap(),
        "passphrase",
        &key,
    );
    assert_eq!(status, FFI_SUCCESS);
    assert_eq!(summary.unwrap().files, 2);

    let target = TemporaryDirectory::new();
    let staging = target.0.join("restore");
    let restored =
        restore_backup(File::open(&backup_path).unwrap(), "passphrase", &staging).unwrap();
    fs::remove_file(&backup_path).unwrap();

    assert_eq!(restored.key.as_bytes(), &key);
    assert_eq!(
        fs::read(staging.join("attachments/m1/notes.txt")).unwrap(),
        b"agenda"
    );
    let count: i64 = rusqlite::Connection::open(staging.join("note_taker.sqlite3"))
        .unwrap()
        .query_row("SELECT COUNT(*) FROM meetings", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn reports_a_key_of_the_wrong_length_and_a_wrong_passphrase() {
    let data = data_folder();
    let backup_path = data.0.join("backup.bin");
    let (status, _) = create_backup(
        &data.0,
        "note_taker.sqlite3",
        File::create(&backup_path).unwrap(),
        "passphrase",
        &[1u8; 16],
    );
    assert_eq!(status, FFI_INVALID_ARGUMENT);

    let (status, _) = create_backup(
        &data.0,
        "note_taker.sqlite3",
        File::create(&backup_path).unwrap(),
        "passphrase",
        &[1u8; 32],
    );
    assert_eq!(status, FFI_SUCCESS);
    let target = TemporaryDirectory::new();
    let error = restore_backup(
        File::open(&backup_path).unwrap(),
        "not it",
        &target.0.join("restore"),
    )
    .unwrap_err();
    assert_eq!(error, BACKUP_WRONG_PASSPHRASE);
}

#[test]
fn every_failure_has_its_own_status() {
    let statuses = [
        backup_status(&BackupError::EmptyPassphrase),
        backup_status(&BackupError::NotABackup),
        backup_status(&BackupError::UnsupportedVersion),
        backup_status(&BackupError::WrongPassphrase),
        backup_status(&BackupError::Damaged),
        backup_status(&BackupError::StagingNotEmpty),
        backup_status(&BackupError::KeyDoesNotOpenDatabase),
        backup_status(&BackupError::Io {
            operation: "write",
            source: std::io::Error::other("disk full"),
        }),
    ];
    assert_eq!(
        statuses,
        [
            BACKUP_EMPTY_PASSPHRASE,
            BACKUP_NOT_A_BACKUP,
            BACKUP_UNSUPPORTED_VERSION,
            BACKUP_WRONG_PASSPHRASE,
            BACKUP_DAMAGED,
            BACKUP_STAGING_NOT_EMPTY,
            BACKUP_KEY_MISMATCH,
            BACKUP_IO_FAILED,
        ]
    );
}
