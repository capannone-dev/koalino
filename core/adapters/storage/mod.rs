//! Exposes local persistence adapters that implement storage ports.

/// Implements the note repository port using SQLite tables and CRDT byte blobs.
pub mod sqlite_adapter;
