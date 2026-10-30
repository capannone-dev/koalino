//! Decodes the chunk audio the recorder hands to diarization (Issue #44).
//!
//! The recorder encodes every chunk as a canonical 16 kHz mono PCM16 WAV
//! (`chunk_audio_store::encode_wav_pcm16`); the models take 16 kHz mono samples in [-1, 1]. Any
//! other format is refused rather than resampled, so a wrong input never yields plausible-looking
//! speakers. The bytes are raw audio and are never logged.

use crate::domain::errors::DiarizationError;

/// The only sample rate the diarization models accept.
pub const SAMPLE_RATE_HZ: u32 = 16_000;

/// Decodes a 16 kHz mono PCM16 WAV.
///
/// Inputs: WAV bytes, as the recorder writes them.
/// Outputs: Samples in [-1, 1], or `InvalidAudio` for anything but RIFF/WAVE PCM, one channel,
/// 16 kHz, 16 bits, with a complete data chunk.
/// Side effects: None.
pub fn decode_wav_pcm16_mono(wav_bytes: &[u8]) -> Result<Vec<f32>, DiarizationError> {
    if wav_bytes.len() < 12 || &wav_bytes[..4] != b"RIFF" || &wav_bytes[8..12] != b"WAVE" {
        return Err(invalid("not a RIFF/WAVE file"));
    }
    let mut format_valid = false;
    let mut position = 12;
    while position + 8 <= wav_bytes.len() {
        let chunk_id = &wav_bytes[position..position + 4];
        let chunk_length = read_u32(wav_bytes, position + 4)? as usize;
        let body_start = position + 8;
        let body_end = body_start
            .checked_add(chunk_length)
            .filter(|end| *end <= wav_bytes.len())
            .ok_or_else(|| invalid("a WAV chunk is truncated"))?;
        let body = &wav_bytes[body_start..body_end];
        match chunk_id {
            b"fmt " => {
                if body.len() < 16 {
                    return Err(invalid("the WAV format chunk is too short"));
                }
                let audio_format = u16::from_le_bytes([body[0], body[1]]);
                let channels = u16::from_le_bytes([body[2], body[3]]);
                let sample_rate = read_u32(body, 4)?;
                let bits_per_sample = u16::from_le_bytes([body[14], body[15]]);
                if audio_format != 1
                    || channels != 1
                    || sample_rate != SAMPLE_RATE_HZ
                    || bits_per_sample != 16
                {
                    return Err(invalid("audio must be 16 kHz mono 16-bit PCM"));
                }
                format_valid = true;
            }
            b"data" => {
                if !format_valid {
                    return Err(invalid("the WAV data chunk precedes its format"));
                }
                if !body.len().is_multiple_of(2) {
                    return Err(invalid("the WAV data chunk ends mid-sample"));
                }
                return Ok(body
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|sample| f32::from(i16::from_le_bytes(*sample)) / 32_768.0)
                    .collect());
            }
            _ => {}
        }
        // Chunks are padded to an even length.
        position = body_end + (chunk_length % 2);
    }
    Err(invalid("the WAV file has no data chunk"))
}

/// Reads a little-endian u32 at `offset`.
fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, DiarizationError> {
    bytes
        .get(offset..offset + 4)
        .and_then(|field| <[u8; 4]>::try_from(field).ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| invalid("a WAV header field is truncated"))
}

/// Builds an `InvalidAudio` error with a privacy-safe reason.
fn invalid(reason: &str) -> DiarizationError {
    DiarizationError::InvalidAudio {
        reason: reason.to_owned(),
    }
}
