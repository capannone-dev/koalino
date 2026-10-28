//! Implements the note repository port with SQLite-backed transcript and Automerge BLOB storage.
//!
//! Voice embeddings in `unassigned_voice_clips`, `unknown_speaker_clusters`, and `cluster_chunks`
//! are sealed through the injected `EmbeddingCipher` (Issue #31); opening the repository
//! re-encrypts plaintext embeddings left by older databases.
//!
//! Meeting titles (Issue #35) live in the nullable `meetings.title` column; opening a database
//! created earlier adds that column, leaving existing meetings untitled (`NULL`). Untitled
//! meetings get their display fallback on read (`MeetingMetadata::display_title`), so nothing is
//! back-filled.

use std::{path::Path, sync::Arc};

use rusqlite::{Connection, OptionalExtension, params};

/// Persists unknown-speaker clusters and their member chunks (Issue #28).
#[path = "sqlite_unknown_speaker_clusters.rs"]
mod unknown_speaker_clusters;

use super::sqlite_embedding_encryption::{
    CLIP_EMBEDDINGS, CLUSTER_CHUNK_EMBEDDINGS, CLUSTER_REPRESENTATIVE_EMBEDDINGS,
    SEALED_EMBEDDING_FORMAT,
};
use crate::{
    domain::{
        errors::StorageError,
        models::{
            AggregatedStats, Attachment, DeviceId, MeetingId, MeetingMetadata, MeetingTitle,
            TaskMetrics, TaskType, Transcript, TranscriptSegment, UnassignedVoiceClip,
            UnknownSpeakerChunk, UnknownSpeakerCluster,
        },
    },
    ports::{
        crypto::EmbeddingCipher,
        storage::{AttachmentRepository, NoteRepository},
        telemetry::MetricsRepository,
    },
};

/// How long a write waits for a lock held by another connection (the voice store, or another
/// process such as the daemon) to the same database file.
const BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Persists meeting records locally without exposing SQLite types beyond the adapter boundary.
pub struct SqliteNoteRepository {
    connection: Connection,
    device_id: DeviceId,
    /// Seals and opens clip and cluster embeddings.
    embedding_cipher: Arc<dyn EmbeddingCipher>,
}

impl SqliteNoteRepository {
    /// Opens a SQLite database at `database_path` and initializes the meeting schema.
    ///
    /// Inputs: `database_path` must be a writable SQLite path; `device_id` identifies structured log
    /// events; `embedding_cipher` holds the database key unlocked by `KeyManager`.
    /// Outputs: A repository ready to implement the storage port, or a portable storage error.
    /// Side effects: Opens or creates a SQLite database, creates the schema when absent, and
    /// re-encrypts plaintext embeddings left by databases from before Issue #31.
    pub fn open(
        database_path: impl AsRef<Path>,
        device_id: DeviceId,
        embedding_cipher: Arc<dyn EmbeddingCipher>,
    ) -> Result<Self, StorageError> {
        let sqlite_connection = Connection::open(database_path)
            .map_err(|error| sqlite_error("unable to open SQLite database", error))?;
        Self::from_connection(sqlite_connection, device_id, embedding_cipher)
    }

    /// Opens an isolated in-memory SQLite repository for tests and ephemeral local workflows.
    ///
    /// Inputs: The cipher for stored embeddings (typically an ephemeral one).
    /// Outputs: An initialized repository or a portable storage error.
    /// Side effects: Allocates an in-memory SQLite database and creates its schema.
    pub fn open_in_memory(
        embedding_cipher: Arc<dyn EmbeddingCipher>,
    ) -> Result<Self, StorageError> {
        let sqlite_connection = Connection::open_in_memory()
            .map_err(|error| sqlite_error("unable to open in-memory SQLite database", error))?;
        Self::from_connection(
            sqlite_connection,
            DeviceId::new("in-memory-test-device"),
            embedding_cipher,
        )
    }

    /// Initializes the required SQLite schema for an already-open connection.
    ///
    /// Inputs: `sqlite_connection` must be a usable connection; `device_id` identifies safe log
    /// metadata; `embedding_cipher` seals embeddings.
    /// Outputs: A repository with a `meetings` table ready for use, or a storage error.
    /// Side effects: Executes SQLite DDL (including the Issue #35 `title` column migration),
    /// seals legacy plaintext embeddings in one transaction per table, and emits a structured
    /// initialization log.
    fn from_connection(
        sqlite_connection: Connection,
        device_id: DeviceId,
        embedding_cipher: Arc<dyn EmbeddingCipher>,
    ) -> Result<Self, StorageError> {
        sqlite_connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(|error| sqlite_error("unable to configure SQLite busy timeout", error))?;
        sqlite_connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS meetings (
                    meeting_id TEXT PRIMARY KEY NOT NULL,
                    created_at_utc TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    acquisition_device_id TEXT NOT NULL,
                    transcript TEXT,
                    automerge_document BLOB NOT NULL,
                    title TEXT
                );
                CREATE TABLE IF NOT EXISTS unassigned_voice_clips (
                    clip_id TEXT PRIMARY KEY NOT NULL,
                    meeting_id TEXT NOT NULL,
                    start_time_ms INTEGER NOT NULL,
                    end_time_ms INTEGER NOT NULL,
                    audio_bytes BLOB NOT NULL,
                    derived_embedding BLOB NOT NULL DEFAULT X'',
                    weak_match_identity_id TEXT,
                    weak_match_confidence_score REAL,
                    cluster_id TEXT,
                    FOREIGN KEY(meeting_id) REFERENCES meetings(meeting_id)
                );
                CREATE TABLE IF NOT EXISTS task_metrics (
                    task_id TEXT PRIMARY KEY NOT NULL,
                    device_id TEXT NOT NULL,
                    task_type TEXT NOT NULL,
                    model_name TEXT NOT NULL,
                    duration_ms INTEGER NOT NULL,
                    latency_ms INTEGER NOT NULL,
                    estimated_power_mwh REAL NOT NULL,
                    timestamp_epoch_ms INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS task_metrics_device_id_index ON task_metrics(device_id);
                CREATE TABLE IF NOT EXISTS device_load_stats (
                    device_id TEXT PRIMARY KEY NOT NULL,
                    current_cpu_load_percent REAL NOT NULL,
                    current_gpu_vram_used_megabytes INTEGER NOT NULL,
                    active_tasks_count INTEGER NOT NULL
                );
                CREATE TABLE IF NOT EXISTS attachments (
                    attachment_id TEXT PRIMARY KEY NOT NULL,
                    meeting_id TEXT,
                    file_path TEXT NOT NULL,
                    mime_type TEXT NOT NULL,
                    original_timestamp_utc TEXT NOT NULL,
                    original_device_id TEXT NOT NULL,
                    FOREIGN KEY(meeting_id) REFERENCES meetings(meeting_id)
                );
                CREATE TABLE IF NOT EXISTS transcript_segments (
                    meeting_id TEXT NOT NULL,
                    start_time_ms INTEGER NOT NULL,
                    end_time_ms INTEGER NOT NULL,
                    speaker_identity_id TEXT,
                    segment_text TEXT NOT NULL,
                    unknown_cluster_id TEXT,
                    FOREIGN KEY(meeting_id) REFERENCES meetings(meeting_id)
                );
                CREATE INDEX IF NOT EXISTS transcript_segments_meeting_index
                    ON transcript_segments(meeting_id, start_time_ms);",
            )
            .map_err(|error| sqlite_error("unable to initialize meetings schema", error))?;
        add_meeting_title_column(&sqlite_connection)?;
        unknown_speaker_clusters::initialize_cluster_schema(&sqlite_connection)?;
        for embedding_column in [
            &CLIP_EMBEDDINGS,
            &CLUSTER_REPRESENTATIVE_EMBEDDINGS,
            &CLUSTER_CHUNK_EMBEDDINGS,
        ] {
            embedding_column.prepare(&sqlite_connection, embedding_cipher.as_ref())?;
        }
        tracing::info!(device_id = %device_id.as_str(), "SQLite meeting repository initialized");
        Ok(Self {
            connection: sqlite_connection,
            device_id,
            embedding_cipher,
        })
    }

    /// Emits structured operation metadata without exposing note text or CRDT bytes.
    ///
    /// Inputs: `operation` identifies the completed storage action; `meeting_id` identifies its meeting.
    /// Outputs: None.
    /// Side effects: Writes a debug tracing event.
    fn log_operation(&self, operation: &str, meeting_id: &MeetingId) {
        tracing::debug!(
            device_id = %self.device_id.as_str(),
            meeting_id = %meeting_id.as_str(),
            operation,
            "SQLite meeting storage operation completed"
        );
    }
}

impl AttachmentRepository for SqliteNoteRepository {
    /// Saves user-approved attachment metadata after verifying any requested meeting exists.
    ///
    /// Inputs: `attachment` contains metadata only and may omit its meeting until confirmation.
    /// Outputs: Success or a portable rejection for invalid meeting references/SQLite failures.
    /// Side effects: Writes one local SQLite attachment row without opening the attachment file.
    fn save_attachment(&self, attachment: &Attachment) -> Result<(), StorageError> {
        let meeting_id = attachment.meeting_id().map(MeetingId::as_str);
        let inserted_row_count = self.connection.execute(
            "INSERT INTO attachments (attachment_id, meeting_id, file_path, mime_type, original_timestamp_utc, original_device_id)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6
             WHERE ?2 IS NULL OR EXISTS (SELECT 1 FROM meetings WHERE meeting_id = ?2)",
            params![
                attachment.attachment_id().to_string(),
                meeting_id,
                attachment.file_path(),
                attachment.mime_type(),
                attachment.original_timestamp().to_rfc3339(),
                attachment.original_device_id(),
            ],
        ).map_err(|error| sqlite_error("unable to save attachment", error))?;
        if inserted_row_count == 0 {
            return Err(StorageError::Rejected {
                reason: String::from("cannot save attachment for an unknown meeting"),
            });
        }
        if let Some(meeting_id) = attachment.meeting_id() {
            self.log_operation("save_attachment", meeting_id);
        }
        Ok(())
    }

    /// Loads every locally persisted attachment explicitly linked to one meeting.
    ///
    /// Inputs: `meeting_id` must identify an existing meeting.
    /// Outputs: Attachment metadata in insertion order, or a portable storage error.
    /// Side effects: Reads SQLite metadata only and never opens attachment files.
    fn get_attachments_for_meeting(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Vec<Attachment>, StorageError> {
        self.load_crdt_document(meeting_id)?;
        self.load_attachments(Some(meeting_id))
    }

    /// Loads every imported attachment awaiting explicit meeting selection.
    ///
    /// Inputs: None.
    /// Outputs: Unlinked attachment metadata in insertion order, or a portable storage error.
    /// Side effects: Reads SQLite metadata only and never opens attachment files.
    fn get_unlinked_attachments(&self) -> Result<Vec<Attachment>, StorageError> {
        self.load_attachments(None)
    }
}

impl SqliteNoteRepository {
    /// Reads attachment rows for either one meeting or the unlinked import queue.
    ///
    /// Inputs: `meeting_id` narrows results when present; `None` selects unlinked records.
    /// Outputs: Parsed attachment metadata in insertion order, or a portable storage error.
    /// Side effects: Reads SQLite rows only and never opens attachment files.
    fn load_attachments(
        &self,
        meeting_id: Option<&MeetingId>,
    ) -> Result<Vec<Attachment>, StorageError> {
        let (query, parameter) = match meeting_id {
            Some(value) => (
                "SELECT attachment_id, meeting_id, file_path, mime_type, original_timestamp_utc, original_device_id FROM attachments WHERE meeting_id = ?1 ORDER BY rowid",
                Some(value.as_str()),
            ),
            None => (
                "SELECT attachment_id, meeting_id, file_path, mime_type, original_timestamp_utc, original_device_id FROM attachments WHERE meeting_id IS NULL ORDER BY rowid",
                None,
            ),
        };
        let mut statement = self
            .connection
            .prepare(query)
            .map_err(|error| sqlite_error("unable to prepare attachment query", error))?;
        let rows = statement
            .query_map(params![parameter], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(|error| sqlite_error("unable to load attachments", error))?;
        rows.map(|row| {
            let (attachment_id, persisted_meeting_id, file_path, mime_type, timestamp, device_id) =
                row.map_err(|error| sqlite_error("unable to decode attachment", error))?;
            let parsed_attachment_id =
                uuid::Uuid::parse_str(&attachment_id).map_err(|error| StorageError::Rejected {
                    reason: format!("stored attachment has an invalid UUID: {error}"),
                })?;
            let parsed_timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp)
                .map_err(|error| StorageError::Rejected {
                    reason: format!("stored attachment has an invalid timestamp: {error}"),
                })?
                .with_timezone(&chrono::Utc);
            Ok(Attachment::new(
                parsed_attachment_id,
                persisted_meeting_id.map(MeetingId::new),
                file_path,
                mime_type,
                parsed_timestamp,
                device_id,
            ))
        })
        .collect()
    }
}

impl NoteRepository for SqliteNoteRepository {
    /// Lists locally persisted meeting identifiers without exposing SQLite rows to callers.
    ///
    /// Inputs: None.
    /// Outputs: Identifiers ordered by creation time, or a portable storage error.
    /// Side effects: Reads only the SQLite meeting index.
    fn list_meeting_ids(&self) -> Result<Vec<MeetingId>, StorageError> {
        let mut statement = self
            .connection
            .prepare("SELECT meeting_id FROM meetings ORDER BY created_at_utc, meeting_id")
            .map_err(|error| sqlite_error("unable to prepare meeting index query", error))?;
        let meeting_identifier_rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| sqlite_error("unable to load meeting index", error))?;
        meeting_identifier_rows
            .map(|row| {
                row.map(MeetingId::new)
                    .map_err(|error| sqlite_error("unable to decode meeting identifier", error))
            })
            .collect()
    }

    /// Reads a meeting's SQLite creation timestamp and title as portable metadata.
    ///
    /// Inputs: `meeting_id` identifies a meeting record.
    /// Outputs: Metadata (untitled when the column is `NULL` or holds text that is no longer a
    /// valid title), `None` for an unknown meeting, or a portable storage error.
    /// Side effects: Reads one SQLite row without transcript or CRDT content; logs a warning
    /// (without the text) when a stored title is invalid.
    fn load_meeting_metadata(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Option<MeetingMetadata>, StorageError> {
        let metadata_row = self
            .connection
            .query_row(
                "SELECT created_at_utc, title FROM meetings WHERE meeting_id = ?1",
                params![meeting_id.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()
            .map_err(|error| sqlite_error("unable to load meeting metadata", error))?;
        let Some((created_at_text, stored_title)) = metadata_row else {
            return Ok(None);
        };
        let title = stored_title.and_then(|stored_title| {
            MeetingTitle::parse(&stored_title)
                .inspect_err(|error| {
                    tracing::warn!(
                        meeting_id = %meeting_id.as_str(),
                        error = %error,
                        "Stored meeting title is invalid; using the fallback title"
                    );
                })
                .ok()
        });
        parse_sqlite_timestamp(&created_at_text).map(|created_at| {
            Some(MeetingMetadata::new(meeting_id.clone(), created_at).with_title(title))
        })
    }

    /// Stores or replaces a meeting's title.
    ///
    /// Inputs: `meeting_id` must exist; `title` is validated user content.
    /// Outputs: Success or an unknown-meeting/SQLite storage error.
    /// Side effects: Updates one SQLite row and emits a debug trace event without the title.
    fn save_meeting_title(
        &self,
        meeting_id: &MeetingId,
        title: &MeetingTitle,
    ) -> Result<(), StorageError> {
        let updated_row_count = self
            .connection
            .execute(
                "UPDATE meetings SET title = ?1 WHERE meeting_id = ?2",
                params![title.as_str(), meeting_id.as_str()],
            )
            .map_err(|error| sqlite_error("unable to save meeting title", error))?;
        ensure_meeting_was_updated(updated_row_count, meeting_id)?;
        self.log_operation("save_meeting_title", meeting_id);
        Ok(())
    }

    /// Replaces a meeting's transcript segments inside one SQLite transaction.
    ///
    /// Inputs: `meeting_id` must exist; `segments` must all reference `meeting_id`.
    /// Outputs: Success or an unknown-meeting/mismatched-segment/SQLite storage error.
    /// Side effects: Deletes and inserts SQLite rows atomically; never logs segment text.
    fn save_transcript_segments(
        &self,
        meeting_id: &MeetingId,
        segments: &[TranscriptSegment],
    ) -> Result<(), StorageError> {
        if segments
            .iter()
            .any(|segment| segment.meeting_id() != meeting_id)
        {
            return Err(StorageError::Rejected {
                reason: String::from("every transcript segment must belong to the target meeting"),
            });
        }
        if self.load_meeting_metadata(meeting_id)?.is_none() {
            return Err(missing_meeting_error(meeting_id));
        }
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| sqlite_error("unable to begin segment transaction", error))?;
        transaction
            .execute(
                "DELETE FROM transcript_segments WHERE meeting_id = ?1",
                params![meeting_id.as_str()],
            )
            .map_err(|error| sqlite_error("unable to clear transcript segments", error))?;
        for segment in segments {
            transaction
                .execute(
                    "INSERT INTO transcript_segments (meeting_id, start_time_ms, end_time_ms, speaker_identity_id, segment_text, unknown_cluster_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        meeting_id.as_str(),
                        sqlite_milliseconds(segment.start_time_ms())?,
                        sqlite_milliseconds(segment.end_time_ms())?,
                        segment.speaker_identity_id().map(|identity_id| identity_id.to_string()),
                        segment.text(),
                        segment.unknown_cluster_id().map(|cluster_id| cluster_id.to_string()),
                    ],
                )
                .map_err(|error| sqlite_error("unable to save transcript segment", error))?;
        }
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit transcript segments", error))?;
        self.log_operation("save_transcript_segments", meeting_id);
        Ok(())
    }

    /// Loads a meeting's transcript segments in timeline order.
    ///
    /// Inputs: `meeting_id` identifies a meeting record.
    /// Outputs: Segments ordered by start time then insertion, or a portable storage error.
    /// Side effects: Reads SQLite rows and emits a debug trace event without segment text.
    fn load_transcript_segments(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Vec<TranscriptSegment>, StorageError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT start_time_ms, end_time_ms, speaker_identity_id, segment_text, unknown_cluster_id FROM transcript_segments WHERE meeting_id = ?1 ORDER BY start_time_ms, rowid",
            )
            .map_err(|error| sqlite_error("unable to prepare transcript segment query", error))?;
        let segment_rows = statement
            .query_map(params![meeting_id.as_str()], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .map_err(|error| sqlite_error("unable to load transcript segments", error))?;
        let segments = segment_rows
            .map(|row| {
                let (
                    start_time_ms,
                    end_time_ms,
                    speaker_identity_id,
                    segment_text,
                    unknown_cluster_id,
                ) = row
                    .map_err(|error| sqlite_error("unable to decode transcript segment", error))?;
                let parsed_unknown_cluster_id = unknown_cluster_id
                    .map(|cluster_id| {
                        uuid::Uuid::parse_str(&cluster_id).map_err(|error| StorageError::Rejected {
                            reason: format!("stored segment has an invalid cluster UUID: {error}"),
                        })
                    })
                    .transpose()?;
                let parsed_speaker_identity_id = speaker_identity_id
                    .map(|identity_id| {
                        uuid::Uuid::parse_str(&identity_id).map_err(|error| {
                            StorageError::Rejected {
                                reason: format!(
                                    "stored segment has an invalid speaker UUID: {error}"
                                ),
                            }
                        })
                    })
                    .transpose()?;
                Ok(TranscriptSegment::new(
                    meeting_id.clone(),
                    u64::try_from(start_time_ms).unwrap_or_default(),
                    u64::try_from(end_time_ms).unwrap_or_default(),
                    parsed_speaker_identity_id,
                    segment_text,
                )
                .with_unknown_cluster_id(parsed_unknown_cluster_id))
            })
            .collect::<Result<Vec<_>, StorageError>>()?;
        self.log_operation("load_transcript_segments", meeting_id);
        Ok(segments)
    }

    /// Creates a meeting row with an initial serialized Automerge document.
    ///
    /// Inputs: `meeting_id` must be unique; `initial_crdt_document` must be non-sensitive serialized CRDT state.
    /// Outputs: Success or a duplicate/SQLite storage error.
    /// Side effects: Inserts one row into SQLite and emits a debug trace event.
    fn create_meeting(
        &self,
        meeting_id: &MeetingId,
        initial_crdt_document: &[u8],
    ) -> Result<(), StorageError> {
        self.connection
            .execute(
                "INSERT INTO meetings (meeting_id, acquisition_device_id, automerge_document) VALUES (?1, ?2, ?3)",
                params![meeting_id.as_str(), self.device_id.as_str(), initial_crdt_document],
            )
            .map_err(|error| sqlite_error("unable to create meeting", error))?;
        self.log_operation("create_meeting", meeting_id);
        Ok(())
    }

    /// Persists text only for a pre-existing meeting row.
    ///
    /// Inputs: `transcript` must belong to a meeting created through this repository.
    /// Outputs: Success or an unknown-meeting/SQLite storage error.
    /// Side effects: Updates one SQLite row and emits a debug trace event without the transcript content.
    fn save_transcript(&self, transcript: &Transcript) -> Result<(), StorageError> {
        let updated_row_count = self
            .connection
            .execute(
                "UPDATE meetings SET transcript = ?1 WHERE meeting_id = ?2",
                params![transcript.content(), transcript.meeting_id().as_str()],
            )
            .map_err(|error| sqlite_error("unable to save transcript", error))?;
        ensure_meeting_was_updated(updated_row_count, transcript.meeting_id())?;
        self.log_operation("save_transcript", transcript.meeting_id());
        Ok(())
    }

    /// Loads transcript text when a meeting row has a stored transcript.
    ///
    /// Inputs: `meeting_id` identifies a meeting record.
    /// Outputs: The transcript, no transcript, or a portable storage error.
    /// Side effects: Reads one SQLite row and emits a debug trace event without transcript content.
    fn load_transcript(&self, meeting_id: &MeetingId) -> Result<Option<Transcript>, StorageError> {
        let optional_transcript = self
            .connection
            .query_row(
                "SELECT transcript FROM meetings WHERE meeting_id = ?1",
                params![meeting_id.as_str()],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .map_err(|error| sqlite_error("unable to load transcript", error))?
            .ok_or_else(|| missing_meeting_error(meeting_id))?;
        self.log_operation("load_transcript", meeting_id);
        Ok(optional_transcript
            .map(|transcript_content| Transcript::new(meeting_id.clone(), transcript_content)))
    }

    /// Replaces opaque Automerge bytes only for an existing meeting row.
    ///
    /// Inputs: `meeting_id` must exist; `crdt_document` contains serialized CRDT state.
    /// Outputs: Success or an unknown-meeting/SQLite storage error.
    /// Side effects: Updates one SQLite row and emits a debug trace event without CRDT bytes.
    fn save_crdt_document(
        &self,
        meeting_id: &MeetingId,
        crdt_document: &[u8],
    ) -> Result<(), StorageError> {
        let updated_row_count = self
            .connection
            .execute(
                "UPDATE meetings SET automerge_document = ?1 WHERE meeting_id = ?2",
                params![crdt_document, meeting_id.as_str()],
            )
            .map_err(|error| sqlite_error("unable to save CRDT document", error))?;
        ensure_meeting_was_updated(updated_row_count, meeting_id)?;
        self.log_operation("save_crdt_document", meeting_id);
        Ok(())
    }

    /// Loads opaque Automerge bytes for an existing meeting row.
    ///
    /// Inputs: `meeting_id` identifies a meeting record.
    /// Outputs: Serialized CRDT bytes or a portable storage error.
    /// Side effects: Reads one SQLite row and emits a debug trace event without CRDT bytes.
    fn load_crdt_document(&self, meeting_id: &MeetingId) -> Result<Vec<u8>, StorageError> {
        let serialized_document = self
            .connection
            .query_row(
                "SELECT automerge_document FROM meetings WHERE meeting_id = ?1",
                params![meeting_id.as_str()],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
            .map_err(|error| sqlite_error("unable to load CRDT document", error))?
            .ok_or_else(|| missing_meeting_error(meeting_id))?;
        self.log_operation("load_crdt_document", meeting_id);
        Ok(serialized_document)
    }

    /// Persists one temporary manual-review clip for an existing meeting.
    ///
    /// Inputs: `clip` must reference a meeting created through this repository.
    /// Outputs: Success or an unknown-meeting/SQLite storage error.
    /// Side effects: Stores temporary review audio and the encrypted derived embedding without
    /// logging either.
    fn save_unassigned_clip(&self, clip: UnassignedVoiceClip) -> Result<(), StorageError> {
        let clip_id = clip.clip_id().to_string();
        let sealed_embedding = CLIP_EMBEDDINGS.seal(
            self.embedding_cipher.as_ref(),
            &[&clip_id],
            clip.derived_embedding(),
        )?;
        let inserted_row_count = self.connection.execute(
            "INSERT INTO unassigned_voice_clips (clip_id, meeting_id, start_time_ms, end_time_ms, audio_bytes, derived_embedding, weak_match_identity_id, weak_match_confidence_score, cluster_id, embedding_format)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10 WHERE EXISTS (SELECT 1 FROM meetings WHERE meeting_id = ?2)",
            params![clip_id, clip.meeting_id().as_str(), clip.start_time_ms(), clip.end_time_ms(), clip.audio_bytes(), sealed_embedding, clip.weak_match_candidate().map(|candidate| candidate.identity_id().to_string()), clip.weak_match_candidate().map(|candidate| candidate.confidence_score()), clip.cluster_id().map(|cluster_id| cluster_id.to_string()), SEALED_EMBEDDING_FORMAT],
        ).map_err(|error| sqlite_error("unable to save unassigned voice clip", error))?;
        ensure_meeting_was_updated(inserted_row_count, clip.meeting_id())?;
        self.log_operation("save_unassigned_clip", clip.meeting_id());
        Ok(())
    }

    /// Loads all temporary manual-review clips belonging to an existing meeting.
    ///
    /// Inputs: `meeting_id` identifies a meeting record.
    /// Outputs: Every unresolved clip in insertion order or a portable storage error.
    /// Side effects: Reads sensitive temporary audio without logging it.
    fn get_unassigned_clips(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Vec<UnassignedVoiceClip>, StorageError> {
        self.load_crdt_document(meeting_id)?;
        let mut statement = self.connection.prepare(
            "SELECT clip_id, start_time_ms, end_time_ms, audio_bytes, derived_embedding, weak_match_identity_id, weak_match_confidence_score, cluster_id FROM unassigned_voice_clips WHERE meeting_id = ?1 ORDER BY rowid",
        ).map_err(|error| sqlite_error("unable to prepare unassigned voice clip query", error))?;
        let rows = statement
            .query_map(params![meeting_id.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u64>(1)?,
                    row.get::<_, u64>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<f32>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            })
            .map_err(|error| sqlite_error("unable to load unassigned voice clips", error))?;
        let mut clips = Vec::new();
        for row in rows {
            let (
                clip_id,
                start_time_ms,
                end_time_ms,
                audio_bytes,
                serialized_embedding,
                weak_match_identity_id,
                weak_match_confidence_score,
                cluster_id,
            ) =
                row.map_err(|error| sqlite_error("unable to decode unassigned voice clip", error))?;
            let parsed_cluster_id = cluster_id
                .map(|cluster_id| {
                    uuid::Uuid::parse_str(&cluster_id).map_err(|error| StorageError::Rejected {
                        reason: format!("stored voice clip has an invalid cluster UUID: {error}"),
                    })
                })
                .transpose()?;
            let parsed_clip_id =
                uuid::Uuid::parse_str(&clip_id).map_err(|error| StorageError::Rejected {
                    reason: format!("stored unassigned voice clip has an invalid UUID: {error}"),
                })?;
            let weak_match_candidate = match (weak_match_identity_id, weak_match_confidence_score) {
                (Some(identity_id), Some(confidence_score)) => {
                    Some(crate::domain::models::WeakVoiceMatchCandidate::new(
                        uuid::Uuid::parse_str(&identity_id).map_err(|error| {
                            StorageError::Rejected {
                                reason: format!(
                                    "stored weak voice match has an invalid UUID: {error}"
                                ),
                            }
                        })?,
                        confidence_score,
                    ))
                }
                (None, None) => None,
                _ => {
                    return Err(StorageError::Rejected {
                        reason: String::from("stored weak voice match is incomplete"),
                    });
                }
            };
            clips.push(
                UnassignedVoiceClip::with_review_context(
                    parsed_clip_id,
                    meeting_id.clone(),
                    start_time_ms,
                    end_time_ms,
                    audio_bytes,
                    CLIP_EMBEDDINGS.open(
                        self.embedding_cipher.as_ref(),
                        &[&clip_id],
                        &serialized_embedding,
                    )?,
                    weak_match_candidate,
                )
                .with_cluster_id(parsed_cluster_id),
            );
        }
        self.log_operation("get_unassigned_clips", meeting_id);
        Ok(clips)
    }

    /// Upserts a cluster and inserts its new member atomically (see `unknown_speaker_clusters`).
    ///
    /// Inputs: Match `NoteRepository::save_unknown_speaker_cluster_chunk`.
    /// Outputs: Success or a rejection for unknown meetings or duplicate members.
    /// Side effects: Writes biometric BLOBs inside one SQLite transaction.
    fn save_unknown_speaker_cluster_chunk(
        &self,
        cluster: &UnknownSpeakerCluster,
        chunk: &UnknownSpeakerChunk,
    ) -> Result<(), StorageError> {
        self.save_cluster_chunk(cluster, chunk)
    }

    /// Loads a meeting's unresolved, non-empty clusters in creation order.
    ///
    /// Inputs: Match `NoteRepository::load_unknown_speaker_clusters`.
    /// Outputs: Clusters or a portable storage error.
    /// Side effects: Reads biometric BLOBs without logging them.
    fn load_unknown_speaker_clusters(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Vec<UnknownSpeakerCluster>, StorageError> {
        self.load_clusters_for_meeting(meeting_id)
    }

    /// Loads one cluster by ID, whether resolved or not.
    ///
    /// Inputs: Match `NoteRepository::load_unknown_speaker_cluster`.
    /// Outputs: The cluster, `None`, or a portable storage error.
    /// Side effects: Reads biometric BLOBs without logging them.
    fn load_unknown_speaker_cluster(
        &self,
        cluster_id: uuid::Uuid,
    ) -> Result<Option<UnknownSpeakerCluster>, StorageError> {
        self.load_cluster(cluster_id)
    }

    /// Loads one cluster's members with their embeddings.
    ///
    /// Inputs: Match `NoteRepository::load_unknown_speaker_chunks`.
    /// Outputs: Members in insertion order or a portable storage error.
    /// Side effects: Reads biometric BLOBs without logging them.
    fn load_unknown_speaker_chunks(
        &self,
        cluster_id: uuid::Uuid,
    ) -> Result<Vec<UnknownSpeakerChunk>, StorageError> {
        self.load_cluster_chunks(cluster_id)
    }

    /// Resolves a cluster and deletes its review clips atomically.
    ///
    /// Inputs: Match `NoteRepository::resolve_unknown_speaker_cluster`.
    /// Outputs: Success or a rejection for an unknown cluster.
    /// Side effects: Updates the cluster row and deletes temporary review audio.
    fn resolve_unknown_speaker_cluster(
        &self,
        cluster_id: uuid::Uuid,
        identity_id: uuid::Uuid,
    ) -> Result<(), StorageError> {
        self.resolve_cluster(cluster_id, identity_id)
    }
}

/// Decodes a legacy plaintext embedding (written before Issue #31) for re-encryption, rejecting
/// malformed byte lengths.
///
/// Inputs: `serialized_embedding` contains adapter-owned SQLite BLOB data.
/// Outputs: Derived dimensions or a portable storage rejection.
/// Side effects: Allocates the returned vector; never logs biometric data.
pub(super) fn decode_embedding(serialized_embedding: &[u8]) -> Result<Vec<f32>, StorageError> {
    let (serialized_float_bytes, remainder) = serialized_embedding.as_chunks::<4>();
    if !remainder.is_empty() {
        return Err(StorageError::Rejected {
            reason: String::from("stored voice embedding has an invalid byte length"),
        });
    }
    Ok(serialized_float_bytes
        .iter()
        .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .collect())
}

/// Converts a SQLite error into the domain-neutral error published by the storage port.
///
/// Inputs: `operation` identifies the failed SQL action; `error` is the adapter-private SQLite error.
/// Outputs: A portable storage rejection.
/// Side effects: None.
impl MetricsRepository for SqliteNoteRepository {
    /// Stores privacy-safe completed-task metrics in the local telemetry table.
    fn save_metrics(&self, metrics: TaskMetrics) -> Result<(), StorageError> {
        self.connection.execute(
            "INSERT INTO task_metrics (task_id, device_id, task_type, model_name, duration_ms, latency_ms, estimated_power_mwh, timestamp_epoch_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![metrics.task_id().to_string(), metrics.device_id().as_str(), metrics.task_type().as_str(), metrics.model_name(), metrics.duration_ms(), metrics.latency_ms(), metrics.estimated_power_mwh(), metrics.timestamp_epoch_ms()],
        ).map_err(|error| sqlite_error("unable to save task metrics", error))?;
        tracing::info!(
            task_type = metrics.task_type().as_str(),
            duration_ms = metrics.duration_ms(),
            "AI task metrics recorded"
        );
        Ok(())
    }

    /// Loads one device's completed metrics in completion order.
    fn get_metrics_for_device(&self, device_id: &str) -> Result<Vec<TaskMetrics>, StorageError> {
        let mut statement = self.connection.prepare("SELECT task_id, task_type, model_name, duration_ms, latency_ms, estimated_power_mwh, timestamp_epoch_ms FROM task_metrics WHERE device_id = ?1 ORDER BY timestamp_epoch_ms").map_err(|error| sqlite_error("unable to prepare device metrics query", error))?;
        let rows = statement
            .query_map(params![device_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, u64>(3)?,
                    row.get::<_, u64>(4)?,
                    row.get::<_, f32>(5)?,
                    row.get::<_, u64>(6)?,
                ))
            })
            .map_err(|error| sqlite_error("unable to load device metrics", error))?;
        rows.map(|row| {
            let (
                task_id,
                task_type,
                model_name,
                duration_ms,
                latency_ms,
                estimated_power_mwh,
                timestamp_epoch_ms,
            ) = row.map_err(|error| sqlite_error("unable to decode device metric", error))?;
            Ok(TaskMetrics::new(
                uuid::Uuid::parse_str(&task_id).map_err(|error| StorageError::Rejected {
                    reason: format!("stored task metric has an invalid UUID: {error}"),
                })?,
                DeviceId::new(device_id),
                parse_task_type(&task_type)?,
                model_name,
                duration_ms,
                latency_ms,
                estimated_power_mwh,
                timestamp_epoch_ms,
            ))
        })
        .collect()
    }

    /// Loads every completed metric so the local dashboard can form cross-device aggregates.
    ///
    /// Inputs: None.
    /// Outputs: Privacy-safe metric metadata in timestamp order or a portable storage error.
    /// Side effects: Reads SQLite rows only; it does not load AI inputs, outputs, or raw audio.
    fn get_all_metrics(&self) -> Result<Vec<TaskMetrics>, StorageError> {
        let mut statement = self.connection.prepare("SELECT task_id, device_id, task_type, model_name, duration_ms, latency_ms, estimated_power_mwh, timestamp_epoch_ms FROM task_metrics ORDER BY timestamp_epoch_ms").map_err(|error| sqlite_error("unable to prepare all metrics query", error))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, u64>(4)?,
                    row.get::<_, u64>(5)?,
                    row.get::<_, f32>(6)?,
                    row.get::<_, u64>(7)?,
                ))
            })
            .map_err(|error| sqlite_error("unable to load all metrics", error))?;
        rows.map(|row| {
            let (
                task_id,
                device_id,
                task_type,
                model_name,
                duration_ms,
                latency_ms,
                estimated_power_mwh,
                timestamp_epoch_ms,
            ) = row.map_err(|error| sqlite_error("unable to decode metric", error))?;
            Ok(TaskMetrics::new(
                uuid::Uuid::parse_str(&task_id).map_err(|error| StorageError::Rejected {
                    reason: format!("stored task metric has an invalid UUID: {error}"),
                })?,
                DeviceId::new(device_id),
                parse_task_type(&task_type)?,
                model_name,
                duration_ms,
                latency_ms,
                estimated_power_mwh,
                timestamp_epoch_ms,
            ))
        })
        .collect()
    }

    /// Aggregates all persisted task metric rows without loading task payloads.
    fn get_aggregated_stats(&self) -> Result<AggregatedStats, StorageError> {
        let (task_count, total_duration_ms, total_latency_ms): (u64, u64, u64) = self.connection.query_row("SELECT COUNT(*), COALESCE(SUM(duration_ms), 0), COALESCE(SUM(latency_ms), 0) FROM task_metrics", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).map_err(|error| sqlite_error("unable to aggregate task metrics", error))?;
        let average_latency_ms = total_latency_ms.checked_div(task_count).unwrap_or(0);
        Ok(AggregatedStats::new(
            task_count,
            total_duration_ms,
            average_latency_ms,
        ))
    }

    /// Upserts one device's latest instantaneous utilization snapshot.
    fn save_device_load(
        &self,
        device_load_stats: crate::domain::models::DeviceLoadStats,
    ) -> Result<(), StorageError> {
        self.connection.execute("INSERT INTO device_load_stats (device_id, current_cpu_load_percent, current_gpu_vram_used_megabytes, active_tasks_count) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(device_id) DO UPDATE SET current_cpu_load_percent = excluded.current_cpu_load_percent, current_gpu_vram_used_megabytes = excluded.current_gpu_vram_used_megabytes, active_tasks_count = excluded.active_tasks_count", params![device_load_stats.device_id().as_str(), device_load_stats.current_cpu_load_percent(), device_load_stats.current_gpu_vram_used_megabytes(), device_load_stats.active_tasks_count()]).map_err(|error| sqlite_error("unable to save device load", error))?;
        Ok(())
    }
}

/// Decodes one stable telemetry task label from SQLite into the domain enum.
fn parse_task_type(task_type: &str) -> Result<TaskType, StorageError> {
    match task_type {
        "transcription" => Ok(TaskType::Transcription),
        "diarization" => Ok(TaskType::Diarization),
        "file_transfer" => Ok(TaskType::FileTransfer),
        "cloud_transcription" => Ok(TaskType::CloudTranscription),
        _ => Err(StorageError::Rejected {
            reason: String::from("stored task metric has an unknown task type"),
        }),
    }
}

/// Parses SQLite's `CURRENT_TIMESTAMP` text (UTC, `YYYY-MM-DD HH:MM:SS`) or RFC 3339 text.
///
/// Inputs: `timestamp_text` is a persisted timestamp column value.
/// Outputs: A UTC instant or a portable rejection for malformed values.
/// Side effects: None.
fn parse_sqlite_timestamp(
    timestamp_text: &str,
) -> Result<chrono::DateTime<chrono::Utc>, StorageError> {
    chrono::NaiveDateTime::parse_from_str(timestamp_text, "%Y-%m-%d %H:%M:%S")
        .map(|naive_timestamp| naive_timestamp.and_utc())
        .or_else(|_| {
            chrono::DateTime::parse_from_rfc3339(timestamp_text)
                .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        })
        .map_err(|error| StorageError::Rejected {
            reason: format!("stored meeting has an invalid creation timestamp: {error}"),
        })
}

/// Converts an unsigned millisecond offset to SQLite's signed integer representation.
///
/// Inputs: `milliseconds` is a segment offset.
/// Outputs: The same value as `i64`, or a rejection when it exceeds SQLite's range.
/// Side effects: None.
fn sqlite_milliseconds(milliseconds: u64) -> Result<i64, StorageError> {
    i64::try_from(milliseconds).map_err(|_| StorageError::Rejected {
        reason: String::from("transcript segment offset exceeds SQLite integer range"),
    })
}

pub(super) fn sqlite_error(operation: &str, error: rusqlite::Error) -> StorageError {
    StorageError::Rejected {
        reason: format!("{operation}: {error}"),
    }
}

/// Creates a consistent storage error for writes and reads targeting an unknown meeting.
///
/// Inputs: `meeting_id` identifies the missing meeting.
/// Outputs: A portable storage rejection.
/// Side effects: None.
fn missing_meeting_error(meeting_id: &MeetingId) -> StorageError {
    StorageError::Rejected {
        reason: format!("meeting {} does not exist", meeting_id.as_str()),
    }
}

/// Adds the nullable `meetings.title` column to databases created before Issue #35.
///
/// Inputs: A connection whose `meetings` table exists.
/// Outputs: Success (also when the column already exists) or a portable storage error.
/// Side effects: May run one `ALTER TABLE`; existing rows keep a `NULL` title.
fn add_meeting_title_column(sqlite_connection: &Connection) -> Result<(), StorageError> {
    let title_column_count: i64 = sqlite_connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('meetings') WHERE name = 'title'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| sqlite_error("unable to inspect meetings schema", error))?;
    if title_column_count == 0 {
        sqlite_connection
            .execute("ALTER TABLE meetings ADD COLUMN title TEXT", [])
            .map_err(|error| sqlite_error("unable to add meeting title column", error))?;
        tracing::info!("Added the meeting title column to an existing database");
    }
    Ok(())
}

/// Ensures an SQL update affected exactly one existing meeting row.
///
/// Inputs: `updated_row_count` is SQLite's update result; `meeting_id` identifies the target record.
/// Outputs: Success when a row changed or an unknown-meeting rejection.
/// Side effects: None.
fn ensure_meeting_was_updated(
    updated_row_count: usize,
    meeting_id: &MeetingId,
) -> Result<(), StorageError> {
    if updated_row_count == 1 {
        Ok(())
    } else {
        Err(missing_meeting_error(meeting_id))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        adapters::storage::sqlite_adapter::SqliteNoteRepository,
        domain::{
            errors::StorageError,
            models::{MeetingId, Transcript, TranscriptSegment, UnassignedVoiceClip},
        },
        ports::storage::NoteRepository,
    };

    /// Verifies speaker-attributed segments are stored in timeline order and replaced atomically.
    #[test]
    fn saves_replaces_and_loads_transcript_segments_in_order() {
        let repository = SqliteNoteRepository::open_in_memory(
            crate::application::crypto::voice_encryptor::ephemeral_cipher(),
        )
        .unwrap_or_else(|error| panic!("in-memory SQLite should initialize: {error}"));
        let meeting_id = MeetingId::new("meeting-segments");
        repository
            .create_meeting(&meeting_id, &[1_u8])
            .unwrap_or_else(|error| panic!("meeting creation should succeed: {error}"));
        let alice_identity_id = uuid::Uuid::from_u128(1);
        let later_segment = TranscriptSegment::new(
            meeting_id.clone(),
            5_000,
            9_000,
            None,
            "Unattributed remark.",
        );
        let earlier_segment = TranscriptSegment::new(
            meeting_id.clone(),
            0,
            5_000,
            Some(alice_identity_id),
            "Opening statement.",
        );

        repository
            .save_transcript_segments(
                &meeting_id,
                &[later_segment.clone(), earlier_segment.clone()],
            )
            .unwrap_or_else(|error| panic!("segment save should succeed: {error}"));
        assert_eq!(
            repository.load_transcript_segments(&meeting_id).ok(),
            Some(vec![earlier_segment.clone(), later_segment])
        );

        repository
            .save_transcript_segments(&meeting_id, std::slice::from_ref(&earlier_segment))
            .unwrap_or_else(|error| panic!("segment replacement should succeed: {error}"));
        assert_eq!(
            repository.load_transcript_segments(&meeting_id).ok(),
            Some(vec![earlier_segment])
        );
    }

    /// Verifies segments cannot be attached to a meeting that was never created.
    #[test]
    fn rejects_transcript_segments_for_a_missing_meeting() {
        let repository = SqliteNoteRepository::open_in_memory(
            crate::application::crypto::voice_encryptor::ephemeral_cipher(),
        )
        .unwrap_or_else(|error| panic!("in-memory SQLite should initialize: {error}"));
        let missing_meeting_id = MeetingId::new("missing-segment-meeting");
        let orphan_segment =
            TranscriptSegment::new(missing_meeting_id.clone(), 0, 1, None, "Orphaned.");

        let storage_error = repository
            .save_transcript_segments(&missing_meeting_id, &[orphan_segment])
            .expect_err("an unknown meeting must reject segment persistence");

        assert!(matches!(storage_error, StorageError::Rejected { .. }));
    }

    /// Verifies meeting metadata exposes the creation instant and reports unknown meetings as absent.
    #[test]
    fn loads_meeting_metadata_with_creation_time() {
        let repository = SqliteNoteRepository::open_in_memory(
            crate::application::crypto::voice_encryptor::ephemeral_cipher(),
        )
        .unwrap_or_else(|error| panic!("in-memory SQLite should initialize: {error}"));
        let meeting_id = MeetingId::new("meeting-metadata");
        let before_creation = chrono::Utc::now() - chrono::Duration::seconds(2);
        repository
            .create_meeting(&meeting_id, &[1_u8])
            .unwrap_or_else(|error| panic!("meeting creation should succeed: {error}"));

        let meeting_metadata = repository
            .load_meeting_metadata(&meeting_id)
            .unwrap_or_else(|error| panic!("metadata load should succeed: {error}"))
            .unwrap_or_else(|| panic!("created meeting must have metadata"));

        assert_eq!(meeting_metadata.meeting_id(), &meeting_id);
        assert!(meeting_metadata.created_at() >= before_creation);
        assert_eq!(
            repository
                .load_meeting_metadata(&MeetingId::new("unknown-meeting"))
                .ok(),
            Some(None)
        );
    }

    /// Verifies a new meeting is untitled until a title is saved, which replaces any earlier one,
    /// and that titles cannot be saved for unknown meetings.
    #[test]
    fn saves_and_loads_a_meeting_title() {
        let repository = SqliteNoteRepository::open_in_memory(
            crate::application::crypto::voice_encryptor::ephemeral_cipher(),
        )
        .unwrap_or_else(|error| panic!("in-memory SQLite should initialize: {error}"));
        let meeting_id = MeetingId::new("meeting-titled");
        repository
            .create_meeting(&meeting_id, &[1_u8])
            .unwrap_or_else(|error| panic!("meeting creation should succeed: {error}"));
        let load_title = || {
            repository
                .load_meeting_metadata(&meeting_id)
                .unwrap_or_else(|error| panic!("metadata load should succeed: {error}"))
                .unwrap_or_else(|| panic!("created meeting must have metadata"))
                .title()
                .map(|title| title.as_str().to_owned())
        };
        assert_eq!(load_title(), None);

        for title_text in ["Project Sync", "Project Sync (renamed)"] {
            repository
                .save_meeting_title(&meeting_id, &meeting_title(title_text))
                .unwrap_or_else(|error| panic!("title save should succeed: {error}"));
            assert_eq!(load_title().as_deref(), Some(title_text));
        }

        let storage_error = repository
            .save_meeting_title(&MeetingId::new("missing"), &meeting_title("Orphan"))
            .expect_err("an unknown meeting must reject a title");
        assert!(matches!(storage_error, StorageError::Rejected { .. }));
    }

    /// Verifies re-titling one meeting (the rename primitive of Issue #38) changes only that
    /// meeting's title, leaving its creation time and every other meeting untouched.
    #[test]
    fn retitling_one_meeting_leaves_other_meetings_unchanged() {
        let repository = SqliteNoteRepository::open_in_memory(
            crate::application::crypto::voice_encryptor::ephemeral_cipher(),
        )
        .unwrap_or_else(|error| panic!("in-memory SQLite should initialize: {error}"));
        let renamed_meeting_id = MeetingId::new("meeting-renamed");
        let other_meeting_id = MeetingId::new("meeting-other");
        for meeting_id in [&renamed_meeting_id, &other_meeting_id] {
            repository
                .create_meeting(meeting_id, &[1_u8])
                .unwrap_or_else(|error| panic!("meeting creation should succeed: {error}"));
            repository
                .save_meeting_title(meeting_id, &meeting_title("Before"))
                .unwrap_or_else(|error| panic!("title save should succeed: {error}"));
        }
        let load_metadata = |meeting_id: &MeetingId| {
            repository
                .load_meeting_metadata(meeting_id)
                .unwrap_or_else(|error| panic!("metadata load should succeed: {error}"))
                .unwrap_or_else(|| panic!("created meeting must have metadata"))
        };
        let renamed_before = load_metadata(&renamed_meeting_id);
        let other_before = load_metadata(&other_meeting_id);

        repository
            .save_meeting_title(&renamed_meeting_id, &meeting_title("After"))
            .unwrap_or_else(|error| panic!("rename should succeed: {error}"));

        let renamed_after = load_metadata(&renamed_meeting_id);
        assert_eq!(
            renamed_after.title().map(|title| title.as_str()),
            Some("After")
        );
        assert_eq!(renamed_after.created_at(), renamed_before.created_at());
        assert_eq!(load_metadata(&other_meeting_id), other_before);
    }

    /// Verifies a database created before Issue #35 gains the `title` column on open, its
    /// meetings read as untitled with a dated display fallback, and they can then be titled.
    #[test]
    fn migrates_a_legacy_database_without_a_title_column() {
        let database_directory =
            std::env::temp_dir().join(format!("note-taker-title-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&database_directory)
            .unwrap_or_else(|error| panic!("test directory must be created: {error}"));
        let database_path = database_directory.join("legacy.sqlite3");
        let legacy_connection = rusqlite::Connection::open(&database_path)
            .unwrap_or_else(|error| panic!("legacy database must open: {error}"));
        legacy_connection
            .execute_batch(
                "CREATE TABLE meetings (
                    meeting_id TEXT PRIMARY KEY NOT NULL,
                    created_at_utc TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    acquisition_device_id TEXT NOT NULL,
                    transcript TEXT,
                    automerge_document BLOB NOT NULL
                );
                INSERT INTO meetings (meeting_id, created_at_utc, acquisition_device_id, automerge_document)
                    VALUES ('legacy-meeting', '2026-10-20 14:30:00', 'old-device', X'01');",
            )
            .unwrap_or_else(|error| panic!("legacy schema must be created: {error}"));
        drop(legacy_connection);
        let open_repository = || {
            SqliteNoteRepository::open(
                &database_path,
                crate::domain::models::DeviceId::new("migration-test"),
                crate::application::crypto::voice_encryptor::ephemeral_cipher(),
            )
            .unwrap_or_else(|error| panic!("legacy database must migrate: {error}"))
        };
        let meeting_id = MeetingId::new("legacy-meeting");

        let repository = open_repository();
        let legacy_metadata = repository
            .load_meeting_metadata(&meeting_id)
            .unwrap_or_else(|error| panic!("metadata load should succeed: {error}"))
            .unwrap_or_else(|| panic!("legacy meeting must have metadata"));
        assert_eq!(legacy_metadata.title(), None);
        assert_eq!(
            legacy_metadata.display_title(),
            "Meeting 2026-10-20 14:30 UTC"
        );
        repository
            .save_meeting_title(&meeting_id, &meeting_title("Retro"))
            .unwrap_or_else(|error| panic!("title save should succeed: {error}"));
        drop(repository);

        let reopened_title = open_repository()
            .load_meeting_metadata(&meeting_id)
            .unwrap_or_else(|error| panic!("metadata load should succeed: {error}"))
            .and_then(|metadata| metadata.title().map(|title| title.as_str().to_owned()));
        let _ = std::fs::remove_dir_all(&database_directory);
        assert_eq!(reopened_title.as_deref(), Some("Retro"));
    }

    /// Parses a fixture title.
    fn meeting_title(title_text: &str) -> crate::domain::models::MeetingTitle {
        crate::domain::models::MeetingTitle::parse(title_text)
            .unwrap_or_else(|error| panic!("fixture title must parse: {error}"))
    }

    /// Verifies adapter initialization creates its schema and stores both transcript and CRDT bytes.
    #[test]
    fn saves_and_loads_a_meeting_in_memory() {
        let repository = SqliteNoteRepository::open_in_memory(
            crate::application::crypto::voice_encryptor::ephemeral_cipher(),
        )
        .unwrap_or_else(|error| panic!("in-memory SQLite should initialize: {error}"));
        let meeting_id = MeetingId::new("meeting-rome");
        let crdt_document = vec![1_u8, 2_u8, 3_u8];
        let transcript = Transcript::new(meeting_id.clone(), "Discussed offline sync.");

        repository
            .create_meeting(&meeting_id, &crdt_document)
            .unwrap_or_else(|error| panic!("meeting creation should succeed: {error}"));
        repository
            .save_transcript(&transcript)
            .unwrap_or_else(|error| panic!("transcript save should succeed: {error}"));

        assert_eq!(
            repository.load_transcript(&meeting_id).ok(),
            Some(Some(transcript))
        );
        assert_eq!(
            repository.load_crdt_document(&meeting_id).ok(),
            Some(crdt_document)
        );
    }

    /// Verifies an orphaned note cannot be persisted without an explicitly created meeting.
    #[test]
    fn rejects_a_transcript_for_a_missing_meeting() {
        let repository = SqliteNoteRepository::open_in_memory(
            crate::application::crypto::voice_encryptor::ephemeral_cipher(),
        )
        .unwrap_or_else(|error| panic!("in-memory SQLite should initialize: {error}"));
        let missing_meeting_transcript =
            Transcript::new(MeetingId::new("missing-meeting"), "Orphaned note.");

        let storage_error = repository
            .save_transcript(&missing_meeting_transcript)
            .expect_err("an unknown meeting must reject note persistence");

        assert!(matches!(storage_error, StorageError::Rejected { .. }));
    }

    /// Verifies temporary clips retain their review range and opaque audio through the repository port.
    #[test]
    fn saves_and_loads_an_unassigned_voice_clip() {
        let repository = SqliteNoteRepository::open_in_memory(
            crate::application::crypto::voice_encryptor::ephemeral_cipher(),
        )
        .unwrap_or_else(|error| panic!("in-memory SQLite should initialize: {error}"));
        let meeting_id = MeetingId::new("meeting-unassigned-clip");
        repository
            .create_meeting(&meeting_id, &[1_u8])
            .unwrap_or_else(|error| panic!("meeting creation should succeed: {error}"));
        let unassigned_clip = UnassignedVoiceClip::new(
            uuid::Uuid::new_v4(),
            meeting_id.clone(),
            100,
            250,
            vec![3_u8, 2_u8, 1_u8],
        );

        repository
            .save_unassigned_clip(unassigned_clip.clone())
            .unwrap_or_else(|error| panic!("unassigned clip save should succeed: {error}"));

        assert_eq!(
            repository.get_unassigned_clips(&meeting_id).ok(),
            Some(vec![unassigned_clip])
        );
    }
}
