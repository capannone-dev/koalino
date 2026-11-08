//! Specifies at-rest encryption across the SQLite adapters (Issue #31): every embedding column is
//! sealed on disk, plaintext databases from before #31 are re-encrypted on first open, another
//! key cannot read the data, and the encryption metadata survives restarts.
//!
//! Each test uses a unique database file and key directory under the OS temp directory, both
//! removed afterwards. Raw `rusqlite` connections inspect the stored bytes directly.

use std::{path::PathBuf, sync::Arc};

use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::{
    adapters::storage::{
        sqlite_adapter::SqliteNoteRepository,
        sqlite_encryption_metadata::SqliteEncryptionMetadataStore,
        sqlite_voice_vector_db::SqliteVoiceVectorDB,
    },
    application::{
        crypto::{
            key_manager::{
                KEY_DIRECTORY_VARIABLE, KeyManager, KeyManagerConfig, PASSPHRASE_VARIABLE,
            },
            voice_encryptor::{SEALED_EMBEDDING_HEADER, ephemeral_cipher},
        },
        unknown_speaker_clusterer::{ClusteringConfig, UnknownSpeakerClusterer},
    },
    domain::{
        errors::CryptoError,
        models::{DeviceId, MeetingId, SpeakerIdentity, UnassignedVoiceClip, VoiceEmbedding},
    },
    ports::{
        crypto::{
            EmbeddingCipher, EncryptionKeySource, EncryptionMetadata, EncryptionMetadataRepository,
            KeyDerivationParameters,
        },
        storage::NoteRepository,
        voice::VoiceVectorDB,
    },
};

/// `(table, embedding column)` pairs that hold biometric vectors.
const EMBEDDING_COLUMNS: [(&str, &str); 4] = [
    ("voice_variants", "embedding"),
    ("unknown_speaker_clusters", "representative_embedding"),
    ("cluster_chunks", "derived_embedding"),
    ("unassigned_voice_clips", "derived_embedding"),
];
/// The fixture's clip embedding.
const CLIP_EMBEDDING: [f32; 2] = [0.1, 0.2];
/// The fixture's cluster (and single member) embedding.
const CLUSTER_EMBEDDING: [f32; 2] = [1.0, 0.0];
/// The fixture's voice variant embedding.
const VARIANT_EMBEDDING: [f32; 2] = [0.3, 0.4];

/// A database file and key directory removed when dropped.
struct TemporaryStorage {
    root: PathBuf,
}

impl TemporaryStorage {
    /// Creates a unique root directory.
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("note-taker-encryption-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root)
            .unwrap_or_else(|error| panic!("test root must be created: {error}"));
        Self { root }
    }

    /// Returns the database file path.
    fn database_path(&self) -> PathBuf {
        self.root.join("note_taker.sqlite3")
    }

    /// Opens a raw connection for inspecting stored bytes.
    fn raw_connection(&self) -> Connection {
        Connection::open(self.database_path())
            .unwrap_or_else(|error| panic!("raw connection must open: {error}"))
    }

    /// Opens the note repository with `cipher`.
    fn note_repository(&self, cipher: &Arc<dyn EmbeddingCipher>) -> SqliteNoteRepository {
        SqliteNoteRepository::open(
            self.database_path(),
            DeviceId::new("encryption-test"),
            Arc::clone(cipher),
        )
        .unwrap_or_else(|error| panic!("note repository must open: {error}"))
    }

    /// Opens the voice store with `cipher`.
    fn voice_store(&self, cipher: &Arc<dyn EmbeddingCipher>) -> SqliteVoiceVectorDB {
        SqliteVoiceVectorDB::open(self.database_path(), Arc::clone(cipher))
            .unwrap_or_else(|error| panic!("voice store must open: {error}"))
    }

    /// Unlocks the database key as the composition roots do, with optional passphrase.
    fn unlock(&self, passphrase: Option<&str>) -> Result<Arc<dyn EmbeddingCipher>, CryptoError> {
        let key_directory = self.root.join("keys").to_string_lossy().into_owned();
        let metadata_store = SqliteEncryptionMetadataStore::open(self.database_path())
            .unwrap_or_else(|error| panic!("metadata store must open: {error}"));
        let config = KeyManagerConfig::from_lookup(|name| match name {
            KEY_DIRECTORY_VARIABLE => Some(key_directory.clone()),
            PASSPHRASE_VARIABLE => passphrase.map(String::from),
            _ => None,
        })
        .with_derivation_parameters(KeyDerivationParameters::new(64, 1, 1));
        KeyManager::new(config)
            .unlock(&metadata_store)
            .map(|encryptor| Arc::new(encryptor) as Arc<dyn EmbeddingCipher>)
    }

    /// Reads every `(format, bytes)` pair of one embedding column in row order.
    fn stored_embeddings(&self, table: &str, column: &str) -> Vec<(i64, Vec<u8>)> {
        let connection = self.raw_connection();
        let mut statement = connection
            .prepare(&format!(
                "SELECT embedding_format, {column} FROM {table} ORDER BY rowid"
            ))
            .unwrap_or_else(|error| panic!("{table} must be readable: {error}"));
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .and_then(Iterator::collect)
            .unwrap_or_else(|error| panic!("{table} rows must decode: {error}"))
    }
}

impl Drop for TemporaryStorage {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Encodes an embedding the way databases before Issue #31 stored it.
fn plaintext_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// Identifiers of the rows written by `write_fixture`.
struct Fixture {
    meeting_id: MeetingId,
    cluster_id: Uuid,
}

/// Writes one clip, one single-member cluster, and one voice variant through the adapters.
fn write_fixture(
    note_repository: &SqliteNoteRepository,
    voice_store: &SqliteVoiceVectorDB,
) -> Fixture {
    let meeting_id = MeetingId::new("encrypted-meeting");
    note_repository
        .create_meeting(&meeting_id, &[1])
        .unwrap_or_else(|error| panic!("meeting must save: {error}"));
    note_repository
        .save_unassigned_clip(UnassignedVoiceClip::with_review_context(
            Uuid::new_v4(),
            meeting_id.clone(),
            0,
            1_000,
            vec![9, 9],
            CLIP_EMBEDDING.to_vec(),
            None,
        ))
        .unwrap_or_else(|error| panic!("clip must save: {error}"));
    let cluster_id = UnknownSpeakerClusterer::new(note_repository, ClusteringConfig::default())
        .find_or_create_cluster(
            &meeting_id,
            &CLUSTER_EMBEDDING,
            String::from("chunk-1"),
            1_000,
        )
        .unwrap_or_else(|error| panic!("cluster must save: {error}"))
        .cluster_id();
    let identity = SpeakerIdentity::new(Uuid::new_v4(), "Grace");
    voice_store
        .save_speaker_identity(&identity)
        .unwrap_or_else(|error| panic!("identity must save: {error}"));
    voice_store
        .store_embedding(VoiceEmbedding::new(
            identity.identity_id(),
            Uuid::new_v4(),
            VARIANT_EMBEDDING.to_vec(),
        ))
        .unwrap_or_else(|error| panic!("variant must save: {error}"));
    Fixture {
        meeting_id,
        cluster_id,
    }
}

/// Asserts that the adapters return the fixture's original embeddings.
fn assert_fixture_readable(
    note_repository: &SqliteNoteRepository,
    voice_store: &SqliteVoiceVectorDB,
    fixture: &Fixture,
) {
    let clips = note_repository
        .get_unassigned_clips(&fixture.meeting_id)
        .unwrap_or_else(|error| panic!("clips must load: {error}"));
    let cluster = note_repository
        .load_unknown_speaker_cluster(fixture.cluster_id)
        .unwrap_or_else(|error| panic!("cluster must load: {error}"));
    let chunks = note_repository
        .load_unknown_speaker_chunks(fixture.cluster_id)
        .unwrap_or_else(|error| panic!("chunks must load: {error}"));
    let variants = voice_store
        .search_similar(&VARIANT_EMBEDDING, 5)
        .unwrap_or_else(|error| panic!("variants must load: {error}"));

    assert_eq!(clips.len(), 1);
    assert_eq!(clips[0].derived_embedding(), CLIP_EMBEDDING);
    assert_eq!(
        cluster
            .as_ref()
            .map(|cluster| cluster.representative_embedding()),
        Some(&CLUSTER_EMBEDDING[..])
    );
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].embedding(), CLUSTER_EMBEDDING);
    assert_eq!(variants.len(), 1);
    assert_eq!(variants[0].0.vector(), VARIANT_EMBEDDING);
}

/// Save encrypted embeddings → the file holds only sealed values and loading decrypts them.
#[test]
fn every_embedding_column_is_sealed_on_disk_and_decrypted_on_load() {
    let storage = TemporaryStorage::new();
    let cipher = ephemeral_cipher();
    let note_repository = storage.note_repository(&cipher);
    let voice_store = storage.voice_store(&cipher);

    let fixture = write_fixture(&note_repository, &voice_store);

    for (table, column) in EMBEDDING_COLUMNS {
        let stored = storage.stored_embeddings(table, column);
        assert_eq!(stored.len(), 1, "{table} must hold one row");
        let (embedding_format, stored_bytes) = &stored[0];
        assert_eq!(*embedding_format, 1, "{table} rows must be marked sealed");
        assert_eq!(&stored_bytes[..4], SEALED_EMBEDDING_HEADER, "{table}");
        for plaintext in [&CLIP_EMBEDDING, &CLUSTER_EMBEDDING, &VARIANT_EMBEDDING] {
            assert_ne!(
                stored_bytes,
                &plaintext_bytes(plaintext),
                "{table} holds plaintext"
            );
        }
    }
    assert_fixture_readable(&note_repository, &voice_store, &fixture);
}

/// Old unencrypted database → every embedding is re-encrypted on first open, readable
/// afterwards, and left untouched by later opens.
#[test]
fn old_unencrypted_database_is_reencrypted_on_first_open() {
    let storage = TemporaryStorage::new();
    let fixture = {
        let setup_cipher = ephemeral_cipher();
        write_fixture(
            &storage.note_repository(&setup_cipher),
            &storage.voice_store(&setup_cipher),
        )
    };
    // Rewrite the file into the exact pre-#31 layout: plaintext BLOBs and no format column. The
    // meeting document was sealed with the setup key, so it goes back to clear too (#92).
    let legacy_connection = storage.raw_connection();
    legacy_connection
        .execute("UPDATE meetings SET automerge_document = X'01'", [])
        .unwrap_or_else(|error| panic!("document must be rewritten as legacy: {error}"));
    for (table, column) in EMBEDDING_COLUMNS {
        let plaintext = match table {
            "voice_variants" => plaintext_bytes(&VARIANT_EMBEDDING),
            "unassigned_voice_clips" => plaintext_bytes(&CLIP_EMBEDDING),
            _ => plaintext_bytes(&CLUSTER_EMBEDDING),
        };
        legacy_connection
            .execute(
                &format!("UPDATE {table} SET {column} = ?1"),
                params![plaintext],
            )
            .and_then(|_| {
                legacy_connection
                    .execute_batch(&format!("ALTER TABLE {table} DROP COLUMN embedding_format"))
            })
            .unwrap_or_else(|error| panic!("{table} must be rewritten as legacy: {error}"));
    }
    // Pre-#50 clips also kept their audio as plaintext, without a format column.
    legacy_connection
        .execute(
            "UPDATE unassigned_voice_clips SET audio_bytes = ?1",
            params![vec![9_u8, 9]],
        )
        .and_then(|_| {
            legacy_connection
                .execute_batch("ALTER TABLE unassigned_voice_clips DROP COLUMN audio_format")
        })
        .unwrap_or_else(|error| panic!("clip audio must be rewritten as legacy: {error}"));
    drop(legacy_connection);

    let cipher = storage
        .unlock(None)
        .unwrap_or_else(|error| panic!("key must unlock: {error}"));
    let note_repository = storage.note_repository(&cipher);
    let voice_store = storage.voice_store(&cipher);
    let migrated: Vec<_> = EMBEDDING_COLUMNS
        .iter()
        .map(|(table, column)| storage.stored_embeddings(table, column))
        .collect();
    assert_fixture_readable(&note_repository, &voice_store, &fixture);
    drop((note_repository, voice_store));

    let reopened_cipher = storage
        .unlock(None)
        .unwrap_or_else(|error| panic!("key must unlock again: {error}"));
    let reopened_repository = storage.note_repository(&reopened_cipher);
    let reopened_store = storage.voice_store(&reopened_cipher);
    let after_second_open: Vec<_> = EMBEDDING_COLUMNS
        .iter()
        .map(|(table, column)| storage.stored_embeddings(table, column))
        .collect();

    for ((table, _), rows) in EMBEDDING_COLUMNS.iter().zip(&migrated) {
        assert_eq!(rows.len(), 1, "{table}");
        assert_eq!(
            rows[0].0, 1,
            "{table} must be marked sealed after migration"
        );
        assert_eq!(&rows[0].1[..4], SEALED_EMBEDDING_HEADER, "{table}");
    }
    assert_eq!(
        migrated, after_second_open,
        "a second open must not re-encrypt"
    );
    assert_fixture_readable(&reopened_repository, &reopened_store, &fixture);
}

/// A different key cannot read the embeddings, and opening with it leaves them unchanged.
#[test]
fn a_different_key_cannot_read_stored_embeddings() {
    let storage = TemporaryStorage::new();
    let fixture = {
        let cipher = ephemeral_cipher();
        write_fixture(
            &storage.note_repository(&cipher),
            &storage.voice_store(&cipher),
        )
    };
    let before: Vec<_> = EMBEDDING_COLUMNS
        .iter()
        .map(|(table, column)| storage.stored_embeddings(table, column))
        .collect();

    let other_cipher = ephemeral_cipher();
    let note_repository = storage.note_repository(&other_cipher);
    let voice_store = storage.voice_store(&other_cipher);
    let after: Vec<_> = EMBEDDING_COLUMNS
        .iter()
        .map(|(table, column)| storage.stored_embeddings(table, column))
        .collect();

    assert!(voice_store.search_similar(&VARIANT_EMBEDDING, 5).is_err());
    assert!(
        note_repository
            .get_unassigned_clips(&fixture.meeting_id)
            .is_err()
    );
    assert!(
        note_repository
            .load_unknown_speaker_cluster(fixture.cluster_id)
            .is_err()
    );
    assert_eq!(
        before, after,
        "opening with another key must not rewrite rows"
    );
}

/// Restart with a key file or a passphrase reads the same data; a wrong passphrase is refused.
#[test]
fn restarts_unlock_the_same_key_and_reject_a_wrong_passphrase() {
    for passphrase in [None, Some("correct horse")] {
        let storage = TemporaryStorage::new();
        let fixture = {
            let cipher = storage
                .unlock(passphrase)
                .unwrap_or_else(|error| panic!("first unlock must succeed: {error}"));
            write_fixture(
                &storage.note_repository(&cipher),
                &storage.voice_store(&cipher),
            )
        };

        let cipher = storage
            .unlock(passphrase)
            .unwrap_or_else(|error| panic!("restart unlock must succeed: {error}"));

        assert_fixture_readable(
            &storage.note_repository(&cipher),
            &storage.voice_store(&cipher),
            &fixture,
        );
        if passphrase.is_some() {
            assert!(matches!(
                storage.unlock(Some("battery staple")),
                Err(CryptoError::WrongKey)
            ));
        }
    }
}

/// The metadata record round-trips, is written once, and survives reopening the file.
#[test]
fn encryption_metadata_is_stored_once_and_survives_reopening() {
    let storage = TemporaryStorage::new();
    let passphrase_metadata = EncryptionMetadata::new(
        EncryptionKeySource::Passphrase {
            salt: vec![7; 16],
            parameters: KeyDerivationParameters::new(64, 2, 1),
        },
        vec![1, 2, 3],
    );
    let key_file_metadata = EncryptionMetadata::new(EncryptionKeySource::KeyFile, vec![4, 5]);
    {
        let metadata_store = SqliteEncryptionMetadataStore::open(storage.database_path())
            .unwrap_or_else(|error| panic!("metadata store must open: {error}"));
        assert_eq!(metadata_store.load_encryption_metadata().ok(), Some(None));
        assert_eq!(
            metadata_store
                .save_encryption_metadata(&passphrase_metadata)
                .ok(),
            Some(true)
        );
        assert_eq!(
            metadata_store
                .save_encryption_metadata(&key_file_metadata)
                .ok(),
            Some(false),
            "an existing record is never overwritten"
        );
    }

    let reopened = SqliteEncryptionMetadataStore::open(storage.database_path())
        .unwrap_or_else(|error| panic!("metadata store must reopen: {error}"));
    let in_memory = SqliteEncryptionMetadataStore::open_in_memory()
        .unwrap_or_else(|error| panic!("in-memory metadata store must open: {error}"));
    let _ = in_memory.save_encryption_metadata(&key_file_metadata);

    assert_eq!(
        reopened.load_encryption_metadata().ok(),
        Some(Some(passphrase_metadata))
    );
    assert_eq!(
        in_memory.load_encryption_metadata().ok(),
        Some(Some(key_file_metadata))
    );
}

/// Reads `(audio_format, audio_bytes)` of every review clip, straight from the file.
fn stored_clip_audio(storage: &TemporaryStorage) -> Vec<(i64, Vec<u8>)> {
    let connection = storage.raw_connection();
    let mut statement = connection
        .prepare("SELECT audio_format, audio_bytes FROM unassigned_voice_clips ORDER BY rowid")
        .unwrap_or_else(|error| panic!("clip audio query must prepare: {error}"));
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .and_then(Iterator::collect)
        .unwrap_or_else(|error| panic!("clip audio must load: {error}"))
}

/// Review clips are raw audio (Issue #50): they are sealed on disk like embeddings and come back
/// unchanged through the repository.
#[test]
fn review_clip_audio_is_sealed_on_disk() {
    let storage = TemporaryStorage::new();
    let cipher = ephemeral_cipher();
    let note_repository = storage.note_repository(&cipher);
    let voice_store = storage.voice_store(&cipher);
    let fixture = write_fixture(&note_repository, &voice_store);

    let stored = stored_clip_audio(&storage);

    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].0, 1, "clip audio must be marked sealed");
    assert_ne!(stored[0].1, vec![9, 9], "clip audio is stored in plaintext");
    let clips = note_repository
        .get_unassigned_clips(&fixture.meeting_id)
        .unwrap_or_else(|error| panic!("clips must load: {error}"));
    assert_eq!(clips[0].audio_bytes(), &[9, 9]);
}

/// A database whose clip audio predates Issue #50 is sealed on first open, once.
#[test]
fn old_plaintext_clip_audio_is_sealed_on_first_open() {
    let storage = TemporaryStorage::new();
    let fixture = {
        let setup_cipher = ephemeral_cipher();
        write_fixture(
            &storage.note_repository(&setup_cipher),
            &storage.voice_store(&setup_cipher),
        )
    };
    // Rewrite the clip into the pre-#50 layout: plaintext audio and no format column. Its
    // embedding and the meeting document were sealed with the setup key, so make them plaintext
    // too for the new key.
    let legacy_connection = storage.raw_connection();
    legacy_connection
        .execute("UPDATE meetings SET automerge_document = X'01'", [])
        .unwrap_or_else(|error| panic!("document must be rewritten as legacy: {error}"));
    legacy_connection
        .execute(
            "UPDATE unassigned_voice_clips SET audio_bytes = ?1, derived_embedding = ?2, embedding_format = 0",
            params![vec![9_u8, 9], plaintext_bytes(&CLIP_EMBEDDING)],
        )
        .and_then(|_| {
            legacy_connection
                .execute_batch("ALTER TABLE unassigned_voice_clips DROP COLUMN audio_format")
        })
        .unwrap_or_else(|error| panic!("clip must be rewritten as legacy: {error}"));
    drop(legacy_connection);

    let cipher = storage
        .unlock(None)
        .unwrap_or_else(|error| panic!("key must unlock: {error}"));
    let note_repository = storage.note_repository(&cipher);
    let migrated = stored_clip_audio(&storage);
    let clips = note_repository
        .get_unassigned_clips(&fixture.meeting_id)
        .unwrap_or_else(|error| panic!("clips must load: {error}"));
    drop(note_repository);
    let reopened_repository = storage.note_repository(&cipher);
    let after_second_open = stored_clip_audio(&storage);

    assert_eq!(migrated[0].0, 1);
    assert_ne!(migrated[0].1, vec![9, 9]);
    assert_eq!(clips[0].audio_bytes(), &[9, 9]);
    assert_eq!(
        migrated, after_second_open,
        "a second open must not re-seal"
    );
    drop(reopened_repository);
}
