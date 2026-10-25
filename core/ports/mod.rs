//! Exposes capability contracts implemented by infrastructure adapters.

/// Declares contracts for transcription and speaker diarization engines.
pub mod ai;
/// Declares portable lifecycle and stream contracts for audio capture.
pub mod audio;
/// Declares provider-neutral external meeting bridge contracts.
pub mod external_bridge;
/// Declares content-addressed attachment transfer and storage contracts.
pub mod file_transfer;
/// Declares attachment metadata extraction contracts.
pub mod media;
/// Declares contracts for encrypted P2P transport and discovery.
pub mod network;
/// Declares contracts for transcript and voice-vector persistence.
pub mod storage;
/// Declares persistence for AI performance and device-load telemetry.
pub mod telemetry;
/// Declares storage and similarity-search capabilities for derived voice embeddings.
pub mod voice;
