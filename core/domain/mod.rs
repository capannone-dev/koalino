//! Exposes pure domain models and errors for the meeting-note business language.

/// Defines failures that may cross a hexagonal boundary.
pub mod errors;
/// Defines typed meeting-note business concepts.
pub mod models;
/// Defines paired devices and the numeric-comparison pairing code (Issue #43).
pub mod network_addresses;
#[cfg(test)]
mod network_addresses_tests;
pub mod peer_trust;
#[cfg(test)]
mod peer_trust_tests;
/// Defines projects: folders of meetings and external resources (Issue #85).
pub mod projects;
#[cfg(test)]
mod projects_tests;
/// Defines the language a recording is transcribed in.
pub mod transcription_language;
#[cfg(test)]
mod transcription_language_tests;
/// Lists the downloadable Whisper models with their pinned hashes.
pub mod whisper_catalog;
#[cfg(test)]
mod whisper_catalog_tests;
/// Proposes a different Whisper model from how the current one runs on this device.
pub mod whisper_proposal;
#[cfg(test)]
mod whisper_proposal_tests;
/// Suggests a Whisper model for a device from what it can run.
pub mod whisper_suggestion;
#[cfg(test)]
mod whisper_suggestion_tests;
