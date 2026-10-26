//! Buffers 16 kHz mono PCM into VAD-sized frames and finalizes raw PCM chunks.

use crate::domain::{
    errors::AudioError,
    models::{AudioChunk, AudioConfig},
};

#[cfg(feature = "vad")]
use webrtc_vad::{SampleRate, Vad, VadMode};

/// WebRTC VAD accepts 10/20/30 ms frames; this adapter standardizes on 30 ms at 16 kHz.
pub const VAD_FRAME_SAMPLES: usize = 480;
const PCM_SAMPLE_RATE_HZ: usize = 16_000;
const SOFT_MAX_MS: u64 = 120_000;
const FALLBACK_MAX_MS: u64 = 30_000;
const LOOKBACK_MS: u64 = 1_000;

/// Explains why a raw PCM chunk became available for downstream processing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkCutType {
    /// VAD found silence lasting the configured language-specific pause duration.
    VadPause,
    /// The bounded duration limit required a split before downstream transcription.
    SoftMaxForced,
    /// The capture stream was interrupted before normal chunk finalization.
    Interrupted,
}

/// Carries raw PCM with its deterministic finalization reason.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedAudioChunk {
    audio_chunk: AudioChunk,
    cut_type: ChunkCutType,
}

impl FinalizedAudioChunk {
    /// Returns why this chunk was finalized.
    ///
    /// Inputs: None.
    /// Outputs: The portable finalization reason.
    /// Side effects: None.
    pub fn cut_type(&self) -> ChunkCutType {
        self.cut_type
    }
}

/// Configures language-sensitive VAD pause limits.
#[derive(Clone, Debug)]
pub struct VadConfig {
    pause_threshold_ms: u64,
}

impl VadConfig {
    /// Creates the English VAD configuration with a 600 ms pause boundary.
    ///
    /// Inputs: None.
    /// Outputs: An English language chunking configuration.
    /// Side effects: None.
    pub fn english() -> Self {
        Self {
            pause_threshold_ms: 600,
        }
    }

    /// Creates the Italian VAD configuration with an 800 ms pause boundary.
    ///
    /// Inputs: None.
    /// Outputs: An Italian language chunking configuration.
    /// Side effects: None.
    pub fn italian() -> Self {
        Self {
            pause_threshold_ms: 800,
        }
    }
}

/// Exposes finalized chunks while dumb nodes continue using raw `AudioCapture` directly.
pub trait ChunkedAudioSource {
    /// Retrieves one finalized chunk, if a boundary has been detected.
    ///
    /// Inputs: None.
    /// Outputs: A finalized raw PCM chunk, no chunk, or a portable audio error.
    /// Side effects: Removes a returned chunk from this adapter's in-memory queue.
    fn next_chunk(&mut self) -> Result<Option<FinalizedAudioChunk>, AudioError>;
}

/// Tracks the current VAD chunk-boundary decision without exposing it outside the adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ChunkingState {
    Idle,
    Listening,
    PauseDetected,
    Finalize,
}

/// Buffers arbitrary PCM input into exact 480-sample VAD frames.
pub struct VadChunker {
    configuration: VadConfig,
    pending_samples: Vec<i16>,
    active_samples: Vec<i16>,
    lookback_samples: Vec<i16>,
    processed_frames: usize,
    silence_duration_ms: u64,
    samples_since_soft_max: usize,
    state: ChunkingState,
    using_time_fallback: bool,
    ready_chunks: Vec<FinalizedAudioChunk>,
    #[cfg(feature = "vad")]
    vad: Option<Vad>,
}

impl VadChunker {
    /// Creates an empty chunker with WebRTC VAD when the optional capability is enabled.
    ///
    /// Inputs: `configuration` selects the language-specific silence duration.
    /// Outputs: An empty VAD chunker or a portable setup error.
    /// Side effects: Emits a warning and selects bounded 30-second time chunking if VAD is disabled.
    pub fn new(configuration: VadConfig) -> Result<Self, AudioError> {
        #[cfg(not(feature = "vad"))]
        tracing::warn!("VAD is disabled; falling back to 30-second time-based chunking");

        Ok(Self {
            configuration,
            pending_samples: Vec::new(),
            active_samples: Vec::new(),
            lookback_samples: Vec::new(),
            processed_frames: 0,
            silence_duration_ms: 0,
            samples_since_soft_max: 0,
            state: ChunkingState::Idle,
            using_time_fallback: !cfg!(feature = "vad"),
            ready_chunks: Vec::new(),
            #[cfg(feature = "vad")]
            vad: Some(Vad::new_with_rate_and_mode(
                SampleRate::Rate16kHz,
                VadMode::Quality,
            )),
        })
    }

    /// Buffers raw PCM and processes every complete 30 ms frame.
    ///
    /// Inputs: `audio_chunk` must contain 16 kHz mono signed PCM samples.
    /// Outputs: Success after buffering complete frames, or an invalid-format error.
    /// Side effects: Retains raw PCM in this adapter until a chunk boundary is reached.
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
            self.process_frame(frame);
        }
        Ok(())
    }

    /// Finalizes an interrupted partial chunk rather than losing already captured audio.
    ///
    /// Inputs: None.
    /// Outputs: Remaining active PCM or `AudioError::Interrupted` when no PCM was buffered.
    /// Side effects: Drains pending PCM and logs a privacy-safe interruption warning.
    pub fn handle_interruption(&mut self) -> Result<FinalizedAudioChunk, AudioError> {
        self.active_samples.append(&mut self.pending_samples);
        if self.active_samples.is_empty() {
            return Err(AudioError::Interrupted);
        }
        tracing::warn!("Audio interrupted while VAD chunk was active");
        self.state = ChunkingState::Finalize;
        Ok(FinalizedAudioChunk {
            audio_chunk: AudioChunk::new(
                std::mem::take(&mut self.active_samples),
                AudioConfig::new(16_000, 1),
            ),
            cut_type: ChunkCutType::Interrupted,
        })
    }

    /// Returns processed exact frame count.
    ///
    /// Inputs: None.
    /// Outputs: Number of fully processed 30 ms frames.
    /// Side effects: None.
    pub fn processed_frame_count(&self) -> usize {
        self.processed_frames
    }

    /// Returns unframed trailing sample count.
    ///
    /// Inputs: None.
    /// Outputs: Number of samples awaiting a complete frame.
    /// Side effects: None.
    pub fn buffered_sample_count(&self) -> usize {
        self.pending_samples.len()
    }

    /// Processes one exact VAD frame through the configured state machine.
    fn process_frame(&mut self, frame: Vec<i16>) {
        self.begin_listening_if_needed();
        self.active_samples.extend_from_slice(&frame);
        self.processed_frames += 1;
        self.samples_since_soft_max += frame.len();

        if self.reached_time_limit() {
            self.finalize(ChunkCutType::SoftMaxForced);
            self.samples_since_soft_max = 0;
            return;
        }

        if self.using_time_fallback {
            return;
        }

        match self.is_voice_frame(&frame) {
            Some(true) => {
                self.silence_duration_ms = 0;
                self.state = ChunkingState::Listening;
            }
            Some(false) => self.record_silence(),
            None => self.enable_time_fallback(),
        }
    }

    /// Starts a new active chunk, prepending preserved soft-max overlap when available.
    fn begin_listening_if_needed(&mut self) {
        if matches!(self.state, ChunkingState::Idle | ChunkingState::Finalize) {
            self.active_samples.append(&mut self.lookback_samples);
            self.silence_duration_ms = 0;
            self.state = ChunkingState::Listening;
        }
    }

    /// Records one silent frame and finalizes after the language-specific pause duration.
    fn record_silence(&mut self) {
        self.state = ChunkingState::PauseDetected;
        self.silence_duration_ms += frame_duration_ms();
        if self.silence_duration_ms >= self.configuration.pause_threshold_ms {
            self.finalize(ChunkCutType::VadPause);
        }
    }

    /// Indicates whether the active mode has reached its bounded time limit.
    fn reached_time_limit(&self) -> bool {
        let maximum_duration_ms = if self.using_time_fallback {
            FALLBACK_MAX_MS
        } else {
            SOFT_MAX_MS
        };
        samples_to_ms(self.samples_since_soft_max) >= maximum_duration_ms
    }

    /// Classifies every complete frame through the feature-gated WebRTC VAD implementation.
    #[cfg(feature = "vad")]
    fn is_voice_frame(&mut self, frame: &[i16]) -> Option<bool> {
        self.vad
            .as_mut()
            .and_then(|vad| vad.is_voice_segment(frame).ok())
    }

    /// Omits VAD classification entirely when the optional dependency is disabled.
    #[cfg(not(feature = "vad"))]
    fn is_voice_frame(&mut self, _frame: &[i16]) -> Option<bool> {
        None
    }

    /// Permanently switches to bounded time chunking after VAD becomes unavailable.
    fn enable_time_fallback(&mut self) {
        if !self.using_time_fallback {
            tracing::warn!("VAD failed; falling back to 30-second time-based chunking");
            self.using_time_fallback = true;
            self.samples_since_soft_max = self.active_samples.len();
        }
    }

    /// Queues active PCM and retains a one-second overlap after soft-max boundaries.
    fn finalize(&mut self, cut_type: ChunkCutType) {
        if self.active_samples.is_empty() {
            return;
        }

        let finalized_samples = std::mem::take(&mut self.active_samples);
        if cut_type == ChunkCutType::SoftMaxForced {
            let lookback_start = finalized_samples
                .len()
                .saturating_sub(milliseconds_to_samples(LOOKBACK_MS));
            self.lookback_samples = finalized_samples[lookback_start..].to_vec();
        } else {
            self.lookback_samples.clear();
        }
        self.ready_chunks.push(FinalizedAudioChunk {
            audio_chunk: AudioChunk::new(finalized_samples, AudioConfig::new(16_000, 1)),
            cut_type,
        });
        self.silence_duration_ms = 0;
        self.state = ChunkingState::Finalize;
    }
}

impl ChunkedAudioSource for VadChunker {
    /// Retrieves the most recently finalized chunk.
    ///
    /// Inputs: None.
    /// Outputs: A chunk when a boundary was reached, otherwise `None`.
    /// Side effects: Removes a returned chunk from the adapter queue.
    fn next_chunk(&mut self) -> Result<Option<FinalizedAudioChunk>, AudioError> {
        Ok(self.ready_chunks.pop())
    }
}

/// Converts a PCM sample count at the fixed stream rate to milliseconds.
fn samples_to_ms(samples: usize) -> u64 {
    (samples as u64).saturating_mul(1_000) / PCM_SAMPLE_RATE_HZ as u64
}

/// Converts a duration at the fixed stream rate to PCM samples.
fn milliseconds_to_samples(milliseconds: u64) -> usize {
    (milliseconds.saturating_mul(PCM_SAMPLE_RATE_HZ as u64) / 1_000) as usize
}

/// Returns the configured 30 ms VAD frame duration.
fn frame_duration_ms() -> u64 {
    samples_to_ms(VAD_FRAME_SAMPLES)
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

/// Normalizes transcription text before suffix/prefix overlap matching.
fn normalize(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .map(|word| word.trim_matches(|character: char| character.is_ascii_punctuation()))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
