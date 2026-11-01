//! Orchestrates one meeting's audio chunks into persisted, speaker-attributed transcript segments.
//!
//! Local VAD chunks flow transcribe → diarize → match voice → cluster unknown voices →
//! align/merge → persist. External bridge chunks (e.g. Teams) skip diarization and use the
//! guaranteed mapped identity. Weak and unmatched voices are grouped per meeting by
//! `UnknownSpeakerClusterer`, so each cluster carries one review clip and its segments can be
//! re-attributed together. The session depends only on the `TranscriptionEngine`,
//! `DiarizationEngine`, `VoiceIdentityMatcher`, `IdentityMapper`, `NoteRepository`, and (for
//! cluster assignment) `VoiceVectorDB` ports. It never logs transcript text, audio, or embeddings.

use crate::domain::transcription_language::TranscriptionLanguage;
use std::{rc::Rc, time::Instant};

use uuid::Uuid;

use crate::{
    domain::{
        errors::{StorageError, TranscriptionError},
        models::{
            AudioRecording, ExternalMediaChunk, MeetingId, SpeakerIdentity, Transcript,
            TranscriptSegment, TranscriptionContext, UnassignedVoiceClip, UnknownSpeakerChunk,
            UnknownSpeakerCluster, VoiceMatchResult, WeakVoiceMatchCandidate,
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

use super::{
    meeting_speaker_pass::{
        SPEAKER_NAME_THRESHOLD, SpeakerPassConfig, VoiceWindow, group_voice_windows, speaker_at,
        speaker_voices,
    },
    task_metrics::TaskMetricsRecorder,
};

use super::alignment::{AlignedRegion, ChunkBounds, SpeakerRegion, align_transcript};
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

/// Some text with its meeting time: one word, or a whole region when the engine gave no word timing.
#[derive(Clone, Debug)]
struct TextPiece {
    chunk_index: usize,
    start_ms: u64,
    end_ms: u64,
    text: String,
}

/// Who a speaker turned out to be at the end of a recording.
#[derive(Clone, Copy, Debug, Default)]
struct FinalAttribution {
    /// The named person, if any.
    identity_id: Option<Uuid>,
    /// The cluster to review, for an unnamed speaker or one the owner named during the recording.
    cluster_id: Option<Uuid>,
}

/// Lists a chunk's text with meeting times.
///
/// Inputs: The chunk's number and bounds, its transcript, and its aligned regions.
/// Outputs: One piece per timed word, or per region without word timing. Side effects: None.
fn text_pieces_of(
    chunk_index: usize,
    chunk_bounds: ChunkBounds,
    transcript: &Transcript,
    aligned_regions: &[AlignedRegion],
) -> Vec<TextPiece> {
    let timed_words = transcript.timed_words();
    if timed_words.is_empty() {
        return aligned_regions
            .iter()
            .map(|region| TextPiece {
                chunk_index,
                start_ms: region.start_time_ms(),
                end_ms: region.end_time_ms(),
                text: region.text().to_owned(),
            })
            .collect();
    }
    timed_words
        .iter()
        .map(|word| TextPiece {
            chunk_index,
            start_ms: chunk_bounds.start_offset_ms() + word.start_ms(),
            end_ms: chunk_bounds.start_offset_ms() + word.end_ms(),
            text: word.text().to_owned(),
        })
        .collect()
}

/// Counts, per speaker, how many of their windows fall in each live cluster's segments.
///
/// Inputs: The windows and speakers, the speaker count, and this recording's live segments.
/// Outputs: For each speaker, `(cluster, windows)` pairs. Side effects: None.
fn live_overlap_per_speaker(
    voice_windows: &[VoiceWindow],
    labels: &[usize],
    speaker_count: usize,
    live_segments: &[TranscriptSegment],
) -> Vec<Vec<(Uuid, usize)>> {
    let mut overlap: Vec<Vec<(Uuid, usize)>> = vec![Vec::new(); speaker_count];
    for (window, speaker) in voice_windows.iter().zip(labels) {
        let center_ms = window.center_ms();
        let cluster_id = live_segments
            .iter()
            .find(|segment| {
                segment.start_time_ms() <= center_ms && center_ms < segment.end_time_ms()
            })
            .and_then(TranscriptSegment::unknown_cluster_id);
        let Some(cluster_id) = cluster_id else {
            continue;
        };
        match overlap[*speaker]
            .iter_mut()
            .find(|(known, _)| *known == cluster_id)
        {
            Some((_, windows)) => *windows += 1,
            None => overlap[*speaker].push((cluster_id, 1)),
        }
    }
    overlap
}

/// One VAD-bounded local audio chunk ready for the pipeline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VadAudioChunk {
    audio_recording: AudioRecording,
    chunk_bounds: ChunkBounds,
    audio_bytes: Vec<u8>,
    language: Option<TranscriptionLanguage>,
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
            language: None,
        })
    }

    /// Stamps the language this chunk is transcribed in, chosen when it was cut, so a later change
    /// does not reach chunks still waiting in the queue.
    ///
    /// Inputs: The language; `None` leaves it to the engine's configuration.
    /// Outputs: The chunk. Side effects: None.
    pub fn with_language(mut self, language: Option<TranscriptionLanguage>) -> Self {
        self.language = language;
        self
    }

    /// Returns the language stamped on this chunk, if any.
    pub fn language(&self) -> Option<TranscriptionLanguage> {
        self.language
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
    /// Segments from before this recording, which the final pass leaves alone.
    earlier_segments: Vec<TranscriptSegment>,
    /// This recording's voice windows, in memory only and in time order, for the final pass.
    voice_windows: Vec<VoiceWindow>,
    /// This recording's text with its timing, in time order, for the final pass.
    text_pieces: Vec<TextPiece>,
    /// How many chunks this recording has processed.
    chunk_count: usize,
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
            earlier_segments: persisted_segments.clone(),
            persisted_segments,
            voice_windows: Vec::new(),
            text_pieces: Vec::new(),
            chunk_count: 0,
            clustering_config: ClusteringConfig::default(),
            task_metrics: None,
        })
    }

    /// Replaces the default unknown-speaker clustering policy (threshold 0.40).
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
        // last words (Issue #51). The chosen language keeps chunks consistent instead.
        let transcription_context =
            TranscriptionContext::new(None).with_language(vad_chunk.language());
        let transcript = self
            .transcription_engine
            .transcribe(vad_chunk.audio_recording(), &transcription_context)
            .map_err(|error| self.log_transcription_failure(chunk_bounds, error))?;
        if let Some(task_metrics) = &self.task_metrics {
            task_metrics.record_transcription(
                processing_started_at.elapsed(),
                chunk_bounds
                    .end_offset_ms()
                    .saturating_sub(chunk_bounds.start_offset_ms()),
            );
        }

        let speaker_regions = self.diarize_and_attribute(vad_chunk).await;
        let aligned_regions = align_transcript(chunk_bounds, speaker_regions, &transcript);
        let chunk_index = self.chunk_count;
        self.chunk_count += 1;
        let chunk_pieces = text_pieces_of(chunk_index, chunk_bounds, &transcript, &aligned_regions);
        let attributed_segments = self.persist_aligned_regions(aligned_regions)?;
        self.text_pieces.extend(chunk_pieces);
        self.collect_voice_windows(vad_chunk, chunk_index).await;
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

    /// Measures a chunk's voice windows for the final pass.
    ///
    /// Inputs: The chunk and its number. Outputs: None; a failure only leaves those windows out.
    /// Side effects: Runs diarization inference and keeps embeddings in memory, without audio.
    async fn collect_voice_windows(&mut self, vad_chunk: &VadAudioChunk, chunk_index: usize) {
        let Some(diarization_engine) = &self.diarization_engine else {
            return;
        };
        let offset_ms = vad_chunk.chunk_bounds().start_offset_ms();
        match diarization_engine
            .voice_windows(vad_chunk.audio_bytes())
            .await
        {
            Ok(windows) => {
                self.voice_windows
                    .extend(windows.into_iter().map(|window| VoiceWindow {
                        chunk_index,
                        start_ms: offset_ms + window.start_time_ms(),
                        end_ms: offset_ms + window.end_time_ms(),
                        embedding: window.embedding().to_vec(),
                    }))
            }
            Err(error) => {
                tracing::warn!(
                    meeting_id = %self.meeting_id.as_str(),
                    error = %error,
                    "Voice windows failed; the final pass will have less to go on"
                );
            }
        }
    }

    /// Redoes who said what using the whole recording, and names each speaker once (#73, #74).
    ///
    /// Call when the recording's last chunk is done. Until then, and after a crash, the live result
    /// stands.
    ///
    /// Inputs: None; uses the windows and text gathered during the recording.
    /// Outputs: The number of segments now standing for this recording, or `None` when there was
    /// nothing to redo, as without diarization. A storage error leaves the live segments in place.
    /// Side effects: Queries saved voices, rewrites this recording's segments, replaces the members of
    /// the clusters still needed with one voice, deletes the others with their review clips, and
    /// drops the embeddings from memory. CPU-heavy on long meetings; call it off the UI thread.
    pub fn finish_meeting(&mut self) -> Result<Option<usize>, MeetingSessionError> {
        let voice_windows = std::mem::take(&mut self.voice_windows);
        let text_pieces = std::mem::take(&mut self.text_pieces);
        if voice_windows.is_empty() || text_pieces.is_empty() {
            return Ok(None);
        }
        let embeddings: Vec<Vec<f32>> = voice_windows
            .iter()
            .map(|window| window.embedding.clone())
            .collect();
        let labels = group_voice_windows(&embeddings, &SpeakerPassConfig::default());
        let voices = speaker_voices(&embeddings, &labels);
        let live_segments: Vec<TranscriptSegment> = self
            .persisted_segments
            .iter()
            .filter(|segment| !self.earlier_segments.contains(segment))
            .cloned()
            .collect();
        let live_overlap =
            live_overlap_per_speaker(&voice_windows, &labels, voices.len(), &live_segments);

        let mut attributions = self.name_speakers(&voices, &live_overlap);
        let mut claimed_clusters: Vec<Uuid> = attributions
            .iter()
            .filter_map(|attribution| attribution.cluster_id)
            .collect();
        for (speaker, voice) in voices.iter().enumerate() {
            if attributions[speaker].identity_id.is_some() {
                continue;
            }
            let cluster_id = live_overlap[speaker]
                .iter()
                .filter(|(cluster_id, _)| !claimed_clusters.contains(cluster_id))
                .max_by_key(|(_, windows)| *windows)
                .map_or_else(Uuid::new_v4, |(cluster_id, _)| *cluster_id);
            claimed_clusters.push(cluster_id);
            attributions[speaker].cluster_id = Some(cluster_id);
            self.save_final_cluster(cluster_id, voice)?;
        }

        let final_segments =
            self.final_segments(&voice_windows, &labels, &text_pieces, &attributions);
        let final_count = final_segments.len();
        let mut timeline = self.earlier_segments.clone();
        timeline.extend(final_segments);
        timeline.sort_by_key(TranscriptSegment::start_time_ms);
        self.note_repository
            .save_transcript_segments(&self.meeting_id, &timeline)?;
        self.persisted_segments = timeline;

        for segment in &live_segments {
            let Some(cluster_id) = segment.unknown_cluster_id() else {
                continue;
            };
            if !claimed_clusters.contains(&cluster_id)
                && let Err(error) = self
                    .note_repository
                    .delete_unknown_speaker_cluster(cluster_id)
            {
                tracing::warn!(
                    meeting_id = %self.meeting_id.as_str(),
                    error = %error,
                    "An unneeded speaker cluster could not be deleted"
                );
            }
        }
        tracing::info!(
            meeting_id = %self.meeting_id.as_str(),
            speaker_count = voices.len(),
            segment_count = final_count,
            "Meeting speakers finalized"
        );
        Ok(Some(final_count))
    }

    /// Names each speaker at most once, preferring Unknown to a wrong name.
    ///
    /// Inputs: Each speaker's voice and how their windows overlap live clusters.
    /// Outputs: One attribution per speaker. A speaker the owner already named during the recording
    /// keeps that name. If two speakers match one person, only the closer keeps the name.
    /// Side effects: Queries saved voices. A failed lookup leaves the speaker unnamed.
    fn name_speakers(
        &self,
        voices: &[super::meeting_speaker_pass::SpeakerVoice],
        live_overlap: &[Vec<(Uuid, usize)>],
    ) -> Vec<FinalAttribution> {
        let mut attributions = vec![FinalAttribution::default(); voices.len()];
        let mut best_score_per_identity: Vec<(Uuid, f32, usize)> = Vec::new();
        for (speaker, voice) in voices.iter().enumerate() {
            let user_named = live_overlap[speaker]
                .iter()
                .max_by_key(|(_, windows)| *windows)
                .and_then(|(cluster_id, windows)| {
                    let resolved = self
                        .note_repository
                        .load_unknown_speaker_cluster(*cluster_id)
                        .ok()
                        .flatten()?
                        .resolved_identity_id()?;
                    (*windows * 2 >= voice.window_count).then_some((*cluster_id, resolved))
                });
            if let Some((cluster_id, identity_id)) = user_named {
                attributions[speaker] = FinalAttribution {
                    identity_id: Some(identity_id),
                    cluster_id: Some(cluster_id),
                };
                continue;
            }
            let matched = self
                .voice_identity_matcher
                .best_voice_match(&voice.centroid)
                .unwrap_or_else(|error| {
                    tracing::warn!(
                        meeting_id = %self.meeting_id.as_str(),
                        error = %error,
                        "Voice matching failed; the speaker stays unnamed"
                    );
                    None
                });
            let Some((identity_id, score)) =
                matched.filter(|(_, score)| *score >= SPEAKER_NAME_THRESHOLD)
            else {
                continue;
            };
            match best_score_per_identity
                .iter_mut()
                .find(|(known, _, _)| *known == identity_id)
            {
                Some(entry) if entry.1 >= score => {}
                Some(entry) => {
                    attributions[entry.2].identity_id = None;
                    attributions[speaker].identity_id = Some(identity_id);
                    *entry = (identity_id, score, speaker);
                }
                None => {
                    attributions[speaker].identity_id = Some(identity_id);
                    best_score_per_identity.push((identity_id, score, speaker));
                }
            }
        }
        attributions
    }

    /// Stores an unnamed speaker's single voice as their cluster's only member.
    ///
    /// Inputs: The cluster to create or reuse, and the voice. Its member records as much clean speech
    /// as the group has, zero when the group isn't clean, so naming it later saves only a clean voice.
    /// Outputs: Success or a storage error.
    /// Side effects: Rewrites the cluster's members, which are biometric vectors.
    fn save_final_cluster(
        &self,
        cluster_id: Uuid,
        voice: &super::meeting_speaker_pass::SpeakerVoice,
    ) -> Result<(), MeetingSessionError> {
        const MILLISECONDS_PER_WINDOW: u64 = 1_000;
        let speech_ms = voice.window_count as u64 * MILLISECONDS_PER_WINDOW;
        let chunk_id = crate::domain::models::meeting_voice_chunk_id(cluster_id);
        let created_at = self
            .note_repository
            .load_unknown_speaker_cluster(cluster_id)?
            .map_or_else(chrono::Utc::now, |existing| existing.created_at());
        let cluster = UnknownSpeakerCluster::new(
            cluster_id,
            self.meeting_id.clone(),
            voice.centroid.clone(),
            vec![chunk_id.clone()],
            speech_ms,
            created_at,
            None,
        );
        let chunk = UnknownSpeakerChunk::new(
            chunk_id,
            voice.centroid.clone(),
            if voice.is_clean() { speech_ms } else { 0 },
        );
        self.note_repository
            .replace_unknown_speaker_cluster_members(&cluster, &[chunk])?;
        Ok(())
    }

    /// Builds this recording's final segments: within a chunk, each run of one speaker's text.
    ///
    /// Inputs: The windows and their speakers, the text in time order, and the speakers' attributions.
    /// Outputs: Segments in time order. Side effects: None.
    fn final_segments(
        &self,
        voice_windows: &[VoiceWindow],
        labels: &[usize],
        text_pieces: &[TextPiece],
        attributions: &[FinalAttribution],
    ) -> Vec<TranscriptSegment> {
        let mut runs: Vec<(usize, usize, u64, u64, Vec<&str>)> = Vec::new();
        for piece in text_pieces {
            let middle_ms = piece.start_ms + piece.end_ms.saturating_sub(piece.start_ms) / 2;
            let speaker =
                speaker_at(voice_windows, labels, piece.chunk_index, middle_ms).unwrap_or(0);
            match runs.last_mut() {
                Some(run) if run.0 == piece.chunk_index && run.1 == speaker => {
                    run.3 = piece.end_ms;
                    run.4.push(&piece.text);
                }
                _ => runs.push((
                    piece.chunk_index,
                    speaker,
                    piece.start_ms,
                    piece.end_ms,
                    vec![&piece.text],
                )),
            }
        }
        runs.into_iter()
            .map(|(_, speaker, start_ms, end_ms, words)| {
                let attribution = attributions.get(speaker).copied().unwrap_or_default();
                TranscriptSegment::new(
                    self.meeting_id.clone(),
                    start_ms,
                    end_ms,
                    attribution.identity_id,
                    words.join(" "),
                )
                .with_unknown_cluster_id(attribution.cluster_id)
            })
            .collect()
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
        let aligned_regions = align_transcript(
            chunk_bounds,
            vec![SpeakerRegion::new(0, duration_ms, attribution)],
            &transcript,
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
