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
/// Defines the language a recording is transcribed in.
pub mod transcription_language;
#[cfg(test)]
mod transcription_language_tests;
