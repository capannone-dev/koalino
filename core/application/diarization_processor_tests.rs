//! Specifies diarization orchestration exclusively with deterministic, dependency-free port mocks.

use std::{cell::RefCell, rc::Rc};

use uuid::Uuid;

use crate::{
    application::diarization_processor::DiarizationProcessor,
    domain::{
        errors::{DiarizationError, StorageError},
        models::{MeetingId, UnassignedVoiceClip, VoiceMatchResult},
    },
    ports::{
        ai::{DiarizationEngine, SpeakerSegment},
        storage::NoteRepository,
        voice::VoiceIdentityMatcher,
    },
};

/// Returns deterministic diarization output without loading an ML model.
struct MockDiarizationEngine {
    segments: Vec<SpeakerSegment>,
}

#[async_trait::async_trait]
impl DiarizationEngine for MockDiarizationEngine {
    /// Returns the fixture segments for any audio buffer.
    async fn diarize(&self, _audio_bytes: &[u8]) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        Ok(self.segments.clone())
    }
}

/// Returns one scripted identity outcome for every segment embedding.
struct MockVoiceIdentityMatcher {
    result: VoiceMatchResult,
}

impl VoiceIdentityMatcher for MockVoiceIdentityMatcher {
    /// Returns the configured deterministic classification.
    fn match_embedding(&self, _embedding: &[f32]) -> Result<VoiceMatchResult, StorageError> {
        Ok(self.result.clone())
    }
}

/// Records only temporary clips saved through the repository port.
struct MockNoteRepository {
    saved_clips: Rc<RefCell<Vec<UnassignedVoiceClip>>>,
}

impl NoteRepository for MockNoteRepository {
    /// Does nothing because processor tests do not create meetings.
    fn create_meeting(
        &self,
        _meeting_id: &MeetingId,
        _initial_crdt_document: &[u8],
    ) -> Result<(), StorageError> {
        Ok(())
    }
    /// Does nothing because processor tests do not save transcripts.
    fn save_transcript(
        &self,
        _transcript: &crate::domain::models::Transcript,
    ) -> Result<(), StorageError> {
        Ok(())
    }
    /// Returns no transcript because processor tests do not load transcripts.
    fn load_transcript(
        &self,
        _meeting_id: &MeetingId,
    ) -> Result<Option<crate::domain::models::Transcript>, StorageError> {
        Ok(None)
    }
    /// Does nothing because processor tests do not save CRDT documents.
    fn save_crdt_document(
        &self,
        _meeting_id: &MeetingId,
        _crdt_document: &[u8],
    ) -> Result<(), StorageError> {
        Ok(())
    }
    /// Returns an empty CRDT document because processor tests do not load it.
    fn load_crdt_document(&self, _meeting_id: &MeetingId) -> Result<Vec<u8>, StorageError> {
        Ok(Vec::new())
    }
    /// Captures temporary manual-review clips for assertions.
    fn save_unassigned_clip(&self, clip: UnassignedVoiceClip) -> Result<(), StorageError> {
        self.saved_clips.borrow_mut().push(clip);
        Ok(())
    }
    /// Returns captured clips belonging to the requested meeting.
    fn get_unassigned_clips(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Vec<UnassignedVoiceClip>, StorageError> {
        Ok(self
            .saved_clips
            .borrow()
            .iter()
            .filter(|clip| clip.meeting_id() == meeting_id)
            .cloned()
            .collect())
    }
}

/// Verifies a confident identity attribution does not retain unnecessary review audio.
#[tokio::test]
async fn strong_match_does_not_create_an_unassigned_clip() {
    let saved_clips = Rc::new(RefCell::new(Vec::new()));
    let matched_identity_id = Uuid::new_v4();
    let processor = DiarizationProcessor::new(
        MockDiarizationEngine {
            segments: vec![SpeakerSegment::new(100, 400, vec![0.9, 0.1])],
        },
        MockVoiceIdentityMatcher {
            result: VoiceMatchResult::StrongMatch(matched_identity_id),
        },
        MockNoteRepository {
            saved_clips: Rc::clone(&saved_clips),
        },
    );

    let processing_result = processor
        .process(&MeetingId::new("meeting-strong"), &[1, 2, 3])
        .await
        .unwrap_or_else(|error| panic!("strong match processing should succeed: {error}"));

    assert!(saved_clips.borrow().is_empty());
    assert_eq!(processing_result.strong_speaker_attributions().len(), 1);
    assert_eq!(
        processing_result.strong_speaker_attributions()[0].identity_id(),
        matched_identity_id
    );
}

/// Verifies an unmatched segment retains its timing and review audio for manual assignment.
#[tokio::test]
async fn no_match_creates_an_unassigned_clip() {
    let saved_clips = Rc::new(RefCell::new(Vec::new()));
    let processor = DiarizationProcessor::new(
        MockDiarizationEngine {
            segments: vec![SpeakerSegment::new(250, 900, vec![0.2, 0.8])],
        },
        MockVoiceIdentityMatcher {
            result: VoiceMatchResult::NoMatch,
        },
        MockNoteRepository {
            saved_clips: Rc::clone(&saved_clips),
        },
    );

    processor
        .process(&MeetingId::new("meeting-unmatched"), &[9, 8, 7])
        .await
        .unwrap_or_else(|error| panic!("unmatched processing should succeed: {error}"));

    let persisted_clips = saved_clips.borrow();
    assert_eq!(persisted_clips.len(), 1);
    assert_eq!(
        persisted_clips[0].meeting_id().as_str(),
        "meeting-unmatched"
    );
    assert_eq!(persisted_clips[0].start_time_ms(), 250);
    assert_eq!(persisted_clips[0].end_time_ms(), 900);
    assert_eq!(persisted_clips[0].audio_bytes(), &[9, 8, 7]);
}

/// Verifies ambiguous matches are preserved so a user can confirm or correct the candidate.
#[tokio::test]
async fn weak_match_creates_an_unassigned_clip() {
    let saved_clips = Rc::new(RefCell::new(Vec::new()));
    let processor = DiarizationProcessor::new(
        MockDiarizationEngine {
            segments: vec![SpeakerSegment::new(0, 125, vec![0.6, 0.8])],
        },
        MockVoiceIdentityMatcher {
            result: VoiceMatchResult::WeakMatch(Uuid::new_v4(), 0.6),
        },
        MockNoteRepository {
            saved_clips: Rc::clone(&saved_clips),
        },
    );

    processor
        .process(&MeetingId::new("meeting-weak"), &[4, 5])
        .await
        .unwrap_or_else(|error| panic!("weak match processing should succeed: {error}"));

    assert_eq!(saved_clips.borrow().len(), 1);
}
