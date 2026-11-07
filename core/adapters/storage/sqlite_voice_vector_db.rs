//! Implements the voice-vector port with SQLite-persisted speaker identities and voice variants.
//!
//! `speaker_identities` holds one row per person (UUID, display name, creation and update
//! times); `voice_variants` holds one row per acoustic signature variant with its derived
//! embedding sealed by the injected `EmbeddingCipher` (AES-256-GCM, bound to the variant ID). Every variant must belong to a stored identity.
//! Similarity search loads the variants and ranks them by cosine similarity in Rust, which is
//! adequate for a personal voice library of tens to hundreds of variants.
//!
//! The adapter opens its own connection, normally to the same database file as
//! `SqliteNoteRepository`; the tables are independent of the meeting schema, so either adapter
//! may create its tables first. Schema creation is idempotent, so databases created before
//! Issue #30 gain the voice tables on first open without any existing table being altered.
//!
//! `external_identity_mappings` (Issue #33) maps stable external-participant IDs, such as Teams
//! user IDs, to identities; see `sqlite_external_identities.rs`.
//!
//! `voice_identity_tombstones` and the merge between paired devices (Issue #56) are in
//! `sqlite_voice_library_sync.rs`.
//!
//! Embeddings are encrypted at rest (Issue #31) and never logged. Opening the store re-encrypts
//! any plaintext variant written before #31 (see `sqlite_embedding_encryption`).

use crate::adapters::storage::sqlite_adapter::WriteTransaction;
use std::{path::Path, sync::Arc, time::Duration};

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

/// Maps external-participant IDs (Teams) to stored identities (Issue #33).
#[path = "sqlite_external_identities.rs"]
mod external_identities;
/// Exports and merges the voice library for paired devices (Issue #56).
#[path = "sqlite_voice_library_sync.rs"]
mod voice_library_sync;

use super::{
    in_memory_voice_vector_db::{cosine_similarity, validate_vector},
    sqlite_adapter::sqlite_error,
    sqlite_embedding_encryption::{SEALED_EMBEDDING_FORMAT, VOICE_VARIANT_EMBEDDINGS},
};
use crate::{
    domain::{
        errors::StorageError,
        models::{SpeakerIdentity, VoiceEmbedding},
    },
    ports::{crypto::EmbeddingCipher, voice::VoiceVectorDB},
};

/// How long a write waits for a lock held by the note repository's connection to the same file.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// Persists speaker identities and voice variants in SQLite.
pub struct SqliteVoiceVectorDB {
    connection: Connection,
    /// Seals and opens every stored variant embedding.
    embedding_cipher: Arc<dyn EmbeddingCipher>,
}

impl SqliteVoiceVectorDB {
    /// Opens (or creates) the voice store in the SQLite database at `database_path`.
    ///
    /// Inputs: `database_path` must be a writable SQLite path; it may already hold the meeting
    /// schema. `embedding_cipher` holds the database key unlocked by `KeyManager`.
    /// Outputs: A ready voice store, or a portable storage error.
    /// Side effects: Opens or creates the database file, creates the voice tables when absent,
    /// and re-encrypts plaintext variants left by databases from before Issue #31.
    pub fn open(
        database_path: impl AsRef<Path>,
        embedding_cipher: Arc<dyn EmbeddingCipher>,
    ) -> Result<Self, StorageError> {
        let sqlite_connection = Connection::open(database_path)
            .map_err(|error| sqlite_error("unable to open voice store database", error))?;
        Self::from_connection(sqlite_connection, embedding_cipher)
    }

    /// Opens an isolated in-memory voice store for tests and ephemeral workflows.
    ///
    /// Inputs: The cipher for stored variants (typically an ephemeral one).
    /// Outputs: A ready voice store, or a portable storage error.
    /// Side effects: Allocates an in-memory SQLite database and creates the voice tables.
    pub fn open_in_memory(
        embedding_cipher: Arc<dyn EmbeddingCipher>,
    ) -> Result<Self, StorageError> {
        let sqlite_connection = Connection::open_in_memory()
            .map_err(|error| sqlite_error("unable to open in-memory voice store", error))?;
        Self::from_connection(sqlite_connection, embedding_cipher)
    }

    /// Reads this connection's `secure_delete` setting, for tests.
    #[cfg(test)]
    pub(super) fn secure_delete_setting(&self) -> Option<i64> {
        self.connection
            .query_row("PRAGMA secure_delete", [], |row| row.get(0))
            .ok()
    }

    /// Configures the connection, creates the voice schema idempotently, and encrypts legacy rows.
    ///
    /// Inputs: An open connection and the embedding cipher.
    /// Outputs: The voice store, or a portable storage error.
    /// Side effects: Sets a busy timeout, executes `CREATE ... IF NOT EXISTS` DDL, adds the
    /// `embedding_format` column when missing, and seals plaintext variants in one transaction.
    fn from_connection(
        sqlite_connection: Connection,
        embedding_cipher: Arc<dyn EmbeddingCipher>,
    ) -> Result<Self, StorageError> {
        super::sqlite_secure_delete::enable(&sqlite_connection)?;
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
        external_identities::initialize_external_identity_schema(&sqlite_connection)?;
        voice_library_sync::initialize_tombstone_schema(&sqlite_connection)?;
        VOICE_VARIANT_EMBEDDINGS.prepare(&sqlite_connection, embedding_cipher.as_ref())?;
        tracing::info!("SQLite voice store initialized");
        Ok(Self {
            connection: sqlite_connection,
            embedding_cipher,
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
    /// Side effects: Encrypts and writes one biometric BLOB; never logs it.
    fn store_embedding(&self, embedding: VoiceEmbedding) -> Result<(), StorageError> {
        validate_vector(embedding.vector())?;
        let variant_id = embedding.variant_id().to_string();
        let sealed_embedding = VOICE_VARIANT_EMBEDDINGS.seal(
            self.embedding_cipher.as_ref(),
            &[&variant_id],
            embedding.vector(),
        )?;
        let inserted_rows = self
            .connection
            .execute(
                "INSERT INTO voice_variants (id, identity_id, embedding, embedding_format, created_at)
                 SELECT ?1, ?2, ?3, ?4, ?5
                 WHERE EXISTS (SELECT 1 FROM speaker_identities WHERE id = ?2)",
                params![
                    variant_id,
                    embedding.identity_id().to_string(),
                    sealed_embedding,
                    SEALED_EMBEDDING_FORMAT,
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
    /// Side effects: Reads and decrypts every stored biometric BLOB in memory; never logs them.
    /// A value that fails to decrypt (wrong key, tampering) fails the whole search.
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
            let stored_vector = VOICE_VARIANT_EMBEDDINGS.open(
                self.embedding_cipher.as_ref(),
                &[&variant_id],
                &serialized_embedding,
            )?;
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
    /// Side effects: Irreversibly deletes biometric variants, external account links and the identity row, and leaves a
    /// deletion record so paired devices delete it too and cannot bring it back. Transcript
    /// segments that referenced the identity keep its UUID and show as unknown afterwards.
    fn delete_identity(&self, identity_id: Uuid) -> Result<(), StorageError> {
        let transaction = self
            .connection
            .write_transaction()
            .map_err(|error| sqlite_error("unable to begin identity deletion", error))?;
        transaction
            .execute(
                "DELETE FROM voice_variants WHERE identity_id = ?1",
                params![identity_id.to_string()],
            )
            .map_err(|error| sqlite_error("unable to delete voice variants", error))?;
        transaction
            .execute(
                "DELETE FROM external_identity_mappings WHERE identity_id = ?1",
                params![identity_id.to_string()],
            )
            .map_err(|error| sqlite_error("unable to delete external links", error))?;
        transaction
            .execute(
                "DELETE FROM speaker_identities WHERE id = ?1",
                params![identity_id.to_string()],
            )
            .map_err(|error| sqlite_error("unable to delete speaker identity", error))?;
        voice_library_sync::record_tombstone(&transaction, identity_id, Utc::now())?;
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
