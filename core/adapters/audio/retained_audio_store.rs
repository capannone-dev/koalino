//! Keeps a meeting's speech sealed on disk until the after-meeting pass has read it (#87).
//!
//! Raw audio is PII and is normally deleted as soon as each chunk is transcribed (CONTRIBUTING.md §7).
//! The owner approved keeping it longer for one purpose: re-transcribing the finished meeting in
//! long windows, on this device or a paired one. While a meeting is recorded each speech chunk is
//! appended to `<directory>/<hex meeting id>.ntra` as one record: a 4-byte little-endian length,
//! then the chunk sealed with AES-256-GCM through the database key's `EmbeddingCipher`. The record
//! names the meeting in its authenticated data, so a record moved to another meeting's file fails
//! to open. Plaintext buffers are wiped after use, and nothing here logs audio. The pass deletes
//! the file once it is done; deleting the meeting must delete it too.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::Arc,
};

use zeroize::Zeroizing;

use crate::{
    application::transcript_refinement::ChunkSpan,
    domain::{models::MeetingId, transcription_language::TranscriptionLanguage},
    ports::crypto::EmbeddingCipher,
};

/// File extension of a retained meeting's audio.
const FILE_EXTENSION: &str = "ntra";
/// Bytes of a record's fixed header: start and end in milliseconds, then the language code's
/// length (0 when the chunk had none), then the code and the samples.
const HEADER_LENGTH: usize = 17;

/// Why retained audio could not be written or read.
#[derive(Debug, thiserror::Error)]
pub enum RetainedAudioError {
    /// The folder or file could not be written or read.
    #[error("retained audio storage failed: {0}")]
    Storage(#[from] std::io::Error),
    /// The audio could not be sealed.
    #[error("retained audio could not be encrypted")]
    Encryption,
    /// The file is cut short, tampered with, sealed under another key, or belongs to another
    /// meeting.
    #[error("retained audio is unreadable")]
    Unreadable,
}

/// One retained speech chunk.
pub struct RetainedChunk {
    span: ChunkSpan,
    language: Option<TranscriptionLanguage>,
    samples: Zeroizing<Vec<i16>>,
}

impl RetainedChunk {
    /// Returns where the chunk sits in the meeting.
    pub fn span(&self) -> ChunkSpan {
        self.span
    }

    /// Returns the language the chunk was recorded with, if any.
    pub fn language(&self) -> Option<TranscriptionLanguage> {
        self.language
    }

    /// Borrows its 16 kHz mono samples; they are wiped when the chunk is dropped.
    pub fn samples(&self) -> &[i16] {
        &self.samples
    }
}

/// Sealed, append-only files of meeting speech, one per meeting.
pub struct RetainedAudioStore {
    directory: PathBuf,
    cipher: Arc<dyn EmbeddingCipher>,
}

impl RetainedAudioStore {
    /// Creates a store over `directory`, created on first write.
    ///
    /// Inputs: The folder (inside the app's data folder) and the database key's cipher.
    /// Outputs: The store. Side effects: None.
    pub fn new(directory: PathBuf, cipher: Arc<dyn EmbeddingCipher>) -> Self {
        Self { directory, cipher }
    }

    /// Returns the file name used for `meeting_id`: its bytes in hex, so any id is a safe name.
    pub fn file_name(meeting_id: &MeetingId) -> String {
        let hex: String = meeting_id
            .as_str()
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!("{hex}.{FILE_EXTENSION}")
    }

    /// The authenticated data binding a record to its meeting.
    fn context(meeting_id: &MeetingId) -> String {
        format!("retained-audio:v1:{}", meeting_id.as_str())
    }

    /// Appends one speech chunk to the meeting's file.
    ///
    /// Inputs: The meeting, the chunk's place in it, its language, and its 16 kHz mono samples.
    /// Outputs: `Ok(())`, or a storage or encryption error (the recording goes on without it).
    /// Side effects: Creates the folder and file as needed and appends a sealed record.
    pub fn append_chunk(
        &self,
        meeting_id: &MeetingId,
        span: ChunkSpan,
        language: Option<TranscriptionLanguage>,
        samples: &[i16],
    ) -> Result<(), RetainedAudioError> {
        let code = language
            .map_or("", TranscriptionLanguage::as_str)
            .as_bytes();
        let code_length = u8::try_from(code.len()).map_err(|_| RetainedAudioError::Encryption)?;
        let mut plaintext = Zeroizing::new(Vec::with_capacity(
            HEADER_LENGTH + code.len() + samples.len() * 2,
        ));
        plaintext.extend_from_slice(&span.start_ms.to_le_bytes());
        plaintext.extend_from_slice(&span.end_ms.to_le_bytes());
        plaintext.push(code_length);
        plaintext.extend_from_slice(code);
        for sample in samples {
            plaintext.extend_from_slice(&sample.to_le_bytes());
        }
        let sealed = self
            .cipher
            .seal_secret(&Self::context(meeting_id), &plaintext)
            .map_err(|_| RetainedAudioError::Encryption)?;
        let length = u32::try_from(sealed.len()).map_err(|_| RetainedAudioError::Encryption)?;
        fs::create_dir_all(&self.directory)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.directory.join(Self::file_name(meeting_id)))?;
        let mut record = Vec::with_capacity(4 + sealed.len());
        record.extend_from_slice(&length.to_le_bytes());
        record.extend_from_slice(&sealed);
        file.write_all(&record)?;
        Ok(())
    }

    /// Reads every chunk retained for a meeting, in the order they were appended.
    ///
    /// Inputs: The meeting. Outputs: Its chunks (empty when nothing is retained), or `Unreadable`
    /// for a damaged or foreign file. Side effects: Reads the file into memory.
    pub fn load(&self, meeting_id: &MeetingId) -> Result<Vec<RetainedChunk>, RetainedAudioError> {
        let path = self.directory.join(Self::file_name(meeting_id));
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let context = Self::context(meeting_id);
        let mut chunks = Vec::new();
        let mut rest = bytes.as_slice();
        while !rest.is_empty() {
            let (length_bytes, after_length) = rest
                .split_first_chunk::<4>()
                .ok_or(RetainedAudioError::Unreadable)?;
            let length = u32::from_le_bytes(*length_bytes) as usize;
            if after_length.len() < length {
                return Err(RetainedAudioError::Unreadable);
            }
            let (sealed, after_record) = after_length.split_at(length);
            let plaintext = self
                .cipher
                .open_secret(&context, sealed)
                .map_err(|_| RetainedAudioError::Unreadable)?;
            chunks.push(decode_record(&plaintext)?);
            rest = after_record;
        }
        Ok(chunks)
    }

    /// Deletes a meeting's retained audio.
    ///
    /// Inputs: The meeting. Outputs: Whether a file existed. Side effects: Removes the file.
    pub fn delete(&self, meeting_id: &MeetingId) -> Result<bool, RetainedAudioError> {
        match fs::remove_file(self.directory.join(Self::file_name(meeting_id))) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    /// Lists the meetings with retained audio, in name order.
    ///
    /// Outputs: Their ids; an unreadable folder or a foreign file name is skipped.
    /// Side effects: Reads the folder.
    pub fn pending_meetings(&self) -> Vec<MeetingId> {
        let Ok(entries) = fs::read_dir(&self.directory) else {
            return Vec::new();
        };
        let mut meetings: Vec<MeetingId> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                let hex = name.strip_suffix(&format!(".{FILE_EXTENSION}"))?;
                decode_hex(hex).map(MeetingId::new)
            })
            .collect();
        meetings.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        meetings
    }
}

/// Splits a record's plaintext into its span, language and samples.
fn decode_record(plaintext: &[u8]) -> Result<RetainedChunk, RetainedAudioError> {
    let (header, rest) = plaintext
        .split_at_checked(HEADER_LENGTH)
        .ok_or(RetainedAudioError::Unreadable)?;
    let (code, body) = rest
        .split_at_checked(usize::from(header[16]))
        .ok_or(RetainedAudioError::Unreadable)?;
    let language = if code.is_empty() {
        None
    } else {
        let text = std::str::from_utf8(code).map_err(|_| RetainedAudioError::Unreadable)?;
        Some(TranscriptionLanguage::parse(text).map_err(|_| RetainedAudioError::Unreadable)?)
    };
    let mut start = [0_u8; 8];
    let mut end = [0_u8; 8];
    start.copy_from_slice(&header[..8]);
    end.copy_from_slice(&header[8..16]);
    let (pairs, odd_byte) = body.as_chunks::<2>();
    if !odd_byte.is_empty() {
        return Err(RetainedAudioError::Unreadable);
    }
    let samples: Vec<i16> = pairs.iter().map(|pair| i16::from_le_bytes(*pair)).collect();
    Ok(RetainedChunk {
        span: ChunkSpan {
            start_ms: u64::from_le_bytes(start),
            end_ms: u64::from_le_bytes(end),
        },
        language,
        samples: Zeroizing::new(samples),
    })
}

/// Decodes a lowercase hex file name back to a meeting id.
fn decode_hex(hex: &str) -> Option<String> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    let bytes: Option<Vec<u8>> = (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(hex.get(index..index + 2)?, 16).ok())
        .collect();
    String::from_utf8(bytes?).ok()
}
