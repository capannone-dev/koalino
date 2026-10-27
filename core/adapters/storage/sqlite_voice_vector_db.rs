//! Implements the voice-vector port with SQLite-persisted speaker identities and voice variants.
//!
//! `speaker_identities` holds one row per person (UUID, display name, creation and update
//! times); `voice_variants` holds one row per acoustic signature variant with its derived
//! embedding as a little-endian `f32` BLOB. Every variant must belong to a stored identity.
//! Similarity search loads the variants and ranks them by cosine similarity in Rust, which is
//! adequate for a personal voice library of tens to hundreds of variants.
//!
//! The adapter opens its own connection, normally to the same database file as
//! `SqliteNoteRepository`; the tables are independent of the meeting schema, so either adapter
//! may create its tables first. Schema creation is idempotent, so databases created before
//! Issue #30 gain the voice tables on first open without any existing table being altered.
//!
//! Embeddings follow the existing clip and cluster at-rest policy: unencrypted until an
//! AES-256-GCM crate is approved. They are never logged.

use std::{path::Path, time::Duration};

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use super::{
    in_memory_voice_vector_db::{cosine_similarity, validate_vector},
    sqlite_adapter::{decode_embedding, encode_embedding, sqlite_error},
};
use crate::{
    domain::{
        errors::StorageError,
        models::{SpeakerIdentity, VoiceEmbedding},
    },
    ports::voice::VoiceVectorDB,
};

/// How long a write waits for a lock held by the note repository's connection to the same file.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Persists speaker identities and voice variants in SQLite.
pub struct SqliteVoiceVectorDB {
    connection: Connection,
}

impl SqliteVoiceVectorDB {
    /// Opens (or creates) the voice store in the SQLite database at `database_path`.
    ///
    /// Inputs: `database_path` must be a writable SQLite path; it may already hold the meeting
    /// schema.
    /// Outputs: A ready voice store, or a portable storage error.
    /// Side effects: Opens or creates the database file and creates the voice tables when absent.
    pub fn open(database_path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let sqlite_connection = Connection::open(database_path)
            .map_err(|error| sqlite_error("unable to open voice store database", error))?;
        Self::from_connection(sqlite_connection)
    }

    /// Opens an isolated in-memory voice store for tests and ephemeral workflows.
    ///
    /// Inputs: None.
    /// Outputs: A ready voice store, or a portable storage error.
    /// Side effects: Allocates an in-memory SQLite database and creates the voice tables.
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let sqlite_connection = Connection::open_in_memory()
            .map_err(|error| sqlite_error("unable to open in-memory voice store", error))?;
        Self::from_connection(sqlite_connection)
    }

    /// Configures the connection and creates the voice schema idempotently.
    ///
    /// Inputs: An open connection.
    /// Outputs: The voice store, or a portable storage error.
    /// Side effects: Sets a busy timeout and executes `CREATE ... IF NOT EXISTS` DDL.
    fn from_connection(sqlite_connection: Connection) -> Result<Self, StorageError> {
        sqlite_connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(|error| sqlite_error("unable to configure voice store busy timeout", error))?;
        sqlite_connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS speaker_identities (
                    id TEXT PRIMARY KEY NOT NULL,
                    display_name TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS voice_variants (
                    id TEXT PRIMARY KEY NOT NULL,
                    identity_id TEXT NOT NULL,
                    embedding BLOB NOT NULL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY(identity_id) REFERENCES speaker_identities(id)
                );
                CREATE INDEX IF NOT EXISTS voice_variants_identity_index
                    ON voice_variants(identity_id);",
            )
            .map_err(|error| sqlite_error("unable to initialize voice store schema", error))?;
        tracing::info!("SQLite voice store initialized");
        Ok(Self {
            connection: sqlite_connection,
        })
    }
}

impl VoiceVectorDB for SqliteVoiceVectorDB {
    /// Lists every stored identity in creation order.
    ///
    /// Inputs: None.
    /// Outputs: Identities without embeddings, or a portable storage error.
    /// Side effects: Reads `speaker_identities` only.
    fn list_speaker_identities(&self) -> Result<Vec<SpeakerIdentity>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, display_name FROM speaker_identities ORDER BY created_at, rowid")
            .map_err(|error| sqlite_error("unable to prepare identity listing", error))?;
        let identity_rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| sqlite_error("unable to list speaker identities", error))?;
        identity_rows
            .map(|row| {
                row.map_err(|error| sqlite_error("unable to decode speaker identity", error))
                    .and_then(|(identity_id, display_name)| {
                        Ok(SpeakerIdentity::new(
                            parse_uuid(&identity_id)?,
                            display_name,
                        ))
                    })
            })
            .collect()
    }

    /// Creates the identity, or renames it and refreshes `updated_at` when its UUID exists.
    ///
    /// Inputs: `identity` with a non-blank display name.
    /// Outputs: Success, or a rejection for a blank name or a SQLite failure.
    /// Side effects: Upserts one `speaker_identities` row; `created_at` is kept on rename.
    fn save_speaker_identity(&self, identity: &SpeakerIdentity) -> Result<(), StorageError> {
        if identity.display_name().trim().is_empty() {
            return Err(StorageError::Rejected {
                reason: String::from("speaker identity display name must not be blank"),
            });
        }
        let saved_at = Utc::now().to_rfc3339();
        self.connection
            .execute(
                "INSERT INTO speaker_identities (id, display_name, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?3)
                 ON CONFLICT(id) DO UPDATE SET
                    display_name = excluded.display_name,
                    updated_at = excluded.updated_at",
                params![
                    identity.identity_id().to_string(),
                    identity.display_name(),
                    saved_at
                ],
            )
            .map_err(|error| sqlite_error("unable to save speaker identity", error))?;
        tracing::info!(identity_id = %identity.identity_id(), "Speaker identity saved");
        Ok(())
    }

    /// Loads one identity by UUID.
    ///
    /// Inputs: `identity_id` identifies the person.
    /// Outputs: The identity, `None` when absent, or a portable storage error.
    /// Side effects: Reads `speaker_identities` only.
    fn load_speaker_identity(
        &self,
        identity_id: Uuid,
    ) -> Result<Option<SpeakerIdentity>, StorageError> {
        self.connection
            .query_row(
                "SELECT display_name FROM speaker_identities WHERE id = ?1",
                params![identity_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| sqlite_error("unable to load speaker identity", error))
            .map(|display_name| {
                display_name.map(|display_name| SpeakerIdentity::new(identity_id, display_name))
            })
    }

    /// Counts the stored variants of one identity.
    ///
    /// Inputs: `identity_id` identifies the person (an unknown identity has zero variants).
    /// Outputs: The variant count, or a portable storage error.
    /// Side effects: Reads `voice_variants` row counts without reading embeddings.
    fn voice_variant_count(&self, identity_id: Uuid) -> Result<usize, StorageError> {
        let variant_count = self
            .connection
            .query_row(
                "SELECT COUNT(*) FROM voice_variants WHERE identity_id = ?1",
                params![identity_id.to_string()],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| sqlite_error("unable to count voice variants", error))?;
        usize::try_from(variant_count).map_err(|_| StorageError::Rejected {
            reason: String::from("voice variant count is out of range"),
        })
    }

    /// Stores one variant for an already-saved identity.
    ///
    /// Inputs: `embedding` with a non-empty finite vector whose identity is stored.
    /// Outputs: Success, or a rejection for an invalid vector, an unknown identity, a duplicate
    /// variant ID, or a SQLite failure.
    /// Side effects: Writes one biometric BLOB; never logs it.
    fn store_embedding(&self, embedding: VoiceEmbedding) -> Result<(), StorageError> {
        validate_vector(embedding.vector())?;
        let inserted_rows = self
            .connection
            .execute(
                "INSERT INTO voice_variants (id, identity_id, embedding, created_at)
                 SELECT ?1, ?2, ?3, ?4
                 WHERE EXISTS (SELECT 1 FROM speaker_identities WHERE id = ?2)",
                params![
                    embedding.variant_id().to_string(),
                    embedding.identity_id().to_string(),
                    encode_embedding(embedding.vector()),
                    Utc::now().to_rfc3339(),
                ],
            )
            .map_err(|error| sqlite_error("unable to store voice variant", error))?;
        if inserted_rows == 0 {
            return Err(StorageError::Rejected {
                reason: format!(
                    "speaker identity {} does not exist",
                    embedding.identity_id()
                ),
            });
        }
        tracing::info!(
            identity_id = %embedding.identity_id(),
            variant_id = %embedding.variant_id(),
            "Voice variant stored"
        );
        Ok(())
    }

    /// Returns up to `top_k` variants in descending cosine-similarity order.
    ///
    /// Inputs: `vector` must be a non-empty finite query; `top_k` limits the result.
    /// Outputs: Variants with scores (dimension mismatches are skipped), or a storage error.
    /// Side effects: Reads every stored biometric BLOB into memory; never logs them.
    fn search_similar(
        &self,
        vector: &[f32],
        top_k: usize,
    ) -> Result<Vec<(VoiceEmbedding, f32)>, StorageError> {
        validate_vector(vector)?;
        if top_k == 0 {
            return Ok(Vec::new());
        }
        let mut statement = self
            .connection
            .prepare("SELECT id, identity_id, embedding FROM voice_variants ORDER BY rowid")
            .map_err(|error| sqlite_error("unable to prepare voice variant search", error))?;
        let variant_rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            })
            .map_err(|error| sqlite_error("unable to search voice variants", error))?;
        let mut scored_variants = Vec::new();
        for variant_row in variant_rows {
            let (variant_id, identity_id, serialized_embedding) = variant_row
                .map_err(|error| sqlite_error("unable to decode voice variant", error))?;
            let stored_vector = decode_embedding(&serialized_embedding)?;
            if let Some(score) = cosine_similarity(vector, &stored_vector) {
                scored_variants.push((
                    VoiceEmbedding::new(
                        parse_uuid(&identity_id)?,
                        parse_uuid(&variant_id)?,
                        stored_vector,
                    ),
                    score,
                ));
            }
        }
        scored_variants.sort_by(|left, right| right.1.total_cmp(&left.1));
        scored_variants.truncate(top_k);
        Ok(scored_variants)
    }

    /// Deletes an identity's name and every one of its variants in one transaction.
    ///
    /// Inputs: `identity_id` identifies the person; an unknown identity is a no-op.
    /// Outputs: Success or a portable storage error.
    /// Side effects: Irreversibly deletes biometric variants and the identity row. Transcript
    /// segments that referenced the identity keep its UUID and show as unknown afterwards.
    fn delete_identity(&self, identity_id: Uuid) -> Result<(), StorageError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| sqlite_error("unable to begin identity deletion", error))?;
        transaction
            .execute(
                "DELETE FROM voice_variants WHERE identity_id = ?1",
                params![identity_id.to_string()],
            )
            .map_err(|error| sqlite_error("unable to delete voice variants", error))?;
        transaction
            .execute(
                "DELETE FROM speaker_identities WHERE id = ?1",
                params![identity_id.to_string()],
            )
            .map_err(|error| sqlite_error("unable to delete speaker identity", error))?;
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit identity deletion", error))?;
        tracing::info!(identity_id = %identity_id, "Speaker identity deleted");
        Ok(())
    }
}

/// Parses a stored UUID column.
///
/// Inputs: Text read from an adapter-owned UUID column.
/// Outputs: The UUID, or a rejection for a corrupt value.
/// Side effects: None.
fn parse_uuid(stored_value: &str) -> Result<Uuid, StorageError> {
    Uuid::parse_str(stored_value).map_err(|_| StorageError::Rejected {
        reason: String::from("stored voice identifier is not a UUID"),
    })
}
