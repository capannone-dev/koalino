//! Specifies that a voice's review clip holds only that voice (#116).
//!
//! The owner names an unknown voice by listening to its clip. A clip that also holds another
//! person makes naming impossible, so a clip keeps only the voice's own span of the chunk, and the
//! final pass cuts it down again to the run of the speaker it settled on.

use std::sync::Mutex;

use super::{MockTranscriptionEngine, sqlite_repository_with_meeting};
use crate::{
    adapters::storage::in_memory_voice_vector_db::InMemoryVoiceVectorDB,
    application::{
        pipeline::meeting_session::{MeetingSession, VadAudioChunk},
        voice_identity_manager::VoiceIdentityManager,
    },
    domain::{
        errors::DiarizationError,
        models::{AudioRecording, DeviceId, MeetingId, RecordingId},
        pcm_wav::{WAV_HEADER_LENGTH, wav_from_pcm16_le},
    },
    ports::{
        ai::{DiarizationEngine, SpeakerSegment},
        storage::NoteRepository,
    },
};

const MEETING: &str = "meeting-review-clips";

/// Builds a 16 kHz chunk whose every sample holds its own time in tenths of a second from the
/// chunk start, so a clip shows which part of the chunk it came from.
fn timed_wav_chunk(sequence: u32, start_offset_ms: u64, duration_ms: u64) -> VadAudioChunk {
    let pcm: Vec<u8> = (0..duration_ms * 16)
        .flat_map(|sample| ((sample / 1_600) as i16).to_le_bytes())
        .collect();
    VadAudioChunk::new(
        AudioRecording::new(
            RecordingId::new(format!("recording-{sequence}")),
            MeetingId::new(MEETING),
            DeviceId::new("device-test"),
        ),
        start_offset_ms,
        start_offset_ms + duration_ms,
        wav_from_pcm16_le(&pcm, 16_000),
    )
    .unwrap_or_else(|error| panic!("chunk bounds should be valid: {error}"))
}

/// Returns the tenths-of-a-second stamps a clip's samples carry.
fn clip_stamps(clip_audio: &[u8]) -> Vec<i16> {
    clip_audio[WAV_HEADER_LENGTH..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|sample| i16::from_le_bytes(*sample))
        .collect()
}

/// Reports two different voices in one chunk: 0–4 s and 4–10 s.
struct TwoVoicesDiarizer;

#[async_trait::async_trait]
impl DiarizationEngine for TwoVoicesDiarizer {
    async fn diarize(&self, _audio_bytes: &[u8]) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        Ok(vec![
            SpeakerSegment::new(0, 4_000, vec![1.0, 0.0, 0.0]),
            SpeakerSegment::new(4_000, 10_000, vec![0.0, 1.0, 0.0]),
        ])
    }

    /// Reports no windows: this test ends before the final pass.
    async fn voice_windows(
        &self,
        _audio_bytes: &[u8],
    ) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        Ok(Vec::new())
    }
}

#[tokio::test]
async fn a_new_voices_clip_holds_only_its_own_span_of_the_chunk() {
    let meeting_id = MeetingId::new(MEETING);
    let mut session = MeetingSession::start(
        meeting_id.clone(),
        MockTranscriptionEngine::returning(&["first second"]),
        Some(TwoVoicesDiarizer),
        VoiceIdentityManager::new(InMemoryVoiceVectorDB::new()),
        sqlite_repository_with_meeting(&meeting_id),
    )
    .unwrap_or_else(|error| panic!("{error}"));

    session
        .process_vad_chunk(&timed_wav_chunk(0, 0, 10_000))
        .await
        .unwrap_or_else(|error| panic!("{error}"));

    let clips = session
        .note_repository()
        .get_unassigned_clips(&meeting_id)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(clips.len(), 2, "each voice has its own clip");
    let mut spans: Vec<(i16, i16, u64, u64)> = clips
        .iter()
        .map(|clip| {
            let stamps = clip_stamps(clip.audio_bytes());
            (
                stamps[0],
                stamps[stamps.len() - 1],
                clip.start_time_ms(),
                clip.end_time_ms(),
            )
        })
        .collect();
    spans.sort_unstable();
    assert_eq!(
        spans,
        vec![(0, 39, 0, 4_000), (40, 99, 0, 6_000)],
        "the first voice's clip is 0–4 s and the second's 4–10 s, each timed from its own start"
    );
}

/// Merges everyone into one live voice, and scripts the final pass's voice windows per chunk.
struct MergedLiveVoiceDiarizer {
    /// Per chunk, in order: the voice of each one-second step.
    chunk_windows: Mutex<Vec<Vec<Vec<f32>>>>,
}

#[async_trait::async_trait]
impl DiarizationEngine for MergedLiveVoiceDiarizer {
    /// Reports one live voice over the whole chunk, as a live pass that merged two people would.
    async fn diarize(&self, audio_bytes: &[u8]) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        let duration_ms = ((audio_bytes.len() - WAV_HEADER_LENGTH) / 32) as u64;
        Ok(vec![SpeakerSegment::new(
            0,
            duration_ms,
            vec![0.0, 0.0, 1.0],
        )])
    }

    /// Reports a two-second window every second, with the chunk's scripted voices.
    async fn voice_windows(
        &self,
        _audio_bytes: &[u8],
    ) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        let voices = self
            .chunk_windows
            .lock()
            .map_err(|_| DiarizationError::Unavailable)?
            .remove(0);
        Ok(voices
            .into_iter()
            .enumerate()
            .map(|(second, voice)| {
                let start_ms = second as u64 * 1_000;
                SpeakerSegment::new(start_ms, start_ms + 2_000, voice)
            })
            .collect())
    }
}

#[tokio::test]
async fn after_the_meeting_a_clip_keeps_only_the_speaker_its_voice_was_settled_on() {
    let meeting_id = MeetingId::new(MEETING);
    let first_person = vec![1.0, 0.0, 0.0];
    let second_person = vec![0.0, 1.0, 0.0];
    // Chunk 0 (20 s): the first person for 9 s, then the second; chunk 1 (10 s): the second.
    let mut first_chunk = vec![first_person.clone(); 9];
    first_chunk.extend(vec![second_person.clone(); 11]);
    let mut session = MeetingSession::start(
        meeting_id.clone(),
        MockTranscriptionEngine::returning(&["one two three four", "five six seven"]),
        Some(MergedLiveVoiceDiarizer {
            chunk_windows: Mutex::new(vec![first_chunk, vec![second_person; 10]]),
        }),
        VoiceIdentityManager::new(InMemoryVoiceVectorDB::new()),
        sqlite_repository_with_meeting(&meeting_id),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    session
        .process_vad_chunk(&timed_wav_chunk(0, 0, 20_000))
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    session
        .process_vad_chunk(&timed_wav_chunk(1, 20_000, 10_000))
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    let live_clips = session
        .note_repository()
        .get_unassigned_clips(&meeting_id)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        live_clips.len(),
        2,
        "the live pass heard one voice and kept a clip of each chunk"
    );

    session
        .finish_meeting()
        .unwrap_or_else(|error| panic!("{error}"));

    let clips = session
        .note_repository()
        .get_unassigned_clips(&meeting_id)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(clips.len(), 2, "one clip for each of the two speakers");
    // The first chunk's clip is cut to one person: the switch is at 9–10 s, where windows of
    // either person cover that second. The second chunk holds only the second person.
    let from_first_chunk = clips
        .iter()
        .find(|clip| clip_stamps(clip.audio_bytes()).len() < 20_000 * 16)
        .unwrap_or_else(|| panic!("the first chunk's clip should be cut"));
    let stamps = clip_stamps(from_first_chunk.audio_bytes());
    assert!(!stamps.is_empty());
    let (first, last) = (stamps[0], stamps[stamps.len() - 1]);
    let only_first_person = last < 95;
    let only_second_person = first >= 95;
    assert!(
        only_first_person || only_second_person,
        "the clip holds one person, not both (it runs from {first} to {last} tenths)"
    );
    for clip in &clips {
        assert_eq!(clip.start_time_ms(), 0);
        assert_eq!(
            clip.end_time_ms(),
            clip_stamps(clip.audio_bytes()).len() as u64 / 16,
            "the clip's times describe its own audio"
        );
    }
}

#[tokio::test]
async fn after_the_meeting_every_unnamed_speaker_gets_a_clip_of_their_own() {
    // The live pass merged two people into one voice. Its later chunks are kept as clips too, so
    // after the meeting each speaker gets the clip that holds most of them alone (#134), and the
    // gallery can play every voice.
    let meeting_id = MeetingId::new(MEETING);
    let first_person = vec![1.0, 0.0, 0.0];
    let second_person = vec![0.0, 1.0, 0.0];
    let mut first_chunk = vec![first_person.clone(); 9];
    first_chunk.extend(vec![second_person.clone(); 11]);
    let mut session = MeetingSession::start(
        meeting_id.clone(),
        MockTranscriptionEngine::returning(&["one two three four", "five six seven"]),
        Some(MergedLiveVoiceDiarizer {
            chunk_windows: Mutex::new(vec![first_chunk, vec![second_person; 10]]),
        }),
        VoiceIdentityManager::new(InMemoryVoiceVectorDB::new()),
        sqlite_repository_with_meeting(&meeting_id),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    session
        .process_vad_chunk(&timed_wav_chunk(0, 0, 20_000))
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    session
        .process_vad_chunk(&timed_wav_chunk(1, 20_000, 10_000))
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    session
        .finish_meeting()
        .unwrap_or_else(|error| panic!("{error}"));

    let clusters = crate::application::unknown_speaker_clusterer::UnknownSpeakerClusterer::new(
        session.note_repository(),
        crate::application::unknown_speaker_clusterer::ClusteringConfig::default(),
    )
    .get_clusters_for_meeting(&meeting_id)
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(clusters.len(), 2, "the final pass hears two people");
    let clips = session
        .note_repository()
        .get_unassigned_clips(&meeting_id)
        .unwrap_or_else(|error| panic!("{error}"));
    for cluster in &clusters {
        let own: Vec<_> = clips
            .iter()
            .filter(|clip| clip.cluster_id() == Some(cluster.cluster_id()))
            .collect();
        assert_eq!(own.len(), 1, "each speaker has exactly one clip: {clips:?}");
    }
}
