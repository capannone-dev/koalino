//! Specifies the before/after transcripts of the after-meeting pass at the `trace` log level
//! (#87): one line per segment with its time and speaker, and nothing at other levels.

use std::sync::{Arc, Mutex};

use uuid::Uuid;

use super::pass_comparison::{PassStage, comparison_text, log_pass_transcript};
use crate::domain::models::{MeetingId, TranscriptSegment};

fn segment(start_ms: u64, speaker: Option<Uuid>, text: &str) -> TranscriptSegment {
    TranscriptSegment::new(
        MeetingId::new("meeting-pass"),
        start_ms,
        start_ms + 1_000,
        speaker,
        text,
    )
}

#[test]
fn each_segment_is_a_timed_line_with_its_speaker_in_order() {
    let anna = Uuid::from_u128(0xabcdef12_0000_0000_0000_000000000000);
    let text = comparison_text(&[
        segment(61_000, None, " Then Friday. "),
        segment(0, Some(anna), "We ship."),
        segment(5_000, None, "   "),
    ]);

    assert_eq!(
        text,
        "[00:00:00] abcdef12: We ship.\n[00:01:01] ?: Then Friday."
    );
}

/// Collects what a subscriber writes.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(|error| panic!("{error}"))
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn logged_at(level: tracing::Level) -> String {
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        log_pass_transcript(&MeetingId::new("meeting-pass"), PassStage::Before, || {
            Ok(vec![segment(0, None, "We ship.")])
        });
    });
    let bytes = captured
        .0
        .lock()
        .unwrap_or_else(|error| panic!("{error}"))
        .clone();
    String::from_utf8(bytes).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn the_transcript_is_logged_only_at_trace() {
    let trace = logged_at(tracing::Level::TRACE);
    assert!(trace.contains("before"), "{trace}");
    assert!(trace.contains("We ship."), "{trace}");
    assert!(trace.contains("meeting-pass"), "{trace}");

    assert_eq!(
        logged_at(tracing::Level::DEBUG),
        "",
        "nothing is read or logged"
    );
}
