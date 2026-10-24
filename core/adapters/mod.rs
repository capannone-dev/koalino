//! Exposes infrastructure adapters; this initial scaffold only configures observability.

/// Configures structured, redaction-aware logging sinks.
pub mod observability;
/// Implements persistence ports with local storage technologies.
pub mod storage;
