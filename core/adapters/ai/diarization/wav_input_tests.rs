//! Specifies how chunk audio reaches the diarization models: the 16 kHz mono PCM16 WAV the
//! recorder writes decodes to samples in [-1, 1], and any other format is refused.

use super::wav_input::decode_wav_pcm16_mono;
use crate::{
    adapters::audio::chunk_audio_store::encode_wav_pcm16, domain::errors::DiarizationError,
};

/// The recorder's WAV decodes to the same samples, scaled to [-1, 1].
#[test]
fn recorder_wav_round_trips() {
    let wav = encode_wav_pcm16(&[0, 16_384, -32_768, 32_767], 16_000);

    let samples = decode_wav_pcm16_mono(&wav)
        .unwrap_or_else(|error| panic!("recorder WAV must decode: {error}"));

    assert_eq!(samples, vec![0.0, 0.5, -1.0, 32_767.0 / 32_768.0]);
}

/// Another sample rate, a truncated file, or bytes that are not WAV are refused.
#[test]
fn other_formats_are_refused() {
    let mut truncated = encode_wav_pcm16(&[1, 2, 3], 16_000);
    truncated.truncate(46);

    for bytes in [
        encode_wav_pcm16(&[1, 2, 3], 44_100),
        truncated,
        b"not a wav file at all, just some text".to_vec(),
    ] {
        assert!(matches!(
            decode_wav_pcm16_mono(&bytes),
            Err(DiarizationError::InvalidAudio { .. })
        ));
    }
}
