//! Specifies how the microphone's copies of the computer's sound leave a timeline (#108).

use super::echo_removal::without_echoes;
use crate::domain::models::{MeetingId, SegmentSource, TranscriptSegment};

fn segment(start_ms: u64, text: &str, source: SegmentSource) -> TranscriptSegment {
    TranscriptSegment::new(
        MeetingId::new("meeting-e"),
        start_ms,
        start_ms + 2_000,
        None,
        text,
    )
    .with_source(source)
}

fn texts(timeline: &[TranscriptSegment]) -> Vec<&str> {
    timeline.iter().map(TranscriptSegment::text).collect()
}

fn timeline() -> Vec<TranscriptSegment> {
    vec![
        segment(
            0,
            "we should move the launch to friday",
            SegmentSource::ComputerSound,
        ),
        segment(
            200,
            "we should move the launch to friday",
            SegmentSource::Microphone,
        ),
        segment(3_000, "yes friday works", SegmentSource::Microphone),
        segment(
            9_000,
            "I will send the notes tonight",
            SegmentSource::Microphone,
        ),
    ]
}

#[test]
fn the_copy_goes_and_the_owner_s_own_words_stay() {
    let (kept, dropped) = without_echoes(timeline(), true, &|_| true);
    assert_eq!(dropped, 1);
    assert_eq!(
        texts(&kept),
        vec![
            "we should move the launch to friday",
            "yes friday works",
            "I will send the notes tonight"
        ]
    );
    assert_eq!(kept[0].source(), SegmentSource::ComputerSound);
}

#[test]
fn nothing_goes_without_echo_heard_or_where_the_microphone_did_not_echo() {
    assert_eq!(without_echoes(timeline(), false, &|_| true).1, 0);
    assert_eq!(without_echoes(timeline(), true, &|_| false).1, 0);
}
