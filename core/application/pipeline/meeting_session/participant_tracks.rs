//! Credits an online-meeting participant's audio track by the voices it carries (#103).
//!
//! Each chunk of a participant's track is diarized. While one voice speaks, the whole chunk goes
//! to the participant, as before. Once the track shows several voices (a meeting room or a shared
//! laptop), each voice is matched against the gallery like local audio, so a room's people get
//! their own names or join unknown-speaker clusters for review, and nothing more is credited to
//! the room. After enough clean speech of one voice, the voice is offered to the identity mapper
//! for the gallery, unless it sounds like someone else already in it; the mapper keeps it only
//! when the owner opted in. The judgement itself lives in `domain::participant_track_voices`.
//! Audio, text and embeddings are never logged.

use std::time::Instant;

use crate::{
    application::pipeline::{
        alignment::{ChunkBounds, SpeakerAttribution, SpeakerRegion},
        meeting_session::{MeetingSession, VadAudioChunk},
    },
    domain::{
        models::{AudioRecording, ExternalMediaChunk, SpeakerIdentity, VoiceMatchResult},
        participant_track_voices::{TrackVerdict, VoiceTurn},
    },
    ports::{
        ai::{DiarizationEngine, TranscriptionEngine},
        external_bridge::IdentityMapper,
        storage::NoteRepository,
        voice::VoiceIdentityMatcher,
    },
};

impl<TranscriptionPort, DiarizationPort, MatcherPort, RepositoryPort>
    MeetingSession<TranscriptionPort, DiarizationPort, MatcherPort, RepositoryPort>
where
    TranscriptionPort: TranscriptionEngine,
    DiarizationPort: DiarizationEngine,
    MatcherPort: VoiceIdentityMatcher,
    RepositoryPort: NoteRepository,
{
    /// Decides who spoke in one chunk of a participant's track.
    ///
    /// Inputs: The mapper (for offering a voice), the chunk and its recording, its meeting-time
    /// bounds, the mapped identity (`None` when mapping failed) and the participant's attribution.
    /// Outputs: Chunk-relative speaker regions: one region for the participant while the track
    /// carries one voice, or when diarization is missing or fails; one region per diarized turn,
    /// matched against the gallery, once the track carries several voices.
    /// Side effects: Runs diarization, records overlap and telemetry, updates the track's state,
    /// may query the gallery, cluster unknown voices, save review clips and offer a voice.
    pub(super) async fn participant_track_regions<MapperPort>(
        &mut self,
        identity_mapper: &MapperPort,
        media_chunk: &ExternalMediaChunk,
        audio_recording: &AudioRecording,
        chunk_bounds: ChunkBounds,
        mapped_identity: Option<&SpeakerIdentity>,
        participant: SpeakerAttribution,
    ) -> Vec<SpeakerRegion>
    where
        MapperPort: IdentityMapper,
    {
        let duration_ms = chunk_bounds
            .end_offset_ms()
            .saturating_sub(chunk_bounds.start_offset_ms());
        let whole_chunk = |attribution| vec![SpeakerRegion::new(0, duration_ms, attribution)];
        let Some(diarization_engine) = &self.diarization_engine else {
            return whole_chunk(participant);
        };
        let diarization_started_at = Instant::now();
        let diarized = match diarization_engine
            .diarize_with_overlap(media_chunk.audio_bytes())
            .await
        {
            Ok(diarized) => diarized,
            Err(error) => {
                tracing::warn!(
                    meeting_id = %self.meeting_id.as_str(),
                    external_id = %media_chunk.external_participant().external_id(),
                    error = %error,
                    "Participant track not diarized; chunk credited to the participant"
                );
                return whole_chunk(participant);
            }
        };
        if let Some(task_metrics) = &self.task_metrics {
            task_metrics.record_diarization(diarization_started_at.elapsed());
        }

        let turns: Vec<VoiceTurn<'_>> = diarized
            .segments
            .iter()
            .map(|segment| VoiceTurn {
                start_ms: segment.start_time_ms(),
                end_ms: segment.end_time_ms(),
                embedding: segment.embedding(),
            })
            .collect();
        let chunk_voices = self
            .track_voice_policy
            .classify_chunk(&turns, &diarized.overlap);
        let policy = self.track_voice_policy;
        let track = self
            .participant_tracks
            .entry(media_chunk.external_participant().external_id().to_owned())
            .or_default();
        let verdict = track.observe(&policy, chunk_voices);
        let offered_voice = track.enrolment_candidate(&policy);
        if offered_voice.is_some() {
            track.settle_enrolment();
        }
        self.record_overlap(chunk_bounds, diarized.overlap);
        if let (Some(identity), Some(voice)) = (mapped_identity, offered_voice) {
            self.offer_participant_voice(identity_mapper, identity, voice);
        }

        match verdict {
            TrackVerdict::Participant => whole_chunk(participant),
            TrackVerdict::SeparateVoices => {
                tracing::info!(
                    meeting_id = %self.meeting_id.as_str(),
                    external_id = %media_chunk.external_participant().external_id(),
                    voice_count = diarized.segments.len(),
                    "Participant track carries several voices; matching each against the gallery"
                );
                let Ok(vad_chunk) = VadAudioChunk::new(
                    audio_recording.clone(),
                    chunk_bounds.start_offset_ms(),
                    chunk_bounds.end_offset_ms(),
                    media_chunk.audio_bytes().to_vec(),
                ) else {
                    return whole_chunk(SpeakerAttribution::Unknown {
                        clip_id: None,
                        cluster_id: None,
                    });
                };
                diarized
                    .segments
                    .iter()
                    .map(|segment| {
                        SpeakerRegion::new(
                            segment.start_time_ms(),
                            segment.end_time_ms(),
                            self.attribute_speaker_segment(&vad_chunk, segment),
                        )
                    })
                    .collect()
            }
        }
    }

    /// Offers a participant's voice to the gallery unless it sounds like someone else.
    ///
    /// Inputs: The mapper, the participant's identity and the voice's mean embedding.
    /// Outputs: None; every outcome is logged by IDs only.
    /// Side effects: Queries the gallery; the mapper may store the voice (owner's opt-in).
    fn offer_participant_voice<MapperPort>(
        &self,
        identity_mapper: &MapperPort,
        identity: &SpeakerIdentity,
        voice: Vec<f32>,
    ) where
        MapperPort: IdentityMapper,
    {
        let resembles = match self.voice_identity_matcher.match_embedding(&voice) {
            Ok(
                VoiceMatchResult::StrongMatch(matched_id)
                | VoiceMatchResult::WeakMatch(matched_id, _),
            ) => Some(matched_id),
            Ok(VoiceMatchResult::NoMatch) => None,
            Err(error) => {
                tracing::warn!(
                    identity_id = %identity.identity_id(),
                    error = %error,
                    "Participant voice not offered: the gallery could not be searched"
                );
                return;
            }
        };
        if let Some(matched_id) = resembles
            && matched_id != identity.identity_id()
        {
            tracing::info!(
                identity_id = %identity.identity_id(),
                resembles_identity_id = %matched_id,
                "Participant voice not offered: it sounds like someone else in the gallery"
            );
            return;
        }
        match identity_mapper.keep_participant_voice(identity, voice) {
            Ok(kept) => tracing::info!(
                identity_id = %identity.identity_id(),
                kept,
                "Participant voice offered to the gallery"
            ),
            Err(error) => tracing::warn!(
                identity_id = %identity.identity_id(),
                error = %error,
                "Participant voice could not be kept"
            ),
        }
    }
}
