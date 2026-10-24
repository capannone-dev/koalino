//! Groups audio-capture adapter implementations and their contract tests.

/// Implements `AudioCapture` using a deterministic local stream seam pending OS integration.
pub mod local_recorder;

#[cfg(test)]
mod local_recorder_tests;
