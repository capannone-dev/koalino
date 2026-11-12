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

/// Cuts one time range out of a mono 16-bit PCM WAV (#116).
///
/// A voice's review clip keeps only that voice's span of the chunk, so naming it never means
/// listening to someone else.
///
/// Inputs: `wav_bytes` as the recorder writes them (RIFF/WAVE, PCM, one channel, 16 bits, any
/// rate); `start_ms` and `end_ms` from the start of that audio. A range past the end is cut at the
/// last sample.
/// Outputs: A canonical WAV of the range, or `None` when the audio is not such a WAV or the range
/// holds no sample.
/// Side effects: None; the bytes are raw audio and must never be logged.
pub fn slice_pcm16_wav(wav_bytes: &[u8], start_ms: u64, end_ms: u64) -> Option<Vec<u8>> {
    let (sample_rate_hz, samples) = pcm16_mono_samples(wav_bytes)?;
    let sample_at = |time_ms: u64| {
        let index = u128::from(time_ms) * u128::from(sample_rate_hz) / 1_000;
        usize::try_from(index)
            .unwrap_or(usize::MAX)
            .saturating_mul(2)
    };
    let first_byte = sample_at(start_ms).min(samples.len());
    let end_byte = sample_at(end_ms).min(samples.len());
    (first_byte < end_byte)
        .then(|| wav_from_pcm16_le(&samples[first_byte..end_byte], sample_rate_hz))
}

/// Finds the sample rate and sample bytes of a mono 16-bit PCM WAV.
///
/// Inputs: WAV bytes. Outputs: `(rate, data bytes)`, or `None` for any other audio or a truncated
/// file. Side effects: None.
pub(crate) fn pcm16_mono_samples(wav_bytes: &[u8]) -> Option<(u32, &[u8])> {
    if wav_bytes.get(..4)? != b"RIFF" || wav_bytes.get(8..12)? != b"WAVE" {
        return None;
    }
    let read_u16 =
        |bytes: &[u8], at: usize| Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?));
    let read_u32 =
        |bytes: &[u8], at: usize| Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
    let mut sample_rate_hz = None;
    let mut position = 12;
    while position + 8 <= wav_bytes.len() {
        let chunk_length = usize::try_from(read_u32(wav_bytes, position + 4)?).ok()?;
        let body_start = position + 8;
        let body = wav_bytes.get(body_start..body_start.checked_add(chunk_length)?)?;
        match &wav_bytes[position..position + 4] {
            b"fmt " => {
                let is_mono_pcm16 =
                    read_u16(body, 0)? == 1 && read_u16(body, 2)? == 1 && read_u16(body, 14)? == 16;
                if !is_mono_pcm16 {
                    return None;
                }
                sample_rate_hz = Some(read_u32(body, 4)?).filter(|rate| *rate > 0);
            }
            b"data" => {
                let whole_samples = &body[..body.len() - body.len() % 2];
                return Some((sample_rate_hz?, whole_samples));
            }
            _ => {}
        }
        // Chunks are padded to an even length.
        position = body_start + chunk_length + chunk_length % 2;
    }
    None
}
