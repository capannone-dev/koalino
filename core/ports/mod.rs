//! Exposes capability contracts implemented by infrastructure adapters.

/// Declares contracts for transcription and speaker diarization engines.
pub mod ai;
/// Declares contracts for encrypted P2P transport and discovery.
pub mod network;
/// Declares contracts for transcript and voice-vector persistence.
pub mod storage;
