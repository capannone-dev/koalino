//! Groups opt-in local AI adapters and their feature-gated contract tests.

/// Implements the portable transcription port with a locally loaded whisper.cpp model.
pub mod local_whisper_cpp;

#[cfg(test)]
mod local_whisper_cpp_tests;
