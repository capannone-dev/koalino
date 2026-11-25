//! Exposes local persistence adapters that implement storage ports.

/// Copies picked files and camera photos privately into a meeting's attachment folder.
pub mod attachment_file_store;
#[cfg(test)]
mod attachment_file_store_tests;

/// Exchanges attachment files with paired devices: manifests, chunks, verified arrivals.
pub mod attachment_sync_store;
#[cfg(test)]
mod attachment_sync_store_tests;

/// Writes and restores the encrypted backup of all of a device's data (#167).
pub mod backup_archive;
#[cfg(test)]
mod backup_archive_tests;
#[cfg(test)]
mod embedding_encryption_tests;
/// Implements a lightweight in-memory cosine-similarity database for tests and local development.
pub mod in_memory_voice_vector_db;
/// Creates private attachment folders and files shared by the attachment stores.
pub mod private_files;
/// Seals attachment files at rest and reads them back (#99).
pub mod sealed_files;
#[cfg(test)]
mod sealed_files_tests;
/// Stores Teams screen-share captures as image files linked to their meeting.
pub mod slide_attachment_store;
#[cfg(test)]
mod slide_attachment_store_tests;
/// Implements the note repository port using SQLite tables and CRDT byte blobs.
pub mod sqlite_adapter;
#[cfg(test)]
mod sqlite_chat_history_tests;
/// Seals review-clip audio through the cipher port and seals legacy plaintext clips once.
mod sqlite_clip_audio_encryption;
#[cfg(test)]
mod sqlite_cluster_share_tests;
#[cfg(test)]
mod sqlite_combined_meetings_tests;
#[cfg(test)]
mod sqlite_deletions_tests;
/// Seals embedding BLOBs through the cipher port and re-encrypts legacy plaintext rows.
mod sqlite_embedding_encryption;
/// Persists the database's encryption metadata (key source, Argon2id salt, key check).
pub mod sqlite_encryption_metadata;
#[cfg(test)]
mod sqlite_external_identities_tests;
#[cfg(test)]
mod sqlite_meeting_document_tests;
/// Seals transcripts, transcript segments and meeting documents at rest (#92).
mod sqlite_note_encryption;
#[cfg(test)]
mod sqlite_note_encryption_tests;
/// Persists the sealed P2P node identity and the trusted-peer allowlist in SQLite.
pub mod sqlite_peer_store;
#[cfg(test)]
mod sqlite_peer_store_tests;
#[cfg(test)]
mod sqlite_projects_tests;
/// Makes SQLite erase deleted data instead of only marking its space as free.
mod sqlite_secure_delete;
#[cfg(test)]
mod sqlite_secure_delete_tests;
#[cfg(test)]
mod sqlite_voice_library_sync_tests;
/// Implements the voice-vector port with SQLite-persisted speaker names and voice variants.
pub mod sqlite_voice_vector_db;
#[cfg(test)]
mod sqlite_voice_vector_db_tests;
