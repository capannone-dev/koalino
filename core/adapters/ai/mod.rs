//! Groups opt-in local AI adapters and their feature-gated contract tests.

/// Implements the portable transcription port with a locally loaded whisper.cpp model.
#[cfg(feature = "local-ai")]
pub mod local_whisper_cpp;
/// Implements the portable transcription port through OpenAI's multipart Whisper API.
pub mod openai_whisper_adapter;

#[cfg(all(test, feature = "local-ai"))]
mod local_whisper_cpp_tests;
#[cfg(test)]
mod openai_whisper_adapter_tests;
