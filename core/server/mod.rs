//! Defines the headless HTTP adapter boundary for the Axum server.
//!
//! Route composition and its test contract live in this module. Application and repository ports
//! remain the only core dependencies available to its handlers.

/// Implements REST/WebSocket routes and dependency-injected state.
pub mod api;
#[cfg(test)]
mod api_tests;
