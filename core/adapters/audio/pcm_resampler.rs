//! Converts interleaved device audio at any sample rate and channel count to the 16 kHz mono
//! PCM16 the recorder expects (Issue #45).
//!
//! Each frame is averaged to mono. Above 16 kHz, a windowed-sinc low-pass filter (Blackman
//! window, cutoff 7 kHz) removes everything the 16 kHz output cannot represent, so it does not
//! fold back into the speech band. Linear interpolation then picks the output samples. The read
//! position is kept as an exact fraction (in 1/16000 of an input sample), so the output is
//! identical however the device splits its buffers. Audio is never logged.

use std::f64::consts::PI;

/// Sample rate of the output, which the recorder and Whisper expect.
pub const TARGET_SAMPLE_RATE_HZ: u32 = 16_000;

/// Upper edge of the passband (Hz); speech intelligibility lives well below it.
const CUTOFF_HZ: f64 = 7_000.0;

/// Width of the filter's transition band (Hz); the stopband starts at 8.6 kHz.
const TRANSITION_HZ: f64 = 1_600.0;

/// Converts one device stream; create one per opened stream.
pub struct PcmResampler {
    /// Interleaved channels per frame (at least 1).
    channels: usize,
    /// Input samples per second.
    input_rate_hz: u64,
    /// Samples of an incomplete frame left over from the previous call.
    partial_frame: Vec<f32>,
    /// Low-pass taps (empty when no filtering is needed, at or below 16 kHz).
    filter_taps: Vec<f32>,
    /// The last `filter_taps.len()` mono samples, as a ring buffer.
    filter_history: Vec<f32>,
    /// Where the next mono sample goes in `filter_history`.
    filter_history_position: usize,
    /// Filtered mono samples not yet passed by the read position.
    filtered: Vec<f32>,
    /// Read position within `filtered`, in 1/16000 of an input sample.
    read_phase: u64,
}

impl PcmResampler {
    /// Creates a converter for one stream format.
    ///
    /// Inputs: The device's sample rate (Hz) and interleaved channel count; zero values are
    /// treated as 16 kHz and mono.
    /// Outputs: A converter with empty history.
    /// Side effects: None.
    pub fn new(input_rate_hz: u32, channels: u16) -> Self {
        let input_rate_hz = if input_rate_hz == 0 {
            TARGET_SAMPLE_RATE_HZ
        } else {
            input_rate_hz
        };
        let filter_taps = if input_rate_hz > TARGET_SAMPLE_RATE_HZ {
            low_pass_taps(f64::from(input_rate_hz))
        } else {
            Vec::new()
        };
        Self {
            channels: usize::from(channels.max(1)),
            input_rate_hz: u64::from(input_rate_hz),
            partial_frame: Vec::new(),
            filter_history: vec![0.0; filter_taps.len()],
            filter_taps,
            filter_history_position: 0,
            filtered: Vec::new(),
            read_phase: 0,
        }
    }

    /// Converts float samples in [-1, 1] (values beyond are clipped).
    ///
    /// Inputs: Interleaved samples; a trailing partial frame is kept for the next call.
    /// Outputs: The 16 kHz mono PCM16 samples now complete.
    /// Side effects: Advances the converter's state.
    pub fn push_f32(&mut self, interleaved: &[f32]) -> Vec<i16> {
        let mut mono_samples = Vec::with_capacity(interleaved.len() / self.channels + 1);
        for &sample in interleaved {
            self.partial_frame.push(sample);
            if self.partial_frame.len() == self.channels {
                let sum: f32 = self.partial_frame.iter().sum();
                mono_samples.push(sum / self.channels as f32);
                self.partial_frame.clear();
            }
        }
        for mono_sample in mono_samples {
            let filtered_sample = self.filter(mono_sample);
            self.filtered.push(filtered_sample);
        }
        self.emit()
    }

    /// Converts signed 16-bit samples. Inputs, outputs, side effects: as `push_f32`.
    pub fn push_i16(&mut self, interleaved: &[i16]) -> Vec<i16> {
        let scaled: Vec<f32> = interleaved
            .iter()
            .map(|&sample| f32::from(sample) / 32_768.0)
            .collect();
        self.push_f32(&scaled)
    }

    /// Converts signed 32-bit samples. Inputs, outputs, side effects: as `push_f32`.
    pub fn push_i32(&mut self, interleaved: &[i32]) -> Vec<i16> {
        let scaled: Vec<f32> = interleaved
            .iter()
            .map(|&sample| (f64::from(sample) / 2_147_483_648.0) as f32)
            .collect();
        self.push_f32(&scaled)
    }

    /// Converts unsigned 16-bit samples (silence at 32768). Inputs, outputs, side effects: as
    /// `push_f32`.
    pub fn push_u16(&mut self, interleaved: &[u16]) -> Vec<i16> {
        let scaled: Vec<f32> = interleaved
            .iter()
            .map(|&sample| (f32::from(sample) - 32_768.0) / 32_768.0)
            .collect();
        self.push_f32(&scaled)
    }

    /// Runs one mono sample through the low-pass filter (a pass-through without taps).
    fn filter(&mut self, mono_sample: f32) -> f32 {
        let tap_count = self.filter_taps.len();
        if tap_count == 0 {
            return mono_sample;
        }
        self.filter_history[self.filter_history_position] = mono_sample;
        self.filter_history_position = (self.filter_history_position + 1) % tap_count;
        // The oldest sample is at the write position; sum oldest to newest in a fixed order.
        let mut sum = 0.0_f32;
        for (tap_index, &tap) in self.filter_taps.iter().enumerate() {
            let history_index = (self.filter_history_position + tap_index) % tap_count;
            sum += tap * self.filter_history[history_index];
        }
        sum
    }

    /// Emits every output sample whose neighbours are available, then drops passed input.
    fn emit(&mut self) -> Vec<i16> {
        let target_rate = u64::from(TARGET_SAMPLE_RATE_HZ);
        let mut output = Vec::new();
        loop {
            let index = (self.read_phase / target_rate) as usize;
            let fraction_units = self.read_phase % target_rate;
            let Some(&current) = self.filtered.get(index) else {
                break;
            };
            let value = if fraction_units == 0 {
                current
            } else {
                let Some(&next) = self.filtered.get(index + 1) else {
                    break;
                };
                let fraction = fraction_units as f32 / target_rate as f32;
                current + (next - current) * fraction
            };
            output.push(to_pcm16(value));
            self.read_phase += self.input_rate_hz;
        }
        let passed = ((self.read_phase / target_rate) as usize).min(self.filtered.len());
        self.filtered.drain(..passed);
        self.read_phase -= passed as u64 * target_rate;
        output
    }
}

/// Converts a float sample to PCM16, clipping to full scale (symmetric, so -1.0 is -32767).
fn to_pcm16(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * 32_767.0).round() as i16
}

/// Designs the low-pass filter for `input_rate_hz`.
///
/// Inputs: The input sample rate, above 16 kHz.
/// Outputs: An odd number of windowed-sinc taps (Blackman window) with unity gain at DC.
/// Side effects: None.
fn low_pass_taps(input_rate_hz: f64) -> Vec<f32> {
    // A Blackman window needs about 5.5 / (transition width as a fraction of the rate) taps.
    let mut tap_count = (5.5 * input_rate_hz / TRANSITION_HZ).ceil() as usize;
    if tap_count.is_multiple_of(2) {
        tap_count += 1;
    }
    let normalized_cutoff = (CUTOFF_HZ + TRANSITION_HZ / 2.0) / input_rate_hz;
    let center = (tap_count - 1) as f64 / 2.0;
    let mut taps: Vec<f64> = (0..tap_count)
        .map(|index| {
            let offset = index as f64 - center;
            let sinc = if offset == 0.0 {
                2.0 * normalized_cutoff
            } else {
                (2.0 * PI * normalized_cutoff * offset).sin() / (PI * offset)
            };
            let window_position = index as f64 / (tap_count - 1) as f64;
            let blackman = 0.42 - 0.5 * (2.0 * PI * window_position).cos()
                + 0.08 * (4.0 * PI * window_position).cos();
            sinc * blackman
        })
        .collect();
    let gain: f64 = taps.iter().sum();
    for tap in &mut taps {
        *tap /= gain;
    }
    taps.into_iter().map(|tap| tap as f32).collect()
}
