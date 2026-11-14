//! Whisper's log-mel spectrogram (#75), computed on the CPU for the NPU encoder.
//!
//! Matches OpenAI Whisper's `log_mel_spectrogram`: the audio is padded with silence to 30 s, a
//! centred short-time Fourier transform (400-sample periodic Hann window, hop 160, reflected
//! edges) gives the power of bins 0..=200, the mel filters (80 x 201, from the whisper.cpp model)
//! sum them, and `log10` is floored 8 below the peak and mapped to `(x + 4) / 4`. The last of the
//! 3,001 frames is dropped. Frames that hold only padding are not transformed: their power is 0.

/// Samples per second.
pub const SAMPLE_RATE: usize = 16_000;
/// The window's length (Whisper's `n_fft`).
pub const N_FFT: usize = 400;
/// Samples between frames.
pub const HOP: usize = 160;
/// Mel bands.
pub const N_MELS: usize = 80;
/// Frames in a 30 s window.
pub const N_FRAMES: usize = 3_000;
/// Samples in a 30 s window.
pub const N_SAMPLES: usize = 30 * SAMPLE_RATE;
/// Frequency bins kept from each frame.
const N_BINS: usize = N_FFT / 2 + 1;

/// Computes the normalized log-mel spectrogram.
///
/// Inputs: Up to 30 s of 16 kHz mono samples (later ones are ignored); `filters`, 80 x 201
/// row-major. Outputs: 80 x 3,000 values, row-major by mel band. Side effects: None.
pub fn log_mel(samples: &[f32], filters: &[f32]) -> Vec<f32> {
    let samples = &samples[..samples.len().min(N_SAMPLES)];
    let window: Vec<f32> = (0..N_FFT)
        .map(|n| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * n as f32 / N_FFT as f32).cos())
        .collect();
    // The padded signal: 30 s with the input at the start, reflected by N_FFT / 2 at both ends.
    let half = N_FFT / 2;
    let padded_sample = |index: isize| -> f32 {
        let position = index - half as isize;
        let reflected = if position < 0 {
            -position
        } else if position >= N_SAMPLES as isize {
            2 * (N_SAMPLES as isize - 1) - position
        } else {
            position
        };
        samples.get(reflected as usize).copied().unwrap_or(0.0)
    };
    let mut mel = vec![0.0_f32; N_MELS * N_FRAMES];
    let mut frame = vec![0.0_f32; N_FFT];
    for frame_index in 0..N_FRAMES {
        let start = frame_index * HOP;
        // Only padding (and the reflection of padding) in this frame: no power.
        if start >= samples.len() + half && start + N_FFT <= N_SAMPLES + half {
            continue;
        }
        for (offset, value) in frame.iter_mut().enumerate() {
            *value = padded_sample((start + offset) as isize) * window[offset];
        }
        let power = dft_power(&frame);
        for band in 0..N_MELS {
            let row = &filters[band * N_BINS..(band + 1) * N_BINS];
            mel[band * N_FRAMES + frame_index] = row
                .iter()
                .zip(&power)
                .map(|(weight, bin)| weight * bin)
                .sum();
        }
    }
    let mut peak = f32::MIN;
    for value in &mut mel {
        *value = value.max(1e-10).log10();
        peak = peak.max(*value);
    }
    for value in &mut mel {
        *value = (value.max(peak - 8.0) + 4.0) / 4.0;
    }
    mel
}

/// The power of bins 0..=N_FFT/2 of one frame.
///
/// Inputs: A frame of `N_FFT` samples. Outputs: `|X_k|^2` for each kept bin. Side effects: None.
pub fn dft_power(frame: &[f32]) -> Vec<f32> {
    let spectrum = fft(frame
        .iter()
        .map(|sample| (f64::from(*sample), 0.0))
        .collect());
    spectrum[..N_BINS.min(spectrum.len())]
        .iter()
        .map(|(real, imaginary)| (real * real + imaginary * imaginary) as f32)
        .collect()
}

/// A recursive radix-2 transform that falls back to a plain DFT for odd lengths (400 = 16 x 25).
fn fft(input: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    let n = input.len();
    if n <= 1 {
        return input;
    }
    if n % 2 == 1 {
        return (0..n)
            .map(|k| {
                input
                    .iter()
                    .enumerate()
                    .fold((0.0, 0.0), |(re, im), (t, (x, y))| {
                        let angle = -2.0 * std::f64::consts::PI * (k * t) as f64 / n as f64;
                        let (sin, cos) = angle.sin_cos();
                        (re + x * cos - y * sin, im + x * sin + y * cos)
                    })
            })
            .collect();
    }
    let even = fft(input.iter().step_by(2).copied().collect());
    let odd = fft(input.iter().skip(1).step_by(2).copied().collect());
    let mut output = vec![(0.0, 0.0); n];
    for k in 0..n / 2 {
        let angle = -2.0 * std::f64::consts::PI * k as f64 / n as f64;
        let (sin, cos) = angle.sin_cos();
        let (x, y) = odd[k];
        let twiddled = (x * cos - y * sin, x * sin + y * cos);
        output[k] = (even[k].0 + twiddled.0, even[k].1 + twiddled.1);
        output[k + n / 2] = (even[k].0 - twiddled.0, even[k].1 - twiddled.1);
    }
    output
}

/// Quantizes values to the encoder's unsigned 16-bit input.
///
/// Inputs: The values, the quantization scale and zero point. Outputs: `round(x / scale) + zero`,
/// clamped to 0..=65535. Side effects: None.
pub fn quantize(values: &[f32], scale: f32, zero_point: i32) -> Vec<u16> {
    values
        .iter()
        .map(|value| {
            let quantized = (value / scale).round() as i64 + i64::from(zero_point);
            quantized.clamp(0, i64::from(u16::MAX)) as u16
        })
        .collect()
}
