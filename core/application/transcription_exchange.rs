//! Encodes the messages that ask a paired device to transcribe a chunk (Issue #59).
//!
//! A request is `NTT1` and a response `NTT2`, each followed by a body sealed with the pair key and
//! bound to the sender and receiver. Raw audio and transcript text are private, so they travel only
//! inside the sealed value, are wiped after use, and are never logged. The sender holds the pair
//! key but is still peer input, so decoding bounds every value.

use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    application::crypto::pair_key::{open_from_peer, seal_for_peer},
    domain::{models::DeviceId, peer_trust::PairKey},
};

/// Marks a request.
const REQUEST_HEADER: &[u8; 4] = b"NTT1";
/// Marks a response.
const RESPONSE_HEADER: &[u8; 4] = b"NTT2";
/// The largest message, leaving room below the transport's 16 MiB payload limit for sealing.
const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024 - 1024;
/// The longest context or transcript text accepted, in bytes.
const MAX_TEXT_BYTES: usize = 1024 * 1024;
/// The job ID's length.
const JOB_ID_BYTES: usize = 16;

/// Why a transcription message could not be built or used.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TranscriptionMessageError {
    /// The chunk does not fit one message.
    #[error("transcription message is too large")]
    TooLarge,
    /// The message or its body cannot be parsed, or holds implausible values.
    #[error("transcription message is malformed")]
    Malformed,
    /// The message does not open with the pair key in this direction, or was changed.
    #[error("transcription message failed authentication")]
    Unauthenticated,
    /// Encryption failed.
    #[error("transcription message could not be encrypted")]
    EncryptionFailed,
}

/// A decoded request.
#[derive(Debug, Eq, PartialEq)]
pub struct TranscriptionRequest {
    /// Identifies the job on the requesting device.
    pub job_id: Uuid,
    /// The previous chunk's text.
    pub previous_text: Option<String>,
    /// 16 kHz mono samples.
    pub samples: Vec<i16>,
}

/// A decoded response.
#[derive(Debug, Eq, PartialEq)]
pub struct TranscriptionResponse {
    /// Identifies the job on the requesting device.
    pub job_id: Uuid,
    /// The text, or `None` when the other device could not transcribe the chunk.
    pub text: Option<String>,
}

/// Reports whether `payload` is a request. Side effects: None.
pub fn is_transcription_request(payload: &[u8]) -> bool {
    payload.starts_with(REQUEST_HEADER)
}

/// Reports whether `payload` is a response. Side effects: None.
pub fn is_transcription_response(payload: &[u8]) -> bool {
    payload.starts_with(RESPONSE_HEADER)
}

/// Seals a request.
///
/// Inputs: The job, the pair key, and the two device IDs the message is bound to.
/// Outputs: The message, or `TooLarge` for a chunk that does not fit. Side effects: None.
pub fn encode_transcription_request(
    request: &TranscriptionRequest,
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<Vec<u8>, TranscriptionMessageError> {
    let previous_text = request.previous_text.as_deref().unwrap_or_default();
    let previous_length =
        u32::try_from(previous_text.len()).map_err(|_| TranscriptionMessageError::TooLarge)?;
    let body_length = JOB_ID_BYTES + 4 + previous_text.len() + request.samples.len() * 2;
    if body_length > MAX_MESSAGE_BYTES || previous_text.len() > MAX_TEXT_BYTES {
        return Err(TranscriptionMessageError::TooLarge);
    }
    let mut body = Zeroizing::new(Vec::with_capacity(body_length));
    body.extend_from_slice(request.job_id.as_bytes());
    body.extend_from_slice(&previous_length.to_be_bytes());
    body.extend_from_slice(previous_text.as_bytes());
    for sample in &request.samples {
        body.extend_from_slice(&sample.to_le_bytes());
    }
    seal(REQUEST_HEADER, &body, pair_key, from, to)
}

/// Opens and validates a request.
///
/// Outputs: The request, or why it was refused. Side effects: None.
pub fn decode_transcription_request(
    message: &[u8],
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<TranscriptionRequest, TranscriptionMessageError> {
    let body = open(REQUEST_HEADER, message, pair_key, from, to)?;
    let (job_id, rest) = split_job_id(&body)?;
    let (text_length, rest) = rest
        .split_first_chunk::<4>()
        .ok_or(TranscriptionMessageError::Malformed)?;
    let text_length = u32::from_be_bytes(*text_length) as usize;
    if text_length > MAX_TEXT_BYTES {
        return Err(TranscriptionMessageError::Malformed);
    }
    let (text, audio) = rest
        .split_at_checked(text_length)
        .ok_or(TranscriptionMessageError::Malformed)?;
    let text = std::str::from_utf8(text).map_err(|_| TranscriptionMessageError::Malformed)?;
    let (samples, odd_byte) = audio.as_chunks::<2>();
    if !odd_byte.is_empty() {
        return Err(TranscriptionMessageError::Malformed);
    }
    Ok(TranscriptionRequest {
        job_id,
        previous_text: (!text.is_empty()).then(|| text.to_owned()),
        samples: samples
            .iter()
            .map(|sample| i16::from_le_bytes(*sample))
            .collect(),
    })
}

/// Seals a response.
///
/// Inputs: The result, the pair key, and the two device IDs the message is bound to.
/// Outputs: The message, or `TooLarge` for an implausibly long text. Side effects: None.
pub fn encode_transcription_response(
    response: &TranscriptionResponse,
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<Vec<u8>, TranscriptionMessageError> {
    let text = response.text.as_deref().unwrap_or_default();
    if text.len() > MAX_TEXT_BYTES {
        return Err(TranscriptionMessageError::TooLarge);
    }
    let mut body = Zeroizing::new(Vec::with_capacity(JOB_ID_BYTES + 1 + text.len()));
    body.extend_from_slice(response.job_id.as_bytes());
    body.push(u8::from(response.text.is_some()));
    body.extend_from_slice(text.as_bytes());
    seal(RESPONSE_HEADER, &body, pair_key, from, to)
}

/// Opens and validates a response.
///
/// Outputs: The response, or why it was refused. Side effects: None.
pub fn decode_transcription_response(
    message: &[u8],
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<TranscriptionResponse, TranscriptionMessageError> {
    let body = open(RESPONSE_HEADER, message, pair_key, from, to)?;
    let (job_id, rest) = split_job_id(&body)?;
    let (succeeded, text) = rest
        .split_first()
        .ok_or(TranscriptionMessageError::Malformed)?;
    if text.len() > MAX_TEXT_BYTES {
        return Err(TranscriptionMessageError::Malformed);
    }
    let text = std::str::from_utf8(text).map_err(|_| TranscriptionMessageError::Malformed)?;
    Ok(TranscriptionResponse {
        job_id,
        text: (*succeeded == 1).then(|| text.to_owned()),
    })
}

/// Wipes a request's audio and context. Side effects: Overwrites them in memory.
pub fn wipe_request(request: &mut TranscriptionRequest) {
    request.samples.zeroize();
    if let Some(previous_text) = request.previous_text.as_mut() {
        previous_text.zeroize();
    }
}

/// Seals `body` under `header`, which is also the sealing purpose.
fn seal(
    header: &[u8; 4],
    body: &[u8],
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<Vec<u8>, TranscriptionMessageError> {
    let sealed = seal_for_peer(pair_key, header, from, to, body)
        .map_err(|_| TranscriptionMessageError::EncryptionFailed)?;
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
) -> Result<Zeroizing<Vec<u8>>, TranscriptionMessageError> {
    let sealed = message
        .strip_prefix(header)
        .ok_or(TranscriptionMessageError::Malformed)?;
    open_from_peer(pair_key, header, from, to, sealed)
        .map_err(|_| TranscriptionMessageError::Unauthenticated)
}

/// Splits the job ID off a body.
fn split_job_id(body: &[u8]) -> Result<(Uuid, &[u8]), TranscriptionMessageError> {
    let (job_id, rest) = body
        .split_first_chunk::<JOB_ID_BYTES>()
        .ok_or(TranscriptionMessageError::Malformed)?;
    Ok((Uuid::from_bytes(*job_id), rest))
}
