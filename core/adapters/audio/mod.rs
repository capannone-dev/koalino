//! Groups audio-capture adapter implementations and their contract tests.

/// Holds each live chunk's PCM until a transcription engine reads (and deletes) it.
pub mod chunk_audio_store;
/// Implements `AudioCapture` using a deterministic local stream seam pending OS integration.
pub mod local_recorder;
/// Reads 16 kHz mono PCM from WAV files or raw streams for file and piped recordings.
pub mod pcm_input;
/// Implements feature-gated 16 kHz VAD chunking over an AudioCapture source.
pub mod vad_chunker;

#[cfg(test)]
mod chunk_audio_store_tests;
#[cfg(test)]
mod local_recorder_tests;
#[cfg(test)]
mod pcm_input_tests;
#[cfg(test)]
mod vad_chunker_tests;
