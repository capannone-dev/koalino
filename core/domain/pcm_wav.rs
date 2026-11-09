//! Wraps mono 16-bit PCM in a RIFF/WAVE header (#102). Online-meeting bridges deliver raw PCM,
//! while the speaker-separation port reads WAV, as the app's own recordings are stored.

/// Size of the canonical RIFF/WAVE header this module writes.
pub const WAV_HEADER_LENGTH: usize = 44;

/// Wraps little-endian mono 16-bit PCM in a canonical RIFF/WAVE file.
///
/// Inputs: `pcm_le_bytes` (an odd trailing byte is dropped) and their `sample_rate_hz`.
/// Outputs: A 44-byte header followed by the samples, unchanged. The size fields saturate for
/// inputs above 4 GiB, which chunks never reach.
/// Side effects: None; the returned bytes are raw audio and must never be logged.
pub fn wav_from_pcm16_le(pcm_le_bytes: &[u8], sample_rate_hz: u32) -> Vec<u8> {
    let sample_bytes = &pcm_le_bytes[..pcm_le_bytes.len() - pcm_le_bytes.len() % 2];
    let data_length = u32::try_from(sample_bytes.len()).unwrap_or(u32::MAX);
    let mut wav_bytes = Vec::with_capacity(WAV_HEADER_LENGTH + sample_bytes.len());
    wav_bytes.extend_from_slice(b"RIFF");
    wav_bytes.extend_from_slice(&data_length.saturating_add(36).to_le_bytes());
    wav_bytes.extend_from_slice(b"WAVEfmt ");
    wav_bytes.extend_from_slice(&16_u32.to_le_bytes()); // fmt chunk length
    wav_bytes.extend_from_slice(&1_u16.to_le_bytes()); // PCM format
    wav_bytes.extend_from_slice(&1_u16.to_le_bytes()); // channels
    wav_bytes.extend_from_slice(&sample_rate_hz.to_le_bytes());
    wav_bytes.extend_from_slice(&sample_rate_hz.saturating_mul(2).to_le_bytes()); // byte rate
    wav_bytes.extend_from_slice(&2_u16.to_le_bytes()); // block align
    wav_bytes.extend_from_slice(&16_u16.to_le_bytes()); // bits per sample
    wav_bytes.extend_from_slice(b"data");
    wav_bytes.extend_from_slice(&data_length.to_le_bytes());
    wav_bytes.extend_from_slice(sample_bytes);
    wav_bytes
}
