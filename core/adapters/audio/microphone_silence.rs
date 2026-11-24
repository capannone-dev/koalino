//! Notices a microphone that is muted, turned off, or blocked, so the app can tell the owner.
//!
//! A muted or blocked microphone usually still opens: the system then hands the app exact digital
//! silence (Linux with the source muted, Windows and Android with microphone access turned off,
//! macOS without permission), or no audio at all. A real room is never exactly silent: even a
//! quiet one gives faint noise. So a few seconds of samples at zero (or one, for rounding), or a
//! few seconds without any block, mean nothing is being heard. Audio is never kept or logged.

use std::time::{Duration, Instant};

/// How long only exact silence must arrive before the microphone counts as muted.
pub const SILENCE_NOTICE_AFTER: Duration = Duration::from_secs(3);

/// How long no audio at all must arrive before the microphone counts as stalled.
///
/// Longer than `SILENCE_NOTICE_AFTER`: some sound servers deliver in fragments of up to 2 s.
pub const STALL_NOTICE_AFTER: Duration = Duration::from_secs(5);

/// Samples per second of the audio observed (16 kHz mono PCM16).
const SAMPLE_RATE_HZ: u64 = 16_000;

/// The largest sample magnitude that still counts as digital silence (rounding in resampling).
const SILENT_SAMPLE_MAGNITUDE: u16 = 1;

/// Follows one open microphone's audio and says whether it is giving only silence.
#[derive(Clone, Debug)]
pub struct MicrophoneSilence {
    /// When the microphone opened, or last delivered a block.
    last_delivery: Instant,
    /// Consecutive samples, up to the latest, that were digital silence.
    silent_samples_in_a_row: u64,
}

impl MicrophoneSilence {
    /// Starts following a microphone that opened at `opened`.
    ///
    /// Inputs: The time it opened. Outputs: The tracker. Side effects: None.
    pub fn new(opened: Instant) -> Self {
        Self {
            last_delivery: opened,
            silent_samples_in_a_row: 0,
        }
    }

    /// Takes in one block of 16 kHz mono PCM16 delivered at `now`.
    ///
    /// Inputs: The block (it is only scanned) and the time it was taken from the queue.
    /// Outputs: None. Side effects: Updates the counters.
    pub fn observe(&mut self, block: &[i16], now: Instant) {
        self.last_delivery = now;
        // Only the samples after the last audible one extend the run.
        match block
            .iter()
            .rposition(|sample| sample.unsigned_abs() > SILENT_SAMPLE_MAGNITUDE)
        {
            Some(last_audible) => {
                self.silent_samples_in_a_row = (block.len() - last_audible - 1) as u64;
            }
            None => self.silent_samples_in_a_row += block.len() as u64,
        }
    }

    /// Whether nothing is being heard at `now`.
    ///
    /// Inputs: The current time. Outputs: `true` after `SILENCE_NOTICE_AFTER` of digital silence,
    /// or after `STALL_NOTICE_AFTER` without any block; `false` otherwise.
    /// Side effects: None.
    pub fn is_silent(&self, now: Instant) -> bool {
        let silent_for =
            Duration::from_millis(self.silent_samples_in_a_row * 1_000 / SAMPLE_RATE_HZ);
        silent_for >= SILENCE_NOTICE_AFTER
            || now.saturating_duration_since(self.last_delivery) >= STALL_NOTICE_AFTER
    }
}

/// The consent-store keys whose `Value` turns the microphone off for this app on Windows: the
/// owner's *Microphone access* switch, *Let desktop apps access your microphone*, and the
/// device-wide switch.
#[cfg(windows)]
const WINDOWS_CONSENT_KEYS: [&str; 3] = [
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone",
    r"HKCU\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone\NonPackaged",
    r"HKLM\Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone",
];

/// Whether `reg query <key> /v Value` printed `Deny`.
///
/// Inputs: The tool's output. Outputs: `true` only for a `Value` of type `REG_SZ` set to `Deny`.
/// Side effects: None.
pub fn consent_value_denies(reg_output: &str) -> bool {
    reg_output.lines().any(|line| {
        let fields: Vec<&str> = line.split_whitespace().collect();
        fields.as_slice() == ["Value", "REG_SZ", "Deny"]
    })
}

/// Whether Windows' privacy settings keep desktop apps such as Koalino from the microphone.
///
/// Outputs: `true` when any of the switches is off; `false` when they are on or unreadable.
/// Side effects: Runs `reg query` up to three times; call it only once the microphone was heard
/// to be silent, off the UI thread.
#[cfg(windows)]
pub fn windows_blocks_microphone() -> bool {
    WINDOWS_CONSENT_KEYS.iter().any(|key| {
        super::microphone_users::run_tool("reg", &["query", key, "/v", "Value"])
            .is_ok_and(|output| consent_value_denies(&output))
    })
}
