//! Orchestrates one meeting's audio chunks into persisted, speaker-attributed transcript segments.
//!
//! Local VAD chunks flow transcribe → diarize → match voice → cluster unknown voices →
//! align/merge → persist. External bridge chunks (e.g. Teams) skip diarization and use the
//! guaranteed mapped identity. Weak and unmatched voices are grouped per meeting by
//! `UnknownSpeakerClusterer`, so each cluster carries one review clip and its segments can be
//! re-attributed together. The session depends only on the `TranscriptionEngine`,
//! `DiarizationEngine`, `VoiceIdentityMatcher`, `IdentityMapper`, `NoteRepository`, and (for
//! cluster assignment) `VoiceVectorDB` ports. It never logs transcript text, audio, or embeddings.

use std::{rc::Rc, time::Instant};

use uuid::Uuid;

use crate::{
    domain::{
        errors::{StorageError, TranscriptionError},
        models::{
            AudioRecording, ExternalMediaChunk, MeetingId, SpeakerIdentity, TranscriptSegment,
            TranscriptionContext, UnassignedVoiceClip, VoiceMatchResult, WeakVoiceMatchCandidate,
        },
    },
    ports::{
        ai::{DiarizationEngine, SpeakerSegment, TranscriptionEngine},
        external_bridge::IdentityMapper,
        storage::NoteRepository,
        voice::{VoiceIdentityMatcher, VoiceVectorDB},
    },
};

use crate::application::unknown_speaker_clusterer::{
    ClusterAssignment, ClusteringConfig, ClusteringError, UnknownSpeakerClusterer,
};

use super::task_metrics::TaskMetricsRecorder;

use super::alignment::{AlignedRegion, ChunkBounds, SpeakerRegion, align_chunk_regions};
pub use super::alignment::{ChunkBoundsError, SpeakerAttribution};

/// Reports a failure that prevented a chunk from producing persisted segments.
#[derive(Debug, thiserror::Error)]
pub enum MeetingSessionError {
    /// Transcription failed; no segment was persisted for the chunk.
    #[error(transparent)]
    Transcription(#[from] TranscriptionError),
    /// Loading or persisting the segment timeline failed.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// The chunk boundaries were inverted.
    #[error(transparent)]
    InvalidChunkBounds(#[from] ChunkBoundsError),
    /// Assigning an identity to an unknown-speaker cluster failed.
    #[error(transparent)]
    Clustering(#[from] ClusteringError),
    /// The chunk's recording belongs to a different meeting than the session.
    #[error("chunk for meeting {received} was sent to the session for meeting {expected}")]
    MeetingMismatch {
        /// The session's meeting identifier.
        expected: String,
        /// The meeting identifier carried by the chunk's recording.
        received: String,
    },
}

/// One VAD-bounded local audio chunk ready for the pipeline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VadAudioChunk {
    audio_recording: AudioRecording,
    chunk_bounds: ChunkBounds,
    audio_bytes: Vec<u8>,
}

impl VadAudioChunk {
    /// Creates a chunk from its recording metadata, meeting-relative VAD bounds, and audio.
    ///
    /// Inputs: `audio_recording` identifies the chunk for the transcription adapter; offsets are
    /// milliseconds from the meeting start with `end_offset_ms >= start_offset_ms`; `audio_bytes`
    /// is opaque audio for diarization and review clips and must never be logged.
    /// Outputs: The chunk, or `ChunkBoundsError` when the bounds are inverted.
    /// Side effects: Takes ownership of raw audio bytes.
    pub fn new(
        audio_recording: AudioRecording,
        start_offset_ms: u64,
        end_offset_ms: u64,
        audio_bytes: Vec<u8>,
    ) -> Result<Self, ChunkBoundsError> {
        Ok(Self {
            audio_recording,
            chunk_bounds: ChunkBounds::new(start_offset_ms, end_offset_ms)?,
            audio_bytes,
        })
    }

    /// Borrows the recording metadata passed to the transcription port.
    pub fn audio_recording(&self) -> &AudioRecording {
        &self.audio_recording
    }

    /// Returns the meeting-relative VAD boundaries.
    pub fn chunk_bounds(&self) -> ChunkBounds {
        self.chunk_bounds
    }

    /// Borrows the opaque chunk audio; callers must never log it.
    pub fn audio_bytes(&self) -> &[u8] {
        &self.audio_bytes
    }
}

/// A persisted transcript segment paired with its detailed attribution category.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttributedSegment {
    segment: TranscriptSegment,
    attribution: SpeakerAttribution,
}

impl AttributedSegment {
    /// Borrows the segment exactly as persisted through `NoteRepository`.
    pub fn segment(&self) -> &TranscriptSegment {
        &self.segment
    }

    /// Borrows the attribution, which distinguishes pending-review, unknown, and unattributed
    /// segments that are all persisted without a speaker identity.
    pub fn attribution(&self) -> &SpeakerAttribution {
        &self.attribution
    }
}

/// Holds one recording's pipeline ports and its accumulated segment timeline.
pub struct MeetingSession<TranscriptionPort, DiarizationPort, MatcherPort, RepositoryPort> {
    meeting_id: MeetingId,
    transcription_engine: TranscriptionPort,
    /// `None` when no diarization engine is available; segments are then `Unattributed`.
    diarization_engine: Option<DiarizationPort>,
    voice_identity_matcher: MatcherPort,
    note_repository: RepositoryPort,
    /// Full timeline mirrored in memory because the repository replaces segments per meeting.
    persisted_segments: Vec<TranscriptSegment>,
    /// Similarity policy used to group unknown voices into clusters.
    clustering_config: ClusteringConfig,
    /// Saves a telemetry row per completed transcription and diarization; `None` records none.
    task_metrics: Option<Rc<TaskMetricsRecorder>>,
}

impl<TranscriptionPort, DiarizationPort, MatcherPort, RepositoryPort>
    MeetingSession<TranscriptionPort, DiarizationPort, MatcherPort, RepositoryPort>
where
    TranscriptionPort: TranscriptionEngine,
    DiarizationPort: DiarizationEngine,
    MatcherPort: VoiceIdentityMatcher,
    RepositoryPort: NoteRepository,
{
    /// Starts a session for an existing meeting when its recording begins.
    ///
    /// Inputs: `meeting_id` must already exist in the repository; `transcription_engine` is the
    /// engine selected by compute routing; `diarization_engine` is optional; the matcher and
    /// repository are the configured voice-identity and storage ports.
    /// Outputs: A ready session, or a storage error if existing segments cannot be loaded.
    /// Side effects: Reads the meeting's existing segments so later chunks append to them.
    pub fn start(
        meeting_id: MeetingId,
        transcription_engine: TranscriptionPort,
        diarization_engine: Option<DiarizationPort>,
        voice_identity_matcher: MatcherPort,
        note_repository: RepositoryPort,
    ) -> Result<Self, MeetingSessionError> {
        let persisted_segments = note_repository.load_transcript_segments(&meeting_id)?;
        tracing::info!(
            meeting_id = %meeting_id.as_str(),
            existing_segment_count = persisted_segments.len(),
            diarization_available = diarization_engine.is_some(),
            "Meeting pipeline session started"
        );
        Ok(Self {
            meeting_id,
            transcription_engine,
            diarization_engine,
            voice_identity_matcher,
            note_repository,
            persisted_segments,
            clustering_config: ClusteringConfig::default(),
            task_metrics: None,
        })
    }

    /// Replaces the default unknown-speaker clustering policy (threshold 0.70).
    ///
    /// Inputs: `clustering_config` holds a validated similarity threshold.
    /// Outputs: The session using that policy for subsequent chunks.
    /// Side effects: None.
    pub fn with_clustering_config(mut self, clustering_config: ClusteringConfig) -> Self {
        self.clustering_config = clustering_config;
        self
    }

    /// Records telemetry for every later chunk (Issue #53).
    ///
    /// Inputs: The recorder, shared with the factory's other sessions.
    /// Outputs: The session saving one row per completed transcription and diarization.
    /// Side effects: None until a chunk is processed.
    pub fn with_task_metrics(mut self, task_metrics: Rc<TaskMetricsRecorder>) -> Self {
        self.task_metrics = Some(task_metrics);
        self
    }

    /// Assigns an identity to an unknown-speaker cluster and refreshes the in-memory timeline.
    ///
    /// Use this instead of calling `UnknownSpeakerClusterer` directly while a session is live:
    /// the session rewrites the whole timeline per chunk, so it must observe the relinked segments.
    ///
    /// Inputs: `voice_vector_database` stores the enrolled variants; `cluster_id` must be an
    /// unresolved cluster; `identity` is an existing identity or a new UUID with its chosen name.
    /// Outputs: The assignment summary, or a clustering/storage error (the in-memory timeline is
    /// reloaded only after success).
    /// Side effects: Saves the identity, stores voice variants, rewrites segments, resolves the
    /// cluster, deletes its review clips, and reloads the session's segment timeline from storage.
    pub fn assign_identity_to_cluster<VectorDatabase>(
        &mut self,
        voice_vector_database: &VectorDatabase,
        cluster_id: Uuid,
        identity: &SpeakerIdentity,
    ) -> Result<ClusterAssignment, MeetingSessionError>
    where
        VectorDatabase: VoiceVectorDB,
    {
        let assignment =
            UnknownSpeakerClusterer::new(&self.note_repository, self.clustering_config)
                .assign_identity_to_cluster(voice_vector_database, cluster_id, identity)?;
        self.persisted_segments = self
            .note_repository
            .load_transcript_segments(&self.meeting_id)?;
        Ok(assignment)
    }

    /// Borrows the injected transcription port.
    pub fn transcription_engine(&self) -> &TranscriptionPort {
        &self.transcription_engine
    }

    /// Borrows the injected repository port, e.g. to read back persisted segments.
    pub fn note_repository(&self) -> &RepositoryPort {
        &self.note_repository
    }

    /// Borrows the session's full segment timeline in start-time order.
    pub fn persisted_segments(&self) -> &[TranscriptSegment] {
        &self.persisted_segments
    }

    /// Processes one local VAD chunk into persisted speaker-attributed segments.
    ///
    /// Inputs: `vad_chunk` must belong to this session's meeting.
    /// Outputs: The newly persisted segments in time order (empty for a blank transcript), or an
    /// error when the meeting mismatches, transcription fails, or the timeline cannot be saved.
    /// Diarization and voice-matching failures degrade to `Unattributed`/`Unknown` instead.
    /// Side effects: Runs transcription (synchronously) and diarization, queries voice identities,
    /// saves review clips for weak/unknown voices, rewrites the meeting's segments in storage,
    /// saves a telemetry row per completed AI task when a recorder is set, and emits structured
    /// logs without text, audio, or embeddings.
    pub async fn process_vad_chunk(
        &mut self,
        vad_chunk: &VadAudioChunk,
    ) -> Result<Vec<AttributedSegment>, MeetingSessionError> {
        self.ensure_session_meeting(vad_chunk.audio_recording().meeting_id())?;
        let processing_started_at = Instant::now();
        let chunk_bounds = vad_chunk.chunk_bounds();
        // No previous text as a prompt: Whisper then repeated or completed the previous chunk's
        // last words (Issue #51). The configured language keeps chunks consistent instead.
        let transcription_context = TranscriptionContext::new(None);
        let transcript = self
            .transcription_engine
            .transcribe(vad_chunk.audio_recording(), &transcription_context)
            .map_err(|error| self.log_transcription_failure(chunk_bounds, error))?;
        if let Some(task_metrics) = &self.task_metrics {
            task_metrics.record_transcription(processing_started_at.elapsed());
        }

        let speaker_regions = self.diarize_and_attribute(vad_chunk).await;
        let aligned_regions =
            align_chunk_regions(chunk_bounds, speaker_regions, transcript.content());
        let attributed_segments = self.persist_aligned_regions(aligned_regions)?;
        tracing::info!(
            meeting_id = %self.meeting_id.as_str(),
            chunk_start_ms = chunk_bounds.start_offset_ms(),
            chunk_end_ms = chunk_bounds.end_offset_ms(),
            segment_count = attributed_segments.len(),
            processing_time_ms = processing_started_at.elapsed().as_millis(),
            "Local audio chunk processed into transcript segments"
        );
        Ok(attributed_segments)
    }

    /// Processes one isolated external-bridge chunk using its guaranteed participant identity.
    ///
    /// Inputs: `identity_mapper` resolves the chunk's external participant; `media_chunk` carries
    /// the isolated audio whose `timestamp_ms` is taken as the meeting-relative start (the bridge
    /// stream is assumed to start with the meeting); `audio_recording` identifies the chunk for
    /// transcription and must belong to this meeting; `duration_ms` is the chunk length.
    /// Outputs: The newly persisted segment (empty for a blank transcript), or an error when the
    /// meeting mismatches, transcription fails, or storage fails. A mapping failure degrades the
    /// segment to `Unknown` without a review clip.
    /// Side effects: Never calls diarization; runs transcription, may persist an identity mapping
    /// through the mapper, rewrites the meeting's segments, and emits safe structured logs.
    pub fn process_external_chunk<MapperPort>(
        &mut self,
        identity_mapper: &MapperPort,
        media_chunk: &ExternalMediaChunk,
        audio_recording: &AudioRecording,
        duration_ms: u64,
    ) -> Result<Vec<AttributedSegment>, MeetingSessionError>
    where
        MapperPort: IdentityMapper,
    {
        self.ensure_session_meeting(audio_recording.meeting_id())?;
        let chunk_start_ms = media_chunk.timestamp_ms();
        let chunk_bounds =
            ChunkBounds::new(chunk_start_ms, chunk_start_ms.saturating_add(duration_ms))?;
        let transcript = self
            .transcription_engine
            .transcribe_external_media(media_chunk, audio_recording)
            .map_err(|error| self.log_transcription_failure(chunk_bounds, error))?;

        let external_participant = media_chunk.external_participant();
        let attribution = match identity_mapper.map_external_to_local(external_participant.clone())
        {
            Ok(local_identity) => SpeakerAttribution::Known {
                identity_id: local_identity.identity_id(),
            },
            Err(error) => {
                tracing::warn!(
                    meeting_id = %self.meeting_id.as_str(),
                    external_id = %external_participant.external_id(),
                    error = %error,
                    "External identity mapping failed; segment persisted as Unknown"
                );
                SpeakerAttribution::Unknown {
                    clip_id: None,
                    cluster_id: None,
                }
            }
        };
        let aligned_regions = align_chunk_regions(
            chunk_bounds,
            vec![SpeakerRegion::new(0, duration_ms, attribution)],
            transcript.content(),
        );
        let attributed_segments = self.persist_aligned_regions(aligned_regions)?;
        tracing::info!(
            meeting_id = %self.meeting_id.as_str(),
            external_id = %external_participant.external_id(),
            chunk_start_ms = chunk_bounds.start_offset_ms(),
            chunk_end_ms = chunk_bounds.end_offset_ms(),
            segment_count = attributed_segments.len(),
            "External bridge chunk processed without diarization"
        );
        Ok(attributed_segments)
    }

    /// Rejects work belonging to a different meeting.
    ///
    /// Inputs: `chunk_meeting_id` is the meeting carried by the chunk's recording.
    /// Outputs: `Ok` when it matches the session, otherwise `MeetingMismatch`.
    /// Side effects: None.
    fn ensure_session_meeting(
        &self,
        chunk_meeting_id: &MeetingId,
    ) -> Result<(), MeetingSessionError> {
        if chunk_meeting_id == &self.meeting_id {
            return Ok(());
        }
        Err(MeetingSessionError::MeetingMismatch {
            expected: self.meeting_id.as_str().to_owned(),
            received: chunk_meeting_id.as_str().to_owned(),
        })
    }

    /// Logs a transcription failure and converts it into a session error.
    ///
    /// Inputs: The failing chunk's bounds and the port error.
    /// Outputs: The error wrapped as `MeetingSessionError::Transcription`.
    /// Side effects: Emits an error-level structured log without audio or text.
    fn log_transcription_failure(
        &self,
        chunk_bounds: ChunkBounds,
        error: TranscriptionError,
    ) -> MeetingSessionError {
        tracing::error!(
            meeting_id = %self.meeting_id.as_str(),
            chunk_start_ms = chunk_bounds.start_offset_ms(),
            chunk_end_ms = chunk_bounds.end_offset_ms(),
            error = %error,
            "Transcription failed; no transcript segment persisted for chunk"
        );
        MeetingSessionError::Transcription(error)
    }

    /// Diarizes a chunk and attributes each speaker segment through the voice-identity port.
    ///
    /// Inputs: The chunk whose audio is diarized.
    /// Outputs: Chunk-relative attributed regions; empty when diarization is unavailable or
    /// fails, which alignment turns into a single `Unattributed` region.
    /// Side effects: Executes diarization, queries voice identities, saves review clips, and logs.
    async fn diarize_and_attribute(&self, vad_chunk: &VadAudioChunk) -> Vec<SpeakerRegion> {
        let Some(diarization_engine) = &self.diarization_engine else {
            tracing::debug!(
                meeting_id = %self.meeting_id.as_str(),
                "No diarization engine configured; chunk persisted as Unattributed"
            );
            return Vec::new();
        };
        let diarization_started_at = Instant::now();
        match diarization_engine.diarize(vad_chunk.audio_bytes()).await {
            Ok(speaker_segments) => {
                if let Some(task_metrics) = &self.task_metrics {
                    task_metrics.record_diarization(diarization_started_at.elapsed());
                }
                speaker_segments
                    .iter()
                    .map(|speaker_segment| {
                        SpeakerRegion::new(
                            speaker_segment.start_time_ms(),
                            speaker_segment.end_time_ms(),
                            self.attribute_speaker_segment(vad_chunk, speaker_segment),
                        )
                    })
                    .collect()
            }
            Err(error) => {
                let chunk_bounds = vad_chunk.chunk_bounds();
                tracing::warn!(
                    meeting_id = %self.meeting_id.as_str(),
                    chunk_start_ms = chunk_bounds.start_offset_ms(),
                    chunk_end_ms = chunk_bounds.end_offset_ms(),
                    error = %error,
                    "Diarization failed; chunk persisted as Unattributed"
                );
                Vec::new()
            }
        }
    }

    /// Classifies one diarized segment and clusters it when it is not a strong match.
    ///
    /// Inputs: The source chunk and one of its diarized segments.
    /// Outputs: `Known` for strong matches, `PendingReview` for weak matches, and `Unknown` for
    /// no match or a matcher failure; the last two carry their clip and cluster IDs.
    /// Side effects: Queries the voice-identity port, writes the unknown-speaker cluster member,
    /// and saves a review clip when the voice founds a cluster (or clustering fails).
    fn attribute_speaker_segment(
        &self,
        vad_chunk: &VadAudioChunk,
        speaker_segment: &SpeakerSegment,
    ) -> SpeakerAttribution {
        match self
            .voice_identity_matcher
            .match_embedding(speaker_segment.embedding())
        {
            Ok(VoiceMatchResult::StrongMatch(identity_id)) => {
                SpeakerAttribution::Known { identity_id }
            }
            Ok(VoiceMatchResult::WeakMatch(candidate_identity_id, confidence_score)) => {
                let (clip_id, cluster_id) = self.track_unknown_voice(
                    vad_chunk,
                    speaker_segment,
                    Some(WeakVoiceMatchCandidate::new(
                        candidate_identity_id,
                        confidence_score,
                    )),
                );
                SpeakerAttribution::PendingReview {
                    clip_id,
                    candidate_identity_id,
                    cluster_id,
                }
            }
            Ok(VoiceMatchResult::NoMatch) => {
                let (clip_id, cluster_id) =
                    self.track_unknown_voice(vad_chunk, speaker_segment, None);
                SpeakerAttribution::Unknown {
                    clip_id,
                    cluster_id,
                }
            }
            Err(error) => {
                tracing::warn!(
                    meeting_id = %self.meeting_id.as_str(),
                    segment_start_ms = speaker_segment.start_time_ms(),
                    segment_end_ms = speaker_segment.end_time_ms(),
                    error = %error,
                    "Voice matching failed; segment persisted as Unknown"
                );
                let (clip_id, cluster_id) =
                    self.track_unknown_voice(vad_chunk, speaker_segment, None);
                SpeakerAttribution::Unknown {
                    clip_id,
                    cluster_id,
                }
            }
        }
    }

    /// Clusters an unattributed voice and keeps one review clip per cluster.
    ///
    /// Inputs: The source chunk, its diarized segment, and an optional weak candidate.
    /// Outputs: `(clip_id, cluster_id)`. A voice that founds a cluster gets a clip linked to it;
    /// one that joins an existing cluster gets no new clip. If clustering fails, the voice falls
    /// back to an unclustered per-chunk clip (`cluster_id` is `None`).
    /// Side effects: Writes the cluster member (biometric embedding) and may save a review clip.
    fn track_unknown_voice(
        &self,
        vad_chunk: &VadAudioChunk,
        speaker_segment: &SpeakerSegment,
        weak_match_candidate: Option<WeakVoiceMatchCandidate>,
    ) -> (Option<Uuid>, Option<Uuid>) {
        // Doubles as the cluster member ID and, for a founding member, the review clip ID.
        let chunk_id = Uuid::new_v4();
        let segment_duration_ms = speaker_segment
            .end_time_ms()
            .saturating_sub(speaker_segment.start_time_ms());
        let clusterer = UnknownSpeakerClusterer::new(&self.note_repository, self.clustering_config);
        match clusterer.find_or_create_cluster(
            &self.meeting_id,
            speaker_segment.embedding(),
            chunk_id.to_string(),
            segment_duration_ms,
        ) {
            Ok(membership) if membership.is_new_cluster() => {
                let clip_id = self.preserve_review_clip(
                    chunk_id,
                    vad_chunk,
                    speaker_segment,
                    weak_match_candidate,
                    Some(membership.cluster_id()),
                );
                (clip_id, Some(membership.cluster_id()))
            }
            Ok(membership) => (None, Some(membership.cluster_id())),
            Err(error) => {
                tracing::warn!(
                    meeting_id = %self.meeting_id.as_str(),
                    error = %error,
                    "Unknown voice could not be clustered; keeping an unclustered review clip"
                );
                let clip_id = self.preserve_review_clip(
                    chunk_id,
                    vad_chunk,
                    speaker_segment,
                    weak_match_candidate,
                    None,
                );
                (clip_id, None)
            }
        }
    }

    /// Saves a temporary review clip so the user can identify the speaker later.
    ///
    /// Inputs: The clip ID to use, the source chunk, the diarized segment (times stay relative to
    /// the clip audio, as in `DiarizationProcessor`), an optional weak candidate, and the cluster
    /// the clip represents (if any).
    /// Outputs: The clip ID, or `None` when storage rejected it (the segment is still persisted).
    /// Side effects: Persists raw chunk audio and the derived embedding through `NoteRepository`.
    fn preserve_review_clip(
        &self,
        clip_id: Uuid,
        vad_chunk: &VadAudioChunk,
        speaker_segment: &SpeakerSegment,
        weak_match_candidate: Option<WeakVoiceMatchCandidate>,
        cluster_id: Option<Uuid>,
    ) -> Option<Uuid> {
        let review_clip = UnassignedVoiceClip::with_review_context(
            clip_id,
            self.meeting_id.clone(),
            speaker_segment.start_time_ms(),
            speaker_segment.end_time_ms(),
            vad_chunk.audio_bytes().to_vec(),
            speaker_segment.embedding().to_vec(),
            weak_match_candidate,
        )
        .with_cluster_id(cluster_id);
        match self.note_repository.save_unassigned_clip(review_clip) {
            Ok(()) => {
                tracing::warn!(meeting_id = %self.meeting_id.as_str(), clip_id = %clip_id, "Unassigned voice clip preserved for manual review");
                Some(clip_id)
            }
            Err(error) => {
                tracing::warn!(meeting_id = %self.meeting_id.as_str(), error = %error, "Review clip could not be preserved; segment kept without clip");
                None
            }
        }
    }

    /// Converts aligned regions to segments and persists the updated meeting timeline.
    ///
    /// Inputs: Meeting-timed regions produced by alignment.
    /// Outputs: The new attributed segments, or a storage error (the in-memory timeline is left
    /// unchanged on failure so a retry stays consistent).
    /// Side effects: Rewrites the meeting's segment rows through `NoteRepository` when non-empty.
    fn persist_aligned_regions(
        &mut self,
        aligned_regions: Vec<AlignedRegion>,
    ) -> Result<Vec<AttributedSegment>, MeetingSessionError> {
        let attributed_segments: Vec<AttributedSegment> = aligned_regions
            .into_iter()
            .map(|aligned_region| AttributedSegment {
                segment: TranscriptSegment::new(
                    self.meeting_id.clone(),
                    aligned_region.start_time_ms(),
                    aligned_region.end_time_ms(),
                    aligned_region.attribution().speaker_identity_id(),
                    aligned_region.text(),
                )
                .with_unknown_cluster_id(aligned_region.attribution().unknown_cluster_id()),
                attribution: aligned_region.attribution().clone(),
            })
            .collect();
        if attributed_segments.is_empty() {
            return Ok(attributed_segments);
        }
        let mut updated_timeline = self.persisted_segments.clone();
        updated_timeline.extend(
            attributed_segments
                .iter()
                .map(|attributed_segment| attributed_segment.segment.clone()),
        );
        updated_timeline.sort_by_key(TranscriptSegment::start_time_ms);
        if let Err(error) = self
            .note_repository
            .save_transcript_segments(&self.meeting_id, &updated_timeline)
        {
            tracing::error!(
                meeting_id = %self.meeting_id.as_str(),
                error = %error,
                "Transcript segments could not be persisted"
            );
            return Err(error.into());
        }
        self.persisted_segments = updated_timeline;
        Ok(attributed_segments)
    }
}
