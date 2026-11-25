//! Writes and restores the encrypted backup of a device's data (#167).
//!
//! A backup is one file holding everything needed to bring a device's data back on another
//! install: a consistent snapshot of the database, the database key, and the kept files beside it
//! (attachments, slides, kept recordings, review clips, the device ID and settings files).
//! Models, logs, spools, temporary files and the platform's wrapped key are left out: models
//! come with the app, and the key travels inside the backup instead.
//!
//! # Format
//!
//! ```text
//! header (clear, 51 bytes): MAGIC (15) | VERSION (1) | Argon2id memory KiB, passes, lanes
//!                           (3 × u32 LE) | salt (16) | nonce prefix (7)
//! chunks: [u32 LE sealed length][AES-256-GCM(chunk)] …
//! ```
//!
//! The chunk key is derived from the owner's passphrase with Argon2id and the header's salt and
//! parameters. Each chunk's nonce is the header's random prefix, a 32-bit chunk counter and a
//! flag marking the last chunk; every chunk authenticates the whole header. So chunks cannot be
//! reordered, dropped, cut off at the end, or moved to another backup without failing to open.
//!
//! The decrypted stream is a list of entries: `ENTRY_FILE`, the path (u16 LE length, UTF-8, `/`
//! separators, relative), the size (u64 LE) and the bytes; then `ENTRY_END`. The first entry is
//! `manifest.json`, with the format, the creation time, the database file name and the database
//! key (base64). Nothing in the file is readable without the passphrase, not even file names.
//!
//! Restoring writes into an empty staging folder, refuses any path outside it (and the `keys`
//! folder), checks that the restored key opens the restored database, and removes the staging
//! folder on any failure. Swapping the staging folder in and protecting the key with the
//! platform's key store are the host's job. Clear buffers are wiped when dropped; contents,
//! names and keys are never logged.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind, Read, Write},
    path::{Component, Path, PathBuf},
};

use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, OsRng, Payload, rand_core::RngCore},
};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    adapters::storage::sqlite_encryption_metadata::SqliteEncryptionMetadataStore,
    application::crypto::{
        key_manager::derive_key_from_passphrase,
        voice_encryptor::{EncryptionKey, VoiceEncryptor},
    },
    ports::crypto::{EncryptionKeySource, EncryptionMetadataRepository, KeyDerivationParameters},
};

/// Marks an entry holding a file.
pub const ENTRY_FILE: u8 = 1;
/// Marks the end of the entries.
pub const ENTRY_END: u8 = 0;

/// The first bytes of every backup.
const MAGIC: &[u8; 15] = b"KOALINO-BACKUP\0";
/// The only format version so far.
const VERSION: u8 = 1;
/// Bytes of the Argon2id salt.
const SALT_BYTES: usize = 16;
/// Bytes of the random nonce prefix; the counter (4) and the last-chunk flag (1) complete it.
const NONCE_PREFIX_BYTES: usize = 7;
/// Bytes of the clear header.
const HEADER_BYTES: usize = MAGIC.len() + 1 + 12 + SALT_BYTES + NONCE_PREFIX_BYTES;
/// Bytes AES-GCM adds to each chunk.
const TAG_BYTES: usize = 16;
/// The largest sealed chunk a reader accepts, so a damaged length cannot ask for gigabytes.
const MAX_SEALED_CHUNK_BYTES: usize = 16 * 1024 * 1024 + TAG_BYTES;
/// The manifest's entry name.
const MANIFEST: &str = "manifest.json";
/// The manifest format written by this version.
const MANIFEST_FORMAT: u32 = 1;
/// Top-level folders a backup leaves out: the wrapped key, models shipped with the app, logs,
/// and the browser bridge's transient audio.
const EXCLUDED_FOLDERS: [&str; 4] = ["keys", "models", "logs", "bridge-spool"];
/// The longest entry path accepted.
const MAX_PATH_BYTES: usize = 4096;

/// How a backup is sealed.
#[derive(Clone, Copy, Debug)]
pub struct BackupSettings {
    /// Argon2id cost for the passphrase.
    pub derivation: KeyDerivationParameters,
    /// Clear bytes per chunk.
    pub chunk_bytes: usize,
}

impl Default for BackupSettings {
    /// Argon2id with 64 MiB and 3 passes (a backup may sit in a cloud folder for years, so it
    /// costs more than the database's own derivation), and 1 MiB chunks.
    fn default() -> Self {
        Self {
            derivation: KeyDerivationParameters::new(64 * 1024, 3, 1),
            chunk_bytes: 1024 * 1024,
        }
    }
}

/// Why a backup could not be written or restored. Messages never hold contents or keys.
#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    /// The owner gave no passphrase.
    #[error("the passphrase is empty")]
    EmptyPassphrase,
    /// The file does not start like a backup.
    #[error("the file is not a Koalino backup")]
    NotABackup,
    /// The file was written by a newer format.
    #[error("the backup was made by a newer version of Koalino")]
    UnsupportedVersion,
    /// The first chunk does not open: a wrong passphrase (or a damaged start).
    #[error("the passphrase is wrong, or the backup is damaged")]
    WrongPassphrase,
    /// A later chunk does not open, the file is cut, or an entry is malformed or unsafe.
    #[error("the backup is damaged or incomplete")]
    Damaged,
    /// The staging folder already holds files.
    #[error("the restore folder is not empty")]
    StagingNotEmpty,
    /// The key in the backup does not open the database in it.
    #[error("the backup's key does not open its database")]
    KeyDoesNotOpenDatabase,
    /// Deriving the passphrase key failed.
    #[error("the passphrase key could not be derived: {0}")]
    KeyDerivation(String),
    /// Reading or writing a file failed.
    #[error("{operation}: {source}")]
    Io {
        /// What was being done.
        operation: &'static str,
        /// The platform's error.
        source: io::Error,
    },
}

/// What a written backup holds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackupSummary {
    /// Files written, the database snapshot included.
    pub files: u64,
    /// Clear bytes of those files.
    pub bytes: u64,
}

/// A backup restored into a staging folder.
pub struct RestoredBackup {
    /// The database key, verified against the restored database.
    pub key: EncryptionKey,
    /// Files restored, the database included.
    pub files: u64,
    /// Clear bytes restored.
    pub bytes: u64,
    /// When the backup was made (RFC 3339), when recorded.
    pub created_at: Option<String>,
}

impl std::fmt::Debug for RestoredBackup {
    /// Shows what was restored, never the key.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RestoredBackup")
            .field("key", &"<redacted>")
            .field("files", &self.files)
            .field("bytes", &self.bytes)
            .field("created_at", &self.created_at)
            .finish()
    }
}

/// The first entry of every backup.
#[derive(Serialize, Deserialize)]
struct Manifest {
    format: u32,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    database: Option<String>,
    database_key: String,
}

fn io_error(operation: &'static str) -> impl FnOnce(io::Error) -> BackupError {
    move |source| BackupError::Io { operation, source }
}

/// Writes a backup of `data_directory` to `output`.
///
/// Inputs: The app's data folder; the database's file name in it; the database key; the owner's
/// passphrase (not empty); the sealing settings (normally `BackupSettings::default()`); where to
/// write.
/// Outputs: How many files and clear bytes were written, or a `BackupError`.
/// Side effects: Takes a consistent copy of the database with `VACUUM INTO` beside it (deleted
/// afterwards), reads the kept files, runs Argon2id, writes the sealed backup to `output`.
pub fn write_backup(
    data_directory: &Path,
    database_name: &str,
    key: &EncryptionKey,
    passphrase: &str,
    settings: &BackupSettings,
    output: &mut impl Write,
) -> Result<BackupSummary, BackupError> {
    if passphrase.is_empty() {
        return Err(BackupError::EmptyPassphrase);
    }
    let snapshot = DatabaseSnapshot::take(data_directory, database_name)?;
    let files = kept_files(data_directory, database_name)?;
    let manifest = Zeroizing::new(
        serde_json::to_vec(&Manifest {
            format: MANIFEST_FORMAT,
            created_at: Some(chrono::Utc::now().to_rfc3339()),
            database: Some(database_name.to_owned()),
            database_key: BASE64.encode(key.as_bytes()),
        })
        .map_err(|error| BackupError::Io {
            operation: "unable to write the backup manifest",
            source: io::Error::other(error),
        })?,
    );
    let mut writer = ChunkWriter::start(passphrase, settings, output)?;
    write_entry_header(&mut writer, MANIFEST, manifest.len() as u64)?;
    writer
        .write_all(&manifest)
        .map_err(io_error("unable to write the backup"))?;
    let mut summary = BackupSummary { files: 0, bytes: 0 };
    copy_file_entry(&mut writer, database_name, &snapshot.path, &mut summary)?;
    for (relative, path) in &files {
        copy_file_entry(&mut writer, relative, path, &mut summary)?;
    }
    writer
        .write_all(&[ENTRY_END])
        .map_err(io_error("unable to write the backup"))?;
    writer.finish()?;
    Ok(summary)
}

/// Writes a backup from ready entries, for tests that need malformed contents.
#[cfg(test)]
pub(crate) fn write_raw_backup(
    entries: &[(u8, &str, Vec<u8>)],
    end_tag: u8,
    passphrase: &str,
    settings: &BackupSettings,
    output: &mut impl Write,
) -> Result<(), BackupError> {
    let mut writer = ChunkWriter::start(passphrase, settings, output)?;
    for (tag, path, contents) in entries {
        writer.write_all(&[*tag]).map_err(io_error("write"))?;
        writer
            .write_all(&(path.len() as u16).to_le_bytes())
            .map_err(io_error("write"))?;
        writer
            .write_all(path.as_bytes())
            .map_err(io_error("write"))?;
        writer
            .write_all(&(contents.len() as u64).to_le_bytes())
            .map_err(io_error("write"))?;
        writer.write_all(contents).map_err(io_error("write"))?;
    }
    writer.write_all(&[end_tag]).map_err(io_error("write"))?;
    writer.finish()
}

/// Restores a backup into `staging_directory`.
///
/// Inputs: The backup; the passphrase; a folder that does not exist yet or is empty.
/// Outputs: The verified database key and what was restored, or a `BackupError`.
/// Side effects: Runs Argon2id; creates the staging folder and writes the restored files there
/// (owner-only on Unix). On any failure the staging folder is removed again, so a failed restore
/// leaves nothing behind.
pub fn stage_restore(
    input: &mut impl Read,
    passphrase: &str,
    staging_directory: &Path,
) -> Result<RestoredBackup, BackupError> {
    if staging_directory.exists() {
        let mut entries = fs::read_dir(staging_directory)
            .map_err(io_error("unable to read the restore folder"))?;
        if entries.next().is_some() {
            return Err(BackupError::StagingNotEmpty);
        }
    }
    let mut reader = ChunkReader::start(passphrase, input)?;
    // Opening the first chunk before creating anything reports a wrong passphrase cleanly.
    reader.fill()?;
    create_private_directory(staging_directory)?;
    let result = restore_entries(&mut reader, staging_directory);
    if result.is_err() {
        let _ = fs::remove_dir_all(staging_directory);
    }
    result
}

/// Reads every entry into the staging folder and verifies the key; see `stage_restore`.
fn restore_entries(
    reader: &mut ChunkReader<'_, impl Read>,
    staging_directory: &Path,
) -> Result<RestoredBackup, BackupError> {
    let (tag, path, size) = read_entry_header(reader)?;
    if tag != ENTRY_FILE || path != MANIFEST || size > 64 * 1024 {
        return Err(BackupError::Damaged);
    }
    let mut manifest_bytes = Zeroizing::new(vec![0u8; size as usize]);
    reader.read_exact_plain(&mut manifest_bytes)?;
    let manifest: Manifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| BackupError::Damaged)?;
    if manifest.format > MANIFEST_FORMAT {
        return Err(BackupError::UnsupportedVersion);
    }
    let key_bytes = Zeroizing::new(
        BASE64
            .decode(manifest.database_key.as_bytes())
            .map_err(|_| BackupError::Damaged)?,
    );
    let key_array: [u8; 32] = key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| BackupError::Damaged)?;
    let key = EncryptionKey::from_bytes(key_array);
    let mut files = 0u64;
    let mut bytes = 0u64;
    loop {
        let (tag, path, size) = read_entry_header(reader)?;
        match tag {
            ENTRY_END => break,
            ENTRY_FILE => {
                let destination = staging_directory.join(safe_relative_path(&path)?);
                if let Some(parent) = destination.parent() {
                    create_private_directory(parent)?;
                }
                let mut file = create_private_file(&destination)?;
                let mut remaining = size;
                let mut buffer = Zeroizing::new(vec![0u8; 64 * 1024]);
                while remaining > 0 {
                    let piece = remaining.min(buffer.len() as u64) as usize;
                    reader.read_exact_plain(&mut buffer[..piece])?;
                    file.write_all(&buffer[..piece])
                        .map_err(io_error("unable to write a restored file"))?;
                    remaining -= piece as u64;
                }
                file.sync_all()
                    .map_err(io_error("unable to write a restored file"))?;
                files += 1;
                bytes += size;
            }
            _ => return Err(BackupError::Damaged),
        }
    }
    reader.expect_end()?;
    let database = manifest.database.as_deref().ok_or(BackupError::Damaged)?;
    let database_path = staging_directory.join(safe_relative_path(database)?);
    if !database_path.is_file() {
        return Err(BackupError::Damaged);
    }
    verify_key_opens(&database_path, &key)?;
    Ok(RestoredBackup {
        key,
        files,
        bytes,
        created_at: manifest.created_at,
    })
}

/// Checks the key against the database's key-check value.
fn verify_key_opens(database_path: &Path, key: &EncryptionKey) -> Result<(), BackupError> {
    let metadata = SqliteEncryptionMetadataStore::open(database_path)
        .and_then(|store| store.load_encryption_metadata())
        .map_err(|_| BackupError::Damaged)?;
    match metadata {
        Some(metadata) if *metadata.key_source() == EncryptionKeySource::KeyFile => {
            VoiceEncryptor::new(key)
                .verify_key_check(metadata.key_check())
                .map_err(|_| BackupError::KeyDoesNotOpenDatabase)
        }
        // A passphrase database opens with its passphrase, and a new one has nothing sealed yet.
        _ => Ok(()),
    }
}

/// A consistent copy of the database beside it, deleted when dropped.
struct DatabaseSnapshot {
    path: PathBuf,
}

impl DatabaseSnapshot {
    fn take(data_directory: &Path, database_name: &str) -> Result<Self, BackupError> {
        let path = data_directory.join(format!(".backup-snapshot-{}.sqlite3", Uuid::new_v4()));
        let snapshot = Self { path };
        let connection = Connection::open(data_directory.join(database_name)).map_err(|error| {
            BackupError::Io {
                operation: "unable to open the database",
                source: io::Error::other(error),
            }
        })?;
        connection
            .busy_timeout(std::time::Duration::from_secs(30))
            .and_then(|()| {
                connection.execute(
                    "VACUUM INTO ?1",
                    [snapshot.path.to_string_lossy().into_owned()],
                )
            })
            .map_err(|error| BackupError::Io {
                operation: "unable to copy the database",
                source: io::Error::other(error),
            })?;
        Ok(snapshot)
    }
}

impl Drop for DatabaseSnapshot {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Lists the files a backup keeps, as (`/`-separated relative path, full path), sorted.
fn kept_files(
    data_directory: &Path,
    database_name: &str,
) -> Result<Vec<(String, PathBuf)>, BackupError> {
    let mut files = Vec::new();
    let mut folders = vec![(data_directory.to_path_buf(), String::new())];
    while let Some((folder, prefix)) = folders.pop() {
        let entries = fs::read_dir(&folder).map_err(io_error("unable to list the data folder"))?;
        for entry in entries {
            let entry = entry.map_err(io_error("unable to list the data folder"))?;
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let relative = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let file_type = entry
                .file_type()
                .map_err(io_error("unable to list the data folder"))?;
            if file_type.is_dir() {
                if !(prefix.is_empty() && EXCLUDED_FOLDERS.contains(&name.as_str())) {
                    folders.push((entry.path(), relative));
                }
            } else if file_type.is_file() && keeps_file(&relative, &name, database_name) {
                files.push((relative, entry.path()));
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Whether a file outside the excluded folders belongs in a backup.
fn keeps_file(relative: &str, name: &str, database_name: &str) -> bool {
    let database_files = [
        database_name.to_owned(),
        format!("{database_name}-wal"),
        format!("{database_name}-shm"),
        format!("{database_name}-journal"),
    ];
    !(database_files.iter().any(|file| file == relative)
        || name.ends_with(".tmp")
        || name.starts_with(".backup-snapshot-"))
}

/// Writes one file as an entry, streaming its bytes.
fn copy_file_entry(
    writer: &mut ChunkWriter<'_, impl Write>,
    relative: &str,
    path: &Path,
    summary: &mut BackupSummary,
) -> Result<(), BackupError> {
    let mut file = File::open(path).map_err(io_error("unable to read a file to back up"))?;
    let size = file
        .metadata()
        .map_err(io_error("unable to read a file to back up"))?
        .len();
    write_entry_header(writer, relative, size)?;
    let copied = io::copy(&mut (&mut file).take(size), writer)
        .map_err(io_error("unable to read a file to back up"))?;
    if copied != size {
        return Err(BackupError::Io {
            operation: "a file changed while it was backed up",
            source: io::Error::from(ErrorKind::UnexpectedEof),
        });
    }
    summary.files += 1;
    summary.bytes += size;
    Ok(())
}

fn write_entry_header(
    writer: &mut ChunkWriter<'_, impl Write>,
    relative: &str,
    size: u64,
) -> Result<(), BackupError> {
    let path_length = u16::try_from(relative.len()).map_err(|_| BackupError::Io {
        operation: "a file name is too long to back up",
        source: io::Error::from(ErrorKind::InvalidInput),
    })?;
    let mut header = Vec::with_capacity(11 + relative.len());
    header.push(ENTRY_FILE);
    header.extend_from_slice(&path_length.to_le_bytes());
    header.extend_from_slice(relative.as_bytes());
    header.extend_from_slice(&size.to_le_bytes());
    writer
        .write_all(&header)
        .map_err(io_error("unable to write the backup"))
}

/// Reads an entry's tag, and for a file its path and size (`ENTRY_END` gives an empty path).
fn read_entry_header(
    reader: &mut ChunkReader<'_, impl Read>,
) -> Result<(u8, String, u64), BackupError> {
    let mut tag = [0u8; 1];
    reader.read_exact_plain(&mut tag)?;
    if tag[0] != ENTRY_FILE {
        return Ok((tag[0], String::new(), 0));
    }
    let mut length = [0u8; 2];
    reader.read_exact_plain(&mut length)?;
    let length = u16::from_le_bytes(length) as usize;
    if length == 0 || length > MAX_PATH_BYTES {
        return Err(BackupError::Damaged);
    }
    let mut path = vec![0u8; length];
    reader.read_exact_plain(&mut path)?;
    let path = String::from_utf8(path).map_err(|_| BackupError::Damaged)?;
    let mut size = [0u8; 8];
    reader.read_exact_plain(&mut size)?;
    Ok((tag[0], path, u64::from_le_bytes(size)))
}

/// Turns an entry path into a relative path inside the staging folder, refusing anything that
/// could leave it, Windows-style paths, the manifest's name and the `keys` folder.
fn safe_relative_path(path: &str) -> Result<PathBuf, BackupError> {
    if path.is_empty() || path.contains('\\') || path.contains(':') || path == MANIFEST {
        return Err(BackupError::Damaged);
    }
    let mut relative = PathBuf::new();
    for (index, part) in path.split('/').enumerate() {
        if part.is_empty() || part == "." || part == ".." || (index == 0 && part == "keys") {
            return Err(BackupError::Damaged);
        }
        relative.push(part);
    }
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(BackupError::Damaged);
    }
    Ok(relative)
}

fn create_private_directory(directory: &Path) -> Result<(), BackupError> {
    fs::create_dir_all(directory).map_err(io_error("unable to create the restore folder"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
            .map_err(io_error("unable to protect the restore folder"))?;
    }
    Ok(())
}

fn create_private_file(path: &Path) -> Result<File, BackupError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(io_error("unable to create a restored file"))
}

/// The nonce of chunk `counter`: prefix, counter (big-endian), last-chunk flag.
fn chunk_nonce(prefix: &[u8; NONCE_PREFIX_BYTES], counter: u32, last: bool) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[..NONCE_PREFIX_BYTES].copy_from_slice(prefix);
    nonce[NONCE_PREFIX_BYTES..11].copy_from_slice(&counter.to_be_bytes());
    nonce[11] = u8::from(last);
    nonce
}

fn chunk_cipher(
    passphrase: &str,
    salt: &[u8],
    derivation: KeyDerivationParameters,
) -> Result<Aes256Gcm, BackupError> {
    let key = derive_key_from_passphrase(passphrase, salt, derivation)
        .map_err(|error| BackupError::KeyDerivation(error.to_string()))?;
    Ok(Aes256Gcm::new(key.as_bytes().into()))
}

/// Seals the clear stream into chunks as it is written.
struct ChunkWriter<'output, W: Write> {
    output: &'output mut W,
    cipher: Aes256Gcm,
    header: [u8; HEADER_BYTES],
    prefix: [u8; NONCE_PREFIX_BYTES],
    counter: u32,
    buffer: Zeroizing<Vec<u8>>,
    chunk_bytes: usize,
}

impl<'output, W: Write> ChunkWriter<'output, W> {
    /// Derives the key, writes the clear header and returns the writer.
    fn start(
        passphrase: &str,
        settings: &BackupSettings,
        output: &'output mut W,
    ) -> Result<Self, BackupError> {
        if passphrase.is_empty() {
            return Err(BackupError::EmptyPassphrase);
        }
        let mut salt = [0u8; SALT_BYTES];
        let mut prefix = [0u8; NONCE_PREFIX_BYTES];
        OsRng.fill_bytes(&mut salt);
        OsRng.fill_bytes(&mut prefix);
        let derivation = settings.derivation;
        let mut header = [0u8; HEADER_BYTES];
        let mut position = 0;
        for part in [
            MAGIC.as_slice(),
            &[VERSION],
            &derivation.memory_kib().to_le_bytes(),
            &derivation.iterations().to_le_bytes(),
            &derivation.parallelism().to_le_bytes(),
            &salt,
            &prefix,
        ] {
            header[position..position + part.len()].copy_from_slice(part);
            position += part.len();
        }
        let cipher = chunk_cipher(passphrase, &salt, derivation)?;
        output
            .write_all(&header)
            .map_err(io_error("unable to write the backup"))?;
        Ok(Self {
            output,
            cipher,
            header,
            prefix,
            counter: 0,
            buffer: Zeroizing::new(Vec::with_capacity(settings.chunk_bytes.max(1))),
            chunk_bytes: settings
                .chunk_bytes
                .clamp(1, MAX_SEALED_CHUNK_BYTES - TAG_BYTES),
        })
    }

    /// Seals the buffered bytes as the next chunk.
    fn seal(&mut self, last: bool) -> io::Result<()> {
        let nonce = chunk_nonce(&self.prefix, self.counter, last);
        let sealed = self
            .cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &self.buffer,
                    aad: &self.header,
                },
            )
            .map_err(|_| io::Error::other("unable to seal a backup chunk"))?;
        self.output
            .write_all(&(sealed.len() as u32).to_le_bytes())?;
        self.output.write_all(&sealed)?;
        self.buffer.clear();
        self.counter = self
            .counter
            .checked_add(1)
            .ok_or_else(|| io::Error::other("the backup has too many chunks"))?;
        Ok(())
    }

    /// Seals the rest as the last chunk.
    fn finish(mut self) -> Result<(), BackupError> {
        self.seal(true)
            .and_then(|()| self.output.flush())
            .map_err(io_error("unable to write the backup"))
    }
}

impl<W: Write> Write for ChunkWriter<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let room = self.chunk_bytes - self.buffer.len();
        let taken = room.min(bytes.len());
        self.buffer.extend_from_slice(&bytes[..taken]);
        if self.buffer.len() == self.chunk_bytes {
            self.seal(false)?;
        }
        Ok(taken)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}

/// Opens chunks as the clear stream is read.
struct ChunkReader<'input, R: Read> {
    input: &'input mut R,
    cipher: Aes256Gcm,
    header: [u8; HEADER_BYTES],
    prefix: [u8; NONCE_PREFIX_BYTES],
    counter: u32,
    clear: Zeroizing<Vec<u8>>,
    position: usize,
    last_seen: bool,
}

impl<'input, R: Read> ChunkReader<'input, R> {
    /// Reads and checks the header, then derives the key.
    fn start(passphrase: &str, input: &'input mut R) -> Result<Self, BackupError> {
        let mut header = [0u8; HEADER_BYTES];
        let mut read = 0;
        while read < HEADER_BYTES {
            match input.read(&mut header[read..]) {
                Ok(0) => return Err(BackupError::NotABackup),
                Ok(count) => read += count,
                Err(error) if error.kind() == ErrorKind::Interrupted => {}
                Err(error) => return Err(io_error("unable to read the backup")(error)),
            }
        }
        if &header[..MAGIC.len()] != MAGIC {
            return Err(BackupError::NotABackup);
        }
        if header[MAGIC.len()] != VERSION {
            return Err(BackupError::UnsupportedVersion);
        }
        let number = |offset: usize| {
            let start = MAGIC.len() + 1 + offset * 4;
            u32::from_le_bytes([
                header[start],
                header[start + 1],
                header[start + 2],
                header[start + 3],
            ])
        };
        let derivation = KeyDerivationParameters::new(number(0), number(1), number(2));
        // Refuse costs that would exhaust a phone (a damaged or hostile header).
        if derivation.memory_kib() > 1024 * 1024 || derivation.iterations() > 64 {
            return Err(BackupError::Damaged);
        }
        let salt_start = MAGIC.len() + 1 + 12;
        let salt = &header[salt_start..salt_start + SALT_BYTES];
        let mut prefix = [0u8; NONCE_PREFIX_BYTES];
        prefix.copy_from_slice(&header[salt_start + SALT_BYTES..]);
        let cipher = chunk_cipher(passphrase, salt, derivation)?;
        Ok(Self {
            input,
            cipher,
            header,
            prefix,
            counter: 0,
            clear: Zeroizing::new(Vec::new()),
            position: 0,
            last_seen: false,
        })
    }

    /// Opens the next chunk when the current one is used up.
    ///
    /// Outputs: `Ok(true)` with clear bytes ready, `Ok(false)` after the last chunk, or
    /// `WrongPassphrase` (first chunk) / `Damaged`.
    fn fill(&mut self) -> Result<bool, BackupError> {
        while self.position == self.clear.len() {
            if self.last_seen {
                return Ok(false);
            }
            let mut length = [0u8; 4];
            self.read_input(&mut length)?;
            let length = u32::from_le_bytes(length) as usize;
            if !(TAG_BYTES..=MAX_SEALED_CHUNK_BYTES).contains(&length) {
                return Err(self.unopenable());
            }
            let mut sealed = vec![0u8; length];
            self.read_input(&mut sealed)?;
            let open = |last: bool| {
                let nonce = chunk_nonce(&self.prefix, self.counter, last);
                self.cipher
                    .decrypt(
                        Nonce::from_slice(&nonce),
                        Payload {
                            msg: &sealed,
                            aad: &self.header,
                        },
                    )
                    .ok()
                    .map(|clear| (clear, last))
            };
            let (clear, last) = open(false)
                .or_else(|| open(true))
                .ok_or_else(|| self.unopenable())?;
            self.clear = Zeroizing::new(clear);
            self.position = 0;
            self.last_seen = last;
            self.counter = self.counter.checked_add(1).ok_or(BackupError::Damaged)?;
        }
        Ok(true)
    }

    /// The error for a chunk that does not open: the first one points at the passphrase.
    fn unopenable(&self) -> BackupError {
        if self.counter == 0 {
            BackupError::WrongPassphrase
        } else {
            BackupError::Damaged
        }
    }

    /// Reads exactly `buffer.len()` raw bytes; an early end is damage.
    fn read_input(&mut self, buffer: &mut [u8]) -> Result<(), BackupError> {
        self.input.read_exact(buffer).map_err(|error| {
            if error.kind() == ErrorKind::UnexpectedEof {
                BackupError::Damaged
            } else {
                io_error("unable to read the backup")(error)
            }
        })
    }

    /// Fills `buffer` with clear bytes; the stream ending first is damage.
    fn read_exact_plain(&mut self, buffer: &mut [u8]) -> Result<(), BackupError> {
        let mut filled = 0;
        while filled < buffer.len() {
            if !self.fill()? {
                return Err(BackupError::Damaged);
            }
            let available = (self.clear.len() - self.position).min(buffer.len() - filled);
            buffer[filled..filled + available]
                .copy_from_slice(&self.clear[self.position..self.position + available]);
            self.position += available;
            filled += available;
        }
        Ok(())
    }

    /// Checks that the entries ended exactly at the end of the last chunk and of the file.
    fn expect_end(&mut self) -> Result<(), BackupError> {
        // Clear bytes after the end entry, in this chunk or a later one, are damage; `fill` also
        // reads an empty last chunk that follows a chunk ending exactly at the end entry.
        if self.position != self.clear.len() || self.fill()? {
            return Err(BackupError::Damaged);
        }
        let mut extra = [0u8; 1];
        match self.input.read(&mut extra) {
            Ok(0) => Ok(()),
            Ok(_) => Err(BackupError::Damaged),
            Err(error) => Err(io_error("unable to read the backup")(error)),
        }
    }
}
