//! Limits how often a capture stream's repeated errors are logged.
//!
//! On Windows, cpal reports every WASAPI data discontinuity as an overrun. On win11 that was 307
//! warnings in a 3-minute recording that still transcribed fine. Each stream now logs its first
//! error, then at most one line a minute saying how many were held back.

use std::time::{Duration, Instant};

/// The shortest time between two logged errors of one stream.
const LOG_INTERVAL: Duration = Duration::from_secs(60);

/// One stream's error log state.
#[derive(Debug, Default)]
pub struct CaptureErrorThrottle {
    /// When an error was last logged; `None` before the first.
    last_logged: Option<Instant>,
    /// Errors since then that were not logged.
    held_back: u64,
}

impl CaptureErrorThrottle {
    /// Notes one error and says whether to log it.
    ///
    /// Inputs: When the error happened.
    /// Outputs: `Some(n)` to log it, `n` being the errors held back since the last logged one;
    /// `None` to skip it. Side effects: Updates the counters.
    pub fn note(&mut self, now: Instant) -> Option<u64> {
        let due = self
            .last_logged
            .is_none_or(|last| now.saturating_duration_since(last) >= LOG_INTERVAL);
        if !due {
            self.held_back += 1;
            return None;
        }
        self.last_logged = Some(now);
        Some(std::mem::take(&mut self.held_back))
    }
}
