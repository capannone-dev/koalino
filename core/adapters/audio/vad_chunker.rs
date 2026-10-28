//! Buffers 16 kHz mono PCM into VAD-sized frames and finalizes raw PCM chunks.
//!
//! Chunks are short so the live transcript keeps up (Issue #52). With VAD a pause ends a chunk
//! once it holds at least three seconds, and nothing runs past fifteen. Without VAD every chunk
//! is cut near fifteen seconds. A forced cut lands at the end of the quietest 30 ms frame of the
//! last three seconds, usually the gap between two words, and the audio after it starts the next
//! chunk. Every sample therefore goes to exactly one chunk: a former one-second overlap made
//! Whisper transcribe the words at a boundary twice (Issue #51).

use crate::domain::{
    errors::AudioError,
    models::{AudioChunk, AudioConfig},
};

#[cfg(feature = "vad")]
use webrtc_vad::{SampleRate, Vad, VadMode};

/// WebRTC VAD accepts 10/20/30 ms frames; this adapter standardizes on 30 ms at 16 kHz.
pub const VAD_FRAME_SAMPLES: usize = 480;
const PCM_SAMPLE_RATE_HZ: usize = 16_000;
/// Longest chunk in either mode; it bounds how far the live transcript lags behind speech.
const MAX_CHUNK_MS: u64 = 15_000;
/// Shortest chunk a pause may end, so short phrases are not transcribed without context.
const MIN_PAUSE_CUT_MS: u64 = 3_000;
/// How far back from the cap a forced cut looks for the quietest frame.
const QUIET_SEARCH_MS: u64 = 3_000;
/// Voiced frames (300 ms) a VAD-classified chunk needs to count as speech.
const MIN_SPEECH_FRAMES: usize = 10;

/// Explains why a raw PCM chunk became available for downstream processing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChunkCutType {
    /// VAD found silence lasting the configured language-specific pause duration.
    VadPause,
    /// The bounded duration limit required a split before downstream transcription.
    SoftMaxForced,
    /// The capture stream was interrupted before normal chunk finalization.
    Interrupted,
    /// The recording stopped and the remaining buffered audio was flushed.
    StreamEnded,
}

/// Carries raw PCM with its deterministic finalization reason.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedAudioChunk {
    audio_chunk: AudioChunk,
    cut_type: ChunkCutType,
    /// False only when VAD classified the chunk and heard (almost) no speech in it.
    contains_speech: bool,
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

    /// Borrows the chunk's raw 16 kHz mono PCM; callers must never log it.
    ///
    /// Inputs: None.
    /// Outputs: The PCM chunk; it starts where the previous chunk ended.
    /// Side effects: None.
    pub fn audio_chunk(&self) -> &AudioChunk {
        &self.audio_chunk
    }

    /// Reports whether the chunk is worth transcribing.
    ///
    /// Inputs: None.
    /// Outputs: `false` when VAD heard less than 300 ms of speech in it; Whisper invents text
    /// such as `[BLANK_AUDIO]` or whole sentences for silence. Always `true` for time-based
    /// chunks, which nothing classified.
    /// Side effects: None.
    pub fn contains_speech(&self) -> bool {
        self.contains_speech
    }

    /// Takes ownership of the chunk's raw PCM.
    ///
    /// Inputs: None.
    /// Outputs: The PCM chunk.
    /// Side effects: Consumes the finalized chunk.
    pub fn into_audio_chunk(self) -> AudioChunk {
        self.audio_chunk
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

/// Owns a WebRTC VAD instance so a chunker can move to another thread (e.g. inside the FFI
/// runtime's mutex).
#[cfg(feature = "vad")]
struct SendableVad(Vad);

// SAFETY: `Vad` holds a raw pointer to a heap-allocated libfvad instance that it owns
// exclusively (created in `new`, freed in `Drop`). libfvad keeps all of its state in that
// instance, with no thread-local or global state, so moving ownership to another thread is
// sound. Classification needs `&mut`, so the instance is never used from two threads at once;
// `Sync` is deliberately not implemented.
#[cfg(feature = "vad")]
unsafe impl Send for SendableVad {}

/// Buffers arbitrary PCM input into exact 480-sample VAD frames.
pub struct VadChunker {
    configuration: VadConfig,
    pending_samples: Vec<i16>,
    /// The current chunk's whole frames; after a forced cut it starts with the audio that
    /// followed the cut.
    active_samples: Vec<i16>,
    /// Whether VAD classified each frame of `active_samples` as voice.
    active_voice_flags: Vec<bool>,
    processed_frames: usize,
    silence_duration_ms: u64,
    state: ChunkingState,
    using_time_fallback: bool,
    ready_chunks: Vec<FinalizedAudioChunk>,
    #[cfg(feature = "vad")]
    vad: Option<SendableVad>,
}

impl VadChunker {
    /// Creates an empty chunker with WebRTC VAD when the optional capability is enabled.
    ///
    /// Inputs: `configuration` selects the language-specific silence duration.
    /// Outputs: An empty VAD chunker or a portable setup error.
    /// Side effects: Emits a warning and selects bounded 30-second time chunking if VAD is disabled.
    pub fn new(configuration: VadConfig) -> Result<Self, AudioError> {
        #[cfg(not(feature = "vad"))]
        tracing::warn!("VAD is disabled; falling back to 15-second time-based chunking");

        Ok(Self::with_mode(configuration, !cfg!(feature = "vad")))
    }

    /// Creates a chunker that cuts near every 15 seconds, without speech detection.
    ///
    /// Inputs: `configuration` (its pause threshold is unused in this mode).
    /// Outputs: An empty time-based chunker.
    /// Side effects: None; used when VAD is disabled at runtime (`NOTE_TAKER_VAD_ENABLED=false`).
    pub fn time_based(configuration: VadConfig) -> Self {
        Self::with_mode(configuration, true)
    }

    /// Builds an empty chunker in VAD or time-based mode.
    ///
    /// Inputs: The language configuration and whether to use time-based cuts.
    /// Outputs: The chunker; the WebRTC classifier is only created in VAD mode.
    /// Side effects: None.
    fn with_mode(configuration: VadConfig, using_time_fallback: bool) -> Self {
        Self {
            configuration,
            pending_samples: Vec::new(),
            active_samples: Vec::new(),
            active_voice_flags: Vec::new(),
            processed_frames: 0,
            silence_duration_ms: 0,
            state: ChunkingState::Idle,
            using_time_fallback,
            ready_chunks: Vec::new(),
            #[cfg(feature = "vad")]
            vad: (!using_time_fallback).then(|| {
                SendableVad(Vad::new_with_rate_and_mode(
                    SampleRate::Rate16kHz,
                    VadMode::Quality,
                ))
            }),
        }
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
        let contains_speech = self.take_speech_verdict();
        Ok(FinalizedAudioChunk {
            audio_chunk: AudioChunk::new(
                std::mem::take(&mut self.active_samples),
                AudioConfig::new(16_000, 1),
            ),
            cut_type: ChunkCutType::Interrupted,
            contains_speech,
        })
    }

    /// Flushes all remaining audio when the recording stops.
    ///
    /// Inputs: None.
    /// Outputs: A `StreamEnded` chunk with the active and unframed samples, or `None` when nothing
    /// was buffered. Chunks already finalized stay available through `drain_ready_chunks`.
    /// Side effects: Clears every internal buffer, so the chunker restarts empty.
    pub fn finish(&mut self) -> Option<FinalizedAudioChunk> {
        self.active_samples.append(&mut self.pending_samples);
        self.silence_duration_ms = 0;
        self.state = ChunkingState::Idle;
        let contains_speech = self.take_speech_verdict();
        if self.active_samples.is_empty() {
            return None;
        }
        Some(FinalizedAudioChunk {
            audio_chunk: AudioChunk::new(
                std::mem::take(&mut self.active_samples),
                AudioConfig::new(16_000, 1),
            ),
            cut_type: ChunkCutType::StreamEnded,
            contains_speech,
        })
    }

    /// Removes every finalized chunk, oldest first.
    ///
    /// Inputs: None.
    /// Outputs: The finalized chunks in the order their boundaries were reached.
    /// Side effects: Empties the ready queue.
    pub fn drain_ready_chunks(&mut self) -> Vec<FinalizedAudioChunk> {
        std::mem::take(&mut self.ready_chunks)
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

        if samples_to_ms(self.active_samples.len()) >= MAX_CHUNK_MS {
            // Classified like any other frame, so the chunk's speech verdict covers it.
            let is_voice = self.using_time_fallback || self.is_voice_frame(&frame) != Some(false);
            self.active_voice_flags.push(is_voice);
            self.cut_at_quietest_recent_frame();
            return;
        }

        if self.using_time_fallback {
            self.active_voice_flags.push(true);
            return;
        }

        match self.is_voice_frame(&frame) {
            Some(true) => {
                self.active_voice_flags.push(true);
                self.silence_duration_ms = 0;
                self.state = ChunkingState::Listening;
            }
            Some(false) => {
                self.active_voice_flags.push(false);
                self.record_silence();
            }
            None => {
                self.active_voice_flags.push(true);
                self.enable_time_fallback();
            }
        }
    }

    /// Starts a new active chunk after a finalized one.
    fn begin_listening_if_needed(&mut self) {
        if matches!(self.state, ChunkingState::Idle | ChunkingState::Finalize) {
            self.silence_duration_ms = 0;
            self.state = ChunkingState::Listening;
        }
    }

    /// Records one silent frame and finalizes after the language-specific pause duration, once
    /// the chunk is long enough to stand alone.
    fn record_silence(&mut self) {
        self.state = ChunkingState::PauseDetected;
        self.silence_duration_ms += frame_duration_ms();
        if self.silence_duration_ms >= self.configuration.pause_threshold_ms
            && samples_to_ms(self.active_samples.len()) >= MIN_PAUSE_CUT_MS
        {
            self.finalize(ChunkCutType::VadPause);
        }
    }

    /// Ends the chunk at the quietest frame of its last `QUIET_SEARCH_MS`; the audio after that
    /// frame stays as the start of the next chunk.
    fn cut_at_quietest_recent_frame(&mut self) {
        let cut_sample = quietest_frame_end(&self.active_samples);
        let following_samples = self.active_samples.split_off(cut_sample);
        let following_voice_flags = self
            .active_voice_flags
            .split_off(cut_sample / VAD_FRAME_SAMPLES);
        self.finalize(ChunkCutType::SoftMaxForced);
        self.active_samples = following_samples;
        self.active_voice_flags = following_voice_flags;
        if !self.active_samples.is_empty() {
            self.state = ChunkingState::Listening;
        }
    }

    /// Classifies every complete frame through the feature-gated WebRTC VAD implementation.
    #[cfg(feature = "vad")]
    fn is_voice_frame(&mut self, frame: &[i16]) -> Option<bool> {
        self.vad
            .as_mut()
            .and_then(|vad| vad.0.is_voice_segment(frame).ok())
    }

    /// Omits VAD classification entirely when the optional dependency is disabled.
    #[cfg(not(feature = "vad"))]
    fn is_voice_frame(&mut self, _frame: &[i16]) -> Option<bool> {
        None
    }

    /// Permanently switches to bounded time chunking after VAD becomes unavailable.
    fn enable_time_fallback(&mut self) {
        if !self.using_time_fallback {
            tracing::warn!("VAD failed; falling back to 15-second time-based chunking");
            self.using_time_fallback = true;
        }
    }

    /// Queues the active PCM as a finished chunk.
    fn finalize(&mut self, cut_type: ChunkCutType) {
        if self.active_samples.is_empty() {
            return;
        }

        let contains_speech = self.take_speech_verdict();
        let finalized_samples = std::mem::take(&mut self.active_samples);
        self.ready_chunks.push(FinalizedAudioChunk {
            audio_chunk: AudioChunk::new(finalized_samples, AudioConfig::new(16_000, 1)),
            cut_type,
            contains_speech,
        });
        self.silence_duration_ms = 0;
        self.state = ChunkingState::Finalize;
    }
}

impl VadChunker {
    /// Decides whether the active chunk holds speech and resets the frame flags.
    ///
    /// Outputs: `true` in time-based mode, or when VAD heard at least `MIN_SPEECH_FRAMES`.
    /// Side effects: Clears the active chunk's voice flags.
    fn take_speech_verdict(&mut self) -> bool {
        let voiced_frames = self
            .active_voice_flags
            .iter()
            .filter(|is_voice| **is_voice)
            .count();
        self.active_voice_flags.clear();
        self.using_time_fallback || voiced_frames >= MIN_SPEECH_FRAMES
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

/// Finds where a forced cut should end a chunk.
///
/// Inputs: The chunk's samples, a whole number of VAD frames counted from its start.
/// Outputs: The sample index just after the frame with the least energy among the last
/// `QUIET_SEARCH_MS`; ties go to the latest frame, so steady audio is cut at the cap.
/// Side effects: None; reads samples without retaining or logging them.
fn quietest_frame_end(samples: &[i16]) -> usize {
    let frame_count = samples.len() / VAD_FRAME_SAMPLES;
    let searched_frames = (milliseconds_to_samples(QUIET_SEARCH_MS) / VAD_FRAME_SAMPLES).max(1);
    let first_frame = frame_count.saturating_sub(searched_frames);
    let mut quietest: Option<(u64, usize)> = None;
    for frame_index in first_frame..frame_count {
        let frame_start = frame_index * VAD_FRAME_SAMPLES;
        let energy: u64 = samples[frame_start..frame_start + VAD_FRAME_SAMPLES]
            .iter()
            .map(|sample| {
                let magnitude = u64::from(sample.unsigned_abs());
                magnitude * magnitude
            })
            .sum();
        if quietest.is_none_or(|(lowest_energy, _)| energy <= lowest_energy) {
            quietest = Some((energy, frame_index));
        }
    }
    quietest.map_or(samples.len(), |(_, frame_index)| {
        (frame_index + 1) * VAD_FRAME_SAMPLES
    })
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
