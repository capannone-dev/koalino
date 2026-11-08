//! Judges how far a transcript can be trusted, so the apps can show it (#90, #91).
//!
//! Three things make a stretch of transcript doubtful, each measured on the project's benchmark
//! meetings:
//! - **An uncertain word.** Whisper's least sure token in a word gives it a probability. With
//!   Whisper small on NOTSOFAR, 42% of the words under 0.4 were wrong, against 3.1% of those at
//!   0.95 or more, so words under `UNCERTAIN_WORD_PERMILLE` are marked.
//! - **Overlapping speech.** About four in five transcription errors are missing words, mostly
//!   where people talk over each other, since one stream carries only one voice well.
//!   Diarization reports the stretches where it hears two or more voices at once
//!   (`OverlapSpan`), and a segment that spends enough time in one is marked.
//! - **An uncertain speaker.** A weak voice match, or a final pass that found no voice near the
//!   text, marks the speaker as a guess.
//!
//! `SegmentConfidence` carries the three marks on a `TranscriptSegment`. Everything here is pure.

/// Words with a lower probability, in thousandths, are marked uncertain.
pub const UNCERTAIN_WORD_PERMILLE: u16 = 400;
/// Overlap spans closer than this are one span.
pub const OVERLAP_MERGE_GAP_MS: u64 = 250;
/// Overlap spans shorter than this, after merging, are dropped as noise.
pub const MIN_OVERLAP_MS: u64 = 200;
/// A segment is overlapping once this much of it, or half of it if shorter, lies in overlap.
pub const SEGMENT_OVERLAP_MS: u64 = 500;

/// A stretch of a meeting, in meeting milliseconds, where two or more voices speak at once.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct OverlapSpan {
    /// Start in meeting milliseconds.
    pub start_ms: u64,
    /// End in meeting milliseconds, never before the start.
    pub end_ms: u64,
}

impl OverlapSpan {
    /// Creates a span; an end before the start is raised to the start. Side effects: None.
    pub fn new(start_ms: u64, end_ms: u64) -> Self {
        Self {
            start_ms,
            end_ms: end_ms.max(start_ms),
        }
    }

    /// Returns the span moved by `offset_ms`, never before the meeting's start. Side effects: None.
    pub fn shifted(self, offset_ms: i64) -> Self {
        Self::new(
            self.start_ms.saturating_add_signed(offset_ms),
            self.end_ms.saturating_add_signed(offset_ms),
        )
    }

    /// Returns how long the span shares with `[start_ms, end_ms)`. Side effects: None.
    fn shared_ms(self, start_ms: u64, end_ms: u64) -> u64 {
        self.end_ms
            .min(end_ms)
            .saturating_sub(self.start_ms.max(start_ms))
    }
}

/// How doubtful one transcript segment is.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SegmentConfidence {
    /// The segment lies largely in overlapping speech, where words are often missed.
    pub overlapping_speech: bool,
    /// The speaker is a guess: a weak voice match, or no voice found near the text.
    pub uncertain_speaker: bool,
    /// Positions, counted from 0 among the segment text's whitespace-separated words, of the
    /// words Whisper was unsure of, in increasing order.
    pub uncertain_words: Vec<u16>,
}

impl SegmentConfidence {
    /// Reports whether nothing about the segment is doubtful. Side effects: None.
    pub fn is_plain(&self) -> bool {
        !self.overlapping_speech && !self.uncertain_speaker && self.uncertain_words.is_empty()
    }
}

/// Reports whether a word's probability, in thousandths, makes it uncertain.
///
/// Inputs: The probability, or `None` when the engine gave none (remote and cloud transcription).
/// Outputs: `true` under `UNCERTAIN_WORD_PERMILLE`; an unknown probability is never uncertain.
/// Side effects: None.
pub fn is_uncertain_probability(probability_permille: Option<u16>) -> bool {
    probability_permille.is_some_and(|permille| permille < UNCERTAIN_WORD_PERMILLE)
}

/// Lists the uncertain words among a segment's words.
///
/// Inputs: The segment's words, in the order they appear in its text, one text word each.
/// Outputs: Their positions, saturating at `u16::MAX`. Side effects: None.
pub fn uncertain_word_indices<'word>(
    words: impl IntoIterator<Item = &'word crate::domain::models::TimedWord>,
) -> Vec<u16> {
    uncertain_positions(words.into_iter().map(|word| word.probability_permille()))
}

/// Lists the positions whose probability is uncertain.
///
/// Inputs: Each word's probability in thousandths, in text order.
/// Outputs: The uncertain positions, saturating at `u16::MAX`. Side effects: None.
pub fn uncertain_positions(probabilities: impl IntoIterator<Item = Option<u16>>) -> Vec<u16> {
    probabilities
        .into_iter()
        .enumerate()
        .filter(|(_, probability)| is_uncertain_probability(*probability))
        .map(|(position, _)| u16::try_from(position).unwrap_or(u16::MAX))
        .collect()
}

/// Sorts spans, joins those closer than `OVERLAP_MERGE_GAP_MS`, and drops those shorter than
/// `MIN_OVERLAP_MS`.
///
/// Inputs: Spans in any order. Outputs: Disjoint spans in time order. Side effects: None.
pub fn merge_overlap_spans(mut spans: Vec<OverlapSpan>) -> Vec<OverlapSpan> {
    spans.sort();
    let mut merged: Vec<OverlapSpan> = Vec::with_capacity(spans.len());
    for span in spans {
        match merged.last_mut() {
            Some(previous) if span.start_ms <= previous.end_ms + OVERLAP_MERGE_GAP_MS => {
                previous.end_ms = previous.end_ms.max(span.end_ms);
            }
            _ => merged.push(span),
        }
    }
    merged.retain(|span| span.end_ms - span.start_ms >= MIN_OVERLAP_MS);
    merged
}

/// Reports whether a segment lies in overlapping speech.
///
/// Inputs: The meeting's overlap spans and the segment's bounds in meeting milliseconds.
/// Outputs: `true` once the spans cover `SEGMENT_OVERLAP_MS` of it, or half of a shorter one.
/// Side effects: None.
pub fn overlaps_segment(spans: &[OverlapSpan], start_ms: u64, end_ms: u64) -> bool {
    let length = end_ms.saturating_sub(start_ms);
    if length == 0 {
        return false;
    }
    let shared: u64 = spans
        .iter()
        .map(|span| span.shared_ms(start_ms, end_ms))
        .sum();
    shared > 0 && shared >= SEGMENT_OVERLAP_MS.min(length.div_ceil(2))
}

/// Marks the segments that lie in overlapping speech, keeping their other marks.
///
/// Inputs: Segments and the meeting's overlap spans. Outputs: The same segments, each with
/// `overlapping_speech` set from the spans. Side effects: None.
pub fn mark_overlapping_segments(
    segments: Vec<crate::domain::models::TranscriptSegment>,
    spans: &[OverlapSpan],
) -> Vec<crate::domain::models::TranscriptSegment> {
    segments
        .into_iter()
        .map(|segment| {
            let mut confidence = segment.confidence().clone();
            confidence.overlapping_speech =
                overlaps_segment(spans, segment.start_time_ms(), segment.end_time_ms());
            segment.with_confidence(confidence)
        })
        .collect()
}
