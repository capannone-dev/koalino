//! Seals attachment files at rest (#99): meeting photos, slides, imported files and project
//! resources.
//!
//! A sealed file starts with `MAGIC` and a random 16-byte file ID, followed by records. Each
//! record is a 9-byte header (kind, clear length, sealed length, the lengths little-endian) and
//! up to `RECORD_BYTES` of contents sealed with AES-256-GCM through the database key's
//! `EmbeddingCipher`. Each record's context names the file ID, the record's position and its
//! kind, so records cannot be reordered, dropped or moved to another file without failing to
//! open. A final `end` record holds the total clear length, so a cut file is refused too.
//! Files are written readable by this user only and are never overwritten in place.
//!
//! Files written before #99 hold their contents in clear. They are still read as they are, and
//! `seal_in_place` seals them once (hosts run it over every attachment at start-up). A clear
//! file cannot be told from one an attacker replaced, but anyone who can replace files can also
//! delete them, so this adds no new power.
//!
//! Readers decrypt one record at a time and wipe each clear record when it is dropped. Contents
//! and names are never logged.

use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    adapters::storage::private_files::file_error,
    domain::errors::StorageError,
    ports::{crypto::EmbeddingCipher, storage::AttachmentFileReader},
};

/// The first bytes of a sealed file.
const MAGIC: &[u8; 8] = b"NTSEALF1";
/// Bytes of the file header: `MAGIC`, then the file ID.
const FILE_HEADER_BYTES: u64 = 24;
/// Bytes of a record header: kind (1), clear length (4), sealed length (4).
const RECORD_HEADER_BYTES: usize = 9;
/// Clear bytes per record, at most.
const RECORD_BYTES: usize = 256 * 1024;
/// Sealed bytes per record, at most: the record plus generous room for the nonce and tag.
const MAX_SEALED_RECORD_BYTES: usize = RECORD_BYTES + 1024;
/// Record kind holding contents.
const KIND_DATA: u8 = 0;
/// Record kind holding the total clear length; always the last record.
const KIND_END: u8 = 1;

/// Seals and opens attachment files with the database key.
#[derive(Clone)]
pub struct SealedFiles {
    cipher: Arc<dyn EmbeddingCipher>,
}

impl std::fmt::Debug for SealedFiles {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SealedFiles")
    }
}

/// Tells whether a file is sealed.
///
/// Inputs: The file. Outputs: `true` when it starts with the sealed-file marker, or the file
/// error. Side effects: Reads up to 8 bytes.
pub fn is_sealed(path: &Path) -> io::Result<bool> {
    let mut marker = [0_u8; 8];
    let mut file = File::open(path)?;
    let mut filled = 0;
    while filled < marker.len() {
        match file.read(&mut marker[filled..])? {
            0 => return Ok(false),
            read => filled += read,
        }
    }
    Ok(&marker == MAGIC)
}

impl SealedFiles {
    /// Creates the sealer for a database's key.
    ///
    /// Inputs: The cipher holding the database key. Side effects: None.
    pub fn new(cipher: Arc<dyn EmbeddingCipher>) -> Self {
        Self { cipher }
    }

    /// Writes a new sealed file from `contents`; never overwrites.
    ///
    /// Inputs: The new file's path and a reader of its contents (the caller bounds its length).
    /// Outputs: The number of clear bytes written, or a rejection. A partly written file is
    /// removed. Side effects: Creates, writes and syncs one file, readable only by this user.
    pub fn write_new(&self, path: &Path, contents: &mut impl Read) -> Result<u64, StorageError> {
        let mut appender = self.create_appender(path)?;
        let mut record = Zeroizing::new(vec![0_u8; RECORD_BYTES]);
        let written = loop {
            let filled = match fill(contents, &mut record) {
                Ok(filled) => filled,
                Err(error) => break Err(file_error("unable to read the file to store", &error)),
            };
            if filled == 0 {
                break Ok(());
            }
            if let Err(error) = appender.append(&record[..filled]) {
                break Err(error);
            }
        };
        match written {
            Ok(()) => appender.finish(),
            Err(error) => {
                appender.discard();
                Err(error)
            }
        }
    }

    /// Starts a new sealed file written in parts, such as one received from a paired device.
    ///
    /// Inputs: The new file's path. Outputs: The appender, or a rejection when the file exists
    /// or cannot be created. Side effects: Creates the file, readable only by this user.
    pub fn create_appender(&self, path: &Path) -> Result<SealedAppender, StorageError> {
        let mut file = create_private(path, true)
            .map_err(|error| file_error("unable to create the attachment file", &error))?;
        let file_id = Uuid::new_v4();
        let header_written = file
            .write_all(MAGIC)
            .and_then(|()| file.write_all(file_id.as_bytes()));
        if let Err(error) = header_written {
            drop(file);
            let _ = std::fs::remove_file(path);
            return Err(file_error("unable to write the attachment file", &error));
        }
        Ok(SealedAppender {
            file,
            path: path.to_path_buf(),
            cipher: Arc::clone(&self.cipher),
            file_id,
            next_index: 0,
            clear_bytes: 0,
        })
    }

    /// Opens a file for reading its clear contents; a clear file from before #99 is read as is.
    ///
    /// Inputs: The file. Outputs: The reader, or a rejection for a missing file or a malformed
    /// header. Side effects: Opens the file.
    pub fn open(&self, path: &Path) -> Result<AttachmentReader, StorageError> {
        let sealed =
            is_sealed(path).map_err(|error| file_error("unable to open the attachment", &error))?;
        let mut file = File::open(path)
            .map_err(|error| file_error("unable to open the attachment", &error))?;
        if !sealed {
            return Ok(AttachmentReader::Clear(file));
        }
        let mut header = [0_u8; FILE_HEADER_BYTES as usize];
        file.read_exact(&mut header)
            .map_err(|_| unreadable_rejection())?;
        let file_id = Uuid::from_slice(&header[8..]).map_err(|_| unreadable_rejection())?;
        Ok(AttachmentReader::Sealed(Box::new(SealedReader {
            file,
            path: path.to_path_buf(),
            cipher: Arc::clone(&self.cipher),
            file_id,
            next_index: 0,
            record: Zeroizing::new(Vec::new()),
            record_position: 0,
            clear_read: 0,
            finished: false,
        })))
    }

    /// Reads a whole file's clear contents.
    ///
    /// Inputs: The file. Outputs: The contents (wiped when dropped), or a rejection for a
    /// missing, cut, altered or foreign file. Side effects: Reads the file.
    pub fn read_all(&self, path: &Path) -> Result<Zeroizing<Vec<u8>>, StorageError> {
        let mut reader = self.open(path)?;
        let mut contents = Zeroizing::new(Vec::new());
        reader
            .read_to_end(&mut contents)
            .map_err(|_| unreadable_rejection())?;
        Ok(contents)
    }

    /// The length of a file's clear contents.
    ///
    /// Inputs: The file. Outputs: The length, or a rejection. Side effects: Reads the record
    /// headers and decrypts the end record.
    pub fn clear_len(&self, path: &Path) -> Result<u64, StorageError> {
        self.open(path)?
            .clear_len()
            .map_err(|_| unreadable_rejection())
    }

    /// Writes a clear copy of a file, replacing `destination`.
    ///
    /// Inputs: The sealed (or older clear) file and where the copy goes. Outputs: The number of
    /// bytes copied, or a rejection; a partial copy is removed. Side effects: Creates or
    /// replaces `destination`, readable only by this user.
    pub fn copy_clear(&self, source: &Path, destination: &Path) -> Result<u64, StorageError> {
        let mut reader = self.open(source)?;
        let mut copy = create_private(destination, false)
            .map_err(|error| file_error("unable to create the copy", &error))?;
        let copied = io::copy(&mut reader, &mut copy).and_then(|copied| {
            copy.flush()?;
            Ok(copied)
        });
        match copied {
            Ok(copied) => Ok(copied),
            Err(_) => {
                drop(copy);
                let _ = std::fs::remove_file(destination);
                Err(unreadable_rejection())
            }
        }
    }

    /// Seals a clear file written before #99, replacing it atomically.
    ///
    /// Inputs: The file. Outputs: `true` when it was sealed now, `false` when it already was or
    /// vanished meanwhile, or a rejection. Side effects: Writes `.<name>.sealing` beside it,
    /// then renames it over the file.
    pub fn seal_in_place(&self, path: &Path) -> Result<bool, StorageError> {
        if is_sealed(path).map_err(|error| file_error("unable to read the attachment", &error))? {
            return Ok(false);
        }
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let sealing = path.with_file_name(format!(".{name}.sealing"));
        let _ = std::fs::remove_file(&sealing);
        let mut clear = File::open(path)
            .map_err(|error| file_error("unable to read the attachment", &error))?;
        self.write_new(&sealing, &mut clear)?;
        drop(clear);
        if !path.exists() {
            // Deleted while it was being sealed: do not bring it back.
            let _ = std::fs::remove_file(&sealing);
            return Ok(false);
        }
        std::fs::rename(&sealing, path).map_err(|error| {
            let _ = std::fs::remove_file(&sealing);
            file_error("unable to replace the attachment", &error)
        })?;
        Ok(true)
    }
}

impl AttachmentFileReader for SealedFiles {
    fn copy_clear(&self, stored_path: &Path, destination: &Path) -> Result<u64, StorageError> {
        SealedFiles::copy_clear(self, stored_path, destination)
    }
}

/// Writes a sealed file in parts.
pub struct SealedAppender {
    file: File,
    path: PathBuf,
    cipher: Arc<dyn EmbeddingCipher>,
    file_id: Uuid,
    next_index: u64,
    clear_bytes: u64,
}

impl SealedAppender {
    /// Seals and appends the next part of the contents.
    ///
    /// Inputs: The part (any length; long parts become several records). Outputs: Success or a
    /// rejection. Side effects: Appends records to the file.
    pub fn append(&mut self, data: &[u8]) -> Result<(), StorageError> {
        for part in data.chunks(RECORD_BYTES) {
            self.write_record(KIND_DATA, part)?;
            self.clear_bytes += part.len() as u64;
        }
        Ok(())
    }

    /// Ends the file with its total length and syncs it.
    ///
    /// Outputs: The number of clear bytes written, or a rejection. Side effects: Appends the
    /// end record; syncs the file.
    pub fn finish(mut self) -> Result<u64, StorageError> {
        let total = self.clear_bytes;
        self.write_record(KIND_END, &total.to_le_bytes())?;
        self.file
            .flush()
            .and_then(|()| self.file.sync_all())
            .map_err(|error| file_error("unable to write the attachment file", &error))?;
        Ok(total)
    }

    /// Removes the unfinished file.
    ///
    /// Side effects: Deletes the file.
    pub fn discard(self) {
        drop(self.file);
        let _ = std::fs::remove_file(&self.path);
    }

    fn write_record(&mut self, kind: u8, clear: &[u8]) -> Result<(), StorageError> {
        let sealed = self
            .cipher
            .seal_secret(&record_context(self.file_id, self.next_index, kind), clear)
            .map_err(|_| StorageError::Rejected {
                reason: String::from("unable to encrypt the attachment"),
            })?;
        let mut header = [0_u8; RECORD_HEADER_BYTES];
        header[0] = kind;
        header[1..5].copy_from_slice(&length_u32(clear.len())?.to_le_bytes());
        header[5..9].copy_from_slice(&length_u32(sealed.len())?.to_le_bytes());
        self.file
            .write_all(&header)
            .and_then(|()| self.file.write_all(&sealed))
            .map_err(|error| file_error("unable to write the attachment file", &error))?;
        self.next_index += 1;
        Ok(())
    }
}

/// Reads an attachment's clear contents.
pub enum AttachmentReader {
    /// A file written before #99.
    Clear(File),
    /// A sealed file.
    Sealed(Box<SealedReader>),
}

impl AttachmentReader {
    /// The length of the clear contents.
    ///
    /// Outputs: The length, or an error for an unreadable file. Side effects: For a sealed file,
    /// reads it again from the start in a second handle and decrypts the end record.
    pub fn clear_len(&mut self) -> io::Result<u64> {
        match self {
            Self::Clear(file) => Ok(file.metadata()?.len()),
            Self::Sealed(reader) => reader.clear_len(),
        }
    }

    /// Moves to a clear offset, so reading continues from there.
    ///
    /// Inputs: The offset, at most the clear length. Outputs: Success, or an error for an offset
    /// past the end or an unreadable file. Side effects: Reads record headers and decrypts the
    /// record holding the offset.
    pub fn seek_to(&mut self, offset: u64) -> io::Result<()> {
        match self {
            Self::Clear(file) => {
                if offset > file.metadata()?.len() {
                    return Err(unreadable());
                }
                file.seek(SeekFrom::Start(offset)).map(|_| ())
            }
            Self::Sealed(reader) => reader.seek_to(offset),
        }
    }
}

impl Read for AttachmentReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Clear(file) => file.read(buffer),
            Self::Sealed(reader) => reader.read(buffer),
        }
    }
}

/// Decrypts a sealed file one record at a time.
pub struct SealedReader {
    file: File,
    path: PathBuf,
    cipher: Arc<dyn EmbeddingCipher>,
    file_id: Uuid,
    /// The position of the next record to read.
    next_index: u64,
    /// The clear contents of the current record.
    record: Zeroizing<Vec<u8>>,
    /// How much of `record` was already given out.
    record_position: usize,
    /// Clear bytes in the records read so far.
    clear_read: u64,
    /// The end record was read and checked.
    finished: bool,
}

/// A record's header.
struct RecordHeader {
    kind: u8,
    clear_len: usize,
    sealed_len: usize,
}

impl SealedReader {
    /// Reads the next record's header; `None` at the end of the file.
    fn next_header(&mut self) -> io::Result<Option<RecordHeader>> {
        let mut header = [0_u8; RECORD_HEADER_BYTES];
        if fill(&mut self.file, &mut header)? != RECORD_HEADER_BYTES {
            return Ok(None);
        }
        let clear_len = u32::from_le_bytes([header[1], header[2], header[3], header[4]]) as usize;
        let sealed_len = u32::from_le_bytes([header[5], header[6], header[7], header[8]]) as usize;
        if !matches!(header[0], KIND_DATA | KIND_END)
            || clear_len > RECORD_BYTES
            || sealed_len > MAX_SEALED_RECORD_BYTES
        {
            return Err(unreadable());
        }
        Ok(Some(RecordHeader {
            kind: header[0],
            clear_len,
            sealed_len,
        }))
    }

    /// Reads and opens the record whose header was just read.
    fn open_record(&mut self, header: &RecordHeader) -> io::Result<Zeroizing<Vec<u8>>> {
        let mut sealed = vec![0_u8; header.sealed_len];
        self.file
            .read_exact(&mut sealed)
            .map_err(|_| unreadable())?;
        let clear = self
            .cipher
            .open_secret(
                &record_context(self.file_id, self.next_index, header.kind),
                &sealed,
            )
            .map_err(|_| unreadable())?;
        if clear.len() != header.clear_len {
            return Err(unreadable());
        }
        self.next_index += 1;
        Ok(clear)
    }

    /// Checks the end record against what was read, and that nothing follows it.
    fn check_end(&mut self, header: &RecordHeader) -> io::Result<()> {
        let clear = self.open_record(header)?;
        let total: [u8; 8] = clear.as_slice().try_into().map_err(|_| unreadable())?;
        let mut trailing = [0_u8; 1];
        if u64::from_le_bytes(total) != self.clear_read || self.file.read(&mut trailing)? != 0 {
            return Err(unreadable());
        }
        self.finished = true;
        Ok(())
    }

    fn clear_len(&mut self) -> io::Result<u64> {
        let mut fresh = SealedReader {
            file: File::open(&self.path)?,
            path: self.path.clone(),
            cipher: Arc::clone(&self.cipher),
            file_id: self.file_id,
            next_index: 0,
            record: Zeroizing::new(Vec::new()),
            record_position: 0,
            clear_read: 0,
            finished: false,
        };
        fresh.file.seek(SeekFrom::Start(FILE_HEADER_BYTES))?;
        while let Some(header) = fresh.next_header()? {
            if header.kind == KIND_END {
                fresh.check_end(&header)?;
                return Ok(fresh.clear_read);
            }
            fresh
                .file
                .seek(SeekFrom::Current(header.sealed_len as i64))?;
            fresh.clear_read += header.clear_len as u64;
            fresh.next_index += 1;
        }
        Err(unreadable())
    }

    fn seek_to(&mut self, offset: u64) -> io::Result<()> {
        self.file.seek(SeekFrom::Start(FILE_HEADER_BYTES))?;
        self.next_index = 0;
        self.clear_read = 0;
        self.record = Zeroizing::new(Vec::new());
        self.record_position = 0;
        self.finished = false;
        loop {
            let record_start = self.file.stream_position()?;
            let Some(header) = self.next_header()? else {
                return Err(unreadable());
            };
            if header.kind == KIND_END {
                if offset != self.clear_read {
                    return Err(unreadable());
                }
                // Leave the end record to be checked by the next read.
                self.file.seek(SeekFrom::Start(record_start))?;
                return Ok(());
            }
            let record_end = self.clear_read + header.clear_len as u64;
            if offset < record_end {
                self.record = self.open_record(&header)?;
                self.record_position = (offset - self.clear_read) as usize;
                self.clear_read = record_end;
                return Ok(());
            }
            self.file
                .seek(SeekFrom::Current(header.sealed_len as i64))?;
            self.clear_read = record_end;
            self.next_index += 1;
        }
    }
}

impl Read for SealedReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.record_position < self.record.len() {
                let count = buffer.len().min(self.record.len() - self.record_position);
                buffer[..count].copy_from_slice(
                    &self.record[self.record_position..self.record_position + count],
                );
                self.record_position += count;
                return Ok(count);
            }
            if self.finished || buffer.is_empty() {
                return Ok(0);
            }
            let Some(header) = self.next_header()? else {
                // Cut short: the end record is missing.
                return Err(unreadable());
            };
            if header.kind == KIND_END {
                self.check_end(&header)?;
                return Ok(0);
            }
            self.record = self.open_record(&header)?;
            self.record_position = 0;
            self.clear_read += header.clear_len as u64;
        }
    }
}

/// The context a record is sealed under.
fn record_context(file_id: Uuid, index: u64, kind: u8) -> String {
    let kind = if kind == KIND_END { "end" } else { "data" };
    format!("attachment-file:v1:{}:{kind}:{index}", file_id.simple())
}

/// Reads until `buffer` is full or the reader ends; returns how much was read.
fn fill(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}

/// Creates a file readable only by this user; `new_only` refuses an existing one, otherwise it
/// is emptied.
fn create_private(path: &Path, new_only: bool) -> io::Result<File> {
    let mut open_options = OpenOptions::new();
    open_options.write(true);
    if new_only {
        open_options.create_new(true);
    } else {
        open_options.create(true).truncate(true);
    }
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut open_options, 0o600);
    open_options.open(path)
}

/// A record length as stored.
fn length_u32(length: usize) -> Result<u32, StorageError> {
    u32::try_from(length).map_err(|_| StorageError::Rejected {
        reason: String::from("an attachment record is too large"),
    })
}

/// The read error for a cut, altered or foreign file.
fn unreadable() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "the attachment file is unreadable",
    )
}

/// The rejection for a cut, altered or foreign file.
fn unreadable_rejection() -> StorageError {
    StorageError::Rejected {
        reason: String::from(
            "the attachment file is unreadable: cut short, altered, or sealed with another key",
        ),
    }
}
