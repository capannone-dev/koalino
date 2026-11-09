//! Specifies the WAV wrapper for bridge PCM (#102).

use super::pcm_wav::{WAV_HEADER_LENGTH, wav_from_pcm16_le};

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
