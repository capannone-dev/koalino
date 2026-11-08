//! Encodes the sealed messages that carry a meeting's speech to the combining device (#88).
//!
//! A part is `NTR3` and its receipt `NTR2`, each followed by a body sealed with the pair key and
//! bound to the sender and receiver (purpose = header). Inside a part:
//!
//! ```text
//! job ID (16) ‖ meeting ID length (u16) ‖ meeting ID ‖ part index (u32) ‖ part count (u32)
//! ‖ chunk count (u32) ‖ per chunk: start ms (u64), end ms (u64), language length (u8), language,
//!   speech length (u32) ‖ every chunk's speech, in chunk order
//! ```
//!
//! The speech is compressed with Opus as the device keeps it (`adapters::audio::opus_speech`);
//! this module carries it without reading it, and the receiver checks it decodes before storing
//! it. Parts of raw samples from builds before Opus (`NTR1`) are not recognised, so a device
//! updated on one side only keeps its speech until both are. A receipt is the job ID and one byte:
//! 1 when the part was stored. Integers are big-endian. Speech travels only inside the sealed
//! value and is wiped after use. The sender holds the pair key but is still peer input, so
//! decoding bounds every value.

use uuid::Uuid;
use zeroize::Zeroizing;

use crate::{
    application::crypto::pair_key::{open_from_peer, seal_for_peer},
    domain::{
        models::{DeviceId, MeetingId},
        peer_trust::PairKey,
        transcription_language::TranscriptionLanguage,
    },
    ports::speech_transfer::{SpeechChunk, SpeechPart},
};

/// Marks a part of compressed speech (`NTR1` carried raw samples).
const PART_HEADER: &[u8; 4] = b"NTR3";
/// Marks a receipt.
const RECEIPT_HEADER: &[u8; 4] = b"NTR2";
/// The largest message, below the transport's 16 MiB payload limit.
const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024 - 1024;
/// The compressed speech a part carries at most (about an hour), leaving room for the chunk table
/// and sealing.
pub const PART_AUDIO_BUDGET_BYTES: usize = 12 * 1024 * 1024;
/// Most parts one meeting may be split into.
const MAX_PARTS: u32 = 2_048;
/// Most chunks in one part.
const MAX_CHUNKS: u32 = 4_096;
/// Longest accepted meeting ID, in bytes.
const MAX_MEETING_ID_BYTES: usize = 128;
/// The job ID's length.
const JOB_ID_BYTES: usize = 16;

/// Why a speech message could not be built or used.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SpeechMessageError {
    /// The part does not fit one message.
    #[error("speech message is too large")]
    TooLarge,
    /// The message or its body cannot be parsed, or holds implausible values.
    #[error("speech message is malformed")]
    Malformed,
    /// The message does not open with the pair key in this direction, or was changed.
    #[error("speech message failed authentication")]
    Unauthenticated,
    /// Encryption failed.
    #[error("speech message could not be encrypted")]
    EncryptionFailed,
}

/// A decoded receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpeechReceipt {
    /// The part's job on the sending device.
    pub job_id: Uuid,
    /// Whether the part was stored.
    pub stored: bool,
}

/// Reports whether `payload` is a part. Side effects: None.
pub fn is_speech_part(payload: &[u8]) -> bool {
    payload.starts_with(PART_HEADER)
}

/// Reports whether `payload` is a receipt. Side effects: None.
pub fn is_speech_receipt(payload: &[u8]) -> bool {
    payload.starts_with(RECEIPT_HEADER)
}

/// Splits a meeting's speech into parts of at most `audio_budget_bytes` of speech each.
///
/// Inputs: The meeting, its chunks in time order, and the budget. Outputs: The numbered parts;
/// a chunk larger than the budget gets a part of its own, and no speech is one empty part.
/// Side effects: None.
pub fn split_into_parts(
    meeting_id: &MeetingId,
    chunks: Vec<SpeechChunk>,
    audio_budget_bytes: usize,
) -> Vec<SpeechPart> {
    let mut groups: Vec<Vec<SpeechChunk>> = vec![Vec::new()];
    let mut group_bytes = 0;
    for chunk in chunks {
        let chunk_bytes = chunk.speech.len();
        if group_bytes + chunk_bytes > audio_budget_bytes
            && groups.last().is_some_and(|group| !group.is_empty())
        {
            groups.push(Vec::new());
            group_bytes = 0;
        }
        group_bytes += chunk_bytes;
        if let Some(group) = groups.last_mut() {
            group.push(chunk);
        }
    }
    let part_count = u32::try_from(groups.len()).unwrap_or(u32::MAX);
    groups
        .into_iter()
        .enumerate()
        .map(|(index, chunks)| SpeechPart {
            meeting_id: meeting_id.clone(),
            part_index: u32::try_from(index).unwrap_or(u32::MAX),
            part_count,
            chunks,
        })
        .collect()
}

/// Seals a part.
///
/// Inputs: The job, the part, the pair key, and the two device IDs the message is bound to.
/// Outputs: The message, or `TooLarge`. Side effects: None.
pub fn encode_speech_part(
    job_id: Uuid,
    part: &SpeechPart,
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<Vec<u8>, SpeechMessageError> {
    let meeting_id = part.meeting_id.as_str().as_bytes();
    let meeting_id_length =
        u16::try_from(meeting_id.len()).map_err(|_| SpeechMessageError::TooLarge)?;
    let chunk_count = u32::try_from(part.chunks.len()).map_err(|_| SpeechMessageError::TooLarge)?;
    let audio_bytes: usize = part.chunks.iter().map(|chunk| chunk.speech.len()).sum();
    let body_length =
        JOB_ID_BYTES + 2 + meeting_id.len() + 12 + part.chunks.len() * 24 + audio_bytes;
    if body_length > MAX_MESSAGE_BYTES {
        return Err(SpeechMessageError::TooLarge);
    }
    let mut body = Zeroizing::new(Vec::with_capacity(body_length));
    body.extend_from_slice(job_id.as_bytes());
    body.extend_from_slice(&meeting_id_length.to_be_bytes());
    body.extend_from_slice(meeting_id);
    body.extend_from_slice(&part.part_index.to_be_bytes());
    body.extend_from_slice(&part.part_count.to_be_bytes());
    body.extend_from_slice(&chunk_count.to_be_bytes());
    for chunk in &part.chunks {
        let language = chunk.language.map_or("", TranscriptionLanguage::as_str);
        let speech_length =
            u32::try_from(chunk.speech.len()).map_err(|_| SpeechMessageError::TooLarge)?;
        body.extend_from_slice(&chunk.start_ms.to_be_bytes());
        body.extend_from_slice(&chunk.end_ms.to_be_bytes());
        body.push(u8::try_from(language.len()).map_err(|_| SpeechMessageError::TooLarge)?);
        body.extend_from_slice(language.as_bytes());
        body.extend_from_slice(&speech_length.to_be_bytes());
    }
    for chunk in &part.chunks {
        body.extend_from_slice(&chunk.speech);
    }
    seal(PART_HEADER, &body, pair_key, from, to)
}

/// Reads `N` bytes off the front of `rest`.
fn take<'body, const N: usize>(
    rest: &mut &'body [u8],
) -> Result<&'body [u8; N], SpeechMessageError> {
    let (taken, remainder) = rest
        .split_first_chunk::<N>()
        .ok_or(SpeechMessageError::Malformed)?;
    *rest = remainder;
    Ok(taken)
}

/// Reads `length` bytes off the front of `rest`.
fn take_bytes<'body>(
    rest: &mut &'body [u8],
    length: usize,
) -> Result<&'body [u8], SpeechMessageError> {
    let (taken, remainder) = rest
        .split_at_checked(length)
        .ok_or(SpeechMessageError::Malformed)?;
    *rest = remainder;
    Ok(taken)
}

/// Opens and validates a part.
///
/// Outputs: The job and the part, or why it was refused. Side effects: None.
pub fn decode_speech_part(
    message: &[u8],
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<(Uuid, SpeechPart), SpeechMessageError> {
    let body = open(PART_HEADER, message, pair_key, from, to)?;
    let mut rest: &[u8] = &body;
    let job_id = Uuid::from_bytes(*take::<JOB_ID_BYTES>(&mut rest)?);
    let meeting_id_length = usize::from(u16::from_be_bytes(*take::<2>(&mut rest)?));
    if meeting_id_length == 0 || meeting_id_length > MAX_MEETING_ID_BYTES {
        return Err(SpeechMessageError::Malformed);
    }
    let meeting_id = std::str::from_utf8(take_bytes(&mut rest, meeting_id_length)?)
        .map_err(|_| SpeechMessageError::Malformed)?;
    if meeting_id.chars().any(char::is_control) {
        return Err(SpeechMessageError::Malformed);
    }
    let part_index = u32::from_be_bytes(*take::<4>(&mut rest)?);
    let part_count = u32::from_be_bytes(*take::<4>(&mut rest)?);
    let chunk_count = u32::from_be_bytes(*take::<4>(&mut rest)?);
    if part_count == 0
        || part_count > MAX_PARTS
        || part_index >= part_count
        || chunk_count > MAX_CHUNKS
    {
        return Err(SpeechMessageError::Malformed);
    }
    let mut table = Vec::with_capacity(chunk_count as usize);
    for _ in 0..chunk_count {
        let start_ms = u64::from_be_bytes(*take::<8>(&mut rest)?);
        let end_ms = u64::from_be_bytes(*take::<8>(&mut rest)?);
        let language_length = usize::from(take::<1>(&mut rest)?[0]);
        let language = match take_bytes(&mut rest, language_length)? {
            [] => None,
            text => Some(
                std::str::from_utf8(text)
                    .ok()
                    .and_then(|text| TranscriptionLanguage::parse(text).ok())
                    .ok_or(SpeechMessageError::Malformed)?,
            ),
        };
        let speech_length = u32::from_be_bytes(*take::<4>(&mut rest)?) as usize;
        if end_ms < start_ms {
            return Err(SpeechMessageError::Malformed);
        }
        table.push((start_ms, end_ms, language, speech_length));
    }
    let total: usize = table.iter().map(|(_, _, _, length)| length).sum();
    if total != rest.len() {
        return Err(SpeechMessageError::Malformed);
    }
    let mut chunks = Vec::with_capacity(table.len());
    for (start_ms, end_ms, language, speech_length) in table {
        chunks.push(SpeechChunk {
            start_ms,
            end_ms,
            language,
            speech: Zeroizing::new(take_bytes(&mut rest, speech_length)?.to_vec()),
        });
    }
    Ok((
        job_id,
        SpeechPart {
            meeting_id: MeetingId::new(meeting_id),
            part_index,
            part_count,
            chunks,
        },
    ))
}

/// Seals a receipt.
///
/// Outputs: The message. Side effects: None.
pub fn encode_speech_receipt(
    receipt: &SpeechReceipt,
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<Vec<u8>, SpeechMessageError> {
    let mut body = Vec::with_capacity(JOB_ID_BYTES + 1);
    body.extend_from_slice(receipt.job_id.as_bytes());
    body.push(u8::from(receipt.stored));
    seal(RECEIPT_HEADER, &body, pair_key, from, to)
}

/// Opens and validates a receipt.
///
/// Outputs: The receipt, or why it was refused. Side effects: None.
pub fn decode_speech_receipt(
    message: &[u8],
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<SpeechReceipt, SpeechMessageError> {
    let body = open(RECEIPT_HEADER, message, pair_key, from, to)?;
    let mut rest: &[u8] = &body;
    let job_id = Uuid::from_bytes(*take::<JOB_ID_BYTES>(&mut rest)?);
    match rest {
        [stored @ (0 | 1)] => Ok(SpeechReceipt {
            job_id,
            stored: *stored == 1,
        }),
        _ => Err(SpeechMessageError::Malformed),
    }
}

/// Seals `body` under `header`, which is also the sealing purpose.
fn seal(
    header: &[u8; 4],
    body: &[u8],
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<Vec<u8>, SpeechMessageError> {
    let sealed = seal_for_peer(pair_key, header, from, to, body)
        .map_err(|_| SpeechMessageError::EncryptionFailed)?;
    let mut message = header.to_vec();
    message.extend_from_slice(&sealed);
    Ok(message)
}

/// Opens a message sealed by `seal`.
fn open(
    header: &[u8; 4],
    message: &[u8],
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<Zeroizing<Vec<u8>>, SpeechMessageError> {
    let sealed = message
        .strip_prefix(header)
        .ok_or(SpeechMessageError::Malformed)?;
    open_from_peer(pair_key, header, from, to, sealed)
        .map_err(|_| SpeechMessageError::Unauthenticated)
}
