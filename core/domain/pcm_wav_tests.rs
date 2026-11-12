//! Specifies the WAV wrapper for bridge PCM (#102).

use super::pcm_wav::{WAV_HEADER_LENGTH, slice_pcm16_wav, wav_from_pcm16_le};

#[test]
fn pcm_is_wrapped_in_a_canonical_mono_16_bit_header() {
    let pcm = [1_u8, 0, 255, 127, 0, 128];
    let wav = wav_from_pcm16_le(&pcm, 16_000);

    assert_eq!(wav.len(), WAV_HEADER_LENGTH + pcm.len());
    assert_eq!(&wav[..4], b"RIFF");
    assert_eq!(u32::from_le_bytes([wav[4], wav[5], wav[6], wav[7]]), 36 + 6);
    assert_eq!(&wav[8..16], b"WAVEfmt ");
    assert_eq!(u16::from_le_bytes([wav[20], wav[21]]), 1, "PCM");
    assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 1, "mono");
    assert_eq!(
        u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]),
        16_000
    );
    assert_eq!(
        u32::from_le_bytes([wav[28], wav[29], wav[30], wav[31]]),
        32_000
    );
    assert_eq!(
        u16::from_le_bytes([wav[34], wav[35]]),
        16,
        "bits per sample"
    );
    assert_eq!(&wav[36..40], b"data");
    assert_eq!(u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]), 6);
    assert_eq!(&wav[WAV_HEADER_LENGTH..], &pcm);
}

#[test]
fn an_odd_trailing_byte_is_dropped() {
    let wav = wav_from_pcm16_le(&[1, 2, 3], 16_000);
    assert_eq!(&wav[WAV_HEADER_LENGTH..], &[1, 2]);
}

/// Builds a 16 kHz mono WAV whose sample `n` holds the value `n`, so a slice shows where it began.
fn counting_wav(sample_count: u16) -> Vec<u8> {
    let pcm: Vec<u8> = (0..sample_count).flat_map(u16::to_le_bytes).collect();
    wav_from_pcm16_le(&pcm, 16_000)
}

#[test]
fn a_slice_keeps_only_the_samples_of_its_time_range() {
    let wav = counting_wav(16_000); // one second

    let slice = slice_pcm16_wav(&wav, 250, 500).unwrap_or_else(|| panic!("a valid range slices"));

    assert_eq!(
        slice.len(),
        WAV_HEADER_LENGTH + 4_000 * 2,
        "250 ms of 16 kHz samples"
    );
    assert_eq!(&slice[..4], b"RIFF");
    assert_eq!(
        u16::from_le_bytes([slice[44], slice[45]]),
        4_000,
        "starts at 250 ms"
    );
    assert_eq!(
        u16::from_le_bytes([slice[slice.len() - 2], slice[slice.len() - 1]]),
        7_999,
        "ends just before 500 ms"
    );
}

#[test]
fn a_slice_past_the_end_is_cut_at_the_last_sample() {
    let wav = counting_wav(16_000);

    let slice = slice_pcm16_wav(&wav, 900, 5_000).unwrap_or_else(|| panic!("the range overlaps"));

    assert_eq!(slice.len(), WAV_HEADER_LENGTH + 1_600 * 2);
}

#[test]
fn audio_that_is_not_a_mono_16_bit_wav_or_an_empty_range_does_not_slice() {
    assert_eq!(slice_pcm16_wav(&[1, 2, 3, 4], 0, 100), None, "not a WAV");
    let wav = counting_wav(16_000);
    assert_eq!(slice_pcm16_wav(&wav, 500, 500), None, "empty range");
    assert_eq!(slice_pcm16_wav(&wav, 2_000, 3_000), None, "past the audio");
    let mut stereo = wav.clone();
    stereo[22] = 2;
    assert_eq!(slice_pcm16_wav(&stereo, 0, 100), None, "two channels");
}

#[test]
fn a_review_clip_plays_only_its_voices_span() {
    use crate::domain::models::{MeetingId, UnassignedVoiceClip};
    // A clip saved before #116 holds the whole chunk and marks its voice at 250–500 ms.
    let whole_chunk = UnassignedVoiceClip::new(
        uuid::Uuid::new_v4(),
        MeetingId::new("meeting"),
        250,
        500,
        counting_wav(16_000),
    );
    let audio = whole_chunk.voice_audio();
    assert_eq!(audio.len(), WAV_HEADER_LENGTH + 4_000 * 2);
    assert_eq!(u16::from_le_bytes([audio[44], audio[45]]), 4_000);

    let not_a_wav = UnassignedVoiceClip::new(
        uuid::Uuid::new_v4(),
        MeetingId::new("meeting"),
        0,
        10,
        vec![1, 2, 3],
    );
    assert_eq!(
        &*not_a_wav.voice_audio(),
        &[1, 2, 3],
        "other audio plays as it is"
    );
}
