//! Implements the note repository port with SQLite-backed transcript and Automerge BLOB storage.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    domain::{
        errors::StorageError,
        models::{
            AggregatedStats, Attachment, DeviceId, MeetingId, TaskMetrics, TaskType, Transcript,
            UnassignedVoiceClip,
        },
    },
    ports::{
        storage::{AttachmentRepository, NoteRepository},
        telemetry::MetricsRepository,
    },
};

/// Persists meeting records locally without exposing SQLite types beyond the adapter boundary.
pub struct SqliteNoteRepository {
    connection: Connection,
    device_id: DeviceId,
}

impl SqliteNoteRepository {
    /// Opens a SQLite database at `database_path` and initializes the meeting schema.
    ///
    /// Inputs: `database_path` must be a writable SQLite path; `device_id` identifies structured log events.
    /// Outputs: A repository ready to implement the storage port, or a portable storage error.
    /// Side effects: Opens or creates a SQLite database and creates the `meetings` table when absent.
    pub fn open(
        database_path: impl AsRef<Path>,
        device_id: DeviceId,
    ) -> Result<Self, StorageError> {
        let sqlite_connection = Connection::open(database_path)
            .map_err(|error| sqlite_error("unable to open SQLite database", error))?;
        Self::from_connection(sqlite_connection, device_id)
    }

    /// Opens an isolated in-memory SQLite repository for tests and ephemeral local workflows.
    ///
    /// Inputs: None.
    /// Outputs: An initialized repository or a portable storage error.
    /// Side effects: Allocates an in-memory SQLite database and creates its schema.
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let sqlite_connection = Connection::open_in_memory()
            .map_err(|error| sqlite_error("unable to open in-memory SQLite database", error))?;
        Self::from_connection(sqlite_connection, DeviceId::new("in-memory-test-device"))
    }

    /// Initializes the required SQLite schema for an already-open connection.
    ///
    /// Inputs: `sqlite_connection` must be a usable connection; `device_id` identifies safe log metadata.
    /// Outputs: A repository with a `meetings` table ready for use, or a storage error.
    /// Side effects: Executes SQLite DDL and emits a structured initialization log.
    fn from_connection(
        sqlite_connection: Connection,
        device_id: DeviceId,
    ) -> Result<Self, StorageError> {
        sqlite_connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS meetings (
                    meeting_id TEXT PRIMARY KEY NOT NULL,
                    created_at_utc TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    acquisition_device_id TEXT NOT NULL,
                    transcript TEXT,
                    automerge_document BLOB NOT NULL
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
                );",
            )
            .map_err(|error| sqlite_error("unable to initialize meetings schema", error))?;
        tracing::info!(device_id = %device_id.as_str(), "SQLite meeting repository initialized");
        Ok(Self {
            connection: sqlite_connection,
            device_id,
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
    /// Side effects: Stores temporary review audio without logging its contents.
    fn save_unassigned_clip(&self, clip: UnassignedVoiceClip) -> Result<(), StorageError> {
        let inserted_row_count = self.connection.execute(
            "INSERT INTO unassigned_voice_clips (clip_id, meeting_id, start_time_ms, end_time_ms, audio_bytes, derived_embedding, weak_match_identity_id, weak_match_confidence_score)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8 WHERE EXISTS (SELECT 1 FROM meetings WHERE meeting_id = ?2)",
            params![clip.clip_id().to_string(), clip.meeting_id().as_str(), clip.start_time_ms(), clip.end_time_ms(), clip.audio_bytes(), encode_embedding(clip.derived_embedding()), clip.weak_match_candidate().map(|candidate| candidate.identity_id().to_string()), clip.weak_match_candidate().map(|candidate| candidate.confidence_score())],
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
            "SELECT clip_id, start_time_ms, end_time_ms, audio_bytes, derived_embedding, weak_match_identity_id, weak_match_confidence_score FROM unassigned_voice_clips WHERE meeting_id = ?1 ORDER BY rowid",
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
            ) =
                row.map_err(|error| sqlite_error("unable to decode unassigned voice clip", error))?;
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
            clips.push(UnassignedVoiceClip::with_review_context(
                parsed_clip_id,
                meeting_id.clone(),
                start_time_ms,
                end_time_ms,
                audio_bytes,
                decode_embedding(&serialized_embedding)?,
                weak_match_candidate,
            ));
        }
        self.log_operation("get_unassigned_clips", meeting_id);
        Ok(clips)
    }
}

/// Serializes derived floating-point features without exposing them outside the storage adapter.
///
/// Inputs: `embedding` contains model-derived values only.
/// Outputs: Native-endian bytes for local SQLite persistence.
/// Side effects: Allocates a byte buffer; never logs biometric data.
fn encode_embedding(embedding: &[f32]) -> Vec<u8> {
    embedding
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// Decodes a locally persisted embedding while rejecting malformed byte lengths.
///
/// Inputs: `serialized_embedding` contains adapter-owned SQLite BLOB data.
/// Outputs: Derived dimensions or a portable storage rejection.
/// Side effects: Allocates the returned vector; never logs biometric data.
fn decode_embedding(serialized_embedding: &[u8]) -> Result<Vec<f32>, StorageError> {
    let remainder = serialized_embedding.len() % std::mem::size_of::<f32>();
    if remainder != 0 {
        return Err(StorageError::Rejected {
            reason: String::from("stored voice embedding has an invalid byte length"),
        });
    }
    Ok(serialized_embedding
        .chunks_exact(std::mem::size_of::<f32>())
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
        _ => Err(StorageError::Rejected {
            reason: String::from("stored task metric has an unknown task type"),
        }),
    }
}

fn sqlite_error(operation: &str, error: rusqlite::Error) -> StorageError {
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
            models::{MeetingId, Transcript, UnassignedVoiceClip},
        },
        ports::storage::NoteRepository,
    };

    /// Verifies adapter initialization creates its schema and stores both transcript and CRDT bytes.
    #[test]
    fn saves_and_loads_a_meeting_in_memory() {
        let repository = SqliteNoteRepository::open_in_memory()
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
        let repository = SqliteNoteRepository::open_in_memory()
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
        let repository = SqliteNoteRepository::open_in_memory()
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
