//! Exposes filesystem-backed attachment metadata adapters.

/// Reads portable local-file timestamps for attachment suggestion workflows.
pub mod local_file_metadata;
/// Reports no screen share and captures nothing, until a conferencing-app adapter exists.
pub mod no_screen_share;
