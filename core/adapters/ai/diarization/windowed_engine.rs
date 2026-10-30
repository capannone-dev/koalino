//! Turns a chunk's audio into speaker turns with a segmentation and an embedding model
//! (Issue #44).
//!
//! The approach follows pyannote's pipeline, sized for the recorder's chunks (at most ~15 s):
//!
//! 1. **Windows:** 10 s windows every 5 s, the last one aligned to the end of the chunk; a chunk
//!    shorter than a window is padded with silence.
//! 2. **Segmentation:** the segmentation model scores each window's frames (`powerset`), giving up
//!    to three local speakers per window.
//! 3. **Embeddings:** each local speaker that talks long enough is embedded from the frames where
//!    it talks alone (or, if those are too short, every frame where it talks).
//! 4. **Linking:** local speakers are linked across windows by cosine similarity of their
//!    embeddings; two speakers of one window are never linked to each other.
//! 5. **Turns:** each instant goes to the speaker that most of the windows covering it hear.
//!    Turns shorter than `min_turn_ms` are dropped and pauses of at most `max_merge_gap_ms`
//!    within one speaker are bridged.
//!
//! Each turn carries its speaker's mean embedding (unit length). Linking speakers across chunks
//! and meetings is left to the voice matcher and the unknown-speaker clusterer. The models sit
//! behind `SegmentationModel` and `SpeakerEmbeddingModel`, so the logic is tested with fakes, and
//! inference runs on a blocking thread, never on the async runtime. Audio and embeddings are
//! never logged.

use std::sync::Arc;

use super::{
    powerset::{LOCAL_SPEAKERS, POWERSET_CLASSES, active_speakers},
    wav_input::{SAMPLE_RATE_HZ, decode_wav_pcm16_mono},
};
use crate::{
    domain::errors::DiarizationError,
    ports::ai::{DiarizationEngine, SpeakerSegment},
};

/// Samples in one segmentation window: 10 s at 16 kHz.
pub const WINDOW_SAMPLES: usize = 10 * SAMPLE_RATE_HZ as usize;
/// Distance between window starts: 5 s.
const WINDOW_STEP_SAMPLES: usize = 5 * SAMPLE_RATE_HZ as usize;
/// Samples per millisecond.
const SAMPLES_PER_MS: f64 = SAMPLE_RATE_HZ as f64 / 1_000.0;

/// Scores the frames of one window.
pub trait SegmentationModel: Send + Sync {
    /// Segments one window.
    ///
    /// Inputs: Exactly `WINDOW_SAMPLES` samples in [-1, 1] (zero-padded past the chunk's end).
    /// Outputs: Per-frame powerset scores, evenly spaced over the window, or an error.
    /// Side effects: Runs inference.
    fn segment(&self, window: &[f32]) -> Result<Vec<[f32; POWERSET_CLASSES]>, DiarizationError>;
}

/// Embeds one speaker's audio.
pub trait SpeakerEmbeddingModel: Send + Sync {
    /// Embeds `samples`.
    ///
    /// Inputs: 16 kHz samples of one speaker. Outputs: The embedding (biometric; never logged), or
    /// an error. Side effects: Runs inference.
    fn embed(&self, samples: &[f32]) -> Result<Vec<f32>, DiarizationError>;
}

/// Thresholds of the turn assembly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiarizationTuning {
    /// Least cosine similarity at which two windows' speakers are the same person.
    pub speaker_link_threshold: f32,
    /// Shortest turn kept, in milliseconds.
    pub min_turn_ms: u64,
    /// Longest pause bridged within one speaker's turn, in milliseconds.
    pub max_merge_gap_ms: u64,
    /// Least speech a local speaker needs to be embedded, in milliseconds.
    pub min_embedding_ms: u64,
}

impl Default for DiarizationTuning {
    fn default() -> Self {
        Self {
            speaker_link_threshold: 0.5,
            min_turn_ms: 250,
            max_merge_gap_ms: 500,
            min_embedding_ms: 400,
        }
    }
}

/// Diarization from a segmentation model and an embedding model.
pub struct WindowedDiarizationEngine<Segmentation, Embedding> {
    segmentation_model: Arc<Segmentation>,
    embedding_model: Arc<Embedding>,
    tuning: DiarizationTuning,
}

impl<Segmentation, Embedding> Clone for WindowedDiarizationEngine<Segmentation, Embedding> {
    fn clone(&self) -> Self {
        Self {
            segmentation_model: Arc::clone(&self.segmentation_model),
            embedding_model: Arc::clone(&self.embedding_model),
            tuning: self.tuning,
        }
    }
}

/// One window's decoded frames.
struct WindowActivity {
    /// First sample of the window in the chunk.
    start_sample: usize,
    /// Samples per frame.
    frame_samples: f64,
    /// Active local speakers per frame, for frames that start inside the chunk.
    frames: Vec<[bool; LOCAL_SPEAKERS]>,
}

impl WindowActivity {
    /// The chunk samples of frame `frame`, clipped to the chunk.
    fn frame_range(&self, frame: usize, chunk_length: usize) -> std::ops::Range<usize> {
        let start = self.start_sample + (frame as f64 * self.frame_samples) as usize;
        let end = self.start_sample + ((frame + 1) as f64 * self.frame_samples) as usize;
        start.min(chunk_length)..end.min(chunk_length)
    }
}

/// A chunk-level speaker: linked local speakers and their summed unit embeddings.
struct ChunkSpeaker {
    embedding_sum: Vec<f32>,
    windows: Vec<usize>,
}

impl ChunkSpeaker {
    /// The unit-length mean embedding.
    fn centroid(&self) -> Vec<f32> {
        unit(&self.embedding_sum).unwrap_or_else(|| self.embedding_sum.clone())
    }
}

/// A contiguous stretch assigned to one chunk speaker, in milliseconds.
#[derive(Clone, Copy)]
struct Turn {
    speaker: usize,
    start_ms: u64,
    end_ms: u64,
}

impl<Segmentation, Embedding> WindowedDiarizationEngine<Segmentation, Embedding>
where
    Segmentation: SegmentationModel,
    Embedding: SpeakerEmbeddingModel,
{
    /// Creates an engine.
    ///
    /// Inputs: The loaded models and the tuning. Outputs: The engine. Side effects: None.
    pub fn new(
        segmentation_model: Segmentation,
        embedding_model: Embedding,
        tuning: DiarizationTuning,
    ) -> Self {
        Self {
            segmentation_model: Arc::new(segmentation_model),
            embedding_model: Arc::new(embedding_model),
            tuning,
        }
    }

    /// Borrows the embedding model (for tests and diagnostics).
    pub fn embedding_model(&self) -> &Embedding {
        &self.embedding_model
    }

    /// Diarizes 16 kHz samples synchronously; callers on an async runtime use the port instead.
    ///
    /// Inputs: The chunk's samples in [-1, 1].
    /// Outputs: Turns in time order with chunk-relative milliseconds, or the first model error.
    /// Side effects: Runs inference (CPU-heavy).
    pub fn diarize_samples(
        &self,
        samples: &[f32],
    ) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        if samples.is_empty() {
            return Ok(Vec::new());
        }
        let windows = window_starts(samples.len())
            .into_iter()
            .map(|start_sample| self.segment_window(samples, start_sample))
            .collect::<Result<Vec<_>, _>>()?;
        let (speakers, local_links) = self.link_speakers(samples, &windows)?;
        if speakers.is_empty() {
            return Ok(Vec::new());
        }
        let turns = self.assemble_turns(samples.len(), &windows, &local_links, speakers.len());
        Ok(turns
            .into_iter()
            .map(|turn| {
                SpeakerSegment::new(
                    turn.start_ms,
                    turn.end_ms,
                    speakers[turn.speaker].centroid(),
                )
            })
            .collect())
    }

    /// Segments the window starting at `start_sample`.
    ///
    /// Outputs: Its decoded frames, or the model's error, or `InvalidAudio` when the model
    /// returns no frames. Side effects: Runs segmentation.
    fn segment_window(
        &self,
        samples: &[f32],
        start_sample: usize,
    ) -> Result<WindowActivity, DiarizationError> {
        let end_sample = (start_sample + WINDOW_SAMPLES).min(samples.len());
        let mut window = samples[start_sample..end_sample].to_vec();
        window.resize(WINDOW_SAMPLES, 0.0);
        let scores = self.segmentation_model.segment(&window)?;
        if scores.is_empty() {
            return Err(DiarizationError::InvalidAudio {
                reason: String::from("the segmentation model returned no frames"),
            });
        }
        let frame_samples = WINDOW_SAMPLES as f64 / scores.len() as f64;
        let frames = scores
            .iter()
            .enumerate()
            .take_while(|(frame, _)| {
                start_sample + ((*frame as f64 * frame_samples) as usize) < samples.len()
            })
            .map(|(_, frame_scores)| active_speakers(frame_scores))
            .collect();
        Ok(WindowActivity {
            start_sample,
            frame_samples,
            frames,
        })
    }

    /// Embeds each window's local speakers and links them into chunk speakers.
    ///
    /// Outputs: The chunk speakers, and for each window the chunk speaker of each local speaker
    /// (`None` when it talked too little to embed). Side effects: Runs embedding inference.
    #[allow(clippy::type_complexity)]
    fn link_speakers(
        &self,
        samples: &[f32],
        windows: &[WindowActivity],
    ) -> Result<(Vec<ChunkSpeaker>, Vec<[Option<usize>; LOCAL_SPEAKERS]>), DiarizationError> {
        let mut speakers: Vec<ChunkSpeaker> = Vec::new();
        let mut local_links = vec![[None; LOCAL_SPEAKERS]; windows.len()];
        let min_embedding_samples = (self.tuning.min_embedding_ms as f64 * SAMPLES_PER_MS) as usize;
        for (window_index, window) in windows.iter().enumerate() {
            for local_speaker in 0..LOCAL_SPEAKERS {
                let gather = |solo_only: bool| -> Vec<f32> {
                    window
                        .frames
                        .iter()
                        .enumerate()
                        .filter(|(_, active)| {
                            active[local_speaker]
                                && (!solo_only || active.iter().filter(|on| **on).count() == 1)
                        })
                        .flat_map(|(frame, _)| {
                            samples[window.frame_range(frame, samples.len())]
                                .iter()
                                .copied()
                        })
                        .collect()
                };
                let solo_audio = gather(true);
                let speech = if solo_audio.len() >= min_embedding_samples {
                    solo_audio
                } else {
                    gather(false)
                };
                if speech.len() < min_embedding_samples {
                    continue;
                }
                let raw_embedding = self.embedding_model.embed(&speech)?;
                if raw_embedding.iter().any(|value| !value.is_finite()) {
                    return Err(DiarizationError::InvalidAudio {
                        reason: String::from("the embedding model returned non-finite values"),
                    });
                }
                let Some(embedding) = unit(&raw_embedding) else {
                    continue;
                };
                if speakers
                    .first()
                    .is_some_and(|speaker| speaker.embedding_sum.len() != embedding.len())
                {
                    return Err(DiarizationError::InvalidAudio {
                        reason: String::from("the embedding model changed its output size"),
                    });
                }
                let best_match = speakers
                    .iter()
                    .enumerate()
                    .filter(|(_, speaker)| !speaker.windows.contains(&window_index))
                    .map(|(index, speaker)| (index, cosine(&speaker.centroid(), &embedding)))
                    .filter(|(_, similarity)| *similarity >= self.tuning.speaker_link_threshold)
                    .max_by(|(_, left), (_, right)| left.total_cmp(right));
                let speaker_index = match best_match {
                    Some((index, _)) => {
                        let speaker = &mut speakers[index];
                        for (sum, value) in speaker.embedding_sum.iter_mut().zip(&embedding) {
                            *sum += value;
                        }
                        speaker.windows.push(window_index);
                        index
                    }
                    None => {
                        speakers.push(ChunkSpeaker {
                            embedding_sum: embedding,
                            windows: vec![window_index],
                        });
                        speakers.len() - 1
                    }
                };
                local_links[window_index][local_speaker] = Some(speaker_index);
            }
        }
        Ok((speakers, local_links))
    }

    /// Assigns each instant to one chunk speaker and builds the turns.
    ///
    /// Outputs: Turns in time order, after dropping short ones and bridging short pauses.
    /// Side effects: None.
    fn assemble_turns(
        &self,
        chunk_length: usize,
        windows: &[WindowActivity],
        local_links: &[[Option<usize>; LOCAL_SPEAKERS]],
        speaker_count: usize,
    ) -> Vec<Turn> {
        let cell_samples = windows.first().map_or(1.0, |window| window.frame_samples);
        let cell_count = (chunk_length as f64 / cell_samples).ceil() as usize;
        let mut coverage = vec![0_u32; cell_count];
        let mut votes = vec![vec![0_u32; speaker_count]; cell_count];
        for (window, links) in windows.iter().zip(local_links) {
            for (frame, active) in window.frames.iter().enumerate() {
                let center =
                    window.start_sample as f64 + (frame as f64 + 0.5) * window.frame_samples;
                let cell = ((center / cell_samples) as usize).min(cell_count.saturating_sub(1));
                coverage[cell] += 1;
                for (local_speaker, is_active) in active.iter().enumerate() {
                    if let (true, Some(speaker)) = (*is_active, links[local_speaker]) {
                        votes[cell][speaker] += 1;
                    }
                }
            }
        }
        let cell_speaker: Vec<Option<usize>> = votes
            .iter()
            .zip(&coverage)
            .map(|(cell_votes, covered)| {
                cell_votes
                    .iter()
                    .enumerate()
                    .filter(|(_, count)| **count > 0 && **count * 2 >= *covered)
                    .max_by(|(left_index, left), (right_index, right)| {
                        left.cmp(right).then(right_index.cmp(left_index))
                    })
                    .map(|(speaker, _)| speaker)
            })
            .collect();
        let to_ms = |cell: usize| -> u64 {
            ((cell as f64 * cell_samples).min(chunk_length as f64) / SAMPLES_PER_MS).round() as u64
        };
        let mut runs: Vec<Turn> = Vec::new();
        let mut run_start = 0;
        for cell in 1..=cell_count {
            if cell == cell_count || cell_speaker[cell] != cell_speaker[run_start] {
                if let Some(speaker) = cell_speaker[run_start] {
                    runs.push(Turn {
                        speaker,
                        start_ms: to_ms(run_start),
                        end_ms: to_ms(cell),
                    });
                }
                run_start = cell;
            }
        }
        let mut turns: Vec<Turn> = Vec::new();
        for run in runs
            .into_iter()
            .filter(|run| run.end_ms - run.start_ms >= self.tuning.min_turn_ms)
        {
            match turns.last_mut() {
                Some(previous)
                    if previous.speaker == run.speaker
                        && run.start_ms - previous.end_ms <= self.tuning.max_merge_gap_ms =>
                {
                    previous.end_ms = run.end_ms;
                }
                _ => turns.push(run),
            }
        }
        turns
    }
}

#[async_trait::async_trait]
impl<Segmentation, Embedding> DiarizationEngine
    for WindowedDiarizationEngine<Segmentation, Embedding>
where
    Segmentation: SegmentationModel + 'static,
    Embedding: SpeakerEmbeddingModel + 'static,
{
    /// Decodes the chunk's WAV and diarizes it on a blocking thread.
    ///
    /// Side effects: Runs inference off the async runtime; never logs audio or embeddings.
    async fn diarize(&self, audio_bytes: &[u8]) -> Result<Vec<SpeakerSegment>, DiarizationError> {
        let samples = decode_wav_pcm16_mono(audio_bytes)?;
        let engine = self.clone();
        tokio::task::spawn_blocking(move || engine.diarize_samples(&samples))
            .await
            .map_err(|_| DiarizationError::Unavailable)?
    }
}

/// Window starts covering `chunk_length` samples: every `WINDOW_STEP_SAMPLES`, the last aligned
/// to the end.
fn window_starts(chunk_length: usize) -> Vec<usize> {
    if chunk_length <= WINDOW_SAMPLES {
        return vec![0];
    }
    let last_start = chunk_length - WINDOW_SAMPLES;
    let mut starts: Vec<usize> = (0..last_start).step_by(WINDOW_STEP_SAMPLES).collect();
    starts.push(last_start);
    starts
}

/// Scales `vector` to unit length; `None` for a zero vector.
fn unit(vector: &[f32]) -> Option<Vec<f32>> {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    (norm > f32::EPSILON).then(|| vector.iter().map(|value| value / norm).collect())
}

/// Cosine similarity of two unit vectors.
fn cosine(left: &[f32], right: &[f32]) -> f32 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}
