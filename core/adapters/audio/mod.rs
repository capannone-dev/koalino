//! Groups audio-capture adapter implementations and their contract tests.

/// Implements `AudioCapture` using a deterministic local stream seam pending OS integration.
pub mod local_recorder;
/// Implements feature-gated 16 kHz VAD chunking over an AudioCapture source.
pub mod vad_chunker;

#[cfg(test)]
mod local_recorder_tests;
#[cfg(test)]
mod vad_chunker_tests;
