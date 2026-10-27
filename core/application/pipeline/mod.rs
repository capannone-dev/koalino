//! Connects transcription, diarization, voice identity, and segment persistence per audio chunk.
//!
//! `meeting_session` owns the per-recording orchestration; `alignment` holds the pure timestamp
//! and text-distribution policy so it can be specified without any port.

/// Aligns diarized speaker regions with VAD chunk boundaries and distributes transcript text.
pub mod alignment;
/// Orchestrates one meeting's local VAD and external bridge chunks into persisted segments.
pub mod meeting_session;
/// Composition root that owns the pipeline ports and opens one session per recording.
pub mod pipeline_factory;
#[cfg(test)]
mod pipeline_factory_tests;
#[cfg(test)]
mod tests;
