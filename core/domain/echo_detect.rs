//! Hears whether the other side's voice reaches the microphone in a meeting (#108).
//!
//! Decided from the audio, not the words: the microphone's loudness envelope is compared with the
//! computer sound's over 2 s windows while both carry sound. Their voice from the speakers makes
//! the two move together at the same lag every time; two people talking over each other correlate
//! now and then at random lags. The lag is searched on both sides of zero: the leak itself comes
//! late, but the two capture paths buffer differently, so on the timeline the microphone may land
//! ahead of their track (on deb-gui, by 60 ms). So echo is a rate (a good share of the active
//! windows) at a steady lag, never a count. Without this evidence `echo_lines` drops nothing.
//!
//! Ported from Yogurt's `app/src/main/asr/echoDetect.ts` (FrigadeHQ/yogurt, MIT License,
//! Copyright (c) 2026 Frigade; see `THIRD_PARTY_NOTICES.md`), without its per-line audio check.
//! Envelopes of the last two hours are kept (10 ms blocks, about 2.9 MB a track); the audio itself
//! is not.

use std::collections::{HashMap, HashSet};

/// One envelope block.
const BLOCK_MS: u64 = 10;
/// Samples in a block at 16 kHz.
const BLOCK_SAMPLES: usize = 160;
/// Blocks in a window.
const WINDOW_BLOCKS: u64 = 200;
const WINDOW_MS: u64 = WINDOW_BLOCKS * BLOCK_MS;
/// The microphone is compared at lags of -300 to +300 ms.
const MAX_LAG_BLOCKS: u64 = 30;
/// Two hours of envelope.
const RING_BLOCKS: u64 = 2 * 3_600 * 100;
/// They are talking in a window from this mean level up (full scale is 1).
const THEM_LOUD: f32 = 0.004;
/// The microphone shows something from this level up.
const ME_SOME: f32 = 0.000_6;
/// A window where the microphone follows their track at least this closely is correlated.
const CORRELATED: f32 = 0.6;
/// Evidence needs this many windows with both sides active.
const MIN_ACTIVE: usize = 10;
/// ...and this share of them correlated, at a steady lag...
const MIN_RATE: f32 = 0.4;
const MIN_STEADY: f32 = 0.7;
/// ...or this share at any lag (a room has more than one path from the speakers).
const STRONG_RATE: f32 = 0.55;
/// A window needs this many blocks of their speech to be compared.
const MIN_SPEAKING_BLOCKS: usize = 20;

/// Which side a block of audio comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EchoTrack {
    /// The microphone: the owner, or the people in the room.
    Microphone,
    /// The computer's sound: the other side of a call.
    ComputerSound,
}

/// One track's envelope, as a ring of 10 ms blocks.
struct Envelope {
    blocks: Vec<f32>,
    /// The latest block written, or `None` before the first.
    latest: Option<u64>,
}

impl Envelope {
    fn new() -> Self {
        Self {
            blocks: Vec::new(),
            latest: None,
        }
    }

    /// The block's level; a block nobody recorded (before the first, a pause, older than two
    /// hours) is silence.
    fn at(&self, block: u64) -> f32 {
        let Some(latest) = self.latest else {
            return 0.0;
        };
        if block > latest || block + RING_BLOCKS <= latest {
            return 0.0;
        }
        self.blocks
            .get((block % RING_BLOCKS) as usize)
            .copied()
            .unwrap_or(0.0)
    }

    fn set(&mut self, block: u64, level: f32) {
        let index = (block % RING_BLOCKS) as usize;
        if self.blocks.len() <= index {
            self.blocks.resize(index + 1, 0.0);
        }
        self.blocks[index] = level;
    }
}

/// Collects the evidence of one meeting.
pub struct EchoDetector {
    microphone: Envelope,
    computer_sound: Envelope,
    /// The window to look at next, by the block it ends at.
    next_check: u64,
    /// Windows where both sides carried sound.
    active: usize,
    /// The lag, in blocks (negative: the microphone ahead), of each window where the microphone
    /// followed their track.
    lags: Vec<i64>,
    /// Those windows, by number.
    correlated: HashSet<u64>,
}

impl Default for EchoDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl EchoDetector {
    /// Creates a detector with no evidence. Side effects: None.
    pub fn new() -> Self {
        Self {
            microphone: Envelope::new(),
            computer_sound: Envelope::new(),
            next_check: WINDOW_BLOCKS,
            active: 0,
            lags: Vec::new(),
            correlated: HashSet::new(),
        }
    }

    /// Adds a stretch of one track.
    ///
    /// Inputs: The track, where the stretch starts in meeting time, and its 16 kHz samples.
    /// Side effects: Updates the envelope and checks every window both tracks now cover.
    pub fn push(&mut self, track: EchoTrack, start_ms: u64, samples: &[i16]) {
        let first = start_ms / BLOCK_MS;
        let envelope = match track {
            EchoTrack::Microphone => &mut self.microphone,
            EchoTrack::ComputerSound => &mut self.computer_sound,
        };
        // A jump forward (a pause, a restart) leaves old blocks in the ring: they are silence now.
        if let Some(latest) = envelope.latest
            && first > latest + 1
        {
            for block in (latest + 1).max(first.saturating_sub(RING_BLOCKS))..first {
                envelope.set(block, 0.0);
            }
        }
        for (offset, block_samples) in samples.chunks(BLOCK_SAMPLES).enumerate() {
            let energy: f64 = block_samples
                .iter()
                .map(|sample| {
                    let value = f64::from(*sample) / 32_768.0;
                    value * value
                })
                .sum();
            let level = (energy / block_samples.len().max(1) as f64).sqrt() as f32;
            let block = first + offset as u64;
            envelope.set(block, level);
            if envelope.latest.is_none_or(|latest| block > latest) {
                envelope.latest = Some(block);
            }
        }
        self.check_ready_windows();
    }

    /// Looks at each window once both tracks reach past it (the microphone's lagged blocks too).
    /// A track that ran ahead waits for the other; a gap reads as silence once the track resumes,
    /// so such a window is simply not active.
    fn check_ready_windows(&mut self) {
        while self
            .computer_sound
            .latest
            .is_some_and(|latest| latest + 1 >= self.next_check)
            && self
                .microphone
                .latest
                .is_some_and(|latest| latest + 1 >= self.next_check + MAX_LAG_BLOCKS)
        {
            // A window older than the ring is gone from it: skip to the oldest still held.
            let oldest_held = self
                .computer_sound
                .latest
                .unwrap_or(0)
                .saturating_sub(RING_BLOCKS - 1);
            if self.next_check - WINDOW_BLOCKS >= oldest_held {
                self.check(self.next_check - WINDOW_BLOCKS, self.next_check);
            }
            self.next_check += WINDOW_BLOCKS;
        }
    }

    /// Compares one 2 s window of their track with the microphone's at lags of -300 to +300 ms.
    fn check(&mut self, from: u64, to: u64) {
        let them: Vec<f32> = (from..to)
            .map(|block| self.computer_sound.at(block))
            .collect();
        let me_at = |lag: i64| -> Vec<f32> {
            (from..to)
                .map(|block| {
                    block
                        .checked_add_signed(lag)
                        .map_or(0.0, |shifted| self.microphone.at(shifted))
                })
                .collect()
        };
        if mean(&them) < THEM_LOUD || mean(&me_at(0)) < ME_SOME {
            return;
        }
        self.active += 1;
        // Only the moments they speak count: the owner's own words would drown a faint leak.
        let speaking: Vec<bool> = them.iter().map(|level| *level >= THEM_LOUD).collect();
        let pick = |levels: &[f32]| -> Vec<f32> {
            levels
                .iter()
                .zip(&speaking)
                .filter(|(_, speaks)| **speaks)
                .map(|(level, _)| *level)
                .collect()
        };
        let their_levels = pick(&them);
        if their_levels.len() < MIN_SPEAKING_BLOCKS {
            return;
        }
        let (mut best, mut best_lag) = (0.0_f32, 0);
        let max_lag = MAX_LAG_BLOCKS as i64;
        for lag in -max_lag..=max_lag {
            let correlation = pearson(&their_levels, &pick(&me_at(lag)));
            if correlation > best {
                best = correlation;
                best_lag = lag;
            }
        }
        if best >= CORRELATED {
            self.lags.push(best_lag);
            self.correlated.insert(from / WINDOW_BLOCKS);
        }
    }

    /// Whether echo was heard in this meeting: a steady share of the active windows, at one lag
    /// (or most of them at any lag). Side effects: None.
    pub fn present(&self) -> bool {
        if self.active < MIN_ACTIVE {
            return false;
        }
        let rate = self.lags.len() as f32 / self.active as f32;
        if rate < MIN_RATE {
            return false;
        }
        if rate >= STRONG_RATE {
            return true;
        }
        let mut count: HashMap<i64, usize> = HashMap::new();
        for lag in &self.lags {
            *count.entry(*lag).or_default() += 1;
        }
        let Some(mode) = count
            .iter()
            .max_by_key(|(lag, times)| (**times, std::cmp::Reverse(lag.abs())))
            .map(|(lag, _)| *lag)
        else {
            return false;
        };
        let steady = self
            .lags
            .iter()
            .filter(|lag| lag.abs_diff(mode) <= 1)
            .count();
        steady as f32 / self.lags.len() as f32 >= MIN_STEADY
    }

    /// Whether the microphone followed their track in the 2 s window holding `ms`.
    /// Side effects: None.
    pub fn correlated_at(&self, ms: u64) -> bool {
        self.correlated.contains(&(ms / WINDOW_MS))
    }

    /// Windows where both sides carried sound, for tests and logs. Side effects: None.
    pub fn active_windows(&self) -> usize {
        self.active
    }
}

fn mean(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f32>() / values.len() as f32
}

/// Pearson's correlation of two equally long series; 0 when either is flat.
fn pearson(a: &[f32], b: &[f32]) -> f32 {
    let length = a.len().min(b.len());
    if length == 0 {
        return 0.0;
    }
    let (mean_a, mean_b) = (mean(&a[..length]), mean(&b[..length]));
    let (mut covariance, mut variance_a, mut variance_b) = (0.0_f32, 0.0_f32, 0.0_f32);
    for index in 0..length {
        let (x, y) = (a[index] - mean_a, b[index] - mean_b);
        covariance += x * y;
        variance_a += x * x;
        variance_b += y * y;
    }
    if variance_a > 0.0 && variance_b > 0.0 {
        covariance / (variance_a * variance_b).sqrt()
    } else {
        0.0
    }
}
