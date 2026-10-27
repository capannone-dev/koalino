//! Exposes use cases that coordinate domain work solely through ports.

/// Defines portable provider and fallback configuration for AI workloads.
pub mod ai_config;
/// Selects registered adapter-neutral AI engines from provider configuration.
pub mod ai_registry;
/// Suggests meeting ownership for externally acquired files through metadata and storage ports.
pub mod attachment_manager;
#[cfg(test)]
mod attachment_manager_tests;
/// Lists unknown-speaker clusters and assigns them to named identities for driving adapters.
pub mod cluster_review;
#[cfg(test)]
mod cluster_review_tests;
/// Defines compute-routing policy seams for future distributed AI selection.
pub mod compute_router;
#[cfg(test)]
mod compute_router_tests;
/// Encrypts voice embeddings at rest and resolves the key protecting them.
pub mod crypto;
/// Coordinates diarization, identity matching, and manual-review clip preservation through ports.
pub mod diarization_processor;
#[cfg(test)]
mod diarization_processor_tests;
/// Orchestrates repository extraction, attachment staging, and dispatch to meeting exporters.
pub mod export_manager;
#[cfg(test)]
mod export_manager_tests;
/// Coordinates content-addressed attachment availability and binary transfers through ports.
pub mod file_sync_manager;
#[cfg(test)]
mod file_sync_manager_tests;
/// Coordinates Automerge meeting documents through storage ports.
pub mod meeting_crdt;
/// Coordinates recording transcription and transcript persistence.
pub mod meeting_recorder;
/// Applies adapter-neutral encrypted-network events to application state.
pub mod network_events;
#[cfg(test)]
mod network_events_tests;
/// Connects transcription, diarization, and voice identity into speaker-attributed segments.
pub mod pipeline;
/// Coordinates Automerge sync messages through encrypted-network and storage ports.
pub mod sync_manager;
#[cfg(test)]
mod sync_manager_tests;
/// Coordinates external meeting bridge media with local identities and AI ports.
pub mod teams_processor;
#[cfg(test)]
mod teams_processor_tests;
/// Defines telemetry requirements before dispatcher and storage implementation.
#[cfg(test)]
mod telemetry_tests;
/// Groups unknown voices within a meeting by cosine similarity and resolves clusters to identities.
pub mod unknown_speaker_clusterer;
#[cfg(test)]
mod unknown_speaker_clusterer_tests;
/// Classifies voice embeddings and records user-confirmed signature variants.
pub mod voice_identity_manager;
#[cfg(test)]
mod voice_identity_manager_tests;
