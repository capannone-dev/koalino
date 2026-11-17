//! Persists stable external-participant IDs (e.g. Teams user IDs) mapped to local speaker
//! identities, as part of the SQLite voice store.
//!
//! `external_identity_mappings` holds one row per external ID pointing at a `speaker_identities`
//! row, so a Teams participant keeps one local identity across meetings and restarts, and a
//! rename in the voice library shows up in later lookups. Only IDs and names are stored here,
//! never audio or embeddings.

use rusqlite::{Connection, OptionalExtension, params};

use super::{SqliteVoiceVectorDB, parse_uuid};
use crate::{
    adapters::storage::sqlite_adapter::sqlite_error,
    domain::{
        errors::StorageError,
        models::{SpeakerIdentity, VoiceEmbedding},
    },
    ports::{external_bridge::ExternalIdentityRepository, voice::VoiceVectorDB},
};

/// Creates the mapping table when absent.
///
/// Inputs: An open voice-store connection whose `speaker_identities` table exists.
/// Outputs: Success or a storage error.
/// Side effects: Executes idempotent DDL.
pub(super) fn initialize_external_identity_schema(
    connection: &Connection,
) -> Result<(), StorageError> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS external_identity_mappings (
                external_id TEXT PRIMARY KEY NOT NULL,
                identity_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY(identity_id) REFERENCES speaker_identities(id)
            );",
        )
        .map_err(|error| sqlite_error("unable to initialize external identity schema", error))
}

impl ExternalIdentityRepository for SqliteVoiceVectorDB {
    /// Finds the one saved person with this name, ignoring case and spacing (#134).
    ///
    /// Inputs: A display name. Outputs: The person when exactly one matches, else `None`, or a
    /// storage error. Side effects: Reads SQLite.
    fn find_by_display_name(
        &self,
        display_name: &str,
    ) -> Result<Option<SpeakerIdentity>, StorageError> {
        let wanted = comparable_name(display_name);
        if wanted.is_empty() {
            return Ok(None);
        }
        let mut matches = self
            .list_speaker_identities()?
            .into_iter()
            .filter(|identity| comparable_name(identity.display_name()) == wanted);
        Ok(match (matches.next(), matches.next()) {
            (Some(only), None) => Some(only),
            _ => None,
        })
    }

    /// Looks up the identity mapped to `external_id`, with its current display name.
    ///
    /// Inputs: A provider-stable external ID.
    /// Outputs: The identity, `None` when unseen (or its identity was deleted), or a storage error.
    /// Side effects: Reads SQLite.
    fn find_by_external_id(
        &self,
        external_id: &str,
    ) -> Result<Option<SpeakerIdentity>, StorageError> {
        let stored_row: Option<(String, String)> = self
            .connection
            .query_row(
                "SELECT identities.id, identities.display_name
                 FROM external_identity_mappings AS mappings
                 JOIN speaker_identities AS identities ON identities.id = mappings.identity_id
                 WHERE mappings.external_id = ?1",
                params![external_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| sqlite_error("unable to look up external identity", error))?;
        stored_row
            .map(|(identity_id, display_name)| {
                Ok(SpeakerIdentity::new(
                    parse_uuid(&identity_id)?,
                    display_name,
                ))
            })
            .transpose()
    }

    /// Saves `identity` and maps `external_id` to it.
    ///
    /// Inputs: A non-blank external ID and the identity (a blank name is rejected).
    /// Outputs: Success or a storage rejection.
    /// Side effects: Upserts the `speaker_identities` row and the mapping row.
    fn save_external_mapping(
        &self,
        external_id: &str,
        identity: &SpeakerIdentity,
    ) -> Result<(), StorageError> {
        if external_id.trim().is_empty() {
            return Err(StorageError::Rejected {
                reason: String::from("external participant ID must not be blank"),
            });
        }
        self.save_speaker_identity(identity)?;
        self.connection
            .execute(
                "INSERT INTO external_identity_mappings (external_id, identity_id, created_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(external_id) DO UPDATE SET identity_id = excluded.identity_id",
                params![
                    external_id,
                    identity.identity_id().to_string(),
                    chrono::Utc::now().to_rfc3339()
                ],
            )
            .map_err(|error| sqlite_error("unable to save external identity mapping", error))?;
        tracing::debug!(identity_id = %identity.identity_id(), "External identity mapping saved");
        Ok(())
    }

    /// Stores a participant's voice as a variant of its identity (#103).
    ///
    /// Inputs: The voice variant; its identity must exist.
    /// Outputs: Success or a storage error.
    /// Side effects: Writes one encrypted embedding row; never logs the vector.
    fn store_participant_voice(&self, embedding: VoiceEmbedding) -> Result<(), StorageError> {
        self.store_embedding(embedding)
    }
}

/// A name as compared for matching: lowercase, single-spaced.
fn comparable_name(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
