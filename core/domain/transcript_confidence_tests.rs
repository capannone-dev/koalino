//! Specifies how transcript confidence is judged: which words are uncertain, which stretches of a
//! meeting hold overlapping speech, and which segments those stretches mark (#90, #91).

use super::transcript_confidence::{
    OverlapSpan, SegmentConfidence, is_uncertain_probability, merge_overlap_spans,
    overlaps_segment, uncertain_word_indices,
};
use crate::domain::models::{MeetingId, TimedWord, TranscriptSegment};

#[test]
fn a_word_is_uncertain_below_four_tenths_and_unknown_is_not_uncertain() {
    assert!(is_uncertain_probability(Some(399)));
    assert!(!is_uncertain_probability(Some(400)));
    assert!(!is_uncertain_probability(None));
}

#[test]
fn uncertain_words_are_counted_by_their_place_in_the_segment_text() {
    let words = [
        TimedWord::new("We", 0, 100).with_probability_permille(Some(980)),
        TimedWord::new("agreed", 100, 300).with_probability_permille(Some(120)),
        TimedWord::new("on", 300, 400),
        TimedWord::new("Friday.", 400, 600).with_probability_permille(Some(390)),
    ];

    assert_eq!(uncertain_word_indices(words.iter()), vec![1, 3]);
}

#[test]
fn a_word_keeps_its_probability_in_thousandths_and_clamps_it() {
    let word = TimedWord::new("x", 0, 1).with_probability(0.4567);
    assert_eq!(word.probability_permille(), Some(457));
    assert_eq!(
        TimedWord::new("x", 0, 1)
            .with_probability(1.7)
            .probability_permille(),
        Some(1000)
    );
    assert_eq!(
        TimedWord::new("x", 0, 1)
            .with_probability(f32::NAN)
            .probability_permille(),
        None
    );
}

#[test]
fn close_or_touching_overlap_spans_merge_and_slivers_are_dropped() {
    let spans = merge_overlap_spans(vec![
        OverlapSpan::new(5_000, 5_100),
        OverlapSpan::new(1_000, 2_000),
        OverlapSpan::new(2_100, 3_000),
        OverlapSpan::new(9_000, 9_120),
        OverlapSpan::new(9_200, 9_400),
    ]);

    assert_eq!(
        spans,
        vec![
            OverlapSpan::new(1_000, 3_000),
            OverlapSpan::new(9_000, 9_400)
        ]
    );
}

#[test]
fn a_segment_is_overlapping_when_enough_of_it_is_covered() {
    let spans = [OverlapSpan::new(10_000, 10_600)];

    assert!(
        overlaps_segment(&spans, 9_000, 20_000),
        "half a second inside a long segment"
    );
    assert!(
        overlaps_segment(&spans, 10_100, 10_500),
        "a short segment inside the span"
    );
    assert!(
        !overlaps_segment(&spans, 10_500, 20_000),
        "a 100 ms touch is not overlap"
    );
    assert!(!overlaps_segment(&[], 0, 1_000));
}

#[test]
fn confidence_is_plain_until_something_is_doubtful_and_rides_on_the_segment() {
    assert!(SegmentConfidence::default().is_plain());
    let confidence = SegmentConfidence {
        overlapping_speech: true,
        uncertain_speaker: false,
        uncertain_words: vec![2],
    };
    assert!(!confidence.is_plain());

    let segment = TranscriptSegment::new(MeetingId::new("m"), 0, 1_000, None, "a b c")
        .with_confidence(confidence.clone());
    assert_eq!(segment.confidence(), &confidence);
    assert_eq!(
        segment.clone().with_speaker_identity_id(None).confidence(),
        &confidence,
        "re-attributing keeps the marks"
    );
}
