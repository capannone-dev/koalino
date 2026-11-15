//! Writes a meeting's transcript to the log before and after the after-meeting pass, at the
//! `trace` level only (#87), so the owner can compare what the pass changed.
//!
//! The owner asked for this in the most verbose log mode. At every other level nothing is read
//! and nothing is logged, so the usual logs keep holding counts only. A `trace` log therefore
//! contains meeting words: it stays on the device like the notes do, and is shared only when
//! the owner exports it.

use crate::domain::{
    errors::StorageError,
    models::{MeetingId, TranscriptSegment},
};

/// When the transcript was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassStage {
    /// The live transcript, before the pass rewrites it.
    Before,
    /// The transcript the pass wrote.
    After,
}

impl PassStage {
    fn as_str(self) -> &'static str {
        match self {
            Self::Before => "before",
            Self::After => "after",
        }
    }
}

/// Renders segments as `[hh:mm:ss] speaker: text` lines in time order.
///
/// Inputs: The meeting's segments in any order. Outputs: One line per non-blank segment; the
/// speaker is the first 8 hex digits of its voice identity, `cluster-` and 8 digits of its
/// unnamed voice, or `?`. Side effects: None.
pub fn comparison_text(segments: &[TranscriptSegment]) -> String {
    let mut ordered: Vec<&TranscriptSegment> = segments.iter().collect();
    ordered.sort_by_key(|segment| (segment.start_time_ms(), segment.end_time_ms()));
    ordered
        .into_iter()
        .filter(|segment| !segment.text().trim().is_empty())
        .map(|segment| {
            let seconds = segment.start_time_ms() / 1_000;
            let speaker = match (segment.speaker_identity_id(), segment.unknown_cluster_id()) {
                (Some(identity), _) => identity.simple().to_string()[..8].to_owned(),
                (None, Some(cluster)) => format!("cluster-{}", &cluster.simple().to_string()[..8]),
                (None, None) => "?".to_owned(),
            };
            format!(
                "[{:02}:{:02}:{:02}] {speaker}: {}",
                seconds / 3_600,
                seconds / 60 % 60,
                seconds % 60,
                segment.text().trim()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Logs the meeting's transcript at `trace`, when that level is on.
///
/// Inputs: The meeting, the stage, and `load`, which reads its segments; it is called only at
/// `trace`. Outputs: None. Side effects: At `trace`, reads the segments and logs one event with
/// the whole transcript (`transcript` field); a failed read is logged without words.
pub fn log_pass_transcript(
    meeting_id: &MeetingId,
    stage: PassStage,
    load: impl FnOnce() -> Result<Vec<TranscriptSegment>, StorageError>,
) {
    if !tracing::enabled!(tracing::Level::TRACE) {
        return;
    }
    match load() {
        Ok(segments) => tracing::trace!(
            meeting_id = %meeting_id.as_str(),
            stage = stage.as_str(),
            segment_count = segments.len(),
            transcript = %comparison_text(&segments),
            "Transcript {} the after-meeting pass",
            stage.as_str()
        ),
        Err(error) => tracing::trace!(
            meeting_id = %meeting_id.as_str(),
            stage = stage.as_str(),
            error = %error,
            "The transcript could not be read for the pass comparison"
        ),
    }
}
