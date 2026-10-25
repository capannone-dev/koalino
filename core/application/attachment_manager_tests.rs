//! Specifies mock-driven time-window matching and confidence ordering for attachments.

use std::{cell::Cell, rc::Rc};

use anyhow::{Result, anyhow};
use chrono::{DateTime, TimeZone, Utc};

use crate::{
    application::attachment_manager::AttachmentManager,
    domain::{
        errors::StorageError,
        models::{MeetingId, MeetingWindow},
    },
    ports::{media::MetadataExtractor, storage::NoteRepository},
};

/// Returns a deterministic UTC instant for tests without inspecting a real file.
fn timestamp_at(epoch_seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(epoch_seconds, 0)
        .single()
        .unwrap_or_else(|| panic!("test epoch timestamp should be valid"))
}

/// Supplies a hardcoded metadata timestamp and records the requested path.
struct MockMetadataExtractor {
    timestamp: Option<DateTime<Utc>>,
    invocation_count: Cell<u32>,
}

impl MockMetadataExtractor {
    /// Creates a successful metadata extractor response with a predetermined capture time.
    fn succeeds_with(timestamp: DateTime<Utc>) -> Self {
        Self {
            timestamp: Some(timestamp),
            invocation_count: Cell::new(0),
        }
    }

    /// Creates a failing metadata extractor response without attempting filesystem access.
    fn fails() -> Self {
        Self {
            timestamp: None,
            invocation_count: Cell::new(0),
        }
    }
}

impl MetadataExtractor for MockMetadataExtractor {
    fn extract_timestamp(&self, _path: &str) -> Result<DateTime<Utc>> {
        self.invocation_count
            .set(self.invocation_count.get().saturating_add(1));
        self.timestamp
            .ok_or_else(|| anyhow!("fixture metadata unavailable"))
    }
}

/// Returns only scripted meeting windows and never opens a database in unit tests.
struct MockNoteRepository {
    overlapping_meetings: Vec<MeetingWindow>,
    query_count: Rc<Cell<u32>>,
}

impl MockNoteRepository {
    /// Creates a repository mock with precisely the matching windows to return.
    fn new(overlapping_meetings: Vec<MeetingWindow>) -> Self {
        Self {
            overlapping_meetings,
            query_count: Rc::new(Cell::new(0)),
        }
    }

    /// Creates a repository mock whose invocation count can be asserted by the test.
    fn with_query_counter(
        overlapping_meetings: Vec<MeetingWindow>,
        query_count: Rc<Cell<u32>>,
    ) -> Self {
        Self {
            overlapping_meetings,
            query_count,
        }
    }
}

impl NoteRepository for MockNoteRepository {
    fn create_meeting(
        &self,
        _meeting_id: &MeetingId,
        _initial_crdt_document: &[u8],
    ) -> Result<(), StorageError> {
        Ok(())
    }

    fn save_transcript(
        &self,
        _transcript: &crate::domain::models::Transcript,
    ) -> Result<(), StorageError> {
        Ok(())
    }

    fn load_transcript(
        &self,
        _meeting_id: &MeetingId,
    ) -> Result<Option<crate::domain::models::Transcript>, StorageError> {
        Ok(None)
    }

    fn save_crdt_document(
        &self,
        _meeting_id: &MeetingId,
        _crdt_document: &[u8],
    ) -> Result<(), StorageError> {
        Ok(())
    }

    fn load_crdt_document(&self, _meeting_id: &MeetingId) -> Result<Vec<u8>, StorageError> {
        Ok(Vec::new())
    }

    fn save_unassigned_clip(
        &self,
        _clip: crate::domain::models::UnassignedVoiceClip,
    ) -> Result<(), StorageError> {
        Ok(())
    }

    fn get_unassigned_clips(
        &self,
        _meeting_id: &MeetingId,
    ) -> Result<Vec<crate::domain::models::UnassignedVoiceClip>, StorageError> {
        Ok(Vec::new())
    }

    fn get_meetings_overlapping(
        &self,
        _timestamp: DateTime<Utc>,
    ) -> Result<Vec<MeetingWindow>, StorageError> {
        self.query_count
            .set(self.query_count.get().saturating_add(1));
        Ok(self.overlapping_meetings.clone())
    }
}

/// Builds one meeting time window fixture.
fn meeting_window(
    meeting_id: &str,
    start_epoch_seconds: i64,
    end_epoch_seconds: i64,
) -> MeetingWindow {
    MeetingWindow::new(
        MeetingId::new(meeting_id),
        timestamp_at(start_epoch_seconds),
        timestamp_at(end_epoch_seconds),
    )
}

/// Proves an attachment at a meeting midpoint gets the maximum possible confidence.
#[test]
fn midpoint_timestamp_returns_one_meeting_with_full_confidence() {
    let metadata_extractor = MockMetadataExtractor::succeeds_with(timestamp_at(1_800));
    let note_repository = MockNoteRepository::new(vec![meeting_window("meeting-a", 1_200, 2_400)]);
    let manager = AttachmentManager::new(metadata_extractor, note_repository);

    let suggestions = manager
        .suggest_meetings("/fixtures/photo.jpg")
        .unwrap_or_else(|error| panic!("suggestion lookup should succeed: {error}"));

    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].meeting_id().as_str(), "meeting-a");
    assert_eq!(suggestions[0].confidence_score(), 1.0);
}

/// Proves overlapping meetings are sorted from the nearest midpoint to the farthest.
#[test]
fn overlapping_meetings_are_sorted_by_descending_confidence() {
    let metadata_extractor = MockMetadataExtractor::succeeds_with(timestamp_at(1_800));
    let note_repository = MockNoteRepository::new(vec![
        meeting_window("meeting-a", 1_200, 3_000),
        meeting_window("meeting-b", 1_600, 2_000),
    ]);
    let manager = AttachmentManager::new(metadata_extractor, note_repository);

    let suggestions = manager
        .suggest_meetings("/fixtures/overlap.pdf")
        .unwrap_or_else(|error| panic!("suggestion lookup should succeed: {error}"));

    assert_eq!(suggestions.len(), 2);
    assert_eq!(suggestions[0].meeting_id().as_str(), "meeting-b");
    assert_eq!(suggestions[0].confidence_score(), 1.0);
    assert_eq!(suggestions[1].meeting_id().as_str(), "meeting-a");
    assert!((suggestions[1].confidence_score() - (2.0 / 3.0)).abs() < f32::EPSILON);
}

/// Proves a timestamp on a meeting boundary remains a candidate but has minimum confidence.
#[test]
fn meeting_boundary_timestamp_returns_zero_confidence() {
    let metadata_extractor = MockMetadataExtractor::succeeds_with(timestamp_at(1_200));
    let note_repository = MockNoteRepository::new(vec![meeting_window("meeting-a", 1_200, 2_400)]);
    let manager = AttachmentManager::new(metadata_extractor, note_repository);

    let suggestions = manager
        .suggest_meetings("/fixtures/meeting-start.jpg")
        .unwrap_or_else(|error| panic!("suggestion lookup should succeed: {error}"));

    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].confidence_score(), 0.0);
}

/// Proves timestamps outside every stored meeting yield no automatic suggestion.
#[test]
fn non_overlapping_timestamp_returns_no_suggestions() {
    let metadata_extractor = MockMetadataExtractor::succeeds_with(timestamp_at(5_000));
    let note_repository = MockNoteRepository::new(Vec::new());
    let manager = AttachmentManager::new(metadata_extractor, note_repository);

    let suggestions = manager
        .suggest_meetings("/fixtures/unrelated.pdf")
        .unwrap_or_else(|error| panic!("suggestion lookup should succeed: {error}"));

    assert!(suggestions.is_empty());
}

/// Proves extractor failures propagate before any repository query, without accessing EXIF data.
#[test]
fn metadata_extraction_failure_does_not_query_meetings() {
    let metadata_extractor = MockMetadataExtractor::fails();
    let repository_query_count = Rc::new(Cell::new(0));
    let note_repository = MockNoteRepository::with_query_counter(
        vec![meeting_window("meeting-a", 1_200, 2_400)],
        Rc::clone(&repository_query_count),
    );
    let manager = AttachmentManager::new(metadata_extractor, note_repository);

    let error = manager
        .suggest_meetings("/fixtures/unreadable.jpg")
        .err()
        .unwrap_or_else(|| panic!("metadata failure should be returned"));

    assert!(error.to_string().contains("fixture metadata unavailable"));
    assert_eq!(repository_query_count.get(), 0);
}
