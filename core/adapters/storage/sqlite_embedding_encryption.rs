//! Seals embedding BLOBs through the cipher port and re-encrypts legacy plaintext rows (#31).
//!
//! Each embedding table carries an `embedding_format` column: `0` marks a plaintext
//! little-endian `f32` BLOB written before Issue #31, `1` a value sealed by `EmbeddingCipher`.
//! Databases from before #31 gain the column with default `0`, so their rows are identified
//! without guessing from the bytes, and `migrate_plaintext_rows` seals them in one transaction
//! when the adapter opens. Reads always go through the cipher: a plaintext value is never
//! accepted. The adapters only call the port; key handling and AES-GCM live in
//! `application::crypto`.

use crate::adapters::storage::sqlite_adapter::WriteTransaction;
use rusqlite::{Connection, params};

use super::sqlite_adapter::{decode_embedding, sqlite_error};
use crate::{domain::errors::StorageError, ports::crypto::EmbeddingCipher};

/// `embedding_format` value of a plaintext embedding written before Issue #31.
const PLAINTEXT_EMBEDDING_FORMAT: i64 = 0;
/// `embedding_format` value of a sealed (AES-256-GCM, format v1) embedding.
pub(super) const SEALED_EMBEDDING_FORMAT: i64 = 1;

/// Describes one adapter-owned table column that stores an embedding.
pub(super) struct EmbeddingColumn {
    /// Table name (trusted, adapter-owned).
    table: &'static str,
    /// BLOB column holding the embedding.
    column: &'static str,
    /// Text columns that identify a row; their values form the sealing context.
    key_columns: &'static [&'static str],
}

/// Identity voice variants (`SqliteVoiceVectorDB`).
pub(super) const VOICE_VARIANT_EMBEDDINGS: EmbeddingColumn = EmbeddingColumn {
    table: "voice_variants",
    column: "embedding",
    key_columns: &["id"],
};
/// Running-mean cluster representatives (`SqliteNoteRepository`).
pub(super) const CLUSTER_REPRESENTATIVE_EMBEDDINGS: EmbeddingColumn = EmbeddingColumn {
    table: "unknown_speaker_clusters",
    column: "representative_embedding",
    key_columns: &["cluster_id"],
};
/// Cluster member embeddings (`SqliteNoteRepository`).
pub(super) const CLUSTER_CHUNK_EMBEDDINGS: EmbeddingColumn = EmbeddingColumn {
    table: "cluster_chunks",
    column: "derived_embedding",
    key_columns: &["cluster_id", "chunk_id"],
};
/// Review-clip embeddings (`SqliteNoteRepository`).
pub(super) const CLIP_EMBEDDINGS: EmbeddingColumn = EmbeddingColumn {
    table: "unassigned_voice_clips",
    column: "derived_embedding",
    key_columns: &["clip_id"],
};

impl EmbeddingColumn {
    /// Builds the sealing context that binds a value to its row.
    ///
    /// Inputs: The row's key values, in `key_columns` order.
    /// Outputs: `table:key1[:key2]`.
    /// Side effects: None.
    fn context(&self, key_values: &[&str]) -> String {
        format!("{}:{}", self.table, key_values.join(":"))
    }

    /// Seals an embedding for the row identified by `key_values`.
    ///
    /// Inputs: The cipher, the row keys, and the embedding.
    /// Outputs: The sealed BLOB, or a portable storage error.
    /// Side effects: Draws a random nonce through the cipher.
    pub(super) fn seal(
        &self,
        cipher: &dyn EmbeddingCipher,
        key_values: &[&str],
        embedding: &[f32],
    ) -> Result<Vec<u8>, StorageError> {
        cipher
            .seal_embedding(&self.context(key_values), embedding)
            .map_err(|error| StorageError::Rejected {
                reason: format!("unable to encrypt {} embedding: {error}", self.table),
            })
    }

    /// Opens the sealed BLOB of the row identified by `key_values`.
    ///
    /// Inputs: The cipher, the row keys, and the stored bytes.
    /// Outputs: The embedding, or a rejection for a wrong key, tampering, or plaintext bytes.
    /// Side effects: None.
    pub(super) fn open(
        &self,
        cipher: &dyn EmbeddingCipher,
        key_values: &[&str],
        sealed: &[u8],
    ) -> Result<Vec<f32>, StorageError> {
        cipher
            .open_embedding(&self.context(key_values), sealed)
            .map_err(|error| StorageError::Rejected {
                reason: format!("unable to decrypt {} embedding: {error}", self.table),
            })
    }

    /// Adds `embedding_format` (default `0`, plaintext) when the table predates Issue #31.
    ///
    /// Inputs: An open connection on which the table exists.
    /// Outputs: Success or a portable storage error.
    /// Side effects: May execute `ALTER TABLE ... ADD COLUMN`.
    pub(super) fn ensure_format_column(
        &self,
        sqlite_connection: &Connection,
    ) -> Result<(), StorageError> {
        let column_exists = sqlite_connection
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM pragma_table_info(?1) WHERE name = 'embedding_format')",
                params![self.table],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|error| sqlite_error("unable to inspect embedding columns", error))?;
        if !column_exists {
            sqlite_connection
                .execute_batch(&format!(
                    "ALTER TABLE {} ADD COLUMN embedding_format INTEGER NOT NULL DEFAULT 0",
                    self.table
                ))
                .map_err(|error| sqlite_error("unable to add embedding format column", error))?;
        }
        Ok(())
    }

    /// Seals every plaintext (format `0`) row in one transaction.
    ///
    /// Only format `0` is migrated: an unknown future format is left untouched (and fails to
    /// read) rather than being mistaken for plaintext and encrypted twice.
    ///
    /// Inputs: An open connection (format column present) and the database cipher.
    /// Outputs: The number of rows re-encrypted, or a storage error; on error nothing changes.
    /// Side effects: Rewrites legacy embedding BLOBs and marks them sealed. A row already sealed
    /// with this key but still marked plaintext is only re-marked, never encrypted twice.
    pub(super) fn migrate_plaintext_rows(
        &self,
        sqlite_connection: &Connection,
        cipher: &dyn EmbeddingCipher,
    ) -> Result<usize, StorageError> {
        let transaction = sqlite_connection
            .write_transaction()
            .map_err(|error| sqlite_error("unable to begin embedding migration", error))?;
        let legacy_rows = {
            let mut statement = transaction
                .prepare(&format!(
                    "SELECT rowid, {}, {} FROM {} WHERE embedding_format = ?1",
                    self.key_columns.join(", "),
                    self.column,
                    self.table
                ))
                .map_err(|error| sqlite_error("unable to prepare embedding migration", error))?;
            let key_column_count = self.key_columns.len();
            statement
                .query_map(params![PLAINTEXT_EMBEDDING_FORMAT], |row| {
                    let key_values = (1..=key_column_count)
                        .map(|column_index| row.get::<_, String>(column_index))
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok((
                        row.get::<_, i64>(0)?,
                        key_values,
                        row.get::<_, Vec<u8>>(key_column_count + 1)?,
                    ))
                })
                .and_then(Iterator::collect::<Result<Vec<_>, _>>)
                .map_err(|error| sqlite_error("unable to read legacy embeddings", error))?
        };
        let mut reencrypted_rows = 0;
        for (row_id, key_values, stored_bytes) in &legacy_rows {
            let key_values: Vec<&str> = key_values.iter().map(String::as_str).collect();
            let sealed_bytes = if self.open(cipher, &key_values, stored_bytes).is_ok() {
                stored_bytes.clone()
            } else {
                reencrypted_rows += 1;
                self.seal(cipher, &key_values, &decode_embedding(stored_bytes)?)?
            };
            transaction
                .execute(
                    &format!(
                        "UPDATE {} SET {} = ?1, embedding_format = ?2 WHERE rowid = ?3",
                        self.table, self.column
                    ),
                    params![sealed_bytes, SEALED_EMBEDDING_FORMAT, row_id],
                )
                .map_err(|error| sqlite_error("unable to store re-encrypted embedding", error))?;
        }
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit embedding migration", error))?;
        if reencrypted_rows > 0 {
            tracing::info!(
                table = self.table,
                reencrypted_rows,
                "Encrypted legacy plaintext embeddings"
            );
        }
        Ok(reencrypted_rows)
    }

    /// Adds the format column if needed and seals any plaintext rows.
    ///
    /// Inputs: An open connection on which the table exists, and the database cipher.
    /// Outputs: Success or a storage error.
    /// Side effects: See `ensure_format_column` and `migrate_plaintext_rows`.
    pub(super) fn prepare(
        &self,
        sqlite_connection: &Connection,
        cipher: &dyn EmbeddingCipher,
    ) -> Result<(), StorageError> {
        self.ensure_format_column(sqlite_connection)?;
        self.migrate_plaintext_rows(sqlite_connection, cipher)
            .map(|_| ())
    }
}
