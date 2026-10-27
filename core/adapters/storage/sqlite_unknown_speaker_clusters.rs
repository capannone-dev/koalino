//! Persists unknown-speaker clusters for the SQLite note repository (Issue #28).
//!
//! `unknown_speaker_clusters` holds one row per cluster (representative embedding, running
//! duration, creation time, and the assigned identity once resolved); `cluster_chunks` holds one
//! row per member chunk with its own embedding so every member can be enrolled on assignment.
//! Embeddings are little-endian `f32` BLOBs, never logged. This child module of `sqlite_adapter`
//! also migrates pre-#28 databases by adding the clip and segment cluster-link columns.

use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use super::{
    SqliteNoteRepository, decode_embedding, encode_embedding, sqlite_error, sqlite_milliseconds,
};
use crate::domain::{
    errors::StorageError,
    models::{MeetingId, UnknownSpeakerChunk, UnknownSpeakerCluster},
};

/// Creates the cluster tables and adds cluster-link columns missing from older databases.
///
/// Inputs: An open connection on which the base meeting schema already exists.
/// Outputs: Success or a portable storage error.
/// Side effects: Executes idempotent DDL, including `ALTER TABLE` for legacy databases.
pub(super) fn initialize_cluster_schema(
    sqlite_connection: &Connection,
) -> Result<(), StorageError> {
    sqlite_connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS unknown_speaker_clusters (
                cluster_id TEXT PRIMARY KEY NOT NULL,
                meeting_id TEXT NOT NULL,
                representative_embedding BLOB NOT NULL,
                total_duration_ms INTEGER NOT NULL,
                created_at_utc TEXT NOT NULL,
                resolved_identity_id TEXT,
                FOREIGN KEY(meeting_id) REFERENCES meetings(meeting_id)
            );
            CREATE INDEX IF NOT EXISTS unknown_speaker_clusters_meeting_index
                ON unknown_speaker_clusters(meeting_id);
            CREATE TABLE IF NOT EXISTS cluster_chunks (
                cluster_id TEXT NOT NULL,
                chunk_id TEXT NOT NULL,
                derived_embedding BLOB NOT NULL,
                duration_ms INTEGER NOT NULL,
                PRIMARY KEY(cluster_id, chunk_id),
                FOREIGN KEY(cluster_id) REFERENCES unknown_speaker_clusters(cluster_id)
            );",
        )
        .map_err(|error| {
            sqlite_error("unable to initialize unknown-speaker cluster schema", error)
        })?;
    add_missing_column(sqlite_connection, "unassigned_voice_clips", "cluster_id")?;
    add_missing_column(
        sqlite_connection,
        "transcript_segments",
        "unknown_cluster_id",
    )
}

/// Adds a nullable TEXT column when an older database lacks it.
///
/// Inputs: Trusted, adapter-owned table and column names (never user input).
/// Outputs: Success or a portable storage error.
/// Side effects: May execute `ALTER TABLE ... ADD COLUMN`.
fn add_missing_column(
    sqlite_connection: &Connection,
    table_name: &str,
    column_name: &str,
) -> Result<(), StorageError> {
    let column_exists = sqlite_connection
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM pragma_table_info(?1) WHERE name = ?2)",
            params![table_name, column_name],
            |row| row.get::<_, bool>(0),
        )
        .map_err(|error| sqlite_error("unable to inspect table columns", error))?;
    if !column_exists {
        sqlite_connection
            .execute_batch(&format!(
                "ALTER TABLE {table_name} ADD COLUMN {column_name} TEXT"
            ))
            .map_err(|error| sqlite_error("unable to migrate cluster link column", error))?;
    }
    Ok(())
}

impl SqliteNoteRepository {
    /// Upserts a cluster row and inserts one member row in a single transaction.
    ///
    /// Inputs: `cluster` belongs to an existing meeting (and to the same meeting as any stored
    /// row with its ID); `chunk` is a new member.
    /// Outputs: Success, or a rejection for an unknown meeting, a meeting mismatch, a duplicate
    /// member, or another SQLite failure.
    /// Side effects: Writes biometric BLOBs; never logs them.
    pub(super) fn save_cluster_chunk(
        &self,
        cluster: &UnknownSpeakerCluster,
        chunk: &UnknownSpeakerChunk,
    ) -> Result<(), StorageError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| sqlite_error("unable to begin cluster transaction", error))?;
        let changed_cluster_rows = transaction
            .execute(
                "INSERT INTO unknown_speaker_clusters (cluster_id, meeting_id, representative_embedding, total_duration_ms, created_at_utc, resolved_identity_id)
                 SELECT ?1, ?2, ?3, ?4, ?5, ?6 WHERE EXISTS (SELECT 1 FROM meetings WHERE meeting_id = ?2)
                 ON CONFLICT(cluster_id) DO UPDATE SET
                    representative_embedding = excluded.representative_embedding,
                    total_duration_ms = excluded.total_duration_ms,
                    resolved_identity_id = excluded.resolved_identity_id
                 WHERE unknown_speaker_clusters.meeting_id = excluded.meeting_id",
                params![
                    cluster.cluster_id().to_string(),
                    cluster.meeting_id().as_str(),
                    encode_embedding(cluster.representative_embedding()),
                    sqlite_milliseconds(cluster.total_duration_ms())?,
                    cluster.created_at().to_rfc3339(),
                    cluster.resolved_identity_id().map(|identity_id| identity_id.to_string()),
                ],
            )
            .map_err(|error| sqlite_error("unable to save unknown-speaker cluster", error))?;
        if changed_cluster_rows == 0 {
            return Err(StorageError::Rejected {
                reason: format!(
                    "cannot save cluster for unknown or different meeting {}",
                    cluster.meeting_id().as_str()
                ),
            });
        }
        transaction
            .execute(
                "INSERT INTO cluster_chunks (cluster_id, chunk_id, derived_embedding, duration_ms) VALUES (?1, ?2, ?3, ?4)",
                params![
                    cluster.cluster_id().to_string(),
                    chunk.chunk_id(),
                    encode_embedding(chunk.embedding()),
                    sqlite_milliseconds(chunk.duration_ms())?,
                ],
            )
            .map_err(|error| sqlite_error("unable to save unknown-speaker cluster chunk", error))?;
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit unknown-speaker cluster", error))?;
        self.log_operation("save_unknown_speaker_cluster_chunk", cluster.meeting_id());
        Ok(())
    }

    /// Loads a meeting's unresolved clusters with at least one member, in creation order.
    ///
    /// Inputs: `meeting_id` identifies the meeting.
    /// Outputs: Clusters or a portable storage error.
    /// Side effects: Reads biometric BLOBs; never logs them.
    pub(super) fn load_clusters_for_meeting(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Vec<UnknownSpeakerCluster>, StorageError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT cluster_id, meeting_id, representative_embedding, total_duration_ms, created_at_utc, resolved_identity_id
                 FROM unknown_speaker_clusters AS clusters
                 WHERE meeting_id = ?1 AND resolved_identity_id IS NULL
                   AND EXISTS (SELECT 1 FROM cluster_chunks WHERE cluster_chunks.cluster_id = clusters.cluster_id)
                 ORDER BY rowid",
            )
            .map_err(|error| sqlite_error("unable to prepare cluster query", error))?;
        let cluster_rows = statement
            .query_map(params![meeting_id.as_str()], read_cluster_row)
            .map_err(|error| sqlite_error("unable to load unknown-speaker clusters", error))?;
        let clusters = cluster_rows
            .map(|row| {
                row.map_err(|error| sqlite_error("unable to decode unknown-speaker cluster", error))
                    .and_then(|cluster_row| self.build_cluster(cluster_row))
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        self.log_operation("load_unknown_speaker_clusters", meeting_id);
        Ok(clusters)
    }

    /// Loads one cluster by ID regardless of resolution or membership.
    ///
    /// Inputs: `cluster_id` identifies the cluster.
    /// Outputs: The cluster, `None` when absent, or a portable storage error.
    /// Side effects: Reads biometric BLOBs; never logs them.
    pub(super) fn load_cluster(
        &self,
        cluster_id: Uuid,
    ) -> Result<Option<UnknownSpeakerCluster>, StorageError> {
        self.connection
            .query_row(
                "SELECT cluster_id, meeting_id, representative_embedding, total_duration_ms, created_at_utc, resolved_identity_id
                 FROM unknown_speaker_clusters WHERE cluster_id = ?1",
                params![cluster_id.to_string()],
                read_cluster_row,
            )
            .optional()
            .map_err(|error| sqlite_error("unable to load unknown-speaker cluster", error))?
            .map(|cluster_row| self.build_cluster(cluster_row))
            .transpose()
    }

    /// Loads every member of one cluster in insertion order.
    ///
    /// Inputs: `cluster_id` identifies the cluster.
    /// Outputs: Members (empty for an unknown cluster) or a portable storage error.
    /// Side effects: Reads biometric BLOBs; never logs them.
    pub(super) fn load_cluster_chunks(
        &self,
        cluster_id: Uuid,
    ) -> Result<Vec<UnknownSpeakerChunk>, StorageError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT chunk_id, derived_embedding, duration_ms FROM cluster_chunks WHERE cluster_id = ?1 ORDER BY rowid",
            )
            .map_err(|error| sqlite_error("unable to prepare cluster chunk query", error))?;
        let chunk_rows = statement
            .query_map(params![cluster_id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(|error| sqlite_error("unable to load cluster chunks", error))?;
        chunk_rows
            .map(|row| {
                let (chunk_id, serialized_embedding, duration_ms) =
                    row.map_err(|error| sqlite_error("unable to decode cluster chunk", error))?;
                Ok(UnknownSpeakerChunk::new(
                    chunk_id,
                    decode_embedding(&serialized_embedding)?,
                    stored_milliseconds(duration_ms)?,
                ))
            })
            .collect()
    }

    /// Marks a cluster resolved and deletes its review clips in one transaction.
    ///
    /// Inputs: `cluster_id` must exist; `identity_id` is the assigned identity.
    /// Outputs: Success, a rejection for an unknown cluster, or another SQLite failure.
    /// Side effects: Updates one cluster row and deletes the cluster's temporary review audio.
    pub(super) fn resolve_cluster(
        &self,
        cluster_id: Uuid,
        identity_id: Uuid,
    ) -> Result<(), StorageError> {
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| sqlite_error("unable to begin cluster resolution", error))?;
        let updated_rows = transaction
            .execute(
                "UPDATE unknown_speaker_clusters SET resolved_identity_id = ?1 WHERE cluster_id = ?2",
                params![identity_id.to_string(), cluster_id.to_string()],
            )
            .map_err(|error| sqlite_error("unable to resolve unknown-speaker cluster", error))?;
        if updated_rows == 0 {
            return Err(StorageError::Rejected {
                reason: format!("unknown-speaker cluster {cluster_id} does not exist"),
            });
        }
        let deleted_clip_count = transaction
            .execute(
                "DELETE FROM unassigned_voice_clips WHERE cluster_id = ?1",
                params![cluster_id.to_string()],
            )
            .map_err(|error| sqlite_error("unable to delete resolved cluster clips", error))?;
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit cluster resolution", error))?;
        tracing::debug!(
            device_id = %self.device_id.as_str(),
            cluster_id = %cluster_id,
            deleted_clip_count,
            "Unknown-speaker cluster resolved and review audio deleted"
        );
        Ok(())
    }

    /// Converts a raw cluster row into a domain value, loading its member chunk IDs.
    ///
    /// Inputs: One decoded `unknown_speaker_clusters` row.
    /// Outputs: The cluster, or a rejection for malformed stored values.
    /// Side effects: Reads the cluster's member IDs.
    fn build_cluster(
        &self,
        cluster_row: ClusterRow,
    ) -> Result<UnknownSpeakerCluster, StorageError> {
        let cluster_id = parse_stored_uuid(&cluster_row.cluster_id, "cluster")?;
        let resolved_identity_id = cluster_row
            .resolved_identity_id
            .as_deref()
            .map(|identity_id| parse_stored_uuid(identity_id, "resolved identity"))
            .transpose()?;
        let created_at = chrono::DateTime::parse_from_rfc3339(&cluster_row.created_at_utc)
            .map_err(|error| StorageError::Rejected {
                reason: format!("stored cluster has an invalid creation time: {error}"),
            })?
            .with_timezone(&chrono::Utc);
        let chunk_ids = self
            .load_cluster_chunks(cluster_id)?
            .into_iter()
            .map(|chunk| chunk.chunk_id().to_owned())
            .collect();
        Ok(UnknownSpeakerCluster::new(
            cluster_id,
            MeetingId::new(cluster_row.meeting_id),
            decode_embedding(&cluster_row.representative_embedding)?,
            chunk_ids,
            stored_milliseconds(cluster_row.total_duration_ms)?,
            created_at,
            resolved_identity_id,
        ))
    }
}

/// Raw `unknown_speaker_clusters` columns before domain validation.
struct ClusterRow {
    cluster_id: String,
    meeting_id: String,
    representative_embedding: Vec<u8>,
    total_duration_ms: i64,
    created_at_utc: String,
    resolved_identity_id: Option<String>,
}

/// Reads one cluster row in the column order used by every cluster query.
fn read_cluster_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClusterRow> {
    Ok(ClusterRow {
        cluster_id: row.get(0)?,
        meeting_id: row.get(1)?,
        representative_embedding: row.get(2)?,
        total_duration_ms: row.get(3)?,
        created_at_utc: row.get(4)?,
        resolved_identity_id: row.get(5)?,
    })
}

/// Parses a stored UUID, naming the field in the rejection.
fn parse_stored_uuid(stored_value: &str, field_name: &str) -> Result<Uuid, StorageError> {
    Uuid::parse_str(stored_value).map_err(|error| StorageError::Rejected {
        reason: format!("stored {field_name} has an invalid UUID: {error}"),
    })
}

/// Converts a stored SQLite integer back to milliseconds, rejecting negative values.
fn stored_milliseconds(stored_value: i64) -> Result<u64, StorageError> {
    u64::try_from(stored_value).map_err(|_| StorageError::Rejected {
        reason: String::from("stored duration is negative"),
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::{Connection, params};
    use uuid::Uuid;

    use crate::{
        adapters::storage::sqlite_adapter::SqliteNoteRepository,
        domain::{
            errors::StorageError,
            models::{
                DeviceId, MeetingId, TranscriptSegment, UnassignedVoiceClip, UnknownSpeakerChunk,
                UnknownSpeakerCluster,
            },
        },
        ports::storage::NoteRepository,
    };

    /// Opens an in-memory repository with one meeting.
    fn repository_with_meeting(meeting_id: &MeetingId) -> SqliteNoteRepository {
        let repository = SqliteNoteRepository::open_in_memory()
            .unwrap_or_else(|error| panic!("in-memory SQLite should initialize: {error}"));
        repository
            .create_meeting(meeting_id, &[1_u8])
            .unwrap_or_else(|error| panic!("meeting creation should succeed: {error}"));
        repository
    }

    /// Builds an unresolved cluster value for persistence tests.
    fn cluster(
        cluster_id: Uuid,
        meeting_id: &MeetingId,
        representative_embedding: Vec<f32>,
        chunk_ids: &[&str],
        total_duration_ms: u64,
    ) -> UnknownSpeakerCluster {
        UnknownSpeakerCluster::new(
            cluster_id,
            meeting_id.clone(),
            representative_embedding,
            chunk_ids
                .iter()
                .map(|chunk_id| (*chunk_id).to_owned())
                .collect(),
            total_duration_ms,
            chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap_or_default(),
            None,
        )
    }

    /// Verifies clusters, their running totals, and member embeddings round-trip through SQLite.
    #[test]
    fn saves_and_loads_clusters_with_member_chunks() {
        let meeting_id = MeetingId::new("meeting-clusters");
        let repository = repository_with_meeting(&meeting_id);
        let cluster_id = Uuid::new_v4();
        repository
            .save_unknown_speaker_cluster_chunk(
                &cluster(cluster_id, &meeting_id, vec![1.0, 0.0], &["c1"], 1_000),
                &UnknownSpeakerChunk::new("c1", vec![1.0, 0.0], 1_000),
            )
            .unwrap_or_else(|error| panic!("first chunk should save: {error}"));
        let updated_cluster = cluster(
            cluster_id,
            &meeting_id,
            vec![0.9, 0.1],
            &["c1", "c2"],
            2_500,
        );
        repository
            .save_unknown_speaker_cluster_chunk(
                &updated_cluster,
                &UnknownSpeakerChunk::new("c2", vec![0.8, 0.2], 1_500),
            )
            .unwrap_or_else(|error| panic!("second chunk should save: {error}"));

        let loaded_clusters = repository
            .load_unknown_speaker_clusters(&meeting_id)
            .unwrap_or_else(|error| panic!("clusters should load: {error}"));
        assert_eq!(loaded_clusters, vec![updated_cluster.clone()]);
        assert_eq!(
            repository
                .load_unknown_speaker_cluster(cluster_id)
                .unwrap_or_else(|error| panic!("cluster should load: {error}")),
            Some(updated_cluster)
        );
        assert_eq!(
            repository
                .load_unknown_speaker_chunks(cluster_id)
                .unwrap_or_else(|error| panic!("chunks should load: {error}")),
            vec![
                UnknownSpeakerChunk::new("c1", vec![1.0, 0.0], 1_000),
                UnknownSpeakerChunk::new("c2", vec![0.8, 0.2], 1_500),
            ]
        );
    }

    /// Verifies clusters of other meetings are excluded and creation order is preserved.
    #[test]
    fn loads_only_the_requested_meetings_clusters_in_creation_order() {
        let meeting_id = MeetingId::new("meeting-clusters");
        let other_meeting_id = MeetingId::new("meeting-other");
        let repository = repository_with_meeting(&meeting_id);
        repository
            .create_meeting(&other_meeting_id, &[1_u8])
            .unwrap_or_else(|error| panic!("meeting creation should succeed: {error}"));
        let (first_id, second_id, foreign_id) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        for (cluster_id, owner, chunk_id) in [
            (first_id, &meeting_id, "c1"),
            (foreign_id, &other_meeting_id, "x1"),
            (second_id, &meeting_id, "c2"),
        ] {
            repository
                .save_unknown_speaker_cluster_chunk(
                    &cluster(cluster_id, owner, vec![1.0], &[chunk_id], 10),
                    &UnknownSpeakerChunk::new(chunk_id, vec![1.0], 10),
                )
                .unwrap_or_else(|error| panic!("chunk should save: {error}"));
        }

        let loaded_ids: Vec<Uuid> = repository
            .load_unknown_speaker_clusters(&meeting_id)
            .unwrap_or_else(|error| panic!("clusters should load: {error}"))
            .iter()
            .map(UnknownSpeakerCluster::cluster_id)
            .collect();
        assert_eq!(loaded_ids, vec![first_id, second_id]);
    }

    /// Verifies a cluster row without member chunks is never listed.
    #[test]
    fn empty_cluster_rows_are_not_listed() {
        let meeting_id = MeetingId::new("meeting-clusters");
        let repository = repository_with_meeting(&meeting_id);
        repository
            .connection
            .execute(
                "INSERT INTO unknown_speaker_clusters (cluster_id, meeting_id, representative_embedding, total_duration_ms, created_at_utc) VALUES (?1, ?2, X'', 0, '2026-10-27T00:00:00+00:00')",
                params![Uuid::new_v4().to_string(), meeting_id.as_str()],
            )
            .unwrap_or_else(|error| panic!("raw empty cluster insert should succeed: {error}"));

        assert!(
            repository
                .load_unknown_speaker_clusters(&meeting_id)
                .unwrap_or_else(|error| panic!("clusters should load: {error}"))
                .is_empty()
        );
    }

    /// Verifies cluster writes for an unknown meeting are rejected.
    #[test]
    fn rejects_clusters_for_a_missing_meeting() {
        let repository = SqliteNoteRepository::open_in_memory()
            .unwrap_or_else(|error| panic!("in-memory SQLite should initialize: {error}"));
        let missing_meeting_id = MeetingId::new("missing");

        let storage_error = repository
            .save_unknown_speaker_cluster_chunk(
                &cluster(Uuid::new_v4(), &missing_meeting_id, vec![1.0], &["c1"], 1),
                &UnknownSpeakerChunk::new("c1", vec![1.0], 1),
            )
            .err();

        assert!(matches!(storage_error, Some(StorageError::Rejected { .. })));
    }

    /// Verifies resolving marks the identity, hides the cluster, and deletes only its clips.
    #[test]
    fn resolving_marks_identity_and_deletes_cluster_clips() {
        let meeting_id = MeetingId::new("meeting-clusters");
        let repository = repository_with_meeting(&meeting_id);
        let (cluster_id, other_cluster_id) = (Uuid::new_v4(), Uuid::new_v4());
        for (id, chunk_id) in [(cluster_id, "c1"), (other_cluster_id, "c2")] {
            repository
                .save_unknown_speaker_cluster_chunk(
                    &cluster(id, &meeting_id, vec![1.0], &[chunk_id], 1),
                    &UnknownSpeakerChunk::new(chunk_id, vec![1.0], 1),
                )
                .unwrap_or_else(|error| panic!("chunk should save: {error}"));
        }
        let kept_clip = UnassignedVoiceClip::new(Uuid::new_v4(), meeting_id.clone(), 0, 1, vec![9])
            .with_cluster_id(Some(other_cluster_id));
        for clip in [
            UnassignedVoiceClip::new(Uuid::new_v4(), meeting_id.clone(), 0, 1, vec![1])
                .with_cluster_id(Some(cluster_id)),
            kept_clip.clone(),
        ] {
            repository
                .save_unassigned_clip(clip)
                .unwrap_or_else(|error| panic!("clip should save: {error}"));
        }
        let identity_id = Uuid::new_v4();

        repository
            .resolve_unknown_speaker_cluster(cluster_id, identity_id)
            .unwrap_or_else(|error| panic!("cluster should resolve: {error}"));

        let resolved_cluster = repository
            .load_unknown_speaker_cluster(cluster_id)
            .unwrap_or_else(|error| panic!("cluster should load: {error}"));
        assert_eq!(
            resolved_cluster.and_then(|loaded| loaded.resolved_identity_id()),
            Some(identity_id)
        );
        let listed_ids: Vec<Uuid> = repository
            .load_unknown_speaker_clusters(&meeting_id)
            .unwrap_or_else(|error| panic!("clusters should load: {error}"))
            .iter()
            .map(UnknownSpeakerCluster::cluster_id)
            .collect();
        assert_eq!(listed_ids, vec![other_cluster_id]);
        assert_eq!(
            repository.get_unassigned_clips(&meeting_id).ok(),
            Some(vec![kept_clip])
        );
        assert!(matches!(
            repository.resolve_unknown_speaker_cluster(Uuid::new_v4(), identity_id),
            Err(StorageError::Rejected { .. })
        ));
    }

    /// Verifies clip and segment cluster links round-trip through SQLite.
    #[test]
    fn clip_and_segment_cluster_links_round_trip() {
        let meeting_id = MeetingId::new("meeting-clusters");
        let repository = repository_with_meeting(&meeting_id);
        let cluster_id = Uuid::new_v4();
        let clip = UnassignedVoiceClip::new(Uuid::new_v4(), meeting_id.clone(), 0, 1, vec![1])
            .with_cluster_id(Some(cluster_id));
        let segment = TranscriptSegment::new(meeting_id.clone(), 0, 1_000, None, "hello")
            .with_unknown_cluster_id(Some(cluster_id));
        repository
            .save_unassigned_clip(clip.clone())
            .unwrap_or_else(|error| panic!("clip should save: {error}"));
        repository
            .save_transcript_segments(&meeting_id, std::slice::from_ref(&segment))
            .unwrap_or_else(|error| panic!("segment should save: {error}"));

        assert_eq!(
            repository.get_unassigned_clips(&meeting_id).ok(),
            Some(vec![clip])
        );
        assert_eq!(
            repository.load_transcript_segments(&meeting_id).ok(),
            Some(vec![segment])
        );
    }

    /// Verifies databases created before Issue #28 gain the new link columns on open.
    #[test]
    fn migrates_legacy_tables_with_cluster_link_columns() {
        let legacy_connection = Connection::open_in_memory()
            .unwrap_or_else(|error| panic!("in-memory SQLite should open: {error}"));
        legacy_connection
            .execute_batch(
                "CREATE TABLE unassigned_voice_clips (
                    clip_id TEXT PRIMARY KEY NOT NULL,
                    meeting_id TEXT NOT NULL,
                    start_time_ms INTEGER NOT NULL,
                    end_time_ms INTEGER NOT NULL,
                    audio_bytes BLOB NOT NULL,
                    derived_embedding BLOB NOT NULL DEFAULT X'',
                    weak_match_identity_id TEXT,
                    weak_match_confidence_score REAL
                );
                CREATE TABLE transcript_segments (
                    meeting_id TEXT NOT NULL,
                    start_time_ms INTEGER NOT NULL,
                    end_time_ms INTEGER NOT NULL,
                    speaker_identity_id TEXT,
                    segment_text TEXT NOT NULL
                );",
            )
            .unwrap_or_else(|error| panic!("legacy schema should be created: {error}"));
        let repository =
            SqliteNoteRepository::from_connection(legacy_connection, DeviceId::new("legacy"))
                .unwrap_or_else(|error| panic!("legacy database should migrate: {error}"));
        let meeting_id = MeetingId::new("meeting-legacy");
        repository
            .create_meeting(&meeting_id, &[1_u8])
            .unwrap_or_else(|error| panic!("meeting creation should succeed: {error}"));
        let segment = TranscriptSegment::new(meeting_id.clone(), 0, 1, None, "legacy")
            .with_unknown_cluster_id(Some(Uuid::new_v4()));

        repository
            .save_transcript_segments(&meeting_id, std::slice::from_ref(&segment))
            .unwrap_or_else(|error| panic!("segment should save after migration: {error}"));

        assert_eq!(
            repository.load_transcript_segments(&meeting_id).ok(),
            Some(vec![segment])
        );
    }
}
