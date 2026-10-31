//! Groups opt-in local AI adapters and their feature-gated contract tests.

/// Local speaker diarization: windowed segmentation, speaker embeddings, and turn assembly.
pub mod diarization;
/// Implements the portable transcription port with a locally loaded whisper.cpp model.
#[cfg(feature = "local-ai")]
pub mod local_whisper_cpp;
/// Implements the portable transcription port through OpenAI's multipart Whisper API.
pub mod openai_whisper_adapter;

/// Downloads a Whisper model and installs it once its pinned hash matches.
pub mod whisper_model_download;
/// Builds timed words from whisper.cpp tokens; kept free of whisper.cpp types so it can be tested.
pub mod whisper_word_timing;

#[cfg(all(test, feature = "local-ai"))]
mod local_whisper_cpp_tests;
#[cfg(test)]
mod openai_whisper_adapter_tests;
#[cfg(test)]
mod whisper_model_download_tests;
#[cfg(test)]
mod whisper_word_timing_tests;
