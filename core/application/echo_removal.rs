//! Drops the microphone's copies of the computer's sound from a meeting's timeline (#108).
//!
//! The words decide which line copies which (`domain::echo_lines`); the audio decides whether the
//! microphone heard the speakers at all (`domain::echo_detect`). The live session asks the
//! detector window by window; the after-meeting passes, whose audio is gone, rely on the meeting's
//! stored `echo_heard` flag.

use crate::domain::{
    echo_lines::{EchoLine, echo_positions},
    models::{SegmentSource, TranscriptSegment},
};

/// Removes the microphone segments that only repeat the computer's sound.
///
/// Inputs: The timeline; whether echo was heard in the meeting; whether the microphone echoed in
/// the window holding a moment (always true once the audio is gone).
/// Outputs: The timeline without those segments, in its order, and how many were dropped. Nothing
/// is dropped without echo heard or without computer-sound segments.
/// Side effects: None.
pub fn without_echoes(
    timeline: Vec<TranscriptSegment>,
    echo_heard: bool,
    echoed_at: &dyn Fn(u64) -> bool,
) -> (Vec<TranscriptSegment>, usize) {
    if !echo_heard {
        return (timeline, 0);
    }
    let (mine_at, mine): (Vec<usize>, Vec<EchoLine<'_>>) = timeline
        .iter()
        .enumerate()
        .filter(|(_, segment)| segment.source() == SegmentSource::Microphone)
        .map(|(index, segment)| (index, line(segment)))
        .unzip();
    let theirs: Vec<EchoLine<'_>> = timeline
        .iter()
        .filter(|segment| segment.source() == SegmentSource::ComputerSound)
        .map(line)
        .collect();
    let dropped: Vec<usize> = echo_positions(&mine, &theirs, true, echoed_at)
        .into_iter()
        .filter_map(|position| mine_at.get(position).copied())
        .collect();
    if dropped.is_empty() {
        return (timeline, 0);
    }
    let count = dropped.len();
    let kept = timeline
        .into_iter()
        .enumerate()
        .filter(|(index, _)| !dropped.contains(index))
        .map(|(_, segment)| segment)
        .collect();
    (kept, count)
}

/// One segment as a line to compare.
fn line(segment: &TranscriptSegment) -> EchoLine<'_> {
    EchoLine {
        start_ms: segment.start_time_ms(),
        end_ms: segment.end_time_ms(),
        text: segment.text(),
    }
}
