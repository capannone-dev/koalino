//! Specifies the computer's sound recorded beside the microphone (#108): its chunks become
//! segments marked as the computer's, which the final speaker pass keeps, and once the microphone
//! is heard following the speakers, the microphone's copies of those words go and the meeting
//! remembers that echo was heard.

use std::sync::Arc;

use super::MockTranscriptionEngine;
use crate::{
    adapters::storage::{
        in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
    },
    application::{
        meeting_document::AutomergeMeetingDocumentCodec,
        pipeline::meeting_session::{MeetingSession, VadAudioChunk},
        voice_identity_manager::VoiceIdentityManager,
    },
    domain::{
        errors::DiarizationError,
        models::{AudioRecording, DeviceId, MeetingId, RecordingId, SegmentSource},
        pcm_wav::{WAV_HEADER_LENGTH, wav_from_pcm16_le},
    },
    ports::{
        ai::{DiarizationEngine, SpeakerSegment},
        storage::NoteRepository,
    },
};

const MEETING: &str = "meeting-computer-sound";
const RATE: usize = 16_000;

/// One voice over each whole chunk, and one voice window too, so the final pass runs.
struct OneVoiceDiarizer;

#[async_trait::async_trait]
impl DiarizationEngine for OneVoiceDiarizer {
    async fn diarize(&self, audio_bytes: &[u8]) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        Ok(vec![SpeakerSegment::new(
            0,
            duration_ms(audio_bytes),
            vec![0.0, 0.0, 1.0],
        )])
    }

    async fn voice_windows(
        &self,
        audio_bytes: &[u8],
    ) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        Ok(vec![SpeakerSegment::new(
            0,
            duration_ms(audio_bytes),
            vec![0.0, 0.0, 1.0],
        )])
    }
}

fn duration_ms(audio_bytes: &[u8]) -> u64 {
    ((audio_bytes.len().saturating_sub(WAV_HEADER_LENGTH)) / 32) as u64
}

/// Speech-like audio: noise in syllables with pauses, each syllable at its own loudness.
fn speech(seed: u64, seconds: usize) -> Vec<i16> {
    let mut state = seed;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state >> 33
    };
    let total = seconds * RATE;
    let mut samples = Vec::with_capacity(total);
    while samples.len() < total {
        let on = (120 + next() % 280) as usize * RATE / 1_000;
        let loudness = 2_000.0 + (next() % 6_000) as f32;
        for _ in 0..on {
            samples.push((((next() % 20_001) as f32 / 10_000.0 - 1.0) * loudness) as i16);
        }
        let off = (80 + next() % 420) as usize * RATE / 1_000;
        samples.extend(std::iter::repeat_n(0, off));
    }
    samples.truncate(total);
    samples
}

/// Their voice as the microphone hears it from the speakers: at 0.3, 60 ms late.
fn leak(theirs: &[i16]) -> Vec<i16> {
    let delay = 60 * RATE / 1_000;
    (0..theirs.len())
        .map(|index| {
            index
                .checked_sub(delay)
                .map_or(0, |earlier| (f32::from(theirs[earlier]) * 0.3) as i16)
        })
        .collect()
}

fn chunk(sequence: u32, start_ms: u64, samples: &[i16], track: SegmentSource) -> VadAudioChunk {
    let pcm: Vec<u8> = samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    VadAudioChunk::new(
        AudioRecording::new(
            RecordingId::new(format!("recording-{sequence}")),
            MeetingId::new(MEETING),
            DeviceId::new("device-test"),
        ),
        start_ms,
        start_ms + (samples.len() * 1_000 / RATE) as u64,
        wav_from_pcm16_le(&pcm, RATE as u32),
    )
    .unwrap_or_else(|error| panic!("{error}"))
    .with_track(track)
}

fn repository() -> SqliteNoteRepository {
    let repository = SqliteNoteRepository::open_in_memory(
        crate::application::crypto::voice_encryptor::ephemeral_cipher(),
    )
    .and_then(|repository| repository.with_document_codec(Arc::new(AutomergeMeetingDocumentCodec)))
    .unwrap_or_else(|error| panic!("{error}"));
    repository
        .create_meeting(
            &MeetingId::new(MEETING),
            &automerge::AutoCommit::new().save(),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    repository
}

type Session = MeetingSession<
    MockTranscriptionEngine,
    OneVoiceDiarizer,
    VoiceIdentityManager<InMemoryVoiceVectorDB>,
    SqliteNoteRepository,
>;

fn session(texts: &[&'static str]) -> Session {
    MeetingSession::start(
        MeetingId::new(MEETING),
        MockTranscriptionEngine::returning(texts),
        Some(OneVoiceDiarizer),
        VoiceIdentityManager::new(InMemoryVoiceVectorDB::new()),
        repository(),
    )
    .unwrap_or_else(|error| panic!("{error}"))
}

fn sources(session: &Session) -> Vec<(String, SegmentSource)> {
    session
        .note_repository()
        .load_transcript_segments(&MeetingId::new(MEETING))
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .map(|segment| (segment.text().to_owned(), segment.source()))
        .collect()
}

#[tokio::test]
async fn the_computer_sound_is_marked_and_survives_the_final_pass() {
    let mut session = session(&["my own words here", "their words over there"]);
    session
        .process_vad_chunk(&chunk(0, 0, &speech(1, 3), SegmentSource::Microphone))
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    session
        .process_vad_chunk(&chunk(
            1,
            4_000,
            &speech(2, 3),
            SegmentSource::ComputerSound,
        ))
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    let expected = vec![
        ("my own words here".to_owned(), SegmentSource::Microphone),
        (
            "their words over there".to_owned(),
            SegmentSource::ComputerSound,
        ),
    ];
    assert_eq!(sources(&session), expected);

    assert!(
        session
            .finish_meeting()
            .unwrap_or_else(|error| panic!("{error}"))
            .is_some(),
        "the final pass ran"
    );
    assert_eq!(sources(&session), expected);
    assert!(
        !session
            .note_repository()
            .load_echo_heard(&MeetingId::new(MEETING))
            .unwrap_or_else(|error| panic!("{error}")),
        "no echo was heard"
    );
}

#[tokio::test]
async fn once_echo_is_heard_the_microphone_s_copy_goes() {
    let words = "we should move the launch to friday next week";
    let mut session = session(&[words, words]);
    let theirs = speech(3, 30);
    session
        .process_vad_chunk(&chunk(0, 0, &theirs, SegmentSource::ComputerSound))
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    session
        .process_vad_chunk(&chunk(1, 0, &leak(&theirs), SegmentSource::Microphone))
        .await
        .unwrap_or_else(|error| panic!("{error}"));

    let only_theirs = vec![(words.to_owned(), SegmentSource::ComputerSound)];
    assert_eq!(sources(&session), only_theirs, "the live copy went");
    assert!(
        session
            .note_repository()
            .load_echo_heard(&MeetingId::new(MEETING))
            .unwrap_or_else(|error| panic!("{error}"))
    );

    session
        .finish_meeting()
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        sources(&session),
        only_theirs,
        "the final pass does not bring it back"
    );
}
