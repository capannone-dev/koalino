//! Defines the headless HTTP adapter boundary for the Axum server.
//!
//! Route composition and its test contract live in this module. Application and repository ports
//! remain the only core dependencies available to its handlers.

/// Implements REST/WebSocket routes and dependency-injected state.
pub mod api;
#[cfg(test)]
mod api_tests;
/// Serves the web Settings page's log level and log download routes (Issue #46).
pub mod log_routes;
#[cfg(test)]
mod log_routes_tests;
/// Serves the `/api/p2p/*` pairing and device routes (Issue #43).
pub mod p2p_routes;
#[cfg(test)]
mod p2p_routes_tests;
/// Refuses DNS-rebinding hosts and foreign browser origins before any route runs.
pub mod request_guard;
#[cfg(test)]
mod request_guard_tests;
