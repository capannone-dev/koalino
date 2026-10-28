//! Exposes local persistence adapters that implement storage ports.

#[cfg(test)]
mod embedding_encryption_tests;
/// Implements verified SHA-256 content-addressed attachment persistence.
pub mod file_storage_adapter;
/// Implements a lightweight in-memory cosine-similarity database for tests and local development.
pub mod in_memory_voice_vector_db;
/// Stores Teams screen-share captures as image files linked to their meeting.
pub mod slide_attachment_store;
#[cfg(test)]
mod slide_attachment_store_tests;
/// Implements the note repository port using SQLite tables and CRDT byte blobs.
pub mod sqlite_adapter;
/// Seals embedding BLOBs through the cipher port and re-encrypts legacy plaintext rows.
mod sqlite_embedding_encryption;
/// Persists the database's encryption metadata (key source, Argon2id salt, key check).
pub mod sqlite_encryption_metadata;
#[cfg(test)]
mod sqlite_external_identities_tests;
/// Persists the sealed P2P node identity and the trusted-peer allowlist in SQLite.
pub mod sqlite_peer_store;
#[cfg(test)]
mod sqlite_peer_store_tests;
/// Implements the voice-vector port with SQLite-persisted speaker names and voice variants.
pub mod sqlite_voice_vector_db;
#[cfg(test)]
mod sqlite_voice_vector_db_tests;
