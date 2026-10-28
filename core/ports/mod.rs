//! Exposes capability contracts implemented by infrastructure adapters.

/// Declares contracts for transcription and speaker diarization engines.
pub mod ai;
/// Declares portable lifecycle and stream contracts for audio capture.
pub mod audio;
/// Declares at-rest encryption contracts for voice embeddings and their key metadata.
pub mod crypto;
/// Declares format-neutral meeting-export rendering contracts.
pub mod export;
/// Declares provider-neutral external meeting bridge contracts.
pub mod external_bridge;
/// Declares content-addressed attachment transfer and storage contracts.
pub mod file_transfer;
/// Declares attachment metadata extraction contracts.
pub mod media;
/// Declares how meeting data is read from and written into the meeting's CRDT document.
pub mod meeting_document;
/// Declares contracts for encrypted P2P transport and discovery.
pub mod network;
/// Declares persistence for this node's P2P identity and its trusted peers.
pub mod peer_trust;
/// Declares local screen-share detection, capture and slide storage contracts.
pub mod screen_share;
/// Declares contracts for transcript and voice-vector persistence.
pub mod storage;
/// Declares persistence for AI performance and device-load telemetry.
pub mod telemetry;
/// Declares storage and similarity-search capabilities for derived voice embeddings.
pub mod voice;
