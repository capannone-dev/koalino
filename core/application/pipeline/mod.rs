//! Connects transcription, diarization, voice identity, and segment persistence per audio chunk.
//!
//! `meeting_session` owns the per-recording orchestration; `alignment` holds the pure timestamp
//! and text-distribution policy so it can be specified without any port.

/// Aligns diarized speaker regions with VAD chunk boundaries and distributes transcript text.
pub mod alignment;
/// Orchestrates one meeting's local VAD and external bridge chunks into persisted segments.
pub mod meeting_session;
/// Groups a whole meeting's voice windows into speakers at its end (#73).
pub mod meeting_speaker_pass;
#[cfg(test)]
mod meeting_speaker_pass_tests;
/// Composition root that owns the pipeline ports and opens one session per recording.
pub mod pipeline_factory;
#[cfg(test)]
mod pipeline_factory_tests;
/// Records one privacy-safe telemetry row per transcribed or diarized chunk.
pub mod task_metrics;
#[cfg(test)]
mod task_metrics_tests;
#[cfg(test)]
mod tests;
