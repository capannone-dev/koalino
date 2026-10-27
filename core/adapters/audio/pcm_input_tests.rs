//! Specifies reading 16 kHz mono PCM from WAV files and raw streams in fixed-size blocks.

use std::io::Cursor;

use crate::adapters::audio::{chunk_audio_store::encode_wav_pcm16, pcm_input::PcmInput};

/// Reads every block from `input` and concatenates the samples.
fn read_all(mut input: PcmInput) -> Vec<i16> {
    let mut samples = Vec::new();
    while let Some(block) = input
        .read_block()
        .unwrap_or_else(|error| panic!("block should read: {error}"))
    {
        assert!(block.len() <= 1_600, "blocks hold at most 100 ms");
        samples.extend(block);
    }
    samples
}

/// Builds a WAV header with arbitrary format fields, followed by `data`.
fn wav_with_format(
    channels: u16,
    sample_rate_hz: u32,
    bits_per_sample: u16,
    data: &[u8],
) -> Vec<u8> {
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate_hz.to_le_bytes());
    wav.extend_from_slice(&(sample_rate_hz * u32::from(channels) * 2).to_le_bytes());
    wav.extend_from_slice(&(channels * 2).to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
    wav.extend_from_slice(data);
    wav
}

/// Verifies a canonical 16 kHz mono WAV round-trips in 100 ms blocks.
#[test]
fn wav_samples_are_read_in_blocks() {
    let samples: Vec<i16> = (0..4_000).map(|index| (index % 300) as i16).collect();
    let input = PcmInput::wav(Box::new(Cursor::new(encode_wav_pcm16(&samples, 16_000))))
        .unwrap_or_else(|error| panic!("WAV header should parse: {error}"));

    assert_eq!(read_all(input), samples);
}

/// Verifies chunks before `data` (e.g. LIST metadata) are skipped, including odd padding.
#[test]
fn unrelated_wav_chunks_are_skipped() {
    let canonical = encode_wav_pcm16(&[7, 8, 9], 16_000);
    let mut wav = canonical[..36].to_vec();
    wav.extend_from_slice(b"LIST");
    wav.extend_from_slice(&3_u32.to_le_bytes());
    wav.extend_from_slice(&[1, 2, 3, 0]);
    wav.extend_from_slice(&canonical[36..]);
    let input = PcmInput::wav(Box::new(Cursor::new(wav)))
        .unwrap_or_else(|error| panic!("WAV header should parse: {error}"));

    assert_eq!(read_all(input), vec![7, 8, 9]);
}

/// Verifies formats the pipeline cannot use are rejected with a readable reason.
#[test]
fn unsupported_wav_formats_are_rejected() {
    for (channels, sample_rate_hz, bits_per_sample) in
        [(2, 16_000, 16), (1, 44_100, 16), (1, 16_000, 8)]
    {
        let error = PcmInput::wav(Box::new(Cursor::new(wav_with_format(
            channels,
            sample_rate_hz,
            bits_per_sample,
            &[0, 0],
        ))))
        .err()
        .unwrap_or_else(|| {
            panic!("format {channels}/{sample_rate_hz}/{bits_per_sample} must fail")
        });
        assert!(error.to_string().contains("16 kHz mono 16-bit"), "{error}");
    }
    assert!(PcmInput::wav(Box::new(Cursor::new(b"not a wav file".to_vec()))).is_err());
}

/// Verifies raw little-endian streams are read as-is, dropping a trailing odd byte.
#[test]
fn raw_streams_are_read_as_little_endian_samples() {
    let mut bytes: Vec<u8> = [1_i16, -2, 300]
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    bytes.push(0xFF);
    let input = PcmInput::raw(Box::new(Cursor::new(bytes)));

    assert_eq!(read_all(input), vec![1, -2, 300]);
}
