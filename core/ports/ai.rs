//! Declares AI capability ports without coupling the core to model libraries.

use crate::domain::{
    errors::TranscriptionError,
    models::{AudioRecording, Transcript},
};

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
}

/// Separates known or unknown speaker segments without retaining voice embeddings in domain state.
pub trait DiarizationEngine {
    /// Requests diarization for a recording.
    ///
    /// Inputs: `audio_recording` identifies adapter-managed audio.
    /// Outputs: Speaker labels in implementation-defined, privacy-safe form or a transcription error.
    /// Side effects: An implementation may execute local or remote AI workloads.
    fn diarize(&self, audio_recording: &AudioRecording) -> Result<Vec<String>, TranscriptionError>;
}
