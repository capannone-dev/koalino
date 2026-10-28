//! Specifies converting interleaved device audio at any rate to 16 kHz mono PCM16: block-size
//! independence, the output length, a passed speech-band tone, a rejected tone above 8 kHz, and
//! the downmix, clipping, and format conversions.

use std::f32::consts::TAU;

use crate::adapters::audio::pcm_resampler::{PcmResampler, TARGET_SAMPLE_RATE_HZ};

/// `seconds` of a sine at `frequency_hz` and `amplitude`, sampled at `sample_rate_hz`.
fn sine(frequency_hz: f32, amplitude: f32, sample_rate_hz: u32, seconds: f32) -> Vec<f32> {
    let sample_count = (sample_rate_hz as f32 * seconds) as usize;
    (0..sample_count)
        .map(|index| amplitude * (TAU * frequency_hz * index as f32 / sample_rate_hz as f32).sin())
        .collect()
}

/// Root mean square of `samples` (PCM16), skipping the filter's warm-up.
fn rms_after_warm_up(samples: &[i16]) -> f32 {
    let steady = &samples[samples.len() / 4..];
    let sum: f64 = steady
        .iter()
        .map(|&sample| f64::from(sample) * f64::from(sample))
        .sum();
    (sum / steady.len() as f64).sqrt() as f32
}

#[test]
fn one_second_at_48_khz_becomes_one_second_at_16_khz() {
    let mut resampler = PcmResampler::new(48_000, 1);

    let output = resampler.push_f32(&sine(440.0, 0.5, 48_000, 1.0));

    let expected = TARGET_SAMPLE_RATE_HZ as usize;
    assert!(
        output.len().abs_diff(expected) <= 2,
        "got {} samples",
        output.len()
    );
}

#[test]
fn the_output_does_not_depend_on_how_the_input_is_split() {
    let input = sine(700.0, 0.4, 44_100, 0.5);
    let mut whole = PcmResampler::new(44_100, 1);
    let mut pieces = PcmResampler::new(44_100, 1);

    let in_one_block = whole.push_f32(&input);
    let in_odd_blocks: Vec<i16> = input
        .chunks(441 - 7)
        .flat_map(|block| pieces.push_f32(block))
        .collect();

    assert_eq!(in_one_block, in_odd_blocks);
}

#[test]
fn a_speech_band_tone_keeps_its_level() {
    let mut resampler = PcmResampler::new(48_000, 1);

    let output = resampler.push_f32(&sine(1_000.0, 0.5, 48_000, 1.0));

    let expected_rms = 0.5 * 32_767.0 / 2_f32.sqrt();
    let ratio = rms_after_warm_up(&output) / expected_rms;
    assert!((0.95..=1.05).contains(&ratio), "level ratio {ratio}");
}

#[test]
fn a_tone_above_8_khz_is_filtered_out_instead_of_aliasing() {
    let mut resampler = PcmResampler::new(48_000, 1);

    let output = resampler.push_f32(&sine(12_000.0, 0.5, 48_000, 1.0));

    let input_rms = 0.5 * 32_767.0 / 2_f32.sqrt();
    let attenuation_db = 20.0 * (rms_after_warm_up(&output) / input_rms).log10();
    assert!(attenuation_db < -30.0, "only {attenuation_db} dB");
}

#[test]
fn audio_already_at_16_khz_passes_through() {
    let mut resampler = PcmResampler::new(16_000, 1);

    let output = resampler.push_f32(&[0.0, 0.25, -0.5, 1.0]);

    assert_eq!(output, vec![0, 8_192, -16_384, 32_767]);
}

#[test]
fn audio_at_8_khz_is_upsampled() {
    let mut resampler = PcmResampler::new(8_000, 1);

    let output = resampler.push_f32(&sine(300.0, 0.5, 8_000, 1.0));

    assert!(output.len().abs_diff(16_000) <= 2, "got {}", output.len());
}

#[test]
fn stereo_is_averaged_to_mono() {
    let mut resampler = PcmResampler::new(16_000, 2);

    let output = resampler.push_f32(&[0.5, -0.5, 0.5, 0.0, 1.0, 1.0]);

    assert_eq!(output, vec![0, 8_192, 32_767]);
}

#[test]
fn a_partial_frame_waits_for_its_other_channels() {
    let mut resampler = PcmResampler::new(16_000, 2);

    let first = resampler.push_f32(&[0.5, 0.5, 0.25]);
    let second = resampler.push_f32(&[0.25]);

    assert_eq!(first, vec![16_384]);
    assert_eq!(second, vec![8_192]);
}

#[test]
fn samples_beyond_full_scale_are_clipped() {
    let mut resampler = PcmResampler::new(16_000, 1);

    let output = resampler.push_f32(&[1.5, -1.5]);

    assert_eq!(output, vec![32_767, -32_767]);
}

#[test]
fn integer_device_formats_are_scaled_to_the_same_level() {
    let mut from_i16 = PcmResampler::new(16_000, 1);
    let mut from_i32 = PcmResampler::new(16_000, 1);
    let mut from_u16 = PcmResampler::new(16_000, 1);

    assert_eq!(from_i16.push_i16(&[16_384, -32_768]), vec![16_384, -32_767]);
    assert_eq!(from_i32.push_i32(&[1 << 30]), vec![16_384]);
    assert_eq!(from_u16.push_u16(&[32_768, 49_152]), vec![0, 16_384]);
}
