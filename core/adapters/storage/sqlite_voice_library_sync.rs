//! Exports and merges the voice library for paired devices (Issue #56).
//!
//! The merge is a union with two rules that make every device converge whatever order messages
//! arrive in:
//!
//! - **Names** follow the newest `updated_at`; at the same instant, the greater name wins.
//! - **Deletions always win.** `delete_identity` leaves a row in `voice_identity_tombstones`, which
//!   travels with the library and is kept forever, so a device that has not heard of the deletion
//!   cannot bring a person's biometric data back, however recent its copy looks.
//!
//! Variants never change, so they merge by ID. An import opens nothing from the sender's database:
//! embeddings arrive decrypted from the pair-key layer and are sealed again with this database's
//! cipher. This child module of `sqlite_voice_vector_db` never logs embeddings or names.

use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    SEALED_EMBEDDING_FORMAT, SqliteVoiceVectorDB, VOICE_VARIANT_EMBEDDINGS, parse_uuid,
    sqlite_error, validate_vector,
};
use crate::{
    domain::{
        errors::StorageError,
        models::{IdentityTombstone, SharedSpeakerIdentity, SharedVoiceVariant, VoiceLibrary},
    },
    ports::voice::VoiceLibrarySync,
};

/// Creates the deletion records' table.
///
/// Side effects: Creates `voice_identity_tombstones` when absent.
pub(super) fn initialize_tombstone_schema(connection: &Connection) -> Result<(), StorageError> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS voice_identity_tombstones (
                identity_id TEXT PRIMARY KEY NOT NULL,
                deleted_at TEXT NOT NULL
            );",
        )
        .map_err(|error| sqlite_error("unable to initialize voice deletion records", error))
}

/// Records that an identity was deleted, keeping the earliest time when it already was.
///
/// Outputs: Whether the record is new, or a storage error. Side effects: May insert one row.
pub(super) fn record_tombstone(
    connection: &Connection,
    identity_id: Uuid,
    deleted_at: DateTime<Utc>,
) -> Result<bool, StorageError> {
    let inserted_rows = connection
        .execute(
            "INSERT OR IGNORE INTO voice_identity_tombstones (identity_id, deleted_at)
             VALUES (?1, ?2)",
            params![identity_id.to_string(), deleted_at.to_rfc3339()],
        )
        .map_err(|error| sqlite_error("unable to record a voice deletion", error))?;
    Ok(inserted_rows > 0)
}

/// Parses a stored time.
fn parse_time(stored_value: &str) -> Result<DateTime<Utc>, StorageError> {
    DateTime::parse_from_rfc3339(stored_value)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|_| StorageError::Rejected {
            reason: String::from("a stored voice library time is invalid"),
        })
}

impl VoiceLibrarySync for SqliteVoiceVectorDB {
    /// Hashes the identities, variant IDs, and deletion records.
    ///
    /// Outputs: The digest, equal on devices holding the same library, or a storage error.
    /// Side effects: Reads metadata only; opens no embedding.
    fn voice_library_digest(&self) -> Result<[u8; 32], StorageError> {
        let mut hasher = Sha256::new();
        let mut add_rows = |query: &str, columns: usize| -> Result<(), StorageError> {
            let mut statement = self
                .connection
                .prepare(query)
                .map_err(|error| sqlite_error("unable to prepare the voice digest", error))?;
            let mut rows = statement
                .query([])
                .map_err(|error| sqlite_error("unable to read the voice digest", error))?;
            while let Some(row) = rows
                .next()
                .map_err(|error| sqlite_error("unable to read the voice digest", error))?
            {
                for column in 0..columns {
                    let value: String = row
                        .get(column)
                        .map_err(|error| sqlite_error("unable to read the voice digest", error))?;
                    hasher.update((value.len() as u64).to_be_bytes());
                    hasher.update(value.as_bytes());
                }
            }
            // Separates the tables, so a row cannot pass for one of another table.
            hasher.update([0xFF; 8]);
            Ok(())
        };
        add_rows(
            "SELECT id, display_name, updated_at FROM speaker_identities ORDER BY id",
            3,
        )?;
        add_rows("SELECT id FROM voice_variants ORDER BY id", 1)?;
        add_rows(
            "SELECT identity_id FROM voice_identity_tombstones ORDER BY identity_id",
            1,
        )?;
        Ok(hasher.finalize().into())
    }

    /// Reads the whole library.
    ///
    /// Outputs: Identities, decrypted variants, and deletion records, or a storage error.
    /// Side effects: Decrypts every stored biometric BLOB in memory; never logs them.
    fn export_voice_library(&self) -> Result<VoiceLibrary, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, display_name, created_at, updated_at FROM speaker_identities")
            .map_err(|error| sqlite_error("unable to prepare the identity export", error))?;
        let identities = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|error| sqlite_error("unable to export identities", error))?
            .map(|row| {
                let (identity_id, display_name, created_at, updated_at) =
                    row.map_err(|error| sqlite_error("unable to read an identity", error))?;
                Ok(SharedSpeakerIdentity {
                    identity_id: parse_uuid(&identity_id)?,
                    display_name,
                    created_at: parse_time(&created_at)?,
                    updated_at: parse_time(&updated_at)?,
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        let mut statement = self
            .connection
            .prepare("SELECT id, identity_id, embedding, created_at FROM voice_variants")
            .map_err(|error| sqlite_error("unable to prepare the variant export", error))?;
        let variants = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|error| sqlite_error("unable to export voice variants", error))?
            .map(|row| {
                let (variant_id, identity_id, sealed_embedding, created_at) =
                    row.map_err(|error| sqlite_error("unable to read a voice variant", error))?;
                Ok(SharedVoiceVariant {
                    vector: VOICE_VARIANT_EMBEDDINGS.open(
                        self.embedding_cipher.as_ref(),
                        &[&variant_id],
                        &sealed_embedding,
                    )?,
                    identity_id: parse_uuid(&identity_id)?,
                    variant_id: parse_uuid(&variant_id)?,
                    created_at: parse_time(&created_at)?,
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        let mut statement = self
            .connection
            .prepare("SELECT identity_id, deleted_at FROM voice_identity_tombstones")
            .map_err(|error| sqlite_error("unable to prepare the deletion export", error))?;
        let tombstones = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| sqlite_error("unable to export voice deletions", error))?
            .map(|row| {
                let (identity_id, deleted_at) =
                    row.map_err(|error| sqlite_error("unable to read a voice deletion", error))?;
                Ok(IdentityTombstone {
                    identity_id: parse_uuid(&identity_id)?,
                    deleted_at: parse_time(&deleted_at)?,
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        Ok(VoiceLibrary {
            identities,
            variants,
            tombstones,
        })
    }

    /// Merges a library from a paired device, in one transaction.
    ///
    /// Inputs: A decoded library. Outputs: Whether anything changed, or a rejection for a blank
    /// name or an invalid vector (then nothing is stored), or another storage error.
    /// Side effects: Applies deletions (removing biometric data), upserts identities by the rules
    /// above, and seals and inserts new variants. Variants of an identity unknown here are skipped.
    fn import_voice_library(&self, library: &VoiceLibrary) -> Result<bool, StorageError> {
        if library
            .identities
            .iter()
            .any(|identity| identity.display_name.trim().is_empty())
        {
            return Err(StorageError::Rejected {
                reason: String::from("speaker identity display name must not be blank"),
            });
        }
        let cipher = self.embedding_cipher.as_ref();
        let sealed_variants = library
            .variants
            .iter()
            .map(|variant| {
                validate_vector(&variant.vector)?;
                let variant_id = variant.variant_id.to_string();
                let sealed =
                    VOICE_VARIANT_EMBEDDINGS.seal(cipher, &[&variant_id], &variant.vector)?;
                Ok((variant, variant_id, sealed))
            })
            .collect::<Result<Vec<_>, StorageError>>()?;

        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| sqlite_error("unable to begin the voice merge", error))?;
        let mut changed = false;
        for tombstone in &library.tombstones {
            let identity_key = tombstone.identity_id.to_string();
            changed |= record_tombstone(&transaction, tombstone.identity_id, tombstone.deleted_at)?;
            let removed_variants = transaction
                .execute(
                    "DELETE FROM voice_variants WHERE identity_id = ?1",
                    params![identity_key],
                )
                .map_err(|error| sqlite_error("unable to delete voice variants", error))?;
            transaction
                .execute(
                    "DELETE FROM external_identity_mappings WHERE identity_id = ?1",
                    params![identity_key],
                )
                .map_err(|error| sqlite_error("unable to delete external links", error))?;
            let removed_identities = transaction
                .execute(
                    "DELETE FROM speaker_identities WHERE id = ?1",
                    params![identity_key],
                )
                .map_err(|error| sqlite_error("unable to delete a speaker identity", error))?;
            changed |= removed_variants + removed_identities > 0;
        }
        for identity in &library.identities {
            let identity_key = identity.identity_id.to_string();
            let updated_at = identity.updated_at.to_rfc3339();
            // Times are compared as instants: SQLite would compare the text, whose fractional
            // digits vary in length.
            let stored: Option<(String, String)> = {
                let mut statement = transaction
                    .prepare(
                        "SELECT display_name, updated_at FROM speaker_identities WHERE id = ?1",
                    )
                    .map_err(|error| sqlite_error("unable to prepare the identity merge", error))?;
                let mut rows = statement
                    .query(params![identity_key])
                    .map_err(|error| sqlite_error("unable to read an identity", error))?;
                rows.next()
                    .map_err(|error| sqlite_error("unable to read an identity", error))?
                    .map(|row| Ok::<_, rusqlite::Error>((row.get(0)?, row.get(1)?)))
                    .transpose()
                    .map_err(|error| sqlite_error("unable to read an identity", error))?
            };
            let incoming_wins = match &stored {
                None => true,
                Some((stored_name, stored_updated_at)) => {
                    (identity.updated_at, identity.display_name.as_str())
                        > (parse_time(stored_updated_at)?, stored_name.as_str())
                }
            };
            if !incoming_wins {
                continue;
            }
            changed |= transaction
                .execute(
                    "INSERT INTO speaker_identities (id, display_name, created_at, updated_at)
                     SELECT ?1, ?2, ?3, ?4
                     WHERE NOT EXISTS (
                        SELECT 1 FROM voice_identity_tombstones WHERE identity_id = ?1
                     )
                     ON CONFLICT(id) DO UPDATE SET
                        display_name = excluded.display_name,
                        updated_at = excluded.updated_at",
                    params![
                        identity_key,
                        identity.display_name,
                        identity.created_at.to_rfc3339(),
                        updated_at
                    ],
                )
                .map_err(|error| sqlite_error("unable to merge a speaker identity", error))?
                > 0;
        }
        for (variant, variant_id, sealed) in &sealed_variants {
            changed |= transaction
                .execute(
                    "INSERT OR IGNORE INTO voice_variants (id, identity_id, embedding, embedding_format, created_at)
                     SELECT ?1, ?2, ?3, ?4, ?5
                     WHERE EXISTS (SELECT 1 FROM speaker_identities WHERE id = ?2)",
                    params![
                        variant_id,
                        variant.identity_id.to_string(),
                        sealed,
                        SEALED_EMBEDDING_FORMAT,
                        variant.created_at.to_rfc3339(),
                    ],
                )
                .map_err(|error| sqlite_error("unable to merge a voice variant", error))?
                > 0;
        }
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit the voice merge", error))?;
        if changed {
            tracing::info!("Voice library merged from a paired device");
        }
        Ok(changed)
    }
}
