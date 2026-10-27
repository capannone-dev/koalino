//! Specifies the SQLite external-ID mappings that let Teams participants keep one local identity.
//!
//! File-backed tests use unique paths under the OS temp directory and delete them afterwards.

use std::{path::PathBuf, sync::Arc};

use uuid::Uuid;

use crate::{
    adapters::storage::sqlite_voice_vector_db::SqliteVoiceVectorDB,
    application::{
        crypto::voice_encryptor::ephemeral_cipher, teams_processor::TeamsIdentityMapper,
    },
    domain::models::{ExternalParticipant, SpeakerIdentity},
    ports::{
        crypto::EmbeddingCipher,
        external_bridge::{ExternalIdentityRepository, IdentityMapper},
        voice::VoiceVectorDB,
    },
};

/// Deletes a temporary SQLite file when dropped.
struct TemporaryDatabase {
    path: PathBuf,
    cipher: Arc<dyn EmbeddingCipher>,
}

impl TemporaryDatabase {
    /// Reserves a unique database path.
    fn new() -> Self {
        Self {
            path: std::env::temp_dir().join(format!(
                "note-taker-external-ids-{}.sqlite3",
                Uuid::new_v4()
            )),
            cipher: ephemeral_cipher(),
        }
    }

    /// Opens the voice store on this file.
    fn open(&self) -> SqliteVoiceVectorDB {
        SqliteVoiceVectorDB::open(&self.path, Arc::clone(&self.cipher))
            .unwrap_or_else(|error| panic!("voice store should open: {error}"))
    }
}

impl Drop for TemporaryDatabase {
    /// Removes the database file, ignoring a file that was never created.
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Verifies an unseen external ID has no mapping.
#[test]
fn unseen_external_ids_are_not_mapped() {
    let database = TemporaryDatabase::new();
    let found = database
        .open()
        .find_by_external_id("teams-user-1")
        .unwrap_or_else(|error| panic!("lookup should succeed: {error}"));
    assert!(found.is_none());
}

/// Verifies a saved mapping survives reopening and creates the speaker identity.
#[test]
fn saved_mappings_persist_and_create_the_identity() {
    let database = TemporaryDatabase::new();
    let identity = SpeakerIdentity::new(Uuid::new_v4(), "Ada Lovelace");
    database
        .open()
        .save_external_mapping("teams-user-1", &identity)
        .unwrap_or_else(|error| panic!("mapping should save: {error}"));

    let reopened = database.open();
    let found = reopened
        .find_by_external_id("teams-user-1")
        .unwrap_or_else(|error| panic!("lookup should succeed: {error}"));
    assert_eq!(found, Some(identity.clone()));
    let stored_identity = reopened
        .load_speaker_identity(identity.identity_id())
        .unwrap_or_else(|error| panic!("identity should load: {error}"));
    assert_eq!(stored_identity, Some(identity));
}

/// Verifies a rename in the voice library is reflected in later lookups.
#[test]
fn lookups_return_the_current_identity_name() {
    let database = TemporaryDatabase::new();
    let voice_store = database.open();
    let identity_id = Uuid::new_v4();
    voice_store
        .save_external_mapping("teams-user-1", &SpeakerIdentity::new(identity_id, "Ada"))
        .unwrap_or_else(|error| panic!("mapping should save: {error}"));
    voice_store
        .save_speaker_identity(&SpeakerIdentity::new(identity_id, "Ada Lovelace"))
        .unwrap_or_else(|error| panic!("rename should save: {error}"));

    let found = voice_store
        .find_by_external_id("teams-user-1")
        .unwrap_or_else(|error| panic!("lookup should succeed: {error}"));
    assert_eq!(
        found.map(|identity| identity.display_name().to_owned()),
        Some("Ada Lovelace".to_owned())
    );
}

/// Verifies the Teams mapper reuses one identity per external ID across store reopenings.
#[test]
fn teams_mapper_reuses_identities_across_restarts() {
    let database = TemporaryDatabase::new();
    let participant =
        ExternalParticipant::new("teams-user-7".to_owned(), "Alan Turing".to_owned(), None);
    let first = TeamsIdentityMapper::new(std::sync::Mutex::new(database.open()))
        .map_external_to_local(participant.clone())
        .unwrap_or_else(|error| panic!("mapping should succeed: {error}"));
    let second = TeamsIdentityMapper::new(std::sync::Mutex::new(database.open()))
        .map_external_to_local(participant)
        .unwrap_or_else(|error| panic!("mapping should succeed: {error}"));
    assert_eq!(first, second);
    assert_eq!(first.display_name(), "Alan Turing");
}

/// Verifies blank external IDs are rejected.
#[test]
fn blank_external_ids_are_rejected() {
    let database = TemporaryDatabase::new();
    let outcome = database
        .open()
        .save_external_mapping("  ", &SpeakerIdentity::new(Uuid::new_v4(), "Nobody"));
    assert!(outcome.is_err());
}
