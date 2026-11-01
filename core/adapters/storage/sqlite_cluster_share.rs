//! Exports and imports the unknown-speaker cluster shares copied between paired devices
//! (Issue #50).
//!
//! A device lists its unresolved clusters with `cluster_digests` (member and clip counts), loads a
//! cluster's share to send it, and imports the shares it receives. An import opens nothing from
//! the sender's database: the share arrives decrypted from the pair-key layer and every embedding
//! and clip is sealed again here with this database's cipher. Imports are idempotent: chunks and
//! clips already stored are kept, and the representative embedding is only replaced by a share
//! holding at least as many members. The exception is the final pass's whole-meeting voice: it
//! replaces the live members, because it records whether the speaker is clean enough to save and a
//! union would lose that, and later live shares no longer change the members. Nothing is stored for a meeting that has not synced yet or a
//! cluster already resolved here (in its row, or in the meeting document when the resolution
//! synced before the share), so a resolved speaker's audio never comes back. This child
//! module of `sqlite_adapter` never logs embeddings or audio.

use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

use super::{
    CLIP_EMBEDDINGS, CLUSTER_CHUNK_EMBEDDINGS, CLUSTER_REPRESENTATIVE_EMBEDDINGS,
    SEALED_AUDIO_FORMAT, SEALED_EMBEDDING_FORMAT, SqliteNoteRepository, read_document,
    seal_clip_audio, sqlite_error, sqlite_milliseconds,
};
use crate::{
    domain::{
        errors::StorageError,
        models::{ClusterDigest, ClusterShare, ClusterShareImport, meeting_voice_chunk_id},
    },
    ports::storage::NoteRepository,
};

/// One clip sealed for this database, ready to insert.
struct SealedClip {
    clip_id: String,
    start_time_ms: i64,
    end_time_ms: i64,
    audio: Vec<u8>,
    embedding: Vec<u8>,
    weak_match_identity_id: Option<String>,
    weak_match_confidence_score: Option<f32>,
}

impl SqliteNoteRepository {
    /// Counts the members and clips of every unresolved cluster that has a member.
    ///
    /// Outputs: Digests in creation order, or a storage error. Side effects: Reads counts only.
    pub(super) fn cluster_digests(&self) -> Result<Vec<ClusterDigest>, StorageError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT cluster_id,
                    (SELECT COUNT(*) FROM cluster_chunks WHERE cluster_chunks.cluster_id = clusters.cluster_id),
                    (SELECT COUNT(*) FROM unassigned_voice_clips WHERE unassigned_voice_clips.cluster_id = clusters.cluster_id),
                    EXISTS (SELECT 1 FROM cluster_chunks WHERE cluster_chunks.cluster_id = clusters.cluster_id
                            AND cluster_chunks.chunk_id = 'meeting-voice-' || clusters.cluster_id)
                 FROM unknown_speaker_clusters AS clusters
                 WHERE resolved_identity_id IS NULL
                   AND EXISTS (SELECT 1 FROM cluster_chunks WHERE cluster_chunks.cluster_id = clusters.cluster_id)
                 ORDER BY rowid",
            )
            .map_err(|error| sqlite_error("unable to prepare the cluster digest query", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, usize>(1)?,
                    row.get::<_, usize>(2)?,
                    row.get::<_, bool>(3)?,
                ))
            })
            .map_err(|error| sqlite_error("unable to list cluster digests", error))?;
        rows.map(|row| {
            let (cluster_id, chunk_count, clip_count, has_final_voice) =
                row.map_err(|error| sqlite_error("unable to read a cluster digest", error))?;
            let cluster_id =
                Uuid::parse_str(&cluster_id).map_err(|error| StorageError::Rejected {
                    reason: format!("stored cluster has an invalid UUID: {error}"),
                })?;
            Ok(ClusterDigest::new(cluster_id, chunk_count, clip_count)
                .with_final_voice(has_final_voice))
        })
        .collect()
    }

    /// Loads one unresolved cluster with every chunk and clip.
    ///
    /// Outputs: The share, `None` for an unknown, resolved, or empty cluster, or a storage error.
    /// Side effects: Reads and decrypts biometric data and audio; never logs them.
    pub(super) fn cluster_share(
        &self,
        cluster_id: Uuid,
    ) -> Result<Option<ClusterShare>, StorageError> {
        let Some(cluster) = self.load_cluster(cluster_id)? else {
            return Ok(None);
        };
        let chunks = self.load_cluster_chunks(cluster_id)?;
        if cluster.is_resolved() || chunks.is_empty() {
            return Ok(None);
        }
        let clips = self
            .get_unassigned_clips(cluster.meeting_id())?
            .into_iter()
            .filter(|clip| clip.cluster_id() == Some(cluster_id))
            .collect();
        Ok(Some(ClusterShare::new(cluster, chunks, clips)))
    }

    /// Stores a share from a paired device, sealed with this database's cipher.
    ///
    /// Inputs: A decoded share whose clips belong to its cluster and meeting.
    /// Outputs: What the import did, or a rejection for clips of another cluster or a cluster
    /// stored here under another meeting, or another storage error.
    /// Side effects: In one transaction, inserts or updates the cluster and inserts new chunks
    /// and clips; writes nothing for a missing meeting or a resolved cluster. A final voice removes
    /// the cluster's live members, and once one is stored only clips are added.
    pub(super) fn import_share(
        &self,
        share: &ClusterShare,
    ) -> Result<ClusterShareImport, StorageError> {
        let cluster = share.cluster();
        let cluster_key = cluster.cluster_id().to_string();
        let meeting_key = cluster.meeting_id().as_str();
        if super::deletions::is_meeting_recorded(&self.connection, cluster.meeting_id())?
            || super::deletions::is_cluster_recorded(&self.connection, cluster.cluster_id())?
        {
            return Ok(ClusterShareImport::Deleted);
        }
        if share.clips().iter().any(|clip| {
            clip.cluster_id() != Some(cluster.cluster_id())
                || clip.meeting_id() != cluster.meeting_id()
        }) {
            return Err(StorageError::Rejected {
                reason: format!("a share of cluster {cluster_key} carries another cluster's clips"),
            });
        }
        let cipher = self.embedding_cipher.as_ref();
        let sealed_representative = CLUSTER_REPRESENTATIVE_EMBEDDINGS.seal(
            cipher,
            &[&cluster_key],
            cluster.representative_embedding(),
        )?;
        let sealed_chunks = share
            .chunks()
            .iter()
            .map(|chunk| {
                Ok((
                    chunk.chunk_id(),
                    CLUSTER_CHUNK_EMBEDDINGS.seal(
                        cipher,
                        &[&cluster_key, chunk.chunk_id()],
                        chunk.embedding(),
                    )?,
                    sqlite_milliseconds(chunk.duration_ms())?,
                ))
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        let sealed_clips = share
            .clips()
            .iter()
            .map(|clip| {
                let clip_id = clip.clip_id().to_string();
                Ok(SealedClip {
                    embedding: CLIP_EMBEDDINGS.seal(
                        cipher,
                        &[&clip_id],
                        clip.derived_embedding(),
                    )?,
                    audio: seal_clip_audio(cipher, &clip_id, clip.audio_bytes())?,
                    start_time_ms: sqlite_milliseconds(clip.start_time_ms())?,
                    end_time_ms: sqlite_milliseconds(clip.end_time_ms())?,
                    weak_match_identity_id: clip
                        .weak_match_candidate()
                        .map(|candidate| candidate.identity_id().to_string()),
                    weak_match_confidence_score: clip
                        .weak_match_candidate()
                        .map(|candidate| candidate.confidence_score()),
                    clip_id,
                })
            })
            .collect::<Result<Vec<_>, StorageError>>()?;

        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| sqlite_error("unable to begin the cluster import", error))?;
        let meeting_exists: bool = transaction
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM meetings WHERE meeting_id = ?1)",
                params![meeting_key],
                |row| row.get(0),
            )
            .map_err(|error| {
                sqlite_error("unable to look up the shared cluster's meeting", error)
            })?;
        if !meeting_exists {
            return Ok(ClusterShareImport::UnknownMeeting);
        }
        // The resolution may have synced before the share: then the cluster is already named.
        if let Some(document_codec) = self.document_codec.as_deref() {
            let resolved_in_document = document_codec
                .project(
                    cluster.meeting_id(),
                    &read_document(&transaction, cluster.meeting_id())?,
                )?
                .cluster_resolutions
                .iter()
                .any(|(resolved_cluster, _)| *resolved_cluster == cluster.cluster_id());
            if resolved_in_document {
                return Ok(ClusterShareImport::AlreadyResolved);
            }
        }
        let stored_cluster: Option<(String, Option<String>)> = transaction
            .query_row(
                "SELECT meeting_id, resolved_identity_id FROM unknown_speaker_clusters WHERE cluster_id = ?1",
                params![cluster_key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| sqlite_error("unable to look up the shared cluster", error))?;
        match &stored_cluster {
            Some((stored_meeting, _)) if stored_meeting != meeting_key => {
                return Err(StorageError::Rejected {
                    reason: format!("cluster {cluster_key} belongs to another meeting here"),
                });
            }
            Some((_, Some(_))) => return Ok(ClusterShareImport::AlreadyResolved),
            _ => {}
        }
        let stored_chunk_count: usize = transaction
            .query_row(
                "SELECT COUNT(*) FROM cluster_chunks WHERE cluster_id = ?1",
                params![cluster_key],
                |row| row.get(0),
            )
            .map_err(|error| sqlite_error("unable to count the shared cluster's chunks", error))?;
        let total_duration_ms = sqlite_milliseconds(cluster.total_duration_ms())?;
        let final_voice_id = meeting_voice_chunk_id(cluster.cluster_id());
        let share_has_final_voice = share
            .chunks()
            .iter()
            .any(|chunk| chunk.chunk_id() == final_voice_id);
        let final_voice_stored: bool = transaction
            .query_row(
                "SELECT EXISTS (SELECT 1 FROM cluster_chunks WHERE cluster_id = ?1 AND chunk_id = ?2)",
                params![cluster_key, final_voice_id],
                |row| row.get(0),
            )
            .map_err(|error| sqlite_error("unable to look up the final voice", error))?;
        let takes_members = share_has_final_voice || !final_voice_stored;
        if share_has_final_voice {
            transaction
                .execute(
                    "DELETE FROM cluster_chunks WHERE cluster_id = ?1",
                    params![cluster_key],
                )
                .map_err(|error| sqlite_error("unable to replace live cluster chunks", error))?;
        }
        if stored_cluster.is_none() {
            transaction
                .execute(
                    "INSERT INTO unknown_speaker_clusters (cluster_id, meeting_id, representative_embedding, total_duration_ms, created_at_utc, resolved_identity_id, embedding_format)
                     VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)",
                    params![
                        cluster_key,
                        meeting_key,
                        sealed_representative,
                        total_duration_ms,
                        cluster.created_at().to_rfc3339(),
                        SEALED_EMBEDDING_FORMAT,
                    ],
                )
                .map_err(|error| sqlite_error("unable to store the shared cluster", error))?;
        } else if share_has_final_voice
            || (takes_members && share.chunks().len() >= stored_chunk_count)
        {
            transaction
                .execute(
                    "UPDATE unknown_speaker_clusters
                     SET representative_embedding = ?1, total_duration_ms = ?2, embedding_format = ?3
                     WHERE cluster_id = ?4",
                    params![
                        sealed_representative,
                        total_duration_ms,
                        SEALED_EMBEDDING_FORMAT,
                        cluster_key
                    ],
                )
                .map_err(|error| sqlite_error("unable to update the shared cluster", error))?;
        }
        let members_to_store = if takes_members {
            sealed_chunks.as_slice()
        } else {
            &[]
        };
        for (chunk_id, sealed_embedding, duration_ms) in members_to_store {
            transaction
                .execute(
                    "INSERT OR IGNORE INTO cluster_chunks (cluster_id, chunk_id, derived_embedding, duration_ms, embedding_format)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        cluster_key,
                        chunk_id,
                        sealed_embedding,
                        duration_ms,
                        SEALED_EMBEDDING_FORMAT
                    ],
                )
                .map_err(|error| sqlite_error("unable to store a shared cluster chunk", error))?;
        }
        for clip in &sealed_clips {
            transaction
                .execute(
                    "INSERT OR IGNORE INTO unassigned_voice_clips (clip_id, meeting_id, start_time_ms, end_time_ms, audio_bytes, derived_embedding, weak_match_identity_id, weak_match_confidence_score, cluster_id, embedding_format, audio_format)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        clip.clip_id,
                        meeting_key,
                        clip.start_time_ms,
                        clip.end_time_ms,
                        clip.audio,
                        clip.embedding,
                        clip.weak_match_identity_id,
                        clip.weak_match_confidence_score,
                        cluster_key,
                        SEALED_EMBEDDING_FORMAT,
                        SEALED_AUDIO_FORMAT,
                    ],
                )
                .map_err(|error| sqlite_error("unable to store a shared review clip", error))?;
        }
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit the cluster import", error))?;
        self.log_operation("import_cluster_share", cluster.meeting_id());
        Ok(ClusterShareImport::Imported)
    }
}
