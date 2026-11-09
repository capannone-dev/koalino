//! Specifies how one online-meeting participant's audio track is judged (#103): one voice keeps the
//! participant's name and may later be kept in the voice gallery, while a track that carries
//! several voices (a meeting room, a shared laptop) has its voices told apart and never kept.

use super::participant_track_voices::{
    ChunkVoices, ParticipantTrackVoices, TrackVerdict, TrackVoicePolicy, VoiceTurn,
};
use crate::domain::transcript_confidence::OverlapSpan;

/// A voice pointing along the first axis.
const VOICE_A: [f32; 3] = [1.0, 0.0, 0.0];
/// A voice close to `VOICE_A` (cosine ~0.99).
const VOICE_A_AGAIN: [f32; 3] = [1.0, 0.1, 0.0];
/// A voice unlike `VOICE_A` (cosine 0).
const VOICE_B: [f32; 3] = [0.0, 1.0, 0.0];

/// A turn of `embedding` from `start_ms` to `end_ms`.
fn turn(start_ms: u64, end_ms: u64, embedding: &[f32]) -> VoiceTurn<'_> {
    VoiceTurn {
        start_ms,
        end_ms,
        embedding,
    }
}

/// One clean single-voice chunk of `speech_ms` with `embedding`.
fn one_voice(speech_ms: u64, embedding: &[f32]) -> ChunkVoices {
    ChunkVoices::OneVoice {
        speech_ms,
        clean_ms: speech_ms,
        embedding: embedding.to_vec(),
    }
}

#[test]
fn a_chunk_without_turns_is_silent() {
    let policy = TrackVoicePolicy::default();
    assert_eq!(policy.classify_chunk(&[], &[]), ChunkVoices::Silent);
}

#[test]
fn turns_of_one_voice_make_one_voice_with_its_mean() {
    let policy = TrackVoicePolicy::default();
    let voices = policy.classify_chunk(
        &[turn(0, 4_000, &VOICE_A), turn(5_000, 9_000, &VOICE_A_AGAIN)],
        &[],
    );
    let ChunkVoices::OneVoice {
        speech_ms,
        clean_ms,
        embedding,
    } = voices
    else {
        panic!("one voice expected, got {voices:?}");
    };
    assert_eq!((speech_ms, clean_ms), (8_000, 8_000));
    assert!((embedding[0] - 1.0).abs() < 1e-6);
    assert!((embedding[1] - 0.05).abs() < 1e-6);
}

#[test]
fn two_voices_with_real_speech_each_are_several_voices() {
    let policy = TrackVoicePolicy::default();
    let voices = policy.classify_chunk(
        &[turn(0, 6_000, &VOICE_A), turn(6_000, 10_000, &VOICE_B)],
        &[],
    );
    assert_eq!(voices, ChunkVoices::SeveralVoices);
}

#[test]
fn a_brief_second_voice_leaves_one_voice() {
    let policy = TrackVoicePolicy::default();
    // 1.5 s of someone else: under the 2 s floor, even though the main voice has only 85%.
    let voices = policy.classify_chunk(
        &[turn(0, 8_500, &VOICE_A), turn(8_500, 10_000, &VOICE_B)],
        &[],
    );
    assert!(matches!(
        voices,
        ChunkVoices::OneVoice {
            speech_ms: 8_500,
            ..
        }
    ));
}

#[test]
fn a_dominant_voice_over_ninety_percent_is_one_voice() {
    let policy = TrackVoicePolicy::default();
    // 2.5 s of someone else, but the main voice holds 92%.
    let voices = policy.classify_chunk(
        &[turn(0, 28_000, &VOICE_A), turn(28_000, 30_500, &VOICE_B)],
        &[],
    );
    assert!(matches!(
        voices,
        ChunkVoices::OneVoice {
            speech_ms: 28_000,
            ..
        }
    ));
}

#[test]
fn overlap_is_not_clean_speech() {
    let policy = TrackVoicePolicy::default();
    let voices = policy.classify_chunk(
        &[turn(0, 10_000, &VOICE_A)],
        &[
            OverlapSpan::new(2_000, 3_000),
            OverlapSpan::new(9_500, 12_000),
        ],
    );
    assert!(matches!(
        voices,
        ChunkVoices::OneVoice {
            speech_ms: 10_000,
            clean_ms: 8_500,
            ..
        }
    ));
}

#[test]
fn a_one_voice_track_keeps_the_participant_and_offers_its_voice_after_thirty_seconds() {
    let policy = TrackVoicePolicy::default();
    let mut track = ParticipantTrackVoices::default();
    for _ in 0..2 {
        assert_eq!(
            track.observe(&policy, one_voice(10_000, &VOICE_A)),
            TrackVerdict::Participant
        );
        assert_eq!(track.enrolment_candidate(&policy), None);
    }
    assert_eq!(
        track.observe(&policy, one_voice(10_000, &VOICE_A_AGAIN)),
        TrackVerdict::Participant
    );

    let candidate = track
        .enrolment_candidate(&policy)
        .unwrap_or_else(|| panic!("30 s of one voice should be offered"));
    assert!((candidate[0] - 1.0).abs() < 1e-6);
    track.settle_enrolment();
    assert_eq!(track.enrolment_candidate(&policy), None, "offered once");
}

#[test]
fn several_voices_in_a_chunk_separate_the_track_for_good() {
    let policy = TrackVoicePolicy::default();
    let mut track = ParticipantTrackVoices::default();
    assert_eq!(
        track.observe(&policy, one_voice(10_000, &VOICE_A)),
        TrackVerdict::Participant
    );
    assert_eq!(
        track.observe(&policy, ChunkVoices::SeveralVoices),
        TrackVerdict::SeparateVoices
    );
    assert_eq!(
        track.observe(&policy, one_voice(30_000, &VOICE_A)),
        TrackVerdict::SeparateVoices
    );
    assert_eq!(
        track.observe(&policy, ChunkVoices::Silent),
        TrackVerdict::SeparateVoices
    );
    assert!(track.has_several_voices());
    assert_eq!(track.enrolment_candidate(&policy), None, "never kept");
}

#[test]
fn a_different_voice_in_a_later_chunk_separates_the_track() {
    let policy = TrackVoicePolicy::default();
    let mut track = ParticipantTrackVoices::default();
    assert_eq!(
        track.observe(&policy, one_voice(20_000, &VOICE_A)),
        TrackVerdict::Participant
    );
    // People in a room taking turns, one per chunk.
    assert_eq!(
        track.observe(&policy, one_voice(20_000, &VOICE_B)),
        TrackVerdict::SeparateVoices
    );
    assert_eq!(track.enrolment_candidate(&policy), None);
}

#[test]
fn a_different_voice_before_the_track_is_known_does_not_separate_it() {
    let policy = TrackVoicePolicy::default();
    let mut track = ParticipantTrackVoices::default();
    // 3 s is too little to know the participant's voice, so nothing is compared yet.
    track.observe(&policy, one_voice(3_000, &VOICE_A));
    assert_eq!(
        track.observe(&policy, one_voice(3_000, &VOICE_B)),
        TrackVerdict::Participant
    );
}

#[test]
fn a_silent_chunk_changes_nothing() {
    let policy = TrackVoicePolicy::default();
    let mut track = ParticipantTrackVoices::default();
    assert_eq!(
        track.observe(&policy, ChunkVoices::Silent),
        TrackVerdict::Participant
    );
    assert!(!track.has_several_voices());
}

#[test]
fn a_track_known_to_carry_several_voices_separates_them_from_the_start() {
    let policy = TrackVoicePolicy::default();
    let mut track = ParticipantTrackVoices::carrying_several_voices();
    assert!(track.has_several_voices());
    assert_eq!(
        track.observe(&policy, one_voice(30_000, &VOICE_A)),
        TrackVerdict::SeparateVoices
    );
    assert_eq!(track.enrolment_candidate(&policy), None, "never kept");
}
