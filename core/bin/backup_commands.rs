//! The backup and restore commands behind the Android app's Settings (#167).
//!
//! `ffi_android.rs` hands these the file Android's document picker opened, the passphrase and
//! (for a backup) the database key the Keystore unwrapped, and passes the status back to
//! Kotlin. Keeping them here, outside the Android-only module, lets the host tests run them.
//! The format and its checks are in `adapters::storage::backup_archive`.

use std::{
    ffi::c_int,
    fs::File,
    io::{BufReader, BufWriter, Write},
    path::Path,
};

use zeroize::Zeroizing;

use crate::{
    adapters::storage::backup_archive::{
        BackupError, BackupSettings, BackupSummary, RestoredBackup, stage_restore, write_backup,
    },
    application::crypto::voice_encryptor::EncryptionKey,
    ffi::{FFI_INVALID_ARGUMENT, FFI_SUCCESS},
};

/// The passphrase is empty.
pub const BACKUP_EMPTY_PASSPHRASE: c_int = 20;
/// The picked file is not a Koalino backup.
pub const BACKUP_NOT_A_BACKUP: c_int = 21;
/// The backup comes from a newer version of Koalino.
pub const BACKUP_UNSUPPORTED_VERSION: c_int = 22;
/// The passphrase is wrong (or the start of the file is damaged).
pub const BACKUP_WRONG_PASSPHRASE: c_int = 23;
/// The backup is damaged or incomplete.
pub const BACKUP_DAMAGED: c_int = 24;
/// The restore folder already holds files.
pub const BACKUP_STAGING_NOT_EMPTY: c_int = 25;
/// The backup's key does not open its database.
pub const BACKUP_KEY_MISMATCH: c_int = 26;
/// Reading or writing failed (for example, a full disk).
pub const BACKUP_IO_FAILED: c_int = 27;

/// Maps a backup failure to its status code.
///
/// Inputs: The failure. Outputs: One of the `BACKUP_*` codes. Side effects: None.
pub fn backup_status(error: &BackupError) -> c_int {
    match error {
        BackupError::EmptyPassphrase => BACKUP_EMPTY_PASSPHRASE,
        BackupError::NotABackup => BACKUP_NOT_A_BACKUP,
        BackupError::UnsupportedVersion => BACKUP_UNSUPPORTED_VERSION,
        BackupError::WrongPassphrase => BACKUP_WRONG_PASSPHRASE,
        BackupError::Damaged => BACKUP_DAMAGED,
        BackupError::StagingNotEmpty => BACKUP_STAGING_NOT_EMPTY,
        BackupError::KeyDoesNotOpenDatabase => BACKUP_KEY_MISMATCH,
        BackupError::KeyDerivation(_) | BackupError::Io { .. } => BACKUP_IO_FAILED,
    }
}

/// Writes a backup of the data folder into `output`.
///
/// Inputs: The app's data folder and the database's file name in it; the file to write (taken
/// over and closed); the passphrase; the 32-byte database key.
/// Outputs: `FFI_SUCCESS` with what was written, `FFI_INVALID_ARGUMENT` for a key of another
/// length, or a `BACKUP_*` code.
/// Side effects: As `write_backup`; logs the outcome without names, contents or keys.
pub fn create_backup(
    data_directory: &Path,
    database_name: &str,
    output: File,
    passphrase: &str,
    key: &[u8],
) -> (c_int, Option<BackupSummary>) {
    let Ok(key_bytes) = <[u8; 32]>::try_from(key) else {
        return (FFI_INVALID_ARGUMENT, None);
    };
    let key_bytes = Zeroizing::new(key_bytes);
    let key = EncryptionKey::from_bytes(*key_bytes);
    let mut writer = BufWriter::new(output);
    let result = write_backup(
        data_directory,
        database_name,
        &key,
        passphrase,
        &BackupSettings::default(),
        &mut writer,
    )
    .and_then(|summary| {
        writer
            .flush()
            .and_then(|()| writer.get_ref().sync_all())
            .map(|()| summary)
            .map_err(|source| BackupError::Io {
                operation: "unable to finish writing the backup",
                source,
            })
    });
    match result {
        Ok(summary) => {
            tracing::info!(
                files = summary.files,
                bytes = summary.bytes,
                "Backup written"
            );
            (FFI_SUCCESS, Some(summary))
        }
        Err(error) => {
            tracing::error!(error = %error, "Backup failed");
            (backup_status(&error), None)
        }
    }
}

/// Restores a backup into the staging folder.
///
/// Inputs: The backup file (taken over and closed); the passphrase; the staging folder (absent
/// or empty).
/// Outputs: The verified key and what was restored, or a `BACKUP_*` code.
/// Side effects: As `stage_restore`; logs the outcome without names, contents or keys.
pub fn restore_backup(
    input: File,
    passphrase: &str,
    staging_directory: &Path,
) -> Result<RestoredBackup, c_int> {
    match stage_restore(&mut BufReader::new(input), passphrase, staging_directory) {
        Ok(restored) => {
            tracing::info!(
                files = restored.files,
                bytes = restored.bytes,
                "Backup restored into the staging folder"
            );
            Ok(restored)
        }
        Err(error) => {
            tracing::error!(error = %error, "Restoring the backup failed");
            Err(backup_status(&error))
        }
    }
}
