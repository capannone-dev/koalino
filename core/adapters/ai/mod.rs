//! Groups opt-in local AI adapters and their feature-gated contract tests.

/// Decides how local inference uses this machine's cores and GPU.
pub mod compute_resources;
/// Local speaker diarization: windowed segmentation, speaker embeddings, and turn assembly.
pub mod diarization;
/// Moves a GPU that once crashed the app back to the CPU at the next start.
pub mod gpu_attempt_guard;
#[cfg(test)]
mod gpu_attempt_guard_tests;
/// Keeps the threads feeding a phone GPU on one CPU cluster while it works.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub mod gpu_thread_affinity;
#[cfg(all(test, any(target_os = "linux", target_os = "android")))]
mod gpu_thread_affinity_tests;
/// Language model adapters for summaries and chat (#95).
pub mod llm;
/// Implements the portable transcription port with a locally loaded whisper.cpp model.
#[cfg(feature = "local-ai")]
pub mod local_whisper_cpp;
/// Implements the portable transcription port through OpenAI's multipart Whisper API.
pub mod openai_whisper_adapter;
/// Opens the OpenCL library only when called, so the build starts on phones without it.
#[cfg(all(feature = "mobile-gpu", target_os = "android"))]
pub mod opencl_loader_shim;
/// Opens the Vulkan loader only when called, so a Vulkan build starts on machines without it.
#[cfg(all(feature = "local-ai", not(target_os = "android")))]
pub mod vulkan_loader_shim;

/// Downloads a Whisper model and installs it once its pinned hash matches.
pub mod whisper_model_download;
/// Builds timed words from whisper.cpp tokens; kept free of whisper.cpp types so it can be tested.
pub mod whisper_word_timing;

#[cfg(test)]
mod compute_resources_tests;
#[cfg(all(test, feature = "local-ai"))]
mod local_whisper_cpp_tests;
#[cfg(test)]
mod openai_whisper_adapter_tests;
#[cfg(test)]
mod whisper_model_download_tests;
#[cfg(test)]
mod whisper_word_timing_tests;
