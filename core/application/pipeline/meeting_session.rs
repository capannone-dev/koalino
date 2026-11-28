//! Orchestrates one meeting's audio chunks into persisted, speaker-attributed transcript segments.
//!
//! Local VAD chunks flow transcribe → diarize → match voice → cluster unknown voices →
//! align/merge → persist. External bridge chunks (e.g. Teams) are one participant's track: they
//! go to the mapped identity while the track carries one voice, and are matched voice by voice
//! once it carries several (a meeting room, #103; see `participant_tracks`). Weak and unmatched voices are grouped per meeting by
//! `UnknownSpeakerClusterer`, so each cluster carries one review clip and its segments can be
//! re-attributed together. The session depends only on the `TranscriptionEngine`,
//! `DiarizationEngine`, `VoiceIdentityMatcher`, `IdentityMapper`, `NoteRepository`, and (for
//! cluster assignment) `VoiceVectorDB` ports. It never logs transcript text, audio, or embeddings.

use crate::domain::transcription_language::TranscriptionLanguage;
use std::{cell::RefCell, collections::HashMap, rc::Rc, time::Instant};

/// Credits participant tracks by the voices they carry (#103).
mod participant_tracks;

use uuid::Uuid;

use crate::application::echo_removal::without_echoes;
use crate::domain::{
    echo_detect::{EchoDetector, EchoTrack},
    participant_track_voices::{ParticipantTrackVoices, TrackVoicePolicy},
    pcm_wav::{pcm16_mono_samples, slice_pcm16_wav},
    transcript_confidence::{
        OverlapSpan, SegmentConfidence, is_uncertain_probability, merge_overlap_spans,
        overlaps_segment,
    },
    transcription_glossary::TranscriptionGlossary,
};
use crate::{
    domain::{
        errors::{StorageError, TranscriptionError},
        models::{
            AudioRecording, ExternalMediaChunk, ExternalTrackKind, MeetingId, SegmentSource,
            SpeakerIdentity, Transcript, TranscriptSegment, TranscriptionContext,
            UnassignedVoiceClip, UnknownSpeakerChunk, UnknownSpeakerCluster, VoiceMatchResult,
            WeakVoiceMatchCandidate,
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
    /// The engine was unsure of this word (#91); always `false` for a region piece.
    uncertain: bool,
}

/// One speaker's run of text within a chunk, as the final pass builds it.
struct FinalRun<'pieces> {
    /// The chunk the run lies in.
    chunk_index: usize,
    /// The final pass's speaker.
    speaker: usize,
    /// Start in meeting milliseconds.
    start_ms: u64,
    /// End in meeting milliseconds.
    end_ms: u64,
    /// The run's text, in order.
    pieces: Vec<&'pieces TextPiece>,
    /// No voice window lay near some of the text, so its speaker is a guess.
    speaker_guessed: bool,
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
                uncertain: false,
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
            uncertain: is_uncertain_probability(word.probability_permille()),
        })
        .collect()
}

/// Least audio, in milliseconds, a review clip keeps after the final pass cuts it (#116).
const MIN_REVIEW_CLIP_MS: u64 = 1_000;
/// Most review clips a live voice keeps during a recording (#134). The final pass may split a
/// live voice into several people and keeps one clip per person, so later chunks are kept too.
const MAX_LIVE_REVIEW_CLIPS_PER_CLUSTER: usize = 3;
/// The shortest turn a live strong voice match names automatically; shorter ones are only
/// suggested (#61: the readers' measurement on 2026-11-15, 346 live decisions).
const MIN_TURN_FOR_LIVE_NAME_MS: u64 = 1_500;

/// Longest silence, in milliseconds, inside one speaker's run of windows.
const MAX_RUN_GAP_MS: u64 = 500;

/// Finds one speaker's longest unbroken stretch inside a time range (#116).
///
/// Inputs: The windows (meeting time) and their speakers; the speaker; the range in meeting
/// milliseconds. Each window counts for its middle half. Another speaker's window, or a gap over
/// `MAX_RUN_GAP_MS`, breaks a run.
/// Outputs: The stretch, cut to the range, or `None` when the speaker has no window there.
/// Side effects: None.
fn speaker_run_within(
    voice_windows: &[VoiceWindow],
    labels: &[usize],
    speaker: usize,
    range_start_ms: u64,
    range_end_ms: u64,
) -> Option<(u64, u64)> {
    let mut cells: Vec<(u64, u64, bool)> = voice_windows
        .iter()
        .zip(labels)
        .filter_map(|(window, label)| {
            let quarter_ms = window.end_ms.saturating_sub(window.start_ms) / 4;
            let start_ms = (window.start_ms + quarter_ms).max(range_start_ms);
            let end_ms = (window.end_ms - quarter_ms).min(range_end_ms);
            (start_ms < end_ms).then_some((start_ms, end_ms, *label == speaker))
        })
        .collect();
    cells.sort_unstable_by_key(|(start_ms, _, _)| *start_ms);
    let mut longest: Option<(u64, u64)> = None;
    let mut current: Option<(u64, u64)> = None;
    for (start_ms, end_ms, is_speaker) in cells {
        current = match current {
            _ if !is_speaker => None,
            Some((run_start, run_end)) if start_ms <= run_end + MAX_RUN_GAP_MS => {
                Some((run_start, run_end.max(end_ms)))
            }
            _ => Some((start_ms, end_ms)),
        };
        if let Some((run_start, run_end)) = current
            && longest
                .is_none_or(|(best_start, best_end)| run_end - run_start > best_end - best_start)
        {
            longest = Some((run_start, run_end));
        }
    }
    longest
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
    /// The track the audio was captured on (#108); the microphone unless set.
    track: SegmentSource,
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
            track: SegmentSource::Microphone,
        })
    }

    /// Marks the track the audio was captured on (#108).
    ///
    /// Inputs: `SegmentSource::ComputerSound` for the computer's own sound.
    /// Outputs: The chunk. Side effects: None.
    #[must_use]
    pub fn with_track(mut self, track: SegmentSource) -> Self {
        self.track = track;
        self
    }

    /// Returns the track the audio was captured on.
    pub fn track(&self) -> SegmentSource {
        self.track
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
    /// Every live cluster this recording's voices joined or founded, including voices that got no
    /// words, so the final pass can remove the ones it doesn't keep (#82).
    live_cluster_ids: Vec<Uuid>,
    /// Languages this recording already stored in the meeting's metadata, so each is written
    /// once per recording rather than once per chunk.
    recorded_languages: Vec<TranscriptionLanguage>,
    /// The language the engine is configured with (`NOTE_TAKER_WHISPER_LANGUAGE`), recorded for
    /// audio that carries no chosen language; `None` records nothing for such audio.
    default_language: Option<TranscriptionLanguage>,
    /// Similarity policy used to group unknown voices into clusters.
    clustering_config: ClusteringConfig,
    /// Saves a telemetry row per completed transcription and diarization; `None` records none.
    task_metrics: Option<Rc<TaskMetricsRecorder>>,
    /// The owner's names and terms, read when the recording started; `None` prompts nothing.
    glossary: Option<TranscriptionGlossary>,
    /// Stretches of this recording, in meeting time, where diarization heard overlapping voices
    /// (#90); disjoint and in time order.
    overlap_spans: Vec<OverlapSpan>,
    /// What each online-meeting participant's track carried so far, by external ID (#103).
    participant_tracks: HashMap<String, ParticipantTrackVoices>,
    /// The thresholds that judge a participant's track.
    track_voice_policy: TrackVoicePolicy,
    /// Each review clip this recording saved: its cluster, its ID, and where in the meeting its
    /// audio begins, so the final pass can give each speaker the clip that holds them (#116, #134).
    review_clip_starts: RefCell<Vec<(Uuid, Uuid, u64)>>,
    /// Compares the microphone with the computer's sound, to hear echo (#108).
    echo_detector: EchoDetector,
    /// Whether this recording already stored that echo was heard.
    echo_recorded: bool,
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
            live_cluster_ids: Vec::new(),
            recorded_languages: Vec::new(),
            default_language: None,
            clustering_config: ClusteringConfig::default(),
            task_metrics: None,
            glossary: None,
            overlap_spans: Vec::new(),
            participant_tracks: HashMap::new(),
            track_voice_policy: TrackVoicePolicy::default(),
            review_clip_starts: RefCell::new(Vec::new()),
            echo_detector: EchoDetector::new(),
            echo_recorded: false,
        })
    }

    /// Transcribes every later chunk with the owner's glossary of names and terms.
    ///
    /// Inputs: The glossary, or `None`. Outputs: The session, which prompts each chunk with it and
    /// drops text that only echoes it. Side effects: None until a chunk is processed.
    pub fn with_glossary(mut self, glossary: Option<TranscriptionGlossary>) -> Self {
        self.glossary = glossary.filter(|glossary| !glossary.is_empty());
        self
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

    /// Sets the language the engine transcribes in when a chunk carries none.
    ///
    /// Inputs: The configured language, or `None` when it is unknown.
    /// Outputs: The session, which records it in the meeting's metadata for such chunks and for
    /// external (Teams) chunks. Side effects: None until a chunk is processed.
    pub fn with_default_language(
        mut self,
        default_language: Option<TranscriptionLanguage>,
    ) -> Self {
        self.default_language = default_language;
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
    /// A chunk of the computer's sound (#108) is transcribed and attributed the same way, and its
    /// segments are marked as the computer's; the final speaker pass and overlap marks are the
    /// microphone's alone. Both tracks feed the echo detector; once it hears the microphone
    /// following the speakers, the microphone's copies of the computer's lines are dropped.
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
        let track = vad_chunk.track();
        self.listen_for_echo(vad_chunk);
        // No previous text as a prompt: Whisper then repeated or completed the previous chunk's
        // last words (Issue #51). The chosen language keeps chunks consistent instead.
        // The glossary prompts each chunk alone, and its echo is dropped below.
        let transcription_context = TranscriptionContext::new(None)
            .with_language(vad_chunk.language())
            .with_glossary(self.glossary.clone());
        let transcript = self
            .transcription_engine
            .transcribe(vad_chunk.audio_recording(), &transcription_context)
            .map_err(|error| self.log_transcription_failure(chunk_bounds, error))?;
        let transcript = match &self.glossary {
            Some(glossary) => glossary.without_echo(transcript),
            None => transcript,
        };
        if let Some(task_metrics) = &self.task_metrics {
            task_metrics.record_transcription(
                processing_started_at.elapsed(),
                chunk_bounds
                    .end_offset_ms()
                    .saturating_sub(chunk_bounds.start_offset_ms()),
            );
        }

        self.record_chunk_language(
            vad_chunk.language().or(self.default_language),
            chunk_bounds.start_offset_ms(),
        );

        let (speaker_regions, chunk_overlap) = self.diarize_and_attribute(vad_chunk).await;
        if track == SegmentSource::Microphone {
            self.record_overlap(chunk_bounds, chunk_overlap);
        }
        for cluster_id in speaker_regions
            .iter()
            .filter_map(|region| region.attribution().unknown_cluster_id())
        {
            if !self.live_cluster_ids.contains(&cluster_id) {
                self.live_cluster_ids.push(cluster_id);
            }
        }
        let aligned_regions = align_transcript(chunk_bounds, speaker_regions, &transcript);
        let attributed_segments = if track == SegmentSource::Microphone {
            let chunk_index = self.chunk_count;
            self.chunk_count += 1;
            let chunk_pieces =
                text_pieces_of(chunk_index, chunk_bounds, &transcript, &aligned_regions);
            let attributed_segments = self.persist_aligned_regions(aligned_regions, track)?;
            self.text_pieces.extend(chunk_pieces);
            self.collect_voice_windows(vad_chunk, chunk_index).await;
            attributed_segments
        } else {
            self.persist_aligned_regions(aligned_regions, track)?
        };
        self.drop_live_echoes();
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

    /// Feeds a chunk's audio to the echo detector (#108).
    ///
    /// Inputs: The chunk; audio that is not 16 kHz mono PCM16 WAV is skipped.
    /// Outputs: None. Side effects: Keeps the chunk's loudness envelope in memory, not its audio.
    fn listen_for_echo(&mut self, vad_chunk: &VadAudioChunk) {
        let Some((sample_rate_hz, pcm)) = pcm16_mono_samples(vad_chunk.audio_bytes()) else {
            return;
        };
        if sample_rate_hz != 16_000 {
            return;
        }
        let samples: Vec<i16> = pcm
            .as_chunks::<2>()
            .0
            .iter()
            .map(|sample| i16::from_le_bytes(*sample))
            .collect();
        let track = match vad_chunk.track() {
            SegmentSource::Microphone => EchoTrack::Microphone,
            SegmentSource::ComputerSound => EchoTrack::ComputerSound,
        };
        self.echo_detector
            .push(track, vad_chunk.chunk_bounds().start_offset_ms(), &samples);
    }

    /// Drops the microphone's copies of the computer's sound once echo was heard (#108).
    ///
    /// Inputs: None; uses the timeline and the echo detector.
    /// Outputs: None; a storage failure is logged and leaves the copies for the next chunk.
    /// Side effects: The first time, stores that echo was heard with the meeting; rewrites the
    /// meeting's segments when a copy goes.
    fn drop_live_echoes(&mut self) {
        if !self.echo_detector.present() {
            return;
        }
        if !self.echo_recorded {
            match self.note_repository.record_echo_heard(&self.meeting_id) {
                Ok(()) => self.echo_recorded = true,
                Err(error) => tracing::warn!(
                    meeting_id = %self.meeting_id.as_str(),
                    error = %error,
                    "Could not store that echo was heard; later passes will keep the copies"
                ),
            }
        }
        let detector = &self.echo_detector;
        let (timeline, dropped) = without_echoes(self.persisted_segments.clone(), true, &|ms| {
            detector.correlated_at(ms)
        });
        if dropped == 0 {
            return;
        }
        match self
            .note_repository
            .save_transcript_segments(&self.meeting_id, &timeline)
        {
            Ok(()) => {
                tracing::info!(
                    meeting_id = %self.meeting_id.as_str(),
                    dropped_segment_count = dropped,
                    "Dropped the microphone's echoes of the computer's sound"
                );
                self.persisted_segments = timeline;
            }
            Err(error) => tracing::warn!(
                meeting_id = %self.meeting_id.as_str(),
                error = %error,
                "Could not drop the microphone's echoes"
            ),
        }
    }

    /// Keeps a chunk's overlapping speech in meeting time and records it with the meeting (#90).
    ///
    /// Inputs: The chunk's bounds and its chunk-relative overlap spans.
    /// Outputs: None; a storage failure is logged, and the spans still mark this recording's
    /// segments.
    /// Side effects: Writes the meeting's document when there is overlap.
    fn record_overlap(&mut self, chunk_bounds: ChunkBounds, chunk_overlap: Vec<OverlapSpan>) {
        if chunk_overlap.is_empty() {
            return;
        }
        let offset_ms = i64::try_from(chunk_bounds.start_offset_ms()).unwrap_or(i64::MAX);
        let meeting_spans: Vec<OverlapSpan> = chunk_overlap
            .into_iter()
            .map(|span| span.shifted(offset_ms))
            .collect();
        if let Err(error) = self
            .note_repository
            .record_overlap_spans(&self.meeting_id, &meeting_spans)
        {
            tracing::warn!(
                meeting_id = %self.meeting_id.as_str(),
                error = %error,
                "Could not record overlapping speech; later passes will not mark it"
            );
        }
        let mut spans = std::mem::take(&mut self.overlap_spans);
        spans.extend(meeting_spans);
        self.overlap_spans = merge_overlap_spans(spans);
    }

    /// Stores the language a chunk was transcribed with in the meeting's metadata, once.
    ///
    /// Inputs: The chunk's language (`None` when it is unknown, which records nothing) and the
    /// chunk's start within the meeting.
    /// Outputs: None; a storage failure is logged and does not stop the recording.
    /// Side effects: Writes the meeting's document the first time a language appears.
    fn record_chunk_language(&mut self, language: Option<TranscriptionLanguage>, start_ms: u64) {
        let Some(language) = language else {
            return;
        };
        if self.recorded_languages.contains(&language) {
            return;
        }
        match self.note_repository.record_transcription_language(
            &self.meeting_id,
            language,
            start_ms,
        ) {
            Ok(()) => self.recorded_languages.push(language),
            Err(error) => tracing::warn!(
                meeting_id = %self.meeting_id.as_str(),
                error = %error,
                "Could not record the transcription language; the meeting will not show it"
            ),
        }
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
        let start_times_ms: Vec<u64> = voice_windows.iter().map(|window| window.start_ms).collect();
        let labels = group_voice_windows(
            &embeddings,
            Some(&start_times_ms),
            &SpeakerPassConfig::default(),
        );
        let voices = speaker_voices(&embeddings, &labels);
        // The computer's sound keeps its live speakers; the pass redoes the microphone's.
        let (computer_sound, live_segments): (Vec<TranscriptSegment>, Vec<TranscriptSegment>) =
            self.persisted_segments
                .iter()
                .filter(|segment| !self.earlier_segments.contains(segment))
                .cloned()
                .partition(|segment| segment.source() == SegmentSource::ComputerSound);
        let live_overlap =
            live_overlap_per_speaker(&voice_windows, &labels, voices.len(), &live_segments);

        let mut attributions = self.name_speakers(&voices, &live_overlap);
        let mut claimed_clusters: Vec<Uuid> = attributions
            .iter()
            .filter_map(|attribution| attribution.cluster_id)
            .chain(
                computer_sound
                    .iter()
                    .filter_map(TranscriptSegment::unknown_cluster_id),
            )
            .collect();
        let mut used_clip_ids: Vec<Uuid> = Vec::new();
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
            self.give_review_clip_to_speaker(
                cluster_id,
                speaker,
                &voice_windows,
                &labels,
                &mut used_clip_ids,
            );
        }
        self.delete_unused_review_clips(&used_clip_ids);
        self.review_clip_starts.borrow_mut().clear();

        let final_segments =
            self.final_segments(&voice_windows, &labels, &text_pieces, &attributions);
        let mut timeline = self.earlier_segments.clone();
        timeline.extend(final_segments);
        timeline.extend(computer_sound);
        timeline.sort_by_key(TranscriptSegment::start_time_ms);
        let detector = &self.echo_detector;
        let (timeline, _) = without_echoes(timeline, detector.present(), &|ms| {
            detector.correlated_at(ms)
        });
        let final_count = timeline
            .iter()
            .filter(|segment| !self.earlier_segments.contains(segment))
            .count();
        self.note_repository
            .save_transcript_segments(&self.meeting_id, &timeline)?;
        self.persisted_segments = timeline;

        let live_cluster_ids = std::mem::take(&mut self.live_cluster_ids);
        let segment_cluster_ids = live_segments
            .iter()
            .filter_map(TranscriptSegment::unknown_cluster_id);
        let mut unneeded_cluster_ids: Vec<Uuid> = Vec::new();
        for cluster_id in live_cluster_ids.into_iter().chain(segment_cluster_ids) {
            if !unneeded_cluster_ids.contains(&cluster_id) {
                unneeded_cluster_ids.push(cluster_id);
            }
        }
        for cluster_id in unneeded_cluster_ids {
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
            // Scores and IDs only, for tuning the naming cutoff (#61); nothing that identifies a person.
            if let Some((identity_id, score)) = matched {
                tracing::debug!(
                    meeting_id = %self.meeting_id.as_str(),
                    speaker,
                    identity_id = %identity_id,
                    score,
                    windows = voice.window_count,
                    consistency = voice.consistency,
                    core_consistency = voice.core_consistency,
                    "final speaker's closest saved voice"
                );
            }
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
        // The member carries the voice that naming saves: the core, steadier than the whole (#196).
        let chunk = UnknownSpeakerChunk::new(
            chunk_id,
            voice.core_centroid.clone(),
            if voice.is_clean() { speech_ms } else { 0 },
        );
        self.note_repository
            .replace_unknown_speaker_cluster_members(&cluster, &[chunk])?;
        Ok(())
    }

    /// Gives an unnamed final speaker the review clip that holds most of them alone (#116, #134).
    ///
    /// The live pass can merge two people into one voice, so a clip may hold someone else too, and
    /// a speaker the final pass separated may have no clip of its own. Every clip this recording
    /// saved and no other speaker took is a candidate. Inside a clip, each voice window counts for
    /// its middle half, where its speaker is surest; the clip keeps the speaker's longest unbroken
    /// stretch of those. The candidate with the longest stretch wins (the speaker's own cluster's
    /// clip on a tie), is cut to it, and is filed under `cluster_id`.
    ///
    /// Inputs: The speaker's cluster, the speaker, this recording's windows and their speakers, and
    /// the clips already given to other speakers (the chosen one is added).
    /// Outputs: None. A speaker with less than `MIN_REVIEW_CLIP_MS` alone in every clip gets none:
    /// no clip is better than one of another person.
    /// Side effects: Rewrites the chosen clip. A storage failure is logged and leaves it as it was.
    fn give_review_clip_to_speaker(
        &self,
        cluster_id: Uuid,
        speaker: usize,
        voice_windows: &[VoiceWindow],
        labels: &[usize],
        used_clip_ids: &mut Vec<Uuid>,
    ) {
        let candidates: Vec<(Uuid, Uuid, u64)> = self
            .review_clip_starts
            .borrow()
            .iter()
            .filter(|(_, clip_id, _)| !used_clip_ids.contains(clip_id))
            .copied()
            .collect();
        if candidates.is_empty() {
            return;
        }
        let clips = match self.note_repository.get_unassigned_clips(&self.meeting_id) {
            Ok(clips) => clips,
            Err(error) => {
                tracing::warn!(meeting_id = %self.meeting_id.as_str(), error = %error, "Review clips could not be read; the clips stay whole");
                return;
            }
        };
        let best = candidates
            .iter()
            .filter_map(|(source_cluster_id, clip_id, clip_start_ms)| {
                let clip = clips
                    .iter()
                    .find(|clip| clip.clip_id() == *clip_id && clip.start_time_ms() == 0)?;
                let clip_end_ms = clip_start_ms + clip.end_time_ms();
                let run =
                    speaker_run_within(voice_windows, labels, speaker, *clip_start_ms, clip_end_ms)
                        .filter(|(start_ms, end_ms)| end_ms - start_ms >= MIN_REVIEW_CLIP_MS)?;
                Some((clip, *source_cluster_id, *clip_start_ms, clip_end_ms, run))
            })
            .max_by_key(|(_, source_cluster_id, _, _, (start_ms, end_ms))| {
                (end_ms - start_ms, *source_cluster_id == cluster_id)
            });
        let Some((clip, source_cluster_id, clip_start_ms, clip_end_ms, (start_ms, end_ms))) = best
        else {
            return;
        };
        used_clip_ids.push(clip.clip_id());
        if source_cluster_id == cluster_id && (start_ms, end_ms) == (clip_start_ms, clip_end_ms) {
            return;
        }
        let audio = if (start_ms, end_ms) == (clip_start_ms, clip_end_ms) {
            Some(clip.audio_bytes().to_vec())
        } else {
            slice_pcm16_wav(
                clip.audio_bytes(),
                start_ms - clip_start_ms,
                end_ms - clip_start_ms,
            )
        };
        let Some(audio) = audio else {
            return;
        };
        let given_clip = UnassignedVoiceClip::with_review_context(
            clip.clip_id(),
            self.meeting_id.clone(),
            0,
            end_ms - start_ms,
            audio,
            clip.derived_embedding().to_vec(),
            clip.weak_match_candidate().cloned(),
        )
        .with_cluster_id(Some(cluster_id));
        let outcome = self
            .note_repository
            .delete_unassigned_clip(clip.clip_id())
            .and_then(|()| self.note_repository.save_unassigned_clip(given_clip));
        if let Err(error) = outcome {
            tracing::warn!(meeting_id = %self.meeting_id.as_str(), error = %error, "A review clip could not be cut to its speaker");
        }
    }

    /// Deletes this recording's review clips that no final speaker took: they hold no one alone
    /// for long enough, or belong to a voice the final pass dropped (#134).
    ///
    /// Side effects: Deletes clips (raw audio); a failure is logged.
    fn delete_unused_review_clips(&self, used_clip_ids: &[Uuid]) {
        let unused: Vec<Uuid> = self
            .review_clip_starts
            .borrow()
            .iter()
            .map(|(_, clip_id, _)| *clip_id)
            .filter(|clip_id| !used_clip_ids.contains(clip_id))
            .collect();
        for clip_id in unused {
            if let Err(error) = self.note_repository.delete_unassigned_clip(clip_id) {
                tracing::warn!(meeting_id = %self.meeting_id.as_str(), error = %error, "An unused review clip could not be deleted");
            }
        }
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
        let mut runs: Vec<FinalRun<'_>> = Vec::new();
        for piece in text_pieces {
            let middle_ms = piece.start_ms + piece.end_ms.saturating_sub(piece.start_ms) / 2;
            let found = speaker_at(voice_windows, labels, piece.chunk_index, middle_ms);
            let speaker = found.unwrap_or(0);
            match runs.last_mut() {
                Some(run) if run.chunk_index == piece.chunk_index && run.speaker == speaker => {
                    run.end_ms = piece.end_ms;
                    run.pieces.push(piece);
                    run.speaker_guessed |= found.is_none();
                }
                _ => runs.push(FinalRun {
                    chunk_index: piece.chunk_index,
                    speaker,
                    start_ms: piece.start_ms,
                    end_ms: piece.end_ms,
                    pieces: vec![piece],
                    speaker_guessed: found.is_none(),
                }),
            }
        }
        runs.into_iter()
            .map(|run| {
                let attribution = attributions.get(run.speaker).copied().unwrap_or_default();
                let text = run
                    .pieces
                    .iter()
                    .map(|piece| piece.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                // Positions count the text's words, as a region piece may hold several.
                let mut uncertain_words = Vec::new();
                let mut position = 0_usize;
                for piece in &run.pieces {
                    let piece_words = piece.text.split_whitespace().count();
                    if piece.uncertain {
                        uncertain_words.extend(
                            (position..position + piece_words)
                                .map(|word| u16::try_from(word).unwrap_or(u16::MAX)),
                        );
                    }
                    position += piece_words;
                }
                TranscriptSegment::new(
                    self.meeting_id.clone(),
                    run.start_ms,
                    run.end_ms,
                    attribution.identity_id,
                    text,
                )
                .with_unknown_cluster_id(attribution.cluster_id)
                .with_confidence(SegmentConfidence {
                    overlapping_speech: overlaps_segment(
                        &self.overlap_spans,
                        run.start_ms,
                        run.end_ms,
                    ),
                    uncertain_speaker: run.speaker_guessed,
                    uncertain_words,
                })
            })
            .collect()
    }

    /// Processes one chunk of an online-meeting participant's track.
    ///
    /// Inputs: `identity_mapper` resolves the chunk's external participant and may keep its
    /// voice; `media_chunk` carries the track's audio whose `timestamp_ms` is taken as the
    /// meeting-relative start (the bridge stream is assumed to start with the meeting);
    /// `audio_recording` identifies the chunk for transcription and must belong to this meeting;
    /// `duration_ms` is the chunk length.
    /// Outputs: The newly persisted segments (none for a blank transcript), or an error when the
    /// meeting mismatches, transcription fails, or storage fails. The participant gets the chunk
    /// while its track carries one voice (or diarization is missing); a track with several voices
    /// has each voice matched against the gallery (#103), as does a browser's mixed remote track
    /// from its first chunk, without any mapping (#102). A mapping failure degrades the
    /// participant's part to `Unknown` without a review clip.
    /// Side effects: Runs transcription and diarization, may persist an identity mapping, may
    /// cluster unknown voices and save review clips, may offer the participant's voice to the
    /// gallery, rewrites the meeting's segments, and emits safe structured logs.
    pub async fn process_external_chunk<MapperPort>(
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
        let transcription_started_at = Instant::now();
        let transcript = self
            .transcription_engine
            .transcribe_external_media(media_chunk, audio_recording)
            .map_err(|error| self.log_transcription_failure(chunk_bounds, error))?;
        // Timed like the microphone's chunks, so Automatic sees a call it can't keep up with.
        if let Some(task_metrics) = &self.task_metrics {
            task_metrics.record_transcription(transcription_started_at.elapsed(), duration_ms);
        }
        // External audio is always transcribed in the configured language.
        self.record_chunk_language(self.default_language, chunk_start_ms);

        let external_participant = media_chunk.external_participant();
        // A mixed remote track (#102) is nobody's own voice, so it maps to no identity.
        let mapped_identity = match media_chunk.track_kind() {
            ExternalTrackKind::MixedRemote => None,
            ExternalTrackKind::Participant => {
                match identity_mapper.map_external_to_local(external_participant.clone()) {
                    Ok(local_identity) => Some(local_identity),
                    Err(error) => {
                        tracing::warn!(
                            meeting_id = %self.meeting_id.as_str(),
                            external_id = %external_participant.external_id(),
                            error = %error,
                            "External identity mapping failed; segment persisted as Unknown"
                        );
                        None
                    }
                }
            }
        };
        let participant = mapped_identity.as_ref().map_or(
            SpeakerAttribution::Unknown {
                clip_id: None,
                cluster_id: None,
            },
            |identity| SpeakerAttribution::Known {
                identity_id: identity.identity_id(),
            },
        );
        let speaker_regions = self
            .participant_track_regions(
                identity_mapper,
                media_chunk,
                audio_recording,
                chunk_bounds,
                mapped_identity.as_ref(),
                participant,
            )
            .await;
        for cluster_id in speaker_regions
            .iter()
            .filter_map(|region| region.attribution().unknown_cluster_id())
        {
            if !self.live_cluster_ids.contains(&cluster_id) {
                self.live_cluster_ids.push(cluster_id);
            }
        }
        let aligned_regions = align_transcript(chunk_bounds, speaker_regions, &transcript);
        let attributed_segments =
            self.persist_aligned_regions(aligned_regions, SegmentSource::Microphone)?;
        tracing::info!(
            meeting_id = %self.meeting_id.as_str(),
            external_id = %external_participant.external_id(),
            chunk_start_ms = chunk_bounds.start_offset_ms(),
            chunk_end_ms = chunk_bounds.end_offset_ms(),
            segment_count = attributed_segments.len(),
            "External bridge chunk processed"
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
    /// Outputs: Chunk-relative attributed regions, empty when diarization is unavailable or
    /// fails (alignment turns that into a single `Unattributed` region), and the chunk-relative
    /// stretches of overlapping speech (#90).
    /// Side effects: Executes diarization, queries voice identities, saves review clips, and logs.
    async fn diarize_and_attribute(
        &self,
        vad_chunk: &VadAudioChunk,
    ) -> (Vec<SpeakerRegion>, Vec<OverlapSpan>) {
        let Some(diarization_engine) = &self.diarization_engine else {
            tracing::debug!(
                meeting_id = %self.meeting_id.as_str(),
                "No diarization engine configured; chunk persisted as Unattributed"
            );
            return (Vec::new(), Vec::new());
        };
        let diarization_started_at = Instant::now();
        match diarization_engine
            .diarize_with_overlap(vad_chunk.audio_bytes())
            .await
        {
            Ok(diarized) => {
                if let Some(task_metrics) = &self.task_metrics {
                    task_metrics.record_diarization(diarization_started_at.elapsed());
                }
                let regions = diarized
                    .segments
                    .iter()
                    .map(|speaker_segment| {
                        SpeakerRegion::new(
                            speaker_segment.start_time_ms(),
                            speaker_segment.end_time_ms(),
                            self.attribute_speaker_segment(vad_chunk, speaker_segment),
                        )
                    })
                    .collect();
                (regions, diarized.overlap)
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
                (Vec::new(), Vec::new())
            }
        }
    }

    /// Matches one diarized turn against the saved voices, naming it only when it is long enough.
    ///
    /// Inputs: The diarized turn (its times and averaged embedding).
    /// Outputs: The matcher's result, except that a strong match on a turn shorter than
    /// `MIN_TURN_FOR_LIVE_NAME_MS` becomes a weak match with the nearest voice's real score (#61):
    /// in the readers' measurement both wrong live names came from turns of 0.4 s and 1.2 s,
    /// while every right one came from a longer turn.
    /// Side effects: Queries the voice-identity port (twice for such a short strong turn).
    fn live_voice_match(
        &self,
        speaker_segment: &SpeakerSegment,
    ) -> Result<VoiceMatchResult, StorageError> {
        let result = self
            .voice_identity_matcher
            .match_embedding(speaker_segment.embedding())?;
        let turn_ms = speaker_segment
            .end_time_ms()
            .saturating_sub(speaker_segment.start_time_ms());
        match result {
            VoiceMatchResult::StrongMatch(identity_id) if turn_ms < MIN_TURN_FOR_LIVE_NAME_MS => {
                let score = self
                    .voice_identity_matcher
                    .best_voice_match(speaker_segment.embedding())?
                    .filter(|(closest, _)| *closest == identity_id)
                    .map_or(1.0, |(_, score)| score);
                Ok(VoiceMatchResult::WeakMatch(identity_id, score))
            }
            other => Ok(other),
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
        let match_result = self.live_voice_match(speaker_segment);
        // Where in the meeting each live decision fell, so the per-turn cutoffs can be measured
        // against who really spoke (#61). Times and the decision only; the score is on the
        // matcher's own line just before.
        let chunk_bounds = vad_chunk.chunk_bounds();
        tracing::debug!(
            meeting_id = %self.meeting_id.as_str(),
            chunk_start_ms = chunk_bounds.start_offset_ms(),
            segment_start_ms = speaker_segment.start_time_ms(),
            segment_end_ms = speaker_segment.end_time_ms(),
            decision = match &match_result {
                Ok(VoiceMatchResult::StrongMatch(_)) => "strong",
                Ok(VoiceMatchResult::WeakMatch(..)) => "weak",
                Ok(VoiceMatchResult::NoMatch) => "none",
                Err(_) => "error",
            },
            "Live voice decision"
        );
        match match_result {
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

    /// Clusters an unattributed voice and keeps a few review clips per cluster.
    ///
    /// Inputs: The source chunk, its diarized segment, and an optional weak candidate.
    /// Outputs: `(clip_id, cluster_id)`. A voice that founds a cluster gets a clip linked to it;
    /// one that joins gets one too while the cluster has fewer than
    /// `MAX_LIVE_REVIEW_CLIPS_PER_CLUSTER` from this recording (#134). If clustering fails, the voice falls
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
            Ok(membership) => {
                // A few clips per voice, so a voice the final pass splits keeps one per person.
                let kept_clips = self
                    .review_clip_starts
                    .borrow()
                    .iter()
                    .filter(|(cluster_id, _, _)| *cluster_id == membership.cluster_id())
                    .count();
                let clip_id = (kept_clips < MAX_LIVE_REVIEW_CLIPS_PER_CLUSTER)
                    .then(|| {
                        self.preserve_review_clip(
                            chunk_id,
                            vad_chunk,
                            speaker_segment,
                            weak_match_candidate,
                            Some(membership.cluster_id()),
                        )
                    })
                    .flatten();
                (clip_id, Some(membership.cluster_id()))
            }
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
    /// The clip holds only the voice's own span of the chunk (#116): a whole chunk often holds
    /// several people, and the owner names a voice by listening to its clip.
    ///
    /// Inputs: The clip ID to use, the source chunk, the diarized segment (chunk-relative times),
    /// an optional weak candidate, and the cluster the clip represents (if any).
    /// Outputs: The clip ID, or `None` when storage rejected it (the segment is still persisted).
    /// Side effects: Persists the span's raw audio and the derived embedding through
    /// `NoteRepository`; audio that is not a mono 16-bit WAV is kept whole, timed by the segment.
    /// Remembers where a clustered clip begins in the meeting, for the final pass.
    fn preserve_review_clip(
        &self,
        clip_id: Uuid,
        vad_chunk: &VadAudioChunk,
        speaker_segment: &SpeakerSegment,
        weak_match_candidate: Option<WeakVoiceMatchCandidate>,
        cluster_id: Option<Uuid>,
    ) -> Option<Uuid> {
        let span_audio = slice_pcm16_wav(
            vad_chunk.audio_bytes(),
            speaker_segment.start_time_ms(),
            speaker_segment.end_time_ms(),
        );
        let (start_time_ms, end_time_ms, audio_bytes, clip_start_in_chunk_ms) = match span_audio {
            Some(audio) => (
                0,
                speaker_segment
                    .end_time_ms()
                    .saturating_sub(speaker_segment.start_time_ms()),
                audio,
                speaker_segment.start_time_ms(),
            ),
            None => (
                speaker_segment.start_time_ms(),
                speaker_segment.end_time_ms(),
                vad_chunk.audio_bytes().to_vec(),
                0,
            ),
        };
        let review_clip = UnassignedVoiceClip::with_review_context(
            clip_id,
            self.meeting_id.clone(),
            start_time_ms,
            end_time_ms,
            audio_bytes,
            speaker_segment.embedding().to_vec(),
            weak_match_candidate,
        )
        .with_cluster_id(cluster_id);
        match self.note_repository.save_unassigned_clip(review_clip) {
            Ok(()) => {
                if let Some(cluster_id) = cluster_id {
                    self.review_clip_starts.borrow_mut().push((
                        cluster_id,
                        clip_id,
                        vad_chunk.chunk_bounds().start_offset_ms() + clip_start_in_chunk_ms,
                    ));
                }
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
    /// Inputs: Meeting-timed regions produced by alignment, and the track they were heard on.
    /// Outputs: The new attributed segments, or a storage error (the in-memory timeline is left
    /// unchanged on failure so a retry stays consistent).
    /// Side effects: Rewrites the meeting's segment rows through `NoteRepository` when non-empty.
    fn persist_aligned_regions(
        &mut self,
        aligned_regions: Vec<AlignedRegion>,
        source: SegmentSource,
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
                .with_unknown_cluster_id(aligned_region.attribution().unknown_cluster_id())
                .with_confidence(SegmentConfidence {
                    overlapping_speech: overlaps_segment(
                        &self.overlap_spans,
                        aligned_region.start_time_ms(),
                        aligned_region.end_time_ms(),
                    ),
                    uncertain_speaker: matches!(
                        aligned_region.attribution(),
                        SpeakerAttribution::PendingReview { .. }
                    ),
                    uncertain_words: aligned_region.uncertain_words().to_vec(),
                })
                .with_source(source),
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
