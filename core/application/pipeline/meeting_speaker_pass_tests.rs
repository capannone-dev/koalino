//! Specifies the whole-meeting grouping with made-up voices whose answers are known.

use super::meeting_speaker_pass::{
    SpeakerPassConfig, SpeakerVoice, VoiceWindow, group_voice_windows, speaker_at, speaker_voices,
    unit_vector,
};

/// Makes `count` slightly different windows around a voice pointing along axis `axis`.
fn voice(axis: usize, count: usize) -> Vec<Vec<f32>> {
    (0..count)
        .map(|index| {
            let mut vector = vec![0.0; 8];
            vector[axis] = 1.0;
            vector[(axis + 1) % 8] = 0.1 + (index % 5) as f32 * 0.02;
            vector
        })
        .collect()
}

#[test]
fn no_windows_give_no_speakers() {
    assert!(group_voice_windows(&[], &SpeakerPassConfig::default()).is_empty());
}

#[test]
fn two_voices_alternating_give_two_speakers() {
    let (first, second) = (voice(0, 20), voice(4, 20));
    let windows: Vec<Vec<f32>> = first
        .into_iter()
        .zip(second)
        .flat_map(|(left, right)| [left, right])
        .collect();
    let labels = group_voice_windows(&windows, &SpeakerPassConfig::default());
    for (position, label) in labels.iter().enumerate() {
        assert_eq!(*label, position % 2);
    }
}

#[test]
fn a_brief_group_joins_its_nearest_neighbour() {
    let mut windows = voice(0, 20);
    windows.extend(voice(3, 20));
    // Far enough from every voice to start its own group, and nearest the voice on axis 0.
    let mut stray = vec![0.0; 8];
    stray[0] = 0.5;
    stray[6] = 1.0;
    windows.extend(vec![stray; 3]);
    let labels = group_voice_windows(&windows, &SpeakerPassConfig::default());
    assert_eq!(labels[40..], [0, 0, 0]);
    assert_eq!(labels.iter().max(), Some(&1));
}

#[test]
fn one_person_stays_one_speaker_even_when_brief() {
    let labels = group_voice_windows(&voice(2, 3), &SpeakerPassConfig::default());
    assert_eq!(labels, [0, 0, 0]);
}

#[test]
fn a_long_meeting_is_sampled_and_every_window_gets_a_speaker() {
    let mut windows = Vec::new();
    for _ in 0..40 {
        windows.extend(voice(0, 5));
        windows.extend(voice(4, 5));
    }
    let config = SpeakerPassConfig {
        max_clustered_windows: 100,
        ..SpeakerPassConfig::default()
    };
    let labels = group_voice_windows(&windows, &config);
    assert_eq!(labels.len(), windows.len());
    assert_eq!(labels.iter().max(), Some(&1));
    assert!((0..5).all(|position| labels[position] == labels[0]));
    assert!((5..10).all(|position| labels[position] != labels[0]));
}

#[test]
fn scale_does_not_change_the_groups() {
    let windows: Vec<Vec<f32>> = voice(0, 10)
        .into_iter()
        .chain(voice(4, 10))
        .map(|vector| vector.iter().map(|value| value * 7.0).collect())
        .collect();
    assert_eq!(
        group_voice_windows(&windows, &SpeakerPassConfig::default())
            .iter()
            .max(),
        Some(&1)
    );
    assert!((unit_vector(&[3.0, 4.0])[0] - 0.6).abs() < 1e-6);
}

/// Builds a window around `center_ms` in chunk `chunk_index`.
fn window(chunk_index: usize, center_ms: u64) -> VoiceWindow {
    VoiceWindow {
        chunk_index,
        start_ms: center_ms - 1_000,
        end_ms: center_ms + 1_000,
        embedding: vec![1.0],
    }
}

#[test]
fn a_moment_takes_the_nearest_window_in_its_chunk() {
    let windows = [window(0, 1_000), window(0, 2_000), window(1, 5_000)];
    let labels = [0, 1, 2];
    assert_eq!(speaker_at(&windows, &labels, 0, 1_400), Some(0));
    assert_eq!(speaker_at(&windows, &labels, 0, 1_600), Some(1));
    assert_eq!(speaker_at(&windows, &labels, 0, 4_900), Some(1));
}

#[test]
fn a_chunk_without_windows_uses_the_nearest_in_the_meeting() {
    let windows = [window(0, 1_000), window(2, 9_000)];
    assert_eq!(speaker_at(&windows, &[0, 1], 1, 8_000), Some(1));
    assert_eq!(speaker_at(&[], &[], 0, 8_000), None);
}

#[test]
fn only_a_long_consistent_voice_is_clean() {
    let voice_of = |window_count, consistency| SpeakerVoice {
        centroid: vec![1.0],
        window_count,
        consistency,
    };
    assert!(voice_of(20, 0.6).is_clean());
    assert!(!voice_of(19, 0.9).is_clean());
    assert!(!voice_of(40, 0.55).is_clean());
}

#[test]
fn a_mixed_group_is_less_consistent_than_a_pure_one() {
    let pure = voice(0, 10);
    let mut mixed = voice(0, 5);
    mixed.extend(voice(4, 5));
    let pure_voices = speaker_voices(&pure, &[0; 10]);
    let mixed_voices = speaker_voices(&mixed, &[0; 10]);
    assert_eq!(pure_voices[0].window_count, 10);
    assert!(pure_voices[0].consistency > 0.95);
    assert!(mixed_voices[0].consistency < 0.8);
}
