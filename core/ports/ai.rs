//! Declares AI capability ports without coupling the core to model libraries.

use crate::domain::{
    errors::{DiarizationError, TranscriptionError},
    models::{AudioRecording, ExternalMediaChunk, Transcript},
};

/// Describes one time-bounded speaker region and its derived biometric representation.
#[derive(Clone, Debug, PartialEq)]
pub struct SpeakerSegment {
    start_time_ms: u64,
    end_time_ms: u64,
    embedding: Vec<f32>,
}

impl SpeakerSegment {
    /// Creates a portable diarization result for one speaker region.
    ///
    /// Inputs: timestamps are milliseconds relative to the input audio; `embedding` contains
    /// model-derived biometric features and must never be logged.
    /// Outputs: An immutable segment that application code can match through a voice-identity port.
    /// Side effects: Takes ownership of the sensitive embedding vector.
    pub fn new(start_time_ms: u64, end_time_ms: u64, embedding: Vec<f32>) -> Self {
        Self {
            start_time_ms,
            end_time_ms,
            embedding,
        }
    }

    /// Returns the segment start offset in milliseconds.
    ///
    /// Inputs: None.
    /// Outputs: Milliseconds relative to the supplied audio buffer.
    /// Side effects: None.
    pub fn start_time_ms(&self) -> u64 {
        self.start_time_ms
    }

    /// Returns the segment end offset in milliseconds.
    ///
    /// Inputs: None.
    /// Outputs: Milliseconds relative to the supplied audio buffer.
    /// Side effects: None.
    pub fn end_time_ms(&self) -> u64 {
        self.end_time_ms
    }

    /// Borrows the derived speaker embedding without copying it.
    ///
    /// Inputs: None.
    /// Outputs: Model-derived biometric features; callers must not log the returned vector.
    /// Side effects: None.
    pub fn embedding(&self) -> &[f32] {
        &self.embedding
    }
}

/// Produces a transcript from recording metadata and adapter-managed audio content.
pub trait TranscriptionEngine {
    /// Transcribes one recording.
    ///
    /// Inputs: `audio_recording` identifies a recording whose raw bytes are resolved by the adapter.
    /// Outputs: A meeting-associated transcript or a portable transcription error.
    /// Side effects: An implementation may consume temporary audio or use local/remote compute.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
    ) -> Result<Transcript, TranscriptionError>;

    /// Transcribes one isolated external-media chunk while preserving the existing recording contract.
    ///
    /// Inputs: `media_chunk` carries opaque bridge audio; `audio_recording` identifies its local
    /// meeting context. Both must remain free of provider SDK types.
    /// Outputs: A meeting-associated transcript or a portable transcription error.
    /// Side effects: Default behavior delegates recording-only transcription; external-aware
    /// adapters override this method to consume `media_chunk.audio_bytes()` directly.
    fn transcribe_external_media(
        &self,
        _media_chunk: &ExternalMediaChunk,
        audio_recording: &AudioRecording,
    ) -> Result<Transcript, TranscriptionError> {
        self.transcribe(audio_recording)
    }
}

/// Separates speaker segments and derives embeddings without exposing model implementation details.
#[async_trait::async_trait]
pub trait DiarizationEngine {
    /// Requests diarization for opaque audio bytes.
    ///
    /// Inputs: `audio_bytes` contains the audio to analyze and must never be logged by an implementation.
    /// Outputs: Time-bounded speaker segments with derived embeddings or a portable diarization error.
    /// Side effects: An implementation may execute local or remote AI workloads.
    async fn diarize(&self, audio_bytes: &[u8]) -> Result<Vec<SpeakerSegment>, DiarizationError>;
}
