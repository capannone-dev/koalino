//! Specifies the windowed diarization engine with fake models (no weights are loaded): speaker
//! turns come out in chunk time, each local speaker is linked across overlapping windows even
//! when a window numbers it differently, short blips are dropped, short pauses are bridged, and
//! failures are reported instead of guessed around.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::{
    powerset::POWERSET_CLASSES,
    windowed_engine::{
        DiarizationTuning, SegmentationModel, SpeakerEmbeddingModel, WINDOW_SAMPLES,
        WindowedDiarizationEngine,
    },
};
use crate::{
    adapters::audio::chunk_audio_store::encode_wav_pcm16,
    domain::errors::DiarizationError,
    ports::ai::{DiarizationEngine, SpeakerSegment},
};

/// Samples per second.
const RATE: usize = 16_000;
/// Frames the fake segmentation reports per window: 100 ms each.
const FAKE_FRAMES: usize = 100;
/// Amplitude of speaker A's fake voice.
const VOICE_A: f32 = 0.1;
/// Amplitude of speaker B's fake voice.
const VOICE_B: f32 = 0.2;

/// Tells speakers apart by amplitude. Every second call swaps the local labels of A and B, as a
/// real model may number the same voices differently in another window.
#[derive(Default)]
struct AmplitudeSegmentation {
    calls: AtomicUsize,
    fail: bool,
}

impl SegmentationModel for AmplitudeSegmentation {
    fn segment(&self, window: &[f32]) -> Result<Vec<[f32; POWERSET_CLASSES]>, DiarizationError> {
        if self.fail {
            return Err(DiarizationError::Unavailable);
        }
        assert_eq!(window.len(), WINDOW_SAMPLES);
        let swapped = self.calls.fetch_add(1, Ordering::SeqCst) % 2 == 1;
        let frame_samples = WINDOW_SAMPLES / FAKE_FRAMES;
        Ok(window
            .chunks(frame_samples)
            .map(|frame| {
                let mut scores = [0.0; POWERSET_CLASSES];
                let class = match voice_of(frame) {
                    None => 0,
                    Some(VoiceLabel::A) => {
                        if swapped {
                            2
                        } else {
                            1
                        }
                    }
                    Some(VoiceLabel::B) => {
                        if swapped {
                            1
                        } else {
                            2
                        }
                    }
                };
                scores[class] = 1.0;
                scores
            })
            .collect())
    }
}

/// The fake voices.
enum VoiceLabel {
    A,
    B,
}

/// Which fake voice dominates `samples`, by mean amplitude.
fn voice_of(samples: &[f32]) -> Option<VoiceLabel> {
    let mean = samples.iter().map(|sample| sample.abs()).sum::<f32>() / samples.len() as f32;
    if mean < VOICE_A / 2.0 {
        None
    } else if mean < (VOICE_A + VOICE_B) / 2.0 {
        Some(VoiceLabel::A)
    } else {
        Some(VoiceLabel::B)
    }
}

/// Embeds A as [1, 0] and B as [0, 1], and counts calls.
#[derive(Default)]
struct AmplitudeEmbedding {
    calls: AtomicUsize,
}

impl SpeakerEmbeddingModel for AmplitudeEmbedding {
    fn embed(&self, samples: &[f32]) -> Result<Vec<f32>, DiarizationError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(match voice_of(samples) {
            Some(VoiceLabel::A) => vec![1.0, 0.0],
            Some(VoiceLabel::B) => vec![0.0, 1.0],
            None => vec![0.5, 0.5],
        })
    }
}

/// Builds audio from (voice amplitude, seconds) parts.
fn audio(parts: &[(f32, f32)]) -> Vec<f32> {
    parts
        .iter()
        .flat_map(|(amplitude, seconds)| {
            std::iter::repeat_n(*amplitude, (seconds * RATE as f32) as usize)
        })
        .collect()
}

/// Diarizes `samples` with the fake models and default tuning.
fn diarize(samples: &[f32]) -> Vec<SpeakerSegment> {
    WindowedDiarizationEngine::new(
        AmplitudeSegmentation::default(),
        AmplitudeEmbedding::default(),
        DiarizationTuning::default(),
    )
    .diarize_samples(samples)
    .unwrap_or_else(|error| panic!("fake models must diarize: {error}"))
}

/// Summarizes segments as (start, end, embedding).
fn turns(segments: &[SpeakerSegment]) -> Vec<(u64, u64, Vec<f32>)> {
    segments
        .iter()
        .map(|segment| {
            (
                segment.start_time_ms(),
                segment.end_time_ms(),
                segment.embedding().to_vec(),
            )
        })
        .collect()
}

/// A chunk shorter than one window: two speakers, one after the other.
#[test]
fn two_speakers_in_a_short_chunk_become_two_turns() {
    let segments = diarize(&audio(&[(VOICE_A, 3.0), (VOICE_B, 3.0)]));

    assert_eq!(
        turns(&segments),
        vec![(0, 3_000, vec![1.0, 0.0]), (3_000, 6_000, vec![0.0, 1.0])]
    );
}

/// A 15 s chunk spans two overlapping windows that number the speakers differently; each speaker
/// still keeps one identity and its turn is not cut at the window edge.
#[test]
fn speakers_keep_one_identity_across_windows() {
    let segments = diarize(&audio(&[(VOICE_A, 7.0), (VOICE_B, 8.0)]));

    assert_eq!(
        turns(&segments),
        vec![(0, 7_000, vec![1.0, 0.0]), (7_000, 15_000, vec![0.0, 1.0])]
    );
}

/// The same speaker before and after a short pause is one turn; after a long pause, two; a blip
/// shorter than the minimum turn is dropped.
#[test]
fn short_pauses_are_bridged_and_blips_dropped() {
    let segments = diarize(&audio(&[
        (VOICE_A, 2.0),
        (0.0, 0.3),
        (VOICE_A, 2.0),
        (0.0, 2.0),
        (VOICE_B, 0.1),
        (0.0, 1.0),
        (VOICE_A, 1.5),
    ]));

    assert_eq!(
        turns(&segments),
        vec![(0, 4_300, vec![1.0, 0.0]), (7_400, 8_900, vec![1.0, 0.0])]
    );
}

/// Silence yields no turns and no embedding work.
#[test]
fn silence_yields_nothing() {
    let embedding = AmplitudeEmbedding::default();
    let engine = WindowedDiarizationEngine::new(
        AmplitudeSegmentation::default(),
        embedding,
        DiarizationTuning::default(),
    );

    let segments = engine
        .diarize_samples(&audio(&[(0.0, 4.0)]))
        .unwrap_or_else(|error| panic!("silence must diarize: {error}"));

    assert!(segments.is_empty());
    assert_eq!(engine.embedding_model().calls.load(Ordering::SeqCst), 0);
}

/// A model failure is reported, not turned into an empty result.
#[test]
fn a_model_failure_is_reported() {
    let engine = WindowedDiarizationEngine::new(
        AmplitudeSegmentation {
            fail: true,
            ..AmplitudeSegmentation::default()
        },
        AmplitudeEmbedding::default(),
        DiarizationTuning::default(),
    );

    assert!(matches!(
        engine.diarize_samples(&audio(&[(VOICE_A, 2.0)])),
        Err(DiarizationError::Unavailable)
    ));
}

/// Through the port, the engine takes the recorder's WAV bytes and refuses anything else.
#[tokio::test]
async fn the_port_takes_recorder_wav() {
    let engine = WindowedDiarizationEngine::new(
        AmplitudeSegmentation::default(),
        AmplitudeEmbedding::default(),
        DiarizationTuning::default(),
    );
    let pcm: Vec<i16> = audio(&[(VOICE_A, 2.0)])
        .iter()
        .map(|sample| (sample * 32_768.0) as i16)
        .collect();

    let segments = engine
        .diarize(&encode_wav_pcm16(&pcm, 16_000))
        .await
        .unwrap_or_else(|error| panic!("WAV must diarize: {error}"));

    assert_eq!(turns(&segments), vec![(0, 2_000, vec![1.0, 0.0])]);
    assert!(matches!(
        engine.diarize(b"not audio").await,
        Err(DiarizationError::InvalidAudio { .. })
    ));
}
