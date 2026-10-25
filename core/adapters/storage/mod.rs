//! Exposes local persistence adapters that implement storage ports.

/// Implements verified SHA-256 content-addressed attachment persistence.
pub mod file_storage_adapter;
/// Implements a lightweight in-memory cosine-similarity database for tests and local development.
pub mod in_memory_voice_vector_db;
/// Implements the note repository port using SQLite tables and CRDT byte blobs.
pub mod sqlite_adapter;
