//! Seals review-clip audio at rest (Issue #50).
//!
//! Review clips are raw audio kept until the owner names the speaker. Like embeddings, their
//! `audio_bytes` are sealed through the `EmbeddingCipher` port (AES-256-GCM), bound to the clip's
//! ID, and marked by an `audio_format` column: `0` is plaintext written before #50, `1` sealed.
//! Older databases gain the column with default `0`, and their rows are sealed once, in one
//! transaction, when the adapter opens. Reads accept only sealed audio. Audio is never logged.

use crate::adapters::storage::sqlite_adapter::WriteTransaction;
use rusqlite::{Connection, params};

use super::sqlite_adapter::sqlite_error;
use crate::{domain::errors::StorageError, ports::crypto::EmbeddingCipher};

/// `audio_format` value of plaintext audio written before Issue #50.
const PLAINTEXT_AUDIO_FORMAT: i64 = 0;
/// `audio_format` value of audio sealed with the database cipher.
pub(super) const SEALED_AUDIO_FORMAT: i64 = 1;

/// The sealing context that binds audio to its clip.
fn context(clip_id: &str) -> String {
    format!("unassigned_voice_clips:audio:{clip_id}")
}

/// Seals one clip's audio.
///
/// Inputs: The database cipher, the clip ID, and the raw audio.
/// Outputs: The sealed bytes, or a storage error. Side effects: Draws a random nonce.
pub(super) fn seal_clip_audio(
    cipher: &dyn EmbeddingCipher,
    clip_id: &str,
    audio_bytes: &[u8],
) -> Result<Vec<u8>, StorageError> {
    cipher
        .seal_secret(&context(clip_id), audio_bytes)
        .map_err(|error| StorageError::Rejected {
            reason: format!("unable to encrypt review clip audio: {error}"),
        })
}

/// Opens one clip's sealed audio.
///
/// Inputs: The database cipher, the clip ID, and the stored bytes.
/// Outputs: The raw audio, or a rejection for a wrong key, tampering, or plaintext bytes.
/// Side effects: None.
pub(super) fn open_clip_audio(
    cipher: &dyn EmbeddingCipher,
    clip_id: &str,
    sealed: &[u8],
) -> Result<Vec<u8>, StorageError> {
    cipher
        .open_secret(&context(clip_id), sealed)
        .map(|audio_bytes| audio_bytes.to_vec())
        .map_err(|error| StorageError::Rejected {
            reason: format!("unable to decrypt review clip audio: {error}"),
        })
}

/// Adds `audio_format` when missing and seals every plaintext clip.
///
/// Inputs: An open connection on which `unassigned_voice_clips` exists, and the cipher.
/// Outputs: The number of clips sealed, or a storage error; on error nothing changes.
/// Side effects: May alter the table; rewrites legacy audio BLOBs in one transaction. Audio that
/// already opens with this key but is still marked plaintext is only re-marked.
pub(super) fn prepare_clip_audio(
    sqlite_connection: &Connection,
    cipher: &dyn EmbeddingCipher,
) -> Result<usize, StorageError> {
    let column_exists = sqlite_connection
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM pragma_table_info('unassigned_voice_clips') WHERE name = 'audio_format')",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|error| sqlite_error("unable to inspect review clip columns", error))?;
    if !column_exists {
        sqlite_connection
            .execute_batch(
                "ALTER TABLE unassigned_voice_clips ADD COLUMN audio_format INTEGER NOT NULL DEFAULT 0",
            )
            .map_err(|error| sqlite_error("unable to add review clip audio format column", error))?;
    }
    let transaction = sqlite_connection
        .write_transaction()
        .map_err(|error| sqlite_error("unable to begin review clip migration", error))?;
    let legacy_clips = {
        let mut statement = transaction
            .prepare(
                "SELECT rowid, clip_id, audio_bytes FROM unassigned_voice_clips WHERE audio_format = ?1",
            )
            .map_err(|error| sqlite_error("unable to prepare review clip migration", error))?;
        statement
            .query_map(params![PLAINTEXT_AUDIO_FORMAT], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            })
            .and_then(Iterator::collect::<Result<Vec<_>, _>>)
            .map_err(|error| sqlite_error("unable to read legacy review clips", error))?
    };
    for (row_id, clip_id, stored_bytes) in &legacy_clips {
        let sealed_bytes = if open_clip_audio(cipher, clip_id, stored_bytes).is_ok() {
            stored_bytes.clone()
        } else {
            seal_clip_audio(cipher, clip_id, stored_bytes)?
        };
        transaction
            .execute(
                "UPDATE unassigned_voice_clips SET audio_bytes = ?1, audio_format = ?2 WHERE rowid = ?3",
                params![sealed_bytes, SEALED_AUDIO_FORMAT, row_id],
            )
            .map_err(|error| sqlite_error("unable to seal legacy review clip", error))?;
    }
    transaction
        .commit()
        .map_err(|error| sqlite_error("unable to commit review clip migration", error))?;
    Ok(legacy_clips.len())
}
