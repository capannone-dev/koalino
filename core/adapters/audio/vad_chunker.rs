//! Buffers 16 kHz mono PCM into VAD-sized frames and finalizes raw PCM chunks.

use crate::domain::{
    errors::AudioError,
    models::{AudioChunk, AudioConfig},
};

/// WebRTC VAD accepts 10/20/30 ms frames; this adapter standardizes on 30 ms at 16 kHz.
pub const VAD_FRAME_SAMPLES: usize = 480;
/// Explains why a raw PCM chunk became available for downstream processing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkCutType {
    VadPause,
    SoftMaxForced,
    Interrupted,
}
/// Carries raw PCM with its deterministic finalization reason.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedAudioChunk {
    audio_chunk: AudioChunk,
    cut_type: ChunkCutType,
}
impl FinalizedAudioChunk {
    pub fn cut_type(&self) -> ChunkCutType {
        self.cut_type
    }
}
/// Configures language-sensitive chunking limits.
#[derive(Clone, Debug)]
pub struct VadConfig {
    soft_max_ms: u64,
}
impl VadConfig {
    pub fn english() -> Self {
        Self {
            soft_max_ms: 120_000,
        }
    }
    pub fn italian() -> Self {
        Self {
            soft_max_ms: 120_000,
        }
    }
}
/// Exposes finalized chunks while dumb nodes continue using raw AudioCapture directly.
pub trait ChunkedAudioSource {
    fn next_chunk(&mut self) -> Result<Option<FinalizedAudioChunk>, AudioError>;
}
/// Buffers arbitrary PCM input into exact 480-sample VAD frames.
pub struct VadChunker {
    configuration: VadConfig,
    pending_samples: Vec<i16>,
    active_samples: Vec<i16>,
    processed_frames: usize,
    ready_chunks: Vec<FinalizedAudioChunk>,
}
impl VadChunker {
    /// Creates an empty chunker; integrations can fall back to time chunking if VAD initialization fails.
    pub fn new(configuration: VadConfig) -> Result<Self, AudioError> {
        Ok(Self {
            configuration,
            pending_samples: Vec::new(),
            active_samples: Vec::new(),
            processed_frames: 0,
            ready_chunks: Vec::new(),
        })
    }
    /// Buffers raw PCM then processes complete 30 ms frames only.
    pub fn push_pcm(&mut self, audio_chunk: AudioChunk) -> Result<(), AudioError> {
        if audio_chunk.config() != AudioConfig::new(16_000, 1) {
            return Err(AudioError::InvalidConfiguration {
                reason: "VAD requires 16 kHz mono PCM".to_owned(),
            });
        }
        self.pending_samples
            .extend_from_slice(audio_chunk.samples());
        while self.pending_samples.len() >= VAD_FRAME_SAMPLES {
            let frame: Vec<i16> = self.pending_samples.drain(..VAD_FRAME_SAMPLES).collect();
            self.active_samples.extend(frame);
            self.processed_frames += 1;
        }
        if u64::try_from(self.active_samples.len()).unwrap_or(u64::MAX) * 1000 / 16_000
            >= self.configuration.soft_max_ms
        {
            self.finalize(ChunkCutType::SoftMaxForced);
        }
        Ok(())
    }
    /// Finalizes an interrupted partial chunk rather than losing already captured audio.
    pub fn handle_interruption(&mut self) -> Result<FinalizedAudioChunk, AudioError> {
        self.active_samples.append(&mut self.pending_samples);
        if self.active_samples.is_empty() {
            return Err(AudioError::Interrupted);
        }
        tracing::warn!("Audio interrupted while VAD chunk was active");
        Ok(FinalizedAudioChunk {
            audio_chunk: AudioChunk::new(
                std::mem::take(&mut self.active_samples),
                AudioConfig::new(16_000, 1),
            ),
            cut_type: ChunkCutType::Interrupted,
        })
    }
    fn finalize(&mut self, cut_type: ChunkCutType) {
        if !self.active_samples.is_empty() {
            self.ready_chunks.push(FinalizedAudioChunk {
                audio_chunk: AudioChunk::new(
                    std::mem::take(&mut self.active_samples),
                    AudioConfig::new(16_000, 1),
                ),
                cut_type,
            });
        }
    }
    /// Returns processed exact frame count.
    pub fn processed_frame_count(&self) -> usize {
        self.processed_frames
    }
    /// Returns unframed trailing sample count.
    pub fn buffered_sample_count(&self) -> usize {
        self.pending_samples.len()
    }
}
impl ChunkedAudioSource for VadChunker {
    fn next_chunk(&mut self) -> Result<Option<FinalizedAudioChunk>, AudioError> {
        Ok(self.ready_chunks.pop())
    }
}
/// Merges normalized text using longest suffix/prefix overlap with a three-character minimum.
pub fn deduplicate_overlap(previous_text: &str, current_text: &str) -> String {
    let previous = normalize(previous_text);
    let current = normalize(current_text);
    for overlap in (3..=previous.len().min(current.len())).rev() {
        if previous.ends_with(&current[..overlap]) {
            let prefix = previous[..previous.len() - overlap].trim_end();
            return if prefix.is_empty() {
                current
            } else {
                format!("{prefix} {current}")
            };
        }
    }
    format!("{previous} {current}").trim().to_owned()
}
fn normalize(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .map(|word| word.trim_matches(|character: char| character.is_ascii_punctuation()))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
