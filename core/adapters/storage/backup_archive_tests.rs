//! Specifies the encrypted backup file (#167): a backup restores every kept file and the
//! database with its key; models, logs, keys and spools stay out; a wrong passphrase, a foreign
//! file, a cut file, an altered byte and an unsafe path are refused; and a restored key must
//! open the restored database. Each test uses its own temporary directory and a fast Argon2id
//! setting, so the suite stays quick.

use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use crate::{
    adapters::storage::{
        backup_archive::{
            BackupError, BackupSettings, ENTRY_END, ENTRY_FILE, stage_restore, write_backup,
            write_raw_backup,
        },
        sqlite_encryption_metadata::SqliteEncryptionMetadataStore,
    },
    application::crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
    ports::crypto::{
        EncryptionKeySource, EncryptionMetadata, EncryptionMetadataRepository,
        KeyDerivationParameters,
    },
};

/// A temporary directory deleted when dropped.
struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("backup-archive-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const DATABASE: &str = "note_taker.sqlite3";
const PASSPHRASE: &str = "correct horse battery staple";

/// Settings with a cheap key derivation, for tests only.
fn fast_settings() -> BackupSettings {
    BackupSettings {
        derivation: KeyDerivationParameters::new(64, 1, 1),
        chunk_bytes: 1024,
    }
}

fn write(path: &Path, contents: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

/// A data folder like the app's: a database sealed under `key`, kept files, and the things a
/// backup leaves out.
fn sample_data_folder(key: &EncryptionKey) -> TemporaryDirectory {
    let folder = TemporaryDirectory::new();
    let data = folder.path();
    let metadata = SqliteEncryptionMetadataStore::open(data.join(DATABASE)).unwrap();
    let key_check = VoiceEncryptor::new(key).seal_key_check().unwrap();
    metadata
        .save_encryption_metadata(&EncryptionMetadata::new(
            EncryptionKeySource::KeyFile,
            key_check,
        ))
        .unwrap();
    drop(metadata);
    write(&data.join("note_taker.sqlite3.device-id"), b"device-1234");
    write(&data.join("attachments/meeting-1/photo.jpg"), &[7u8; 5000]);
    write(&data.join("retained-audio/meeting-1.sealed"), &[1u8; 3000]);
    write(&data.join("voice_review_clips/clip.sealed"), b"clip");
    write(&data.join("models/ggml-base.bin"), b"model weights");
    write(&data.join("models/diarization/segmentation.onnx"), b"onnx");
    write(&data.join("logs/app_log.2026-11-25"), b"log line");
    write(
        &data.join("keys/encryption.key.wrapped"),
        b"wrapped by the keystore",
    );
    write(&data.join("bridge-spool/chunk.pcm"), b"spool");
    write(&data.join("upload.tmp"), b"partial");
    folder
}

fn key() -> EncryptionKey {
    EncryptionKey::from_bytes([42u8; 32])
}

fn backup_of(data: &Path, key: &EncryptionKey) -> Vec<u8> {
    let mut output = Vec::new();
    write_backup(
        data,
        DATABASE,
        key,
        PASSPHRASE,
        &fast_settings(),
        &mut output,
    )
    .unwrap();
    output
}

#[test]
fn restores_the_database_key_and_every_kept_file() {
    let key = key();
    let data = sample_data_folder(&key);
    let backup = backup_of(data.path(), &key);
    let target = TemporaryDirectory::new();
    let staging = target.path().join("staging");

    let restored = stage_restore(&mut Cursor::new(backup), PASSPHRASE, &staging).unwrap();

    assert_eq!(restored.key.as_bytes(), key.as_bytes());
    assert_eq!(
        fs::read(staging.join("attachments/meeting-1/photo.jpg")).unwrap(),
        vec![7u8; 5000]
    );
    assert_eq!(
        fs::read(staging.join("retained-audio/meeting-1.sealed")).unwrap(),
        vec![1u8; 3000]
    );
    assert_eq!(
        fs::read(staging.join("note_taker.sqlite3.device-id")).unwrap(),
        b"device-1234"
    );
    assert_eq!(
        fs::read(staging.join("voice_review_clips/clip.sealed")).unwrap(),
        b"clip"
    );
    // The database snapshot opens and still matches the key.
    let metadata = SqliteEncryptionMetadataStore::open(staging.join(DATABASE)).unwrap();
    let stored = metadata.load_encryption_metadata().unwrap().unwrap();
    VoiceEncryptor::new(&restored.key)
        .verify_key_check(stored.key_check())
        .unwrap();
    assert_eq!(restored.files, 5);
}

#[test]
fn leaves_out_models_logs_keys_spools_and_temporary_files() {
    let key = key();
    let data = sample_data_folder(&key);
    let backup = backup_of(data.path(), &key);
    let target = TemporaryDirectory::new();
    let staging = target.path().join("staging");

    stage_restore(&mut Cursor::new(backup), PASSPHRASE, &staging).unwrap();

    for left_out in [
        "models",
        "logs",
        "keys",
        "bridge-spool",
        "upload.tmp",
        "manifest.json",
    ] {
        assert!(!staging.join(left_out).exists(), "{left_out} was restored");
    }
}

#[test]
fn never_holds_the_contents_or_the_key_in_clear() {
    let key = key();
    let data = sample_data_folder(&key);
    let backup = backup_of(data.path(), &key);

    let contains = |needle: &[u8]| backup.windows(needle.len()).any(|window| window == needle);
    assert!(!contains(b"device-1234"));
    assert!(!contains(&[7u8; 64]));
    assert!(!contains(key.as_bytes()));
    assert!(!contains(b"attachments/meeting-1"));
}

#[test]
fn refuses_a_wrong_passphrase_without_writing_anything() {
    let key = key();
    let data = sample_data_folder(&key);
    let backup = backup_of(data.path(), &key);
    let target = TemporaryDirectory::new();
    let staging = target.path().join("staging");

    let error = stage_restore(&mut Cursor::new(backup), "wrong", &staging).unwrap_err();

    assert!(matches!(error, BackupError::WrongPassphrase), "{error:?}");
    assert!(!staging.exists());
}

#[test]
fn refuses_a_file_that_is_not_a_backup() {
    let target = TemporaryDirectory::new();
    let error = stage_restore(
        &mut Cursor::new(b"PK\x03\x04 a zip file".to_vec()),
        PASSPHRASE,
        &target.path().join("staging"),
    )
    .unwrap_err();
    assert!(matches!(error, BackupError::NotABackup), "{error:?}");
}

#[test]
fn refuses_a_cut_file_and_removes_what_it_staged() {
    let key = key();
    let data = sample_data_folder(&key);
    let mut backup = backup_of(data.path(), &key);
    backup.truncate(backup.len() - 40);
    let target = TemporaryDirectory::new();
    let staging = target.path().join("staging");

    let error = stage_restore(&mut Cursor::new(backup), PASSPHRASE, &staging).unwrap_err();

    assert!(matches!(error, BackupError::Damaged), "{error:?}");
    assert!(!staging.exists());
}

#[test]
fn refuses_an_altered_byte() {
    let key = key();
    let data = sample_data_folder(&key);
    let mut backup = backup_of(data.path(), &key);
    let middle = backup.len() / 2;
    backup[middle] ^= 0x01;
    let target = TemporaryDirectory::new();

    let error = stage_restore(
        &mut Cursor::new(backup),
        PASSPHRASE,
        &target.path().join("staging"),
    )
    .unwrap_err();

    assert!(matches!(error, BackupError::Damaged), "{error:?}");
}

#[test]
fn refuses_to_restore_over_an_existing_folder() {
    let key = key();
    let data = sample_data_folder(&key);
    let backup = backup_of(data.path(), &key);
    let target = TemporaryDirectory::new();
    let staging = target.path().join("staging");
    write(&staging.join("keep.txt"), b"mine");

    let error = stage_restore(&mut Cursor::new(backup), PASSPHRASE, &staging).unwrap_err();

    assert!(matches!(error, BackupError::StagingNotEmpty), "{error:?}");
    assert_eq!(fs::read(staging.join("keep.txt")).unwrap(), b"mine");
}

#[test]
fn refuses_paths_that_leave_the_restore_folder() {
    let target = TemporaryDirectory::new();
    for unsafe_path in [
        "../escape.txt",
        "/etc/passwd",
        "keys/encryption.key",
        "a/../../b",
    ] {
        let manifest =
            br#"{"format":1,"database_key":"KioqKioqKioqKioqKioqKioqKioqKioqKioqKioqKio="}"#;
        let entries = [
            (ENTRY_FILE, "manifest.json", manifest.to_vec()),
            (ENTRY_FILE, unsafe_path, b"evil".to_vec()),
        ];
        let mut backup = Vec::new();
        write_raw_backup(
            &entries,
            ENTRY_END,
            PASSPHRASE,
            &fast_settings(),
            &mut backup,
        )
        .unwrap();
        let staging = target.path().join(format!("staging-{}", Uuid::new_v4()));

        let error = stage_restore(&mut Cursor::new(backup), PASSPHRASE, &staging).unwrap_err();

        assert!(
            matches!(error, BackupError::Damaged),
            "{unsafe_path}: {error:?}"
        );
        assert!(!staging.exists());
    }
    assert!(!target.path().join("escape.txt").exists());
}

#[test]
fn refuses_a_key_that_does_not_open_the_restored_database() {
    let key = key();
    let data = sample_data_folder(&key);
    let other_key = EncryptionKey::from_bytes([9u8; 32]);
    let mut backup = Vec::new();
    write_backup(
        data.path(),
        DATABASE,
        &other_key,
        PASSPHRASE,
        &fast_settings(),
        &mut backup,
    )
    .unwrap();
    let target = TemporaryDirectory::new();
    let staging = target.path().join("staging");

    let error = stage_restore(&mut Cursor::new(backup), PASSPHRASE, &staging).unwrap_err();

    assert!(
        matches!(error, BackupError::KeyDoesNotOpenDatabase),
        "{error:?}"
    );
    assert!(!staging.exists());
}

#[test]
fn refuses_an_empty_passphrase() {
    let key = key();
    let data = sample_data_folder(&key);
    let error = write_backup(
        data.path(),
        DATABASE,
        &key,
        "",
        &fast_settings(),
        &mut Vec::new(),
    )
    .unwrap_err();
    assert!(matches!(error, BackupError::EmptyPassphrase), "{error:?}");
}

#[test]
fn restores_when_the_end_falls_on_a_chunk_boundary() {
    let key = key();
    let data = sample_data_folder(&key);
    let mut backup = Vec::new();
    let one_byte_chunks = BackupSettings {
        chunk_bytes: 1,
        ..fast_settings()
    };
    write_backup(
        data.path(),
        DATABASE,
        &key,
        PASSPHRASE,
        &one_byte_chunks,
        &mut backup,
    )
    .unwrap();
    let target = TemporaryDirectory::new();

    let restored = stage_restore(
        &mut Cursor::new(backup),
        PASSPHRASE,
        &target.path().join("staging"),
    )
    .unwrap();

    assert_eq!(restored.files, 5);
}

#[test]
fn refuses_a_backup_whose_last_chunk_was_dropped() {
    let key = key();
    let data = sample_data_folder(&key);
    let backup = backup_of(data.path(), &key);
    // Walk the chunks and cut the file right before the last one.
    let mut position = 51;
    let mut starts = Vec::new();
    while position < backup.len() {
        starts.push(position);
        let length = u32::from_le_bytes(backup[position..position + 4].try_into().unwrap());
        position += 4 + length as usize;
    }
    let cut = backup[..*starts.last().unwrap()].to_vec();
    let target = TemporaryDirectory::new();

    let error = stage_restore(
        &mut Cursor::new(cut),
        PASSPHRASE,
        &target.path().join("staging"),
    )
    .unwrap_err();

    assert!(matches!(error, BackupError::Damaged), "{error:?}");
}
