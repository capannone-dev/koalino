//! Specifies that deleted content does not stay readable in the database file.

use std::{path::PathBuf, sync::Arc};

use rusqlite::Connection;
use uuid::Uuid;

use super::{
    sqlite_adapter::SqliteNoteRepository,
    sqlite_encryption_metadata::SqliteEncryptionMetadataStore, sqlite_peer_store::SqlitePeerStore,
    sqlite_voice_vector_db::SqliteVoiceVectorDB,
};
use crate::{
    application::crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
    domain::models::{DeviceId, MeetingId, Transcript},
    ports::storage::NoteRepository,
};

const MARKER: &[u8] = b"PRIVATE-MARKER-7f3a";

fn cipher() -> Arc<VoiceEncryptor> {
    Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes([5; 32])))
}

/// A database path in a fresh folder, removed on drop.
struct ScratchDatabase(PathBuf);

impl ScratchDatabase {
    fn new() -> Self {
        let folder = std::env::temp_dir().join(format!("secure-delete-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&folder).unwrap_or_else(|error| panic!("folder: {error}"));
        Self(folder.join("notes.sqlite3"))
    }

    /// Whether the marker is readable in the file.
    fn holds_marker(&self) -> bool {
        std::fs::read(&self.0)
            .unwrap_or_else(|error| panic!("database must read: {error}"))
            .windows(MARKER.len())
            .any(|window| window == MARKER)
    }

    fn repository(&self) -> SqliteNoteRepository {
        SqliteNoteRepository::open(&self.0, DeviceId::new("device"), cipher())
            .unwrap_or_else(|error| panic!("repository must open: {error}"))
    }
}

impl Drop for ScratchDatabase {
    fn drop(&mut self) {
        if let Some(folder) = self.0.parent() {
            let _ = std::fs::remove_dir_all(folder);
        }
    }
}

/// A deleted meeting's text is gone from the file, not only from the tables.
#[test]
fn a_deleted_meeting_leaves_no_readable_text_in_the_file() {
    let database = ScratchDatabase::new();
    let repository = database.repository();
    let meeting_id = MeetingId::new("meeting-private");
    let text = String::from_utf8_lossy(MARKER).into_owned();
    repository
        .create_meeting(&meeting_id, &[1])
        .and_then(|()| repository.save_transcript(&Transcript::new(meeting_id.clone(), text)))
        .unwrap_or_else(|error| panic!("meeting must be stored: {error}"));
    assert!(database.holds_marker(), "the test must see stored text");

    repository
        .delete_meeting(&meeting_id)
        .unwrap_or_else(|error| panic!("deletion must succeed: {error}"));

    assert!(!database.holds_marker());
}

/// Text deleted before this setting existed is cleared at the next start, once.
#[test]
fn opening_clears_what_earlier_deletions_left_behind() {
    let database = ScratchDatabase::new();
    {
        let legacy = Connection::open(&database.0).unwrap_or_else(|error| panic!("{error}"));
        legacy
            .execute_batch(&format!(
                "PRAGMA secure_delete = OFF;
                 CREATE TABLE leftovers (body TEXT);
                 INSERT INTO leftovers VALUES ('{}');
                 DELETE FROM leftovers;",
                String::from_utf8_lossy(MARKER)
            ))
            .unwrap_or_else(|error| panic!("{error}"));
    }
    assert!(database.holds_marker(), "the test must see leftover text");

    drop(database.repository());

    assert!(!database.holds_marker());
    let recorded: i64 = Connection::open(&database.0)
        .and_then(|connection| {
            connection.query_row(
                "SELECT COUNT(*) FROM storage_migrations WHERE name = 'secure_delete_vacuum'",
                [],
                |row| row.get(0),
            )
        })
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(recorded, 1);
}

/// Every store on the shared database erases, since the setting belongs to each connection.
#[test]
fn every_store_turns_secure_delete_on() {
    let database = ScratchDatabase::new();
    let notes = database.repository();
    let voices = SqliteVoiceVectorDB::open(&database.0, cipher())
        .unwrap_or_else(|error| panic!("voice store must open: {error}"));
    let peers = SqlitePeerStore::open(&database.0, cipher())
        .unwrap_or_else(|error| panic!("peer store must open: {error}"));
    let metadata = SqliteEncryptionMetadataStore::open(&database.0)
        .unwrap_or_else(|error| panic!("metadata store must open: {error}"));

    for setting in [
        notes.secure_delete_setting(),
        voices.secure_delete_setting(),
        peers.secure_delete_setting(),
        metadata.secure_delete_setting(),
    ] {
        assert_eq!(setting, Some(1));
    }
}
