//! Exposes use cases that coordinate domain work solely through ports.

/// Defines compute-routing policy seams for future distributed AI selection.
pub mod compute_router;
/// Coordinates Automerge meeting documents through storage ports.
pub mod meeting_crdt;
/// Coordinates recording transcription and transcript persistence.
pub mod meeting_recorder;
/// Applies adapter-neutral encrypted-network events to application state.
pub mod network_events;
#[cfg(test)]
mod network_events_tests;
/// Coordinates Automerge sync messages through encrypted-network and storage ports.
pub mod sync_manager;
#[cfg(test)]
mod sync_manager_tests;
