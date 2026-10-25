//! Coordinates metadata extraction and meeting-window ranking through hexagonal ports only.

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::{
    domain::models::{MeetingSuggestion, MeetingWindow},
    ports::{media::MetadataExtractor, storage::NoteRepository},
};

/// Finds likely meeting owners for an external file without knowing its metadata format.
pub struct AttachmentManager<MetadataPort, RepositoryPort> {
    metadata_extractor: MetadataPort,
    note_repository: RepositoryPort,
}

impl<MetadataPort, RepositoryPort> AttachmentManager<MetadataPort, RepositoryPort>
where
    MetadataPort: MetadataExtractor,
    RepositoryPort: NoteRepository,
{
    /// Creates an attachment workflow using injected metadata and meeting-timeline ports.
    ///
    /// Inputs: `metadata_extractor` obtains capture times; `note_repository` supplies meeting windows.
    /// Outputs: A manager ready to generate user-confirmable meeting suggestions.
    /// Side effects: None until `suggest_meetings` is called.
    pub fn new(metadata_extractor: MetadataPort, note_repository: RepositoryPort) -> Self {
        Self {
            metadata_extractor,
            note_repository,
        }
    }

    /// Extracts an attachment timestamp, finds overlapping meetings, and ranks them by midpoint proximity.
    ///
    /// Inputs: `file_path` identifies an attachment known to the metadata adapter.
    /// Outputs: Descending-confidence candidate meetings, or extraction/storage errors.
    /// Side effects: Reads adapter-managed metadata and local timeline data; logs only counts.
    pub fn suggest_meetings(&self, file_path: &str) -> Result<Vec<MeetingSuggestion>> {
        let attachment_timestamp = self.metadata_extractor.extract_timestamp(file_path)?;
        tracing::info!("Attachment metadata timestamp processed");
        let overlapping_windows = self
            .note_repository
            .get_meetings_overlapping(attachment_timestamp)?;
        let mut suggestions: Vec<_> = overlapping_windows
            .iter()
            .filter(|meeting_window| meeting_window.contains(attachment_timestamp))
            .map(|meeting_window| suggestion_for(meeting_window, attachment_timestamp))
            .collect();
        suggestions.sort_by(|left, right| {
            right
                .confidence_score()
                .total_cmp(&left.confidence_score())
                .then_with(|| left.meeting_id().as_str().cmp(right.meeting_id().as_str()))
        });
        tracing::info!(
            suggestion_count = suggestions.len(),
            "Attachment meeting suggestions generated"
        );
        Ok(suggestions)
    }
}

/// Converts one overlapping meeting window into a normalized midpoint-proximity suggestion.
///
/// Inputs: `meeting_window` must contain `timestamp`; `timestamp` is a UTC attachment capture time.
/// Outputs: A score of one at the exact midpoint and zero at either inclusive edge.
/// Side effects: None.
fn suggestion_for(meeting_window: &MeetingWindow, timestamp: DateTime<Utc>) -> MeetingSuggestion {
    let total_window_milliseconds = (meeting_window.meeting_end_time()
        - meeting_window.meeting_start_time())
    .num_milliseconds();
    let confidence_score = if total_window_milliseconds <= 0 {
        0.0
    } else {
        let midpoint_offset_milliseconds = total_window_milliseconds / 2;
        let timestamp_offset_milliseconds =
            (timestamp - meeting_window.meeting_start_time()).num_milliseconds();
        let distance_from_midpoint_milliseconds =
            (timestamp_offset_milliseconds - midpoint_offset_milliseconds).abs();
        (1.0 - (distance_from_midpoint_milliseconds as f32
            / midpoint_offset_milliseconds.max(1) as f32))
            .clamp(0.0, 1.0)
    };
    MeetingSuggestion::new(
        meeting_window.meeting_id().clone(),
        meeting_window.meeting_start_time(),
        meeting_window.meeting_end_time(),
        confidence_score,
    )
}
