//! Recognises two devices' recordings of one meeting from their transcripts (#88).
//!
//! The owner may record a meeting on the laptop and on a phone placed near the other person. Both
//! recordings sync to every paired device as ordinary meetings. Each device transcribes the same
//! conversation, so their transcripts share long runs of words at the same moments, which different
//! meetings never do. Speech timing alone does not tell them apart, because busy meetings have
//! little silence.
//!
//! The rule: count the words of each transcript that start a run of three words also found in the
//! other within `TIME_TOLERANCE_MS` of wall-clock time (clocks of paired devices agree to well
//! within that). Two recordings are one meeting when those words are at least `MIN_SHARED_SHARE`
//! of the shorter transcript and at least `MIN_SHARED_WORDS`.
//!
//! Calibrated on NOTSOFAR: 92 device pairs of the same meeting share 60–89% (at least 60% even in
//! the first 2 minutes), and 190 pairs of different meetings at most 3.6% (`DECISION_LOG.md`).
//! The clock offset between the recordings is the median gap between the shared words. Everything
//! here is pure and logs nothing.

use std::collections::HashMap;

use crate::domain::models::TranscriptSegment;

/// Largest gap in wall-clock time between the same words on the two devices. It covers clocks a
/// few seconds apart and word times estimated from segments.
const TIME_TOLERANCE_MS: i64 = 15_000;
/// Smallest share of the shorter transcript that must be shared.
const MIN_SHARED_SHARE: f64 = 0.25;
/// Fewest shared words: a greeting heard by two devices is not a meeting.
const MIN_SHARED_WORDS: usize = 15;
/// Words in a shared run.
const RUN_LENGTH: usize = 3;

/// One transcribed word: when it was said in its recording and its comparison form.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpokenWord {
    time_ms: u64,
    key: String,
}

impl SpokenWord {
    /// Returns when the word was said, in milliseconds from the recording's start.
    pub fn time_ms(&self) -> u64 {
        self.time_ms
    }

    /// Returns the word lowercased, without punctuation.
    pub fn key(&self) -> &str {
        &self.key
    }
}

/// A recording's words and when it started, as one device's synced meeting shows them.
#[derive(Clone, Debug, Default)]
pub struct RecordingText {
    started_at_ms: i64,
    words: Vec<SpokenWord>,
}

impl RecordingText {
    /// Builds the words of a meeting from its segments.
    ///
    /// Inputs: When the recording started (Unix milliseconds on its device's clock) and its
    /// segments. Outputs: Each segment's words, spread evenly over its span (segments carry no
    /// word times), lowercased and stripped of punctuation; words of punctuation alone are dropped.
    /// Side effects: None.
    pub fn from_segments(started_at_ms: i64, segments: &[TranscriptSegment]) -> Self {
        let mut words = Vec::new();
        for segment in segments {
            let keys: Vec<String> = segment
                .text()
                .split_whitespace()
                .map(comparison_key)
                .filter(|key| !key.is_empty())
                .collect();
            let start_ms = segment.start_time_ms();
            let duration_ms = segment.end_time_ms().saturating_sub(start_ms);
            let count = keys.len() as u64;
            words.extend(keys.into_iter().enumerate().map(|(index, key)| SpokenWord {
                time_ms: start_ms + index as u64 * duration_ms / count.max(1),
                key,
            }));
        }
        words.sort_by_key(|word| word.time_ms);
        Self {
            started_at_ms,
            words,
        }
    }

    /// Builds a recording's text from timed words.
    ///
    /// Inputs: When the recording started (Unix milliseconds, or any common reference) and its
    /// words with their times in the recording. Outputs: The words in time order, compared as in
    /// `from_segments`. Side effects: None.
    pub fn from_words<'text>(
        started_at_ms: i64,
        words: impl IntoIterator<Item = (u64, &'text str)>,
    ) -> Self {
        let mut words: Vec<SpokenWord> = words
            .into_iter()
            .map(|(time_ms, text)| SpokenWord {
                time_ms,
                key: comparison_key(text),
            })
            .filter(|word| !word.key.is_empty())
            .collect();
        words.sort_by_key(|word| word.time_ms);
        Self {
            started_at_ms,
            words,
        }
    }

    /// Returns the words in time order.
    pub fn words(&self) -> &[SpokenWord] {
        &self.words
    }

    /// Returns a word's wall-clock time.
    fn wall_ms(&self, word: &SpokenWord) -> i64 {
        self.started_at_ms
            .saturating_add(i64::try_from(word.time_ms).unwrap_or(i64::MAX))
    }
}

/// Two recordings found to be one meeting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SameMeeting {
    offset_ms: i64,
    shared_share: f64,
}

impl SameMeeting {
    /// Returns how to place the second recording on the first's timeline: a moment at time `t` in
    /// the second recording is at `t + offset_ms` in the first.
    pub fn offset_ms(&self) -> i64 {
        self.offset_ms
    }

    /// Returns the share of the shorter transcript that both share, from 0 to 1.
    pub fn shared_share(&self) -> f64 {
        self.shared_share
    }
}

/// Decides whether two recordings are the same meeting.
///
/// Inputs: The two recordings' texts, from different devices.
/// Outputs: The match and the clock offset between them, or `None` when they share too little.
/// The result does not depend on the order, except that the offset changes sign.
/// Side effects: None.
pub fn recognise_same_meeting(
    first: &RecordingText,
    second: &RecordingText,
) -> Option<SameMeeting> {
    let shorter = first.words.len().min(second.words.len());
    if shorter < MIN_SHARED_WORDS {
        return None;
    }
    let (first_shared, gaps) = shared_words(first, second);
    let (second_shared, _) = shared_words(second, first);
    let shared = first_shared.min(second_shared);
    let shared_share = shared as f64 / shorter as f64;
    if shared < MIN_SHARED_WORDS || shared_share < MIN_SHARED_SHARE {
        return None;
    }
    Some(SameMeeting {
        offset_ms: median(gaps)?,
        shared_share,
    })
}

/// Counts the words of `text` inside a run of three also found in `other` at about the same
/// wall-clock moment.
///
/// Outputs: That count, and for each shared run the gap `text time - other time` in recording
/// time (the run's nearest occurrence in `other`).
fn shared_words(text: &RecordingText, other: &RecordingText) -> (usize, Vec<i64>) {
    let mut runs: HashMap<[&str; RUN_LENGTH], Vec<&SpokenWord>> = HashMap::new();
    for window in other.words.windows(RUN_LENGTH) {
        runs.entry([&window[0].key, &window[1].key, &window[2].key])
            .or_default()
            .push(&window[0]);
    }
    let mut shared = vec![false; text.words.len()];
    let mut gaps = Vec::new();
    for (index, window) in text.words.windows(RUN_LENGTH).enumerate() {
        let wall_ms = text.wall_ms(&window[0]);
        let nearest = runs
            .get(&[
                window[0].key.as_str(),
                window[1].key.as_str(),
                window[2].key.as_str(),
            ])
            .and_then(|starts| {
                starts
                    .iter()
                    .map(|start| (other.wall_ms(start).abs_diff(wall_ms), *start))
                    .min_by_key(|(distance, _)| *distance)
            });
        if let Some((distance, start)) = nearest
            && distance <= TIME_TOLERANCE_MS.unsigned_abs()
        {
            shared[index..index + RUN_LENGTH].fill(true);
            gaps.push(signed_ms(window[0].time_ms) - signed_ms(start.time_ms));
        }
    }
    (shared.iter().filter(|word| **word).count(), gaps)
}

/// Converts recording milliseconds to a signed value.
fn signed_ms(time_ms: u64) -> i64 {
    i64::try_from(time_ms).unwrap_or(i64::MAX)
}

/// Returns the median, the lower middle value for an even count; `None` when empty.
fn median(mut values: Vec<i64>) -> Option<i64> {
    values.sort_unstable();
    values.get(values.len().checked_sub(1)? / 2).copied()
}

/// Lowercases a word and keeps only its letters, digits and apostrophes.
pub(crate) fn comparison_key(word: &str) -> String {
    word.chars()
        .filter(|character| character.is_alphanumeric() || *character == '\'')
        .flat_map(char::to_lowercase)
        .collect()
}
