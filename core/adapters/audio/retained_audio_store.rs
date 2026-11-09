//! Keeps a meeting's speech sealed on disk until the after-meeting pass has read it (#87).
//!
//! Raw audio is PII and is normally deleted as soon as each chunk is transcribed (CONTRIBUTING.md §7).
//! The owner approved keeping it longer for one purpose: re-transcribing the finished meeting in
//! long windows, on this device or a paired one. While a meeting is recorded each speech chunk is
//! appended to `<directory>/<hex meeting id>.ntra` as one record: a 4-byte little-endian length,
//! then the chunk sealed with AES-256-GCM through the database key's `EmbeddingCipher`. The record
//! names the meeting in its authenticated data, so a record moved to another meeting's file fails
//! to open. The speech is compressed with Opus (`opus_speech`) before it is sealed, about a tenth
//! of the raw size, and is sent to a paired device as kept, so it is compressed only once. Records
//! of builds before Opus hold raw samples under the `v1` context; they are still read (and
//! compressed when sent), while new records use `v2`. Plaintext buffers are wiped after use, and nothing here logs audio. The pass deletes
//! the file once it is done; deleting the meeting must delete it too. Files are readable by this
//! user only. Speech received from a paired device for combining (#88) is written to
//! `<file>.incoming` and becomes the meeting's file only once complete. An empty `<file>.passed`
//! notes that the meeting already took the pass alone while its speech waits to be combined with
//! a late recording; new speech or deletion drops it.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::Arc,
};

use zeroize::Zeroizing;

use super::opus_speech::{decode_speech, encode_speech};
use crate::{
    application::transcript_refinement::ChunkSpan,
    domain::{models::MeetingId, transcription_language::TranscriptionLanguage},
    ports::crypto::EmbeddingCipher,
};

/// File extension of a retained meeting's audio.
const FILE_EXTENSION: &str = "ntra";
/// Bytes of a record's fixed header: start and end in milliseconds, then the language code's
/// length (0 when the chunk had none), then the code and the speech: Opus under `v2`, raw 16-bit
/// little-endian samples under `v1`.
const HEADER_LENGTH: usize = 17;

/// Why retained audio could not be written or read.
#[derive(Debug, thiserror::Error)]
pub enum RetainedAudioError {
    /// The folder or file could not be written or read.
    #[error("retained audio storage failed: {0}")]
    Storage(#[from] std::io::Error),
    /// The audio could not be compressed or sealed.
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

/// One retained speech chunk as it is kept and sent: compressed with Opus (#88).
pub struct EncodedChunk {
    span: ChunkSpan,
    language: Option<TranscriptionLanguage>,
    speech: Zeroizing<Vec<u8>>,
}

impl EncodedChunk {
    /// Returns where the chunk sits in the meeting.
    pub fn span(&self) -> ChunkSpan {
        self.span
    }

    /// Returns the language the chunk was recorded with, if any.
    pub fn language(&self) -> Option<TranscriptionLanguage> {
        self.language
    }

    /// Borrows its speech as `opus_speech::encode_speech` lays it out.
    pub fn speech(&self) -> &[u8] {
        &self.speech
    }

    /// Takes its speech, still wiped when dropped.
    pub fn into_speech(self) -> Zeroizing<Vec<u8>> {
        self.speech
    }
}

/// A record's speech as it was kept.
enum KeptSpeech {
    /// Opus, as this version keeps it.
    Opus(Zeroizing<Vec<u8>>),
    /// Raw samples, as builds before Opus kept them.
    Raw(Zeroizing<Vec<i16>>),
}

/// One opened record.
struct Record {
    span: ChunkSpan,
    language: Option<TranscriptionLanguage>,
    speech: KeptSpeech,
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

    /// The authenticated data binding a record of compressed speech to its meeting.
    fn context(meeting_id: &MeetingId) -> String {
        format!("retained-audio:v2:{}", meeting_id.as_str())
    }

    /// The authenticated data of a raw record written before Opus.
    fn raw_context(meeting_id: &MeetingId) -> String {
        format!("retained-audio:v1:{}", meeting_id.as_str())
    }

    /// Appends one speech chunk to the meeting's file.
    ///
    /// Inputs: The meeting, the chunk's place in it, its language, and its 16 kHz mono samples.
    /// Outputs: `Ok(())`, or a storage or encryption error (the recording goes on without it).
    /// Side effects: Compresses the samples, creates the folder and file as needed and appends a
    /// sealed record.
    pub fn append_chunk(
        &self,
        meeting_id: &MeetingId,
        span: ChunkSpan,
        language: Option<TranscriptionLanguage>,
        samples: &[i16],
    ) -> Result<(), RetainedAudioError> {
        let speech = encode_speech(samples).map_err(|_| RetainedAudioError::Encryption)?;
        self.forget_pass_alone(meeting_id)?;
        self.append_record(
            meeting_id,
            &Self::file_name(meeting_id),
            span,
            language,
            &speech,
        )
    }

    /// Notes that the meeting took the pass alone with the speech held now (#88).
    ///
    /// Side effects: Creates an empty `<file>.passed`, readable by this user only.
    pub fn mark_passed_alone(&self, meeting_id: &MeetingId) -> Result<(), RetainedAudioError> {
        fs::create_dir_all(&self.directory)?;
        private_append(&self.passed_path(meeting_id))?;
        Ok(())
    }

    /// Reports whether the meeting took the pass alone since its speech last changed.
    ///
    /// Side effects: Reads the folder.
    pub fn passed_alone(&self, meeting_id: &MeetingId) -> bool {
        self.passed_path(meeting_id).exists()
    }

    /// The path of a meeting's pass-alone note.
    fn passed_path(&self, meeting_id: &MeetingId) -> PathBuf {
        passed_path(&self.directory, meeting_id)
    }

    /// Drops a meeting's pass-alone note. Side effects: Removes `<file>.passed` if present.
    fn forget_pass_alone(&self, meeting_id: &MeetingId) -> Result<(), RetainedAudioError> {
        match fs::remove_file(self.passed_path(meeting_id)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    /// Seals one chunk's compressed speech for `meeting_id` and appends it to `file_name`.
    fn append_record(
        &self,
        meeting_id: &MeetingId,
        file_name: &str,
        span: ChunkSpan,
        language: Option<TranscriptionLanguage>,
        speech: &[u8],
    ) -> Result<(), RetainedAudioError> {
        let code = language
            .map_or("", TranscriptionLanguage::as_str)
            .as_bytes();
        let code_length = u8::try_from(code.len()).map_err(|_| RetainedAudioError::Encryption)?;
        let mut plaintext = Zeroizing::new(Vec::with_capacity(
            HEADER_LENGTH + code.len() + speech.len(),
        ));
        plaintext.extend_from_slice(&span.start_ms.to_le_bytes());
        plaintext.extend_from_slice(&span.end_ms.to_le_bytes());
        plaintext.push(code_length);
        plaintext.extend_from_slice(code);
        plaintext.extend_from_slice(speech);
        let sealed = self
            .cipher
            .seal_secret(&Self::context(meeting_id), &plaintext)
            .map_err(|_| RetainedAudioError::Encryption)?;
        let length = u32::try_from(sealed.len()).map_err(|_| RetainedAudioError::Encryption)?;
        fs::create_dir_all(&self.directory)?;
        let mut file = private_append(&self.directory.join(file_name))?;
        let mut record = Vec::with_capacity(4 + sealed.len());
        record.extend_from_slice(&length.to_le_bytes());
        record.extend_from_slice(&sealed);
        file.write_all(&record)?;
        Ok(())
    }

    /// Starts receiving a meeting's speech from a paired device (#88), dropping any earlier
    /// unfinished copy. Until `finish_incoming`, the pass does not see it.
    ///
    /// Side effects: Creates an empty `<file>.incoming`.
    pub fn begin_incoming(&self, meeting_id: &MeetingId) -> Result<(), RetainedAudioError> {
        fs::create_dir_all(&self.directory)?;
        let path = self.incoming_path(meeting_id);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        private_append(&path)?;
        Ok(())
    }

    /// Appends one received chunk to the meeting's incoming copy, kept compressed as received.
    ///
    /// Inputs: The meeting, the chunk's place and language, and its speech as a paired device
    /// sent it (`opus_speech` layout). Outputs: `Ok(())`, `Unreadable` for speech that does not
    /// decode, or a storage or encryption error.
    /// Side effects: Decodes the speech once to check it, then appends a sealed record to
    /// `<file>.incoming`.
    pub fn append_incoming_encoded(
        &self,
        meeting_id: &MeetingId,
        span: ChunkSpan,
        language: Option<TranscriptionLanguage>,
        speech: &[u8],
    ) -> Result<(), RetainedAudioError> {
        decode_speech(speech).map_err(|_| RetainedAudioError::Unreadable)?;
        self.append_record(
            meeting_id,
            &Self::incoming_name(meeting_id),
            span,
            language,
            speech,
        )
    }

    /// Makes a fully received copy available to the pass, replacing any earlier one.
    ///
    /// Side effects: Renames `<file>.incoming` to the meeting's file.
    pub fn finish_incoming(&self, meeting_id: &MeetingId) -> Result<(), RetainedAudioError> {
        self.forget_pass_alone(meeting_id)?;
        fs::rename(
            self.incoming_path(meeting_id),
            self.directory.join(Self::file_name(meeting_id)),
        )?;
        Ok(())
    }

    /// Returns when the meeting's speech was last written here, or `None` when none is held.
    ///
    /// Side effects: Reads the file's metadata.
    pub fn written_at(&self, meeting_id: &MeetingId) -> Option<std::time::SystemTime> {
        written_at(&self.directory, meeting_id)
    }

    /// The name of a meeting's copy being received.
    fn incoming_name(meeting_id: &MeetingId) -> String {
        format!("{}.incoming", Self::file_name(meeting_id))
    }

    /// The path of a meeting's copy being received.
    fn incoming_path(&self, meeting_id: &MeetingId) -> PathBuf {
        self.directory.join(Self::incoming_name(meeting_id))
    }

    /// Reads every chunk retained for a meeting, decoded, in the order they were appended.
    ///
    /// Inputs: The meeting. Outputs: Its chunks (empty when nothing is retained), or `Unreadable`
    /// for a damaged or foreign file. Side effects: Reads the file into memory and decodes it.
    pub fn load(&self, meeting_id: &MeetingId) -> Result<Vec<RetainedChunk>, RetainedAudioError> {
        self.read_records(meeting_id)?
            .into_iter()
            .map(|record| {
                let samples = match record.speech {
                    KeptSpeech::Opus(speech) => {
                        decode_speech(&speech).map_err(|_| RetainedAudioError::Unreadable)?
                    }
                    KeptSpeech::Raw(samples) => samples,
                };
                Ok(RetainedChunk {
                    span: record.span,
                    language: record.language,
                    samples,
                })
            })
            .collect()
    }

    /// Reads every chunk retained for a meeting as it is sent to a paired device: compressed.
    ///
    /// Inputs: The meeting. Outputs: Its chunks in append order; kept Opus comes back as kept,
    /// raw speech from before Opus is compressed now. `Unreadable` for a damaged or foreign file.
    /// Side effects: Reads the file into memory.
    pub fn load_encoded(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Vec<EncodedChunk>, RetainedAudioError> {
        self.read_records(meeting_id)?
            .into_iter()
            .map(|record| {
                let speech = match record.speech {
                    KeptSpeech::Opus(speech) => speech,
                    KeptSpeech::Raw(samples) => {
                        encode_speech(&samples).map_err(|_| RetainedAudioError::Encryption)?
                    }
                };
                Ok(EncodedChunk {
                    span: record.span,
                    language: record.language,
                    speech,
                })
            })
            .collect()
    }

    /// Opens every record of a meeting's file, trying the current context and then the raw one.
    fn read_records(&self, meeting_id: &MeetingId) -> Result<Vec<Record>, RetainedAudioError> {
        let path = self.directory.join(Self::file_name(meeting_id));
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let context = Self::context(meeting_id);
        let raw_context = Self::raw_context(meeting_id);
        let mut records = Vec::new();
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
            let record = match self.cipher.open_secret(&context, sealed) {
                Ok(plaintext) => decode_record(&plaintext, false)?,
                Err(_) => {
                    let plaintext = self
                        .cipher
                        .open_secret(&raw_context, sealed)
                        .map_err(|_| RetainedAudioError::Unreadable)?;
                    decode_record(&plaintext, true)?
                }
            };
            records.push(record);
            rest = after_record;
        }
        Ok(records)
    }

    /// Deletes a meeting's retained audio.
    ///
    /// Inputs: The meeting. Outputs: Whether a file existed. Side effects: Removes the file, any
    /// unfinished received copy, and the pass-alone note.
    pub fn delete(&self, meeting_id: &MeetingId) -> Result<bool, RetainedAudioError> {
        self.forget_pass_alone(meeting_id)?;
        match fs::remove_file(self.incoming_path(meeting_id)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
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

/// Reports which meetings' speech a device holds, for the apps' status (#88), without opening it.
pub struct RetainedSpeechIndex {
    directory: PathBuf,
}

impl RetainedSpeechIndex {
    /// Creates the index over a store's folder.
    ///
    /// Inputs: The folder `RetainedAudioStore` writes. Outputs: The index. Side effects: None.
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    /// Reports whether any meeting's speech waits here, so the app can start the pass for speech
    /// left from an earlier run (#87).
    ///
    /// Outputs: True when the folder holds a speech file; false when it is empty or missing.
    /// Side effects: Lists the folder only.
    pub fn holds_any(&self) -> bool {
        let suffix = format!(".{FILE_EXTENSION}");
        fs::read_dir(&self.directory).is_ok_and(|entries| {
            entries
                .filter_map(Result::ok)
                .any(|entry| entry.file_name().to_string_lossy().ends_with(&suffix))
        })
    }

    /// Returns the meeting's speech held here: how long ago it was last written, as of `now`, and
    /// whether it already took the pass alone; `None` when none is held.
    ///
    /// Side effects: Reads file metadata only.
    pub fn held_speech(
        &self,
        meeting_id: &MeetingId,
        now: std::time::SystemTime,
    ) -> Option<crate::application::combined_meetings::HeldSpeech> {
        let written = written_at(&self.directory, meeting_id)?;
        Some(crate::application::combined_meetings::HeldSpeech {
            age: now.duration_since(written).unwrap_or_default(),
            passed_alone: passed_path(&self.directory, meeting_id).exists(),
        })
    }
}

/// When a meeting's speech file in `directory` was last written, or `None` when it has none.
fn written_at(
    directory: &std::path::Path,
    meeting_id: &MeetingId,
) -> Option<std::time::SystemTime> {
    fs::metadata(directory.join(RetainedAudioStore::file_name(meeting_id)))
        .and_then(|metadata| metadata.modified())
        .ok()
}

/// The path of a meeting's pass-alone note in `directory`.
fn passed_path(directory: &std::path::Path, meeting_id: &MeetingId) -> PathBuf {
    directory.join(format!(
        "{}.passed",
        RetainedAudioStore::file_name(meeting_id)
    ))
}

/// Opens `path` for appending, creating it readable and writable by this user only.
fn private_append(path: &std::path::Path) -> std::io::Result<fs::File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

/// Splits a record's plaintext into its span, language and speech: raw samples when `raw` (a
/// `v1` record), Opus otherwise.
fn decode_record(plaintext: &[u8], raw: bool) -> Result<Record, RetainedAudioError> {
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
    let speech = if raw {
        let (pairs, odd_byte) = body.as_chunks::<2>();
        if !odd_byte.is_empty() {
            return Err(RetainedAudioError::Unreadable);
        }
        KeptSpeech::Raw(Zeroizing::new(
            pairs.iter().map(|pair| i16::from_le_bytes(*pair)).collect(),
        ))
    } else {
        KeptSpeech::Opus(Zeroizing::new(body.to_vec()))
    };
    Ok(Record {
        span: ChunkSpan {
            start_ms: u64::from_le_bytes(start),
            end_ms: u64::from_le_bytes(end),
        },
        language,
        speech,
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
