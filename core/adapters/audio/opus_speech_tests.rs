//! Specifies how kept speech is compressed with Opus (#88): it keeps its length and timing, is far
//! smaller than raw samples, and damaged data is refused rather than misread.

use super::opus_speech::{OpusSpeechError, decode_speech, encode_speech, encode_speech_at};

/// A voiced, speech-like signal: a 140 Hz voice with harmonics whose loudness changes like
/// syllables, at 16 kHz.
pub(super) fn voice(seconds: f32) -> Vec<i16> {
    let count = (seconds * 16_000.0) as usize;
    (0..count)
        .map(|index| {
            let time = index as f32 / 16_000.0;
            let syllables = 0.55 + 0.45 * (2.0 * std::f32::consts::PI * 4.0 * time).sin();
            let voiced: f32 = (1..=12)
                .map(|harmonic| {
                    let frequency = 140.0 * harmonic as f32;
                    (2.0 * std::f32::consts::PI * frequency * time).sin() / harmonic as f32
                })
                .sum();
            (voiced * syllables * 5_000.0) as i16
        })
        .collect()
}

/// Correlation of the two signals with `second` shifted by `lag` samples.
pub(super) fn correlation(first: &[i16], second: &[i16], lag: isize) -> f64 {
    let (mut product, mut first_energy, mut second_energy) = (0.0, 0.0, 0.0);
    for (index, sample) in first.iter().enumerate() {
        let Some(other) = index
            .checked_add_signed(lag)
            .and_then(|shifted| second.get(shifted))
        else {
            continue;
        };
        let (a, b) = (f64::from(*sample), f64::from(*other));
        product += a * b;
        first_energy += a * a;
        second_energy += b * b;
    }
    product / (first_energy.sqrt() * second_energy.sqrt()).max(1.0)
}

#[test]
fn speech_comes_back_as_long_as_it_went_in_and_in_time() {
    let original = voice(3.0);

    let encoded = encode_speech(&original).unwrap_or_else(|error| panic!("{error}"));
    let decoded = decode_speech(&encoded).unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(decoded.len(), original.len());
    let aligned = correlation(&original, &decoded, 0);
    assert!(aligned > 0.8, "correlation {aligned}");
    for lag in [-48, -16, 16, 48] {
        assert!(
            correlation(&original, &decoded, lag) < aligned,
            "the decoded speech is shifted by {lag} samples"
        );
    }
}

#[test]
fn speech_is_a_tenth_of_its_raw_size_or_less() {
    let original = voice(10.0);

    let encoded = encode_speech(&original).unwrap_or_else(|error| panic!("{error}"));

    let raw_bytes = original.len() * 2;
    assert!(
        encoded.len() * 10 <= raw_bytes,
        "{} bytes for {raw_bytes} raw",
        encoded.len()
    );
}

#[test]
fn lengths_that_are_not_whole_frames_and_nothing_at_all_survive() {
    for length in [0, 1, 319, 321, 16_007] {
        let original = voice(1.1);
        let original = &original[..length];

        let encoded = encode_speech(original).unwrap_or_else(|error| panic!("{error}"));
        let decoded = decode_speech(&encoded).unwrap_or_else(|error| panic!("{error}"));

        assert_eq!(decoded.len(), length);
    }
}

#[test]
fn damaged_speech_is_refused() {
    let encoded = encode_speech(&voice(1.0)).unwrap_or_else(|error| panic!("{error}"));

    for damaged in [
        &encoded[..3],
        &encoded[..encoded.len() - 1],
        &[0xff; 64][..],
    ] {
        assert!(
            matches!(decode_speech(damaged), Err(OpusSpeechError::Malformed)),
            "{} bytes were accepted",
            damaged.len()
        );
    }
    // A claimed length the frames cannot hold is refused before anything is allocated.
    let mut huge = encoded.to_vec();
    huge[..4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        decode_speech(&huge),
        Err(OpusSpeechError::Malformed)
    ));
}

/// Passes every WAV in `NOTE_TAKER_OPUS_IN_DIR` through Opus at `NOTE_TAKER_OPUS_BITRATE` (bits
/// per second), in 30 s chunks as the recorder keeps them, and writes the decoded speech under the
/// same names to `NOTE_TAKER_OPUS_OUT_DIR`, to measure how compression changes transcription.
/// Run by hand: `cargo test --release --lib opus_speech_tests::transcode -- --ignored`.
#[test]
#[ignore = "measurement tool; needs a folder of 16 kHz mono WAV files"]
fn transcode_a_folder_of_recordings() {
    let setting = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("set {name}"));
    let input = std::path::PathBuf::from(setting("NOTE_TAKER_OPUS_IN_DIR"));
    let output = std::path::PathBuf::from(setting("NOTE_TAKER_OPUS_OUT_DIR"));
    let bitrate: i32 = setting("NOTE_TAKER_OPUS_BITRATE")
        .parse()
        .unwrap_or_else(|error| panic!("{error}"));
    std::fs::create_dir_all(&output).unwrap_or_else(|error| panic!("{error}"));
    let mut entries: Vec<std::path::PathBuf> = std::fs::read_dir(&input)
        .unwrap_or_else(|error| panic!("{error}"))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "wav"))
        .collect();
    entries.sort();
    let (mut raw_bytes, mut encoded_bytes) = (0_usize, 0_usize);
    for path in entries {
        let file = std::fs::File::open(&path).unwrap_or_else(|error| panic!("{error}"));
        let mut pcm =
            super::pcm_input::PcmInput::wav(Box::new(file) as Box<dyn std::io::Read + Send>)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let mut samples = Vec::new();
        while let Some(block) = pcm.read_block().unwrap_or_else(|error| panic!("{error}")) {
            samples.extend(block);
        }
        let mut decoded = Vec::with_capacity(samples.len());
        for chunk in samples.chunks(30 * 16_000) {
            let encoded =
                encode_speech_at(chunk, bitrate).unwrap_or_else(|error| panic!("{error}"));
            encoded_bytes += encoded.len();
            decoded.extend_from_slice(
                &decode_speech(&encoded).unwrap_or_else(|error| panic!("{error}")),
            );
        }
        raw_bytes += samples.len() * 2;
        let name = path.file_name().unwrap_or_else(|| panic!("a file name"));
        std::fs::write(
            output.join(name),
            super::chunk_audio_store::encode_wav_pcm16(&decoded, 16_000),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    }
    println!("{bitrate} bit/s: {encoded_bytes} bytes encoded for {raw_bytes} raw");
}
