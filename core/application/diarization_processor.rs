//! Coordinates port-provided diarization, identity matching, and manual-review clip persistence.

use std::time::Instant;

use uuid::Uuid;

use crate::{
    domain::{
        errors::{DiarizationError, StorageError},
        models::{MeetingId, UnassignedVoiceClip, VoiceMatchResult},
    },
    ports::{ai::DiarizationEngine, storage::NoteRepository, voice::VoiceIdentityMatcher},
};

/// Reports a speaker segment that application policy attributed without manual confirmation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongSpeakerAttribution {
    identity_id: Uuid,
    start_time_ms: u64,
    end_time_ms: u64,
}

impl StrongSpeakerAttribution {
    /// Creates a strong attribution for a diarized segment.
    ///
    /// Inputs: `identity_id` is the high-confidence matched identity; timestamps identify its segment.
    /// Outputs: An immutable application result that links a segment to its known speaker.
    /// Side effects: None.
    fn new(identity_id: Uuid, start_time_ms: u64, end_time_ms: u64) -> Self {
        Self {
            identity_id,
            start_time_ms,
            end_time_ms,
        }
    }

    /// Returns the matched speaker identity UUID.
    ///
    /// Inputs: None.
    /// Outputs: The identity associated with this segment.
    /// Side effects: None.
    pub fn identity_id(&self) -> Uuid {
        self.identity_id
    }
}

/// Summarizes diarization processing without exposing raw audio or embeddings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiarizationProcessingResult {
    strong_speaker_attributions: Vec<StrongSpeakerAttribution>,
    unassigned_clip_ids: Vec<Uuid>,
}

impl DiarizationProcessingResult {
    /// Returns strong speaker links created during the processing request.
    ///
    /// Inputs: None.
    /// Outputs: A borrow of high-confidence segment-to-identity links.
    /// Side effects: None.
    pub fn strong_speaker_attributions(&self) -> &[StrongSpeakerAttribution] {
        &self.strong_speaker_attributions
    }

    /// Returns the review-clip UUIDs created for weak or unmatched speaker segments.
    ///
    /// Inputs: None.
    /// Outputs: A borrow of newly persisted temporary clip identifiers.
    /// Side effects: None.
    pub fn unassigned_clip_ids(&self) -> &[Uuid] {
        &self.unassigned_clip_ids
    }
}

/// Reports a failure while coordinating diarization, identity matching, and clip persistence.
#[derive(Debug, thiserror::Error)]
pub enum DiarizationProcessingError {
    /// The diarization port could not produce speaker segments.
    #[error(transparent)]
    Diarization(#[from] DiarizationError),
    /// The identity-matching or repository port could not complete a storage operation.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// Coordinates diarization, voice matching, and review-clip persistence.
pub struct DiarizationProcessor<DiarizationPort, IdentityMatcherPort, RepositoryPort> {
    diarization_engine: DiarizationPort,
    voice_identity_matcher: IdentityMatcherPort,
    note_repository: RepositoryPort,
}

impl<DiarizationPort, IdentityMatcherPort, RepositoryPort>
    DiarizationProcessor<DiarizationPort, IdentityMatcherPort, RepositoryPort>
where
    DiarizationPort: DiarizationEngine,
    IdentityMatcherPort: VoiceIdentityMatcher,
    RepositoryPort: NoteRepository,
{
    /// Diarizes audio, links strong identities, and preserves weak or unknown segments for review.
    ///
    /// Inputs: `meeting_id` owns the result; `audio_bytes` is opaque source audio and must not be logged.
    /// Outputs: Strong attributions and review-clip IDs, or a portable port failure.
    /// Side effects: Executes AI, queries voice identities, persists temporary audio, and logs safe metadata.
    pub async fn process(
        &self,
        meeting_id: &MeetingId,
        audio_bytes: &[u8],
    ) -> Result<DiarizationProcessingResult, DiarizationProcessingError> {
        let processing_started_at = Instant::now();
        let speaker_segments = self.diarization_engine.diarize(audio_bytes).await?;
        let mut strong_speaker_attributions = Vec::new();
        let mut unassigned_clip_ids = Vec::new();
        for speaker_segment in speaker_segments {
            match self
                .voice_identity_matcher
                .match_embedding(speaker_segment.embedding())?
            {
                VoiceMatchResult::StrongMatch(identity_id) => {
                    strong_speaker_attributions.push(StrongSpeakerAttribution::new(
                        identity_id,
                        speaker_segment.start_time_ms(),
                        speaker_segment.end_time_ms(),
                    ))
                }
                VoiceMatchResult::WeakMatch(_, _) | VoiceMatchResult::NoMatch => {
                    let clip_id = Uuid::new_v4();
                    self.note_repository
                        .save_unassigned_clip(UnassignedVoiceClip::new(
                            clip_id,
                            meeting_id.clone(),
                            speaker_segment.start_time_ms(),
                            speaker_segment.end_time_ms(),
                            audio_bytes.to_vec(),
                        ))?;
                    unassigned_clip_ids.push(clip_id);
                    tracing::warn!(meeting_id = %meeting_id.as_str(), clip_id = %clip_id, "Unassigned voice clip preserved for manual review");
                }
            }
        }
        tracing::info!(meeting_id = %meeting_id.as_str(), processing_time_ms = processing_started_at.elapsed().as_millis(), segment_count = strong_speaker_attributions.len() + unassigned_clip_ids.len(), "Diarization processing completed");
        Ok(DiarizationProcessingResult {
            strong_speaker_attributions,
            unassigned_clip_ids,
        })
    }
}

impl<DiarizationPort, IdentityMatcherPort, RepositoryPort>
    DiarizationProcessor<DiarizationPort, IdentityMatcherPort, RepositoryPort>
{
    /// Creates a processor from injected hexagonal ports.
    ///
    /// Inputs: Each value implements one externally supplied capability.
    /// Outputs: A processor whose orchestration behavior is defined by unit tests.
    /// Side effects: Takes ownership of the injected ports without calling them.
    pub fn new(
        diarization_engine: DiarizationPort,
        voice_identity_matcher: IdentityMatcherPort,
        note_repository: RepositoryPort,
    ) -> Self {
        Self {
            diarization_engine,
            voice_identity_matcher,
            note_repository,
        }
    }
}
