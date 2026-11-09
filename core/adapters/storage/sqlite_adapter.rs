//! Implements the note repository port with SQLite-backed transcript and Automerge BLOB storage.
//!
//! Voice embeddings in `unassigned_voice_clips`, `unknown_speaker_clusters`, and `cluster_chunks`
//! are sealed through the injected `EmbeddingCipher` (Issue #31); opening the repository
//! re-encrypts plaintext embeddings left by older databases. Review-clip audio is sealed the
//! same way (Issue #50), and unresolved clusters can be exported to and imported from paired
//! devices as `ClusterShare`s.
//!
//! Meeting titles (Issue #35) live in the nullable `meetings.title` column; opening a database
//! created earlier adds that column, leaving existing meetings untitled (`NULL`). Untitled
//! meetings get their display fallback on read (`MeetingMetadata::display_title`), so nothing is
//! back-filled.

use std::{path::Path, sync::Arc};

use rusqlite::{Connection, OptionalExtension, params};

/// Lists and stores the attachments exchanged with paired devices (Issue #57).
#[path = "sqlite_attachment_sync.rs"]
mod attachment_sync;
/// Exports and imports the cluster shares copied between paired devices (Issue #50).
#[path = "sqlite_cluster_share.rs"]
mod cluster_share;
/// Persists unknown-speaker clusters and their member chunks (Issue #28).
#[path = "sqlite_deletions.rs"]
mod deletions;

/// Keeps chats with notes, sealed (#97).
#[path = "sqlite_chat_history.rs"]
mod chat_history;
/// Indexes meetings folded into another device's recording of the same meeting (#88).
#[path = "sqlite_combined_meetings.rs"]
mod combined_meetings;
/// Stores projects, indexes their meetings, and exchanges them with paired devices (Issue #85).
#[path = "sqlite_projects.rs"]
mod projects;
#[path = "sqlite_unknown_speaker_clusters.rs"]
mod unknown_speaker_clusters;

use super::sqlite_clip_audio_encryption::{
    SEALED_AUDIO_FORMAT, open_clip_audio, prepare_clip_audio, seal_clip_audio,
};
use super::sqlite_embedding_encryption::{
    CLIP_EMBEDDINGS, CLUSTER_CHUNK_EMBEDDINGS, CLUSTER_REPRESENTATIVE_EMBEDDINGS,
    SEALED_EMBEDDING_FORMAT,
};
use super::sqlite_note_encryption::{self as note_encryption, NoteField};
use crate::{
    domain::{
        errors::StorageError,
        models::{
            AggregatedStats, Attachment, ClusterDigest, ClusterShare, ClusterShareImport, DeviceId,
            MeetingId, MeetingMetadata, MeetingTitle, TaskMetrics, TaskType, Transcript,
            TranscriptSegment, UnassignedVoiceClip, UnknownSpeakerChunk, UnknownSpeakerCluster,
        },
        projects::ProjectId,
        transcript_confidence::{OverlapSpan, SegmentConfidence},
        transcription_language::TranscriptionLanguage,
    },
    ports::{
        crypto::EmbeddingCipher,
        meeting_document::{
            MeetingCombination, MeetingDocumentCodec, MeetingOrigin, MeetingProjection,
        },
        storage::{AttachmentRepository, DocumentChanges, MeetingRecording, NoteRepository},
        telemetry::MetricsRepository,
    },
};
use rusqlite::types::Value as SqliteValue;

/// How long a write waits for a lock held by another connection (the voice store, or another
/// process such as the daemon) to the same database file.
const BUSY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Opens SQLite write transactions that take the write lock at once (`BEGIN IMMEDIATE`).
///
/// A deferred transaction that reads and then writes cannot wait for another connection doing
/// the same: SQLite fails one of them at once ("database is locked") to avoid a deadlock. The
/// pipeline, the P2P task, the after-meeting worker and the apps each have their own connection,
/// so every write transaction here starts immediate and waits up to `BUSY_TIMEOUT` instead.
pub(crate) trait WriteTransaction {
    /// Begins an immediate transaction on a shared connection.
    ///
    /// Outputs: The transaction (rolled back on drop unless committed), or `SQLITE_BUSY` after
    /// the busy timeout. Side effects: Holds the database's write lock until it ends.
    fn write_transaction(&self) -> rusqlite::Result<rusqlite::Transaction<'_>>;
}

impl WriteTransaction for rusqlite::Connection {
    fn write_transaction(&self) -> rusqlite::Result<rusqlite::Transaction<'_>> {
        rusqlite::Transaction::new_unchecked(self, rusqlite::TransactionBehavior::Immediate)
    }
}

/// Names the one-time copy of pre-#43 rows into meeting documents in `storage_migrations`.
const DOCUMENT_MIGRATION_NAME: &str = "meeting_documents_v1";

/// Persists meeting records locally without exposing SQLite types beyond the adapter boundary.
pub struct SqliteNoteRepository {
    connection: Connection,
    device_id: DeviceId,
    /// Seals and opens clip and cluster embeddings.
    embedding_cipher: Arc<dyn EmbeddingCipher>,
    /// When set, the meeting document is the source of truth and the transcript and segment
    /// rows are its index (Issue #43). Production composition always sets it.
    document_codec: Option<Arc<dyn MeetingDocumentCodec>>,
}

impl SqliteNoteRepository {
    /// Returns the acquisition device ID this repository stamps on records it creates.
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

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
        let repository = Self::from_connection(sqlite_connection, device_id, embedding_cipher)?;
        super::sqlite_secure_delete::vacuum_once(&repository.connection);
        Ok(repository)
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

    /// Reads this connection's `secure_delete` setting, for tests.
    #[cfg(test)]
    pub(super) fn secure_delete_setting(&self) -> Option<i64> {
        self.connection
            .query_row("PRAGMA secure_delete", [], |row| row.get(0))
            .ok()
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
        super::sqlite_secure_delete::enable(&sqlite_connection)?;
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
                    meeting_time_ms INTEGER,
                    FOREIGN KEY(meeting_id) REFERENCES meetings(meeting_id)
                );
                CREATE TABLE IF NOT EXISTS transcript_segments (
                    meeting_id TEXT NOT NULL,
                    start_time_ms INTEGER NOT NULL,
                    end_time_ms INTEGER NOT NULL,
                    speaker_identity_id TEXT,
                    segment_text TEXT NOT NULL,
                    unknown_cluster_id TEXT,
                    confidence TEXT,
                    FOREIGN KEY(meeting_id) REFERENCES meetings(meeting_id)
                );
                CREATE INDEX IF NOT EXISTS transcript_segments_meeting_index
                    ON transcript_segments(meeting_id, start_time_ms);",
            )
            .map_err(|error| sqlite_error("unable to initialize meetings schema", error))?;
        add_meeting_title_column(&sqlite_connection)?;
        add_meeting_transcription_languages_column(&sqlite_connection)?;
        add_task_audio_duration_column(&sqlite_connection)?;
        add_document_revision_column(&sqlite_connection)?;
        add_attachment_meeting_time_column(&sqlite_connection)?;
        add_segment_confidence_column(&sqlite_connection)?;
        attachment_sync::add_attachment_content_columns(&sqlite_connection)?;
        unknown_speaker_clusters::initialize_cluster_schema(&sqlite_connection)?;
        deletions::initialize_deletion_schema(&sqlite_connection)?;
        projects::initialize_project_schema(&sqlite_connection)?;
        chat_history::initialize_chat_schema(&sqlite_connection)?;
        combined_meetings::initialize_combination_schema(&sqlite_connection)?;
        for embedding_column in [
            &CLIP_EMBEDDINGS,
            &CLUSTER_REPRESENTATIVE_EMBEDDINGS,
            &CLUSTER_CHUNK_EMBEDDINGS,
        ] {
            embedding_column.prepare(&sqlite_connection, embedding_cipher.as_ref())?;
        }
        prepare_clip_audio(&sqlite_connection, embedding_cipher.as_ref())?;
        note_encryption::seal_clear_notes(&sqlite_connection, embedding_cipher.as_ref())?;
        tracing::info!(device_id = %device_id.as_str(), "SQLite meeting repository initialized");
        Ok(Self {
            connection: sqlite_connection,
            device_id,
            embedding_cipher,
            document_codec: None,
        })
    }

    /// Makes the meeting document the source of truth for origin, transcript, and segments.
    ///
    /// Inputs: The document codec (`application::meeting_document::AutomergeMeetingDocumentCodec`).
    /// Outputs: The repository, or a storage error when migrating legacy rows fails.
    /// Side effects: Copies transcript, segment, and origin rows that predate Issue #43 into their
    /// meeting documents once, in one transaction, so later document saves cannot wipe them.
    pub fn with_document_codec(
        mut self,
        document_codec: Arc<dyn MeetingDocumentCodec>,
    ) -> Result<Self, StorageError> {
        self.document_codec = Some(document_codec);
        self.migrate_rows_into_documents()?;
        Ok(self)
    }

    /// Returns the device that recorded `meeting_id`, or `None` for an unknown meeting.
    ///
    /// Side effects: Reads one row.
    pub fn acquisition_device_id(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Option<DeviceId>, StorageError> {
        self.connection
            .query_row(
                "SELECT acquisition_device_id FROM meetings WHERE meeting_id = ?1",
                params![meeting_id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map(|device_id| device_id.map(DeviceId::new))
            .map_err(|error| sqlite_error("unable to load the acquisition device", error))
    }

    /// Copies index rows that their documents lack into those documents.
    ///
    /// Side effects: Rewrites the documents of legacy meetings in one transaction; logs the count.
    fn migrate_rows_into_documents(&self) -> Result<(), StorageError> {
        let Some(document_codec) = self.document_codec.as_deref() else {
            return Ok(());
        };
        let transaction = self
            .connection
            .write_transaction()
            .map_err(|error| sqlite_error("unable to begin document migration", error))?;
        transaction
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS storage_migrations (name TEXT PRIMARY KEY NOT NULL);",
            )
            .map_err(|error| sqlite_error("unable to prepare the migration record", error))?;
        let already_migrated: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM storage_migrations WHERE name = ?1)",
                params![DOCUMENT_MIGRATION_NAME],
                |row| row.get(0),
            )
            .map_err(|error| sqlite_error("unable to read the migration record", error))?;
        if already_migrated {
            return Ok(());
        }
        let mut migrated_meetings = 0_usize;
        for meeting_id in self.list_meeting_ids()? {
            let original_document =
                read_document(&transaction, self.embedding_cipher.as_ref(), &meeting_id)?;
            let projection = document_codec.project(&meeting_id, &original_document)?;
            let mut document = original_document.clone();
            if projection.origin.is_none() {
                let created_at = self
                    .load_meeting_metadata(&meeting_id)?
                    .map(|metadata| metadata.created_at())
                    .unwrap_or_else(chrono::Utc::now);
                let acquisition_device_id = self
                    .acquisition_device_id(&meeting_id)?
                    .unwrap_or_else(|| self.device_id.clone());
                document = document_codec.with_origin(
                    &document,
                    &MeetingOrigin {
                        created_at,
                        acquisition_device_id,
                    },
                )?;
            }
            if projection.transcript.is_none()
                && let Some(transcript) = self.load_transcript(&meeting_id)?
            {
                document = document_codec.with_transcript(&document, transcript.content())?;
            }
            if projection.segments.is_none() {
                let segments = self.load_transcript_segments(&meeting_id)?;
                if !segments.is_empty() {
                    document = document_codec.with_segments(&document, &segments)?;
                }
            }
            if document != original_document {
                write_document(
                    &transaction,
                    self.embedding_cipher.as_ref(),
                    &meeting_id,
                    &document,
                )?;
                migrated_meetings += 1;
            }
        }
        transaction
            .execute(
                "INSERT INTO storage_migrations (name) VALUES (?1)",
                params![DOCUMENT_MIGRATION_NAME],
            )
            .map_err(|error| sqlite_error("unable to record the document migration", error))?;
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit document migration", error))?;
        if migrated_meetings > 0 {
            tracing::info!(
                migrated_meetings,
                "Copied legacy meeting rows into CRDT documents"
            );
        }
        Ok(())
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
    fn read_attachment_file(
        &self,
        stored_path: &std::path::Path,
        max_bytes: u64,
    ) -> Option<zeroize::Zeroizing<Vec<u8>>> {
        let sealed_files = self.sealed_files();
        let length = sealed_files.clear_len(stored_path).ok()?;
        if length == 0 || length > max_bytes {
            return None;
        }
        sealed_files.read_all(stored_path).ok()
    }

    /// Saves user-approved attachment metadata after verifying any requested meeting exists.
    ///
    /// Inputs: `attachment` contains metadata only and may omit its meeting until confirmation.
    /// Outputs: Success or a portable rejection for invalid meeting references/SQLite failures.
    /// Side effects: Writes one local SQLite attachment row without opening the attachment file.
    fn save_attachment(&self, attachment: &Attachment) -> Result<(), StorageError> {
        let meeting_id = attachment.meeting_id().map(MeetingId::as_str);
        let inserted_row_count = self.connection.execute(
            "INSERT INTO attachments (attachment_id, meeting_id, file_path, mime_type, original_timestamp_utc, original_device_id, meeting_time_ms, project_id)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8
             WHERE (?2 IS NULL OR EXISTS (SELECT 1 FROM meetings WHERE meeting_id = ?2))
               AND (?8 IS NULL OR EXISTS (SELECT 1 FROM projects WHERE project_id = ?8))",
            params![
                attachment.attachment_id().to_string(),
                meeting_id,
                attachment.file_path(),
                attachment.mime_type(),
                attachment.original_timestamp().to_rfc3339(),
                attachment.original_device_id(),
                attachment
                    .meeting_time_ms()
                    .map(|meeting_time_ms| i64::try_from(meeting_time_ms).unwrap_or(i64::MAX)),
                attachment.project_id().map(|project_id| project_id.to_string()),
            ],
        ).map_err(|error| sqlite_error("unable to save attachment", error))?;
        if inserted_row_count == 0 {
            return Err(StorageError::Rejected {
                reason: String::from("cannot save attachment for an unknown meeting or project"),
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
        self.load_attachments(AttachmentOwnerFilter::Meeting(meeting_id))
    }

    /// Loads every imported attachment awaiting explicit meeting selection.
    ///
    /// Inputs: None.
    /// Outputs: Unlinked attachment metadata in insertion order, or a portable storage error.
    /// Side effects: Reads SQLite metadata only and never opens attachment files.
    fn get_unlinked_attachments(&self) -> Result<Vec<Attachment>, StorageError> {
        self.load_attachments(AttachmentOwnerFilter::Unlinked)
    }
}

impl SqliteNoteRepository {
    /// Reads the attachment rows of one meeting, of one project, or of the unlinked import queue.
    ///
    /// Inputs: Which owner's rows to read. Outputs: Parsed attachment metadata in insertion
    /// order, or a portable storage error.
    /// Side effects: Reads SQLite rows only and never opens attachment files.
    pub(super) fn load_attachments(
        &self,
        owner: AttachmentOwnerFilter<'_>,
    ) -> Result<Vec<Attachment>, StorageError> {
        const COLUMNS: &str = "SELECT attachment_id, meeting_id, file_path, mime_type, original_timestamp_utc, original_device_id, meeting_time_ms, project_id FROM attachments";
        let project_text;
        let (condition, parameter) = match owner {
            AttachmentOwnerFilter::Meeting(meeting_id) => {
                ("meeting_id = ?1", Some(meeting_id.as_str()))
            }
            AttachmentOwnerFilter::Project(project_id) => {
                project_text = project_id.to_string();
                ("project_id = ?1", Some(project_text.as_str()))
            }
            // Project resources have no meeting but are not waiting for one.
            AttachmentOwnerFilter::Unlinked => (
                "meeting_id IS NULL AND project_id IS NULL AND ?1 IS NULL",
                None,
            ),
            AttachmentOwnerFilter::Id(attachment_id) => {
                project_text = attachment_id.to_string();
                ("attachment_id = ?1", Some(project_text.as_str()))
            }
        };
        let query = format!("{COLUMNS} WHERE {condition} ORDER BY rowid");
        let query = query.as_str();
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
                    row.get::<_, Option<i64>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            })
            .map_err(|error| sqlite_error("unable to load attachments", error))?;
        rows.map(|row| {
            let (
                attachment_id,
                persisted_meeting_id,
                file_path,
                mime_type,
                timestamp,
                device_id,
                meeting_time_ms,
                project_text,
            ) = row.map_err(|error| sqlite_error("unable to decode attachment", error))?;
            let parsed_attachment_id =
                uuid::Uuid::parse_str(&attachment_id).map_err(|error| StorageError::Rejected {
                    reason: format!("stored attachment has an invalid UUID: {error}"),
                })?;
            let parsed_timestamp = chrono::DateTime::parse_from_rfc3339(&timestamp)
                .map_err(|error| StorageError::Rejected {
                    reason: format!("stored attachment has an invalid timestamp: {error}"),
                })?
                .with_timezone(&chrono::Utc);
            let attachment = Attachment::new(
                parsed_attachment_id,
                persisted_meeting_id.map(MeetingId::new),
                file_path,
                mime_type,
                parsed_timestamp,
                device_id,
            );
            // A negative value can only come from outside the app; it is treated as unknown.
            let attachment = match meeting_time_ms.and_then(|value| u64::try_from(value).ok()) {
                Some(meeting_time_ms) => attachment.with_meeting_time_ms(meeting_time_ms),
                None => attachment,
            };
            Ok(match project_text.as_deref().and_then(ProjectId::parse) {
                Some(project_id) => attachment.with_project_id(project_id),
                None => attachment,
            })
        })
        .collect()
    }
}

/// Selects whose attachment rows `load_attachments` reads.
#[derive(Clone, Copy, Debug)]
pub(super) enum AttachmentOwnerFilter<'owner> {
    /// The attachments of one meeting.
    Meeting(&'owner MeetingId),
    /// The resources of one project (Issue #85).
    Project(ProjectId),
    /// Imports waiting for the owner to pick a meeting.
    Unlinked,
    /// One attachment or resource, whoever owns it (#99).
    Id(uuid::Uuid),
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

    fn meeting_recordings(&self) -> Result<Vec<MeetingRecording>, StorageError> {
        self.read_meeting_recordings()
    }

    fn combine_meeting(
        &self,
        meeting_id: &MeetingId,
        combination: &MeetingCombination,
    ) -> Result<(), StorageError> {
        if meeting_id == &combination.into {
            return Err(StorageError::Rejected {
                reason: String::from("a meeting cannot be combined with itself"),
            });
        }
        self.read_combination(&combination.into)?;
        self.rewrite_combination(meeting_id, |codec, document| {
            codec.with_combination(document, Some(combination))
        })?;
        self.log_operation("combine_meeting", meeting_id);
        Ok(())
    }

    fn split_meeting(&self, meeting_id: &MeetingId) -> Result<bool, StorageError> {
        let Some(into) = self.read_combination(meeting_id)? else {
            return Ok(false);
        };
        self.rewrite_combination(meeting_id, |codec, document| {
            codec
                .with_combination(document, None)
                .and_then(|document| codec.with_kept_apart(&document, &into))
        })?;
        self.log_operation("split_meeting", meeting_id);
        Ok(true)
    }

    fn describe_recording_device(
        &self,
        meeting_id: &MeetingId,
        on_phone: bool,
    ) -> Result<(), StorageError> {
        self.read_combination(meeting_id)?;
        self.rewrite_combination(meeting_id, |codec, document| {
            codec.with_recording_device_kind(document, on_phone)
        })?;
        Ok(())
    }

    fn mark_combined(
        &self,
        meeting_id: &MeetingId,
        recordings: &[MeetingId],
    ) -> Result<(), StorageError> {
        self.read_combination(meeting_id)?;
        self.rewrite_combination(meeting_id, |codec, document| {
            codec.with_combined_with(document, recordings)
        })?;
        self.log_operation("mark_combined", meeting_id);
        Ok(())
    }

    /// Reads a meeting's SQLite creation timestamp, title and transcription languages as
    /// portable metadata.
    ///
    /// Inputs: `meeting_id` identifies a meeting record.
    /// Outputs: Metadata (untitled when the column is `NULL` or holds text that is no longer a
    /// valid title; no languages for meetings recorded before they were kept), `None` for an
    /// unknown meeting, or a portable storage error.
    /// Side effects: Reads one SQLite row without transcript or CRDT content; logs a warning
    /// (without the text) when a stored title or language is invalid.
    fn load_meeting_metadata(
        &self,
        meeting_id: &MeetingId,
    ) -> Result<Option<MeetingMetadata>, StorageError> {
        let metadata_row = self
            .connection
            .query_row(
                "SELECT created_at_utc, title, transcription_languages FROM meetings WHERE meeting_id = ?1",
                params![meeting_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| sqlite_error("unable to load meeting metadata", error))?;
        let Some((created_at_text, stored_title, stored_languages)) = metadata_row else {
            return Ok(None);
        };
        let transcription_languages =
            parse_stored_transcription_languages(meeting_id, stored_languages.as_deref());
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
            Some(
                MeetingMetadata::new(meeting_id.clone(), created_at)
                    .with_title(title)
                    .with_transcription_languages(transcription_languages),
            )
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
            .write_transaction()
            .map_err(|error| sqlite_error("unable to begin segment transaction", error))?;
        if let Some(document_codec) = self.document_codec.as_deref() {
            let document = document_codec.with_segments(
                &read_document(&transaction, self.embedding_cipher.as_ref(), meeting_id)?,
                segments,
            )?;
            write_document(
                &transaction,
                self.embedding_cipher.as_ref(),
                meeting_id,
                &document,
            )?;
        }
        write_segment_rows(
            &transaction,
            self.embedding_cipher.as_ref(),
            meeting_id,
            segments,
        )?;
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
                "SELECT start_time_ms, end_time_ms, speaker_identity_id, segment_text, unknown_cluster_id, confidence FROM transcript_segments WHERE meeting_id = ?1 ORDER BY start_time_ms, rowid",
            )
            .map_err(|error| sqlite_error("unable to prepare transcript segment query", error))?;
        let segment_rows = statement
            .query_map(params![meeting_id.as_str()], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, SqliteValue>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
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
                    confidence,
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
                    note_encryption::open_text(
                        self.embedding_cipher.as_ref(),
                        NoteField::SegmentText,
                        meeting_id,
                        segment_text,
                    )?,
                )
                .with_unknown_cluster_id(parsed_unknown_cluster_id)
                .with_confidence(confidence_from_column(confidence.as_deref())))
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
        let Some(document_codec) = self.document_codec.as_deref() else {
            self.connection
                .execute(
                    &format!(
                        "INSERT INTO meetings (meeting_id, acquisition_device_id, automerge_document, document_revision) VALUES (?1, ?2, ?3, {NEXT_DOCUMENT_REVISION_SQL})"
                    ),
                    params![
                        meeting_id.as_str(),
                        self.device_id.as_str(),
                        note_encryption::seal_field(
                            self.embedding_cipher.as_ref(),
                            NoteField::Document,
                            meeting_id,
                            initial_crdt_document,
                        )?
                    ],
                )
                .map_err(|error| sqlite_error("unable to create meeting", error))?;
            self.log_operation("create_meeting", meeting_id);
            return Ok(());
        };
        // A document from a peer already names its origin; a new local meeting gets this device.
        let origin = match document_codec
            .project(meeting_id, initial_crdt_document)?
            .origin
        {
            Some(peer_origin) => peer_origin,
            None => MeetingOrigin {
                created_at: whole_seconds(chrono::Utc::now()),
                acquisition_device_id: self.device_id.clone(),
            },
        };
        let document = document_codec.with_origin(initial_crdt_document, &origin)?;
        let transaction = self
            .connection
            .write_transaction()
            .map_err(|error| sqlite_error("unable to begin meeting creation", error))?;
        transaction
            .execute(
                &format!(
                    "INSERT INTO meetings (meeting_id, created_at_utc, acquisition_device_id, automerge_document, document_revision) VALUES (?1, ?2, ?3, ?4, {NEXT_DOCUMENT_REVISION_SQL})"
                ),
                params![
                    meeting_id.as_str(),
                    sqlite_timestamp_text(origin.created_at),
                    origin.acquisition_device_id.as_str(),
                    note_encryption::seal_field(
                        self.embedding_cipher.as_ref(),
                        NoteField::Document,
                        meeting_id,
                        &document,
                    )?
                ],
            )
            .map_err(|error| sqlite_error("unable to create meeting", error))?;
        apply_projection(
            &transaction,
            self.embedding_cipher.as_ref(),
            meeting_id,
            &document_codec.project(meeting_id, &document)?,
        )?;
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit meeting creation", error))?;
        self.log_operation("create_meeting", meeting_id);
        Ok(())
    }

    /// Persists text only for a pre-existing meeting row.
    ///
    /// Inputs: `transcript` must belong to a meeting created through this repository.
    /// Outputs: Success or an unknown-meeting/SQLite storage error.
    /// Side effects: Updates one SQLite row and emits a debug trace event without the transcript content.
    fn save_transcript(&self, transcript: &Transcript) -> Result<(), StorageError> {
        if let Some(document_codec) = self.document_codec.as_deref() {
            let meeting_id = transcript.meeting_id();
            let transaction = self
                .connection
                .write_transaction()
                .map_err(|error| sqlite_error("unable to begin transcript transaction", error))?;
            let document = document_codec.with_transcript(
                &read_document(&transaction, self.embedding_cipher.as_ref(), meeting_id)?,
                transcript.content(),
            )?;
            write_document(
                &transaction,
                self.embedding_cipher.as_ref(),
                meeting_id,
                &document,
            )?;
            transaction
                .execute(
                    "UPDATE meetings SET transcript = ?1 WHERE meeting_id = ?2",
                    params![
                        note_encryption::seal_text(
                            self.embedding_cipher.as_ref(),
                            NoteField::Transcript,
                            meeting_id,
                            transcript.content(),
                        )?,
                        meeting_id.as_str()
                    ],
                )
                .map_err(|error| sqlite_error("unable to save transcript", error))?;
            transaction
                .commit()
                .map_err(|error| sqlite_error("unable to commit transcript", error))?;
            self.log_operation("save_transcript", meeting_id);
            return Ok(());
        }
        let updated_row_count = self
            .connection
            .execute(
                "UPDATE meetings SET transcript = ?1 WHERE meeting_id = ?2",
                params![
                    note_encryption::seal_text(
                        self.embedding_cipher.as_ref(),
                        NoteField::Transcript,
                        transcript.meeting_id(),
                        transcript.content(),
                    )?,
                    transcript.meeting_id().as_str()
                ],
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
                |row| row.get::<_, SqliteValue>(0),
            )
            .optional()
            .map_err(|error| sqlite_error("unable to load transcript", error))?
            .ok_or_else(|| missing_meeting_error(meeting_id))?;
        self.log_operation("load_transcript", meeting_id);
        if optional_transcript == SqliteValue::Null {
            return Ok(None);
        }
        let transcript_content = note_encryption::open_text(
            self.embedding_cipher.as_ref(),
            NoteField::Transcript,
            meeting_id,
            optional_transcript,
        )?;
        Ok(Some(Transcript::new(
            meeting_id.clone(),
            transcript_content,
        )))
    }

    /// Replaces opaque Automerge bytes only for an existing meeting row.
    ///
    /// Inputs: `meeting_id` must exist; `crdt_document` contains serialized CRDT state.
    /// Outputs: Success or an unknown-meeting/SQLite storage error.
    /// Side effects: Updates one SQLite row and emits a debug trace event without CRDT bytes.
    fn document_changes_since(&self, after_revision: u64) -> Result<DocumentChanges, StorageError> {
        let after_revision = i64::try_from(after_revision).unwrap_or(i64::MAX);
        let mut statement = self
            .connection
            .prepare_cached(
                "SELECT meeting_id, document_revision FROM meetings WHERE document_revision > ?1 ORDER BY document_revision",
            )
            .map_err(|error| sqlite_error("unable to list document changes", error))?;
        let changed_rows = statement
            .query_map(params![after_revision], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(|error| sqlite_error("unable to list document changes", error))?
            .collect::<Result<Vec<(String, i64)>, _>>()
            .map_err(|error| sqlite_error("unable to read document changes", error))?;
        let latest_revision = changed_rows
            .last()
            .map_or(after_revision, |(_, revision)| *revision);
        Ok(DocumentChanges {
            meeting_ids: changed_rows
                .into_iter()
                .map(|(meeting_id, _)| MeetingId::new(meeting_id))
                .collect(),
            latest_revision: u64::try_from(latest_revision).unwrap_or(0),
        })
    }

    fn save_crdt_document(
        &self,
        meeting_id: &MeetingId,
        crdt_document: &[u8],
    ) -> Result<(), StorageError> {
        let transaction = self
            .connection
            .write_transaction()
            .map_err(|error| sqlite_error("unable to begin document transaction", error))?;
        write_document(
            &transaction,
            self.embedding_cipher.as_ref(),
            meeting_id,
            crdt_document,
        )?;
        // The document may carry a peer's changes; the index follows it.
        if let Some(document_codec) = self.document_codec.as_deref() {
            apply_projection(
                &transaction,
                self.embedding_cipher.as_ref(),
                meeting_id,
                &document_codec.project(meeting_id, crdt_document)?,
            )?;
        }
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit CRDT document", error))?;
        self.log_operation("save_crdt_document", meeting_id);
        Ok(())
    }

    /// Changes a meeting's document inside one write transaction (#95).
    ///
    /// Inputs: Match `NoteRepository::edit_crdt_document`. Outputs: Success, or the change's or
    /// SQLite's error. Side effects: Rewrites the sealed document and its index when it changed.
    fn edit_crdt_document(
        &self,
        meeting_id: &MeetingId,
        edit: &crate::ports::storage::DocumentEdit<'_>,
    ) -> Result<(), StorageError> {
        let transaction = self
            .connection
            .write_transaction()
            .map_err(|error| sqlite_error("unable to begin document edit", error))?;
        let document = read_document(&transaction, self.embedding_cipher.as_ref(), meeting_id)?;
        let updated = edit(&document)?;
        if updated != document {
            write_document(
                &transaction,
                self.embedding_cipher.as_ref(),
                meeting_id,
                &updated,
            )?;
            if let Some(document_codec) = self.document_codec.as_deref() {
                apply_projection(
                    &transaction,
                    self.embedding_cipher.as_ref(),
                    meeting_id,
                    &document_codec.project(meeting_id, &updated)?,
                )?;
            }
        }
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit document edit", error))?;
        self.log_operation("edit_crdt_document", meeting_id);
        Ok(())
    }

    /// Loads opaque Automerge bytes for an existing meeting row.
    ///
    /// Inputs: `meeting_id` identifies a meeting record.
    /// Outputs: Serialized CRDT bytes or a portable storage error.
    /// Side effects: Reads one SQLite row and emits a debug trace event without CRDT bytes.
    fn load_crdt_document(&self, meeting_id: &MeetingId) -> Result<Vec<u8>, StorageError> {
        let serialized_document =
            read_document(&self.connection, self.embedding_cipher.as_ref(), meeting_id)?;
        self.log_operation("load_crdt_document", meeting_id);
        Ok(serialized_document)
    }

    /// Persists one temporary manual-review clip for an existing meeting.
    ///
    /// Inputs: `clip` must reference a meeting created through this repository.
    /// Outputs: Success or an unknown-meeting/SQLite storage error.
    /// Side effects: Stores the temporary review audio and the derived embedding, both sealed
    /// with the database cipher, without logging either.
    fn save_unassigned_clip(&self, clip: UnassignedVoiceClip) -> Result<(), StorageError> {
        let clip_id = clip.clip_id().to_string();
        let sealed_embedding = CLIP_EMBEDDINGS.seal(
            self.embedding_cipher.as_ref(),
            &[&clip_id],
            clip.derived_embedding(),
        )?;
        let sealed_audio =
            seal_clip_audio(self.embedding_cipher.as_ref(), &clip_id, clip.audio_bytes())?;
        let inserted_row_count = self.connection.execute(
            "INSERT INTO unassigned_voice_clips (clip_id, meeting_id, start_time_ms, end_time_ms, audio_bytes, derived_embedding, weak_match_identity_id, weak_match_confidence_score, cluster_id, embedding_format, audio_format)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11 WHERE EXISTS (SELECT 1 FROM meetings WHERE meeting_id = ?2)",
            params![clip_id, clip.meeting_id().as_str(), clip.start_time_ms(), clip.end_time_ms(), sealed_audio, sealed_embedding, clip.weak_match_candidate().map(|candidate| candidate.identity_id().to_string()), clip.weak_match_candidate().map(|candidate| candidate.confidence_score()), clip.cluster_id().map(|cluster_id| cluster_id.to_string()), SEALED_EMBEDDING_FORMAT, SEALED_AUDIO_FORMAT],
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
                    open_clip_audio(self.embedding_cipher.as_ref(), &clip_id, &audio_bytes)?,
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

    /// Replaces a cluster's members atomically.
    ///
    /// Inputs: Match `NoteRepository::replace_unknown_speaker_cluster_members`.
    /// Outputs: Success or a portable storage error.
    /// Side effects: Rewrites sealed biometric BLOBs.
    fn replace_unknown_speaker_cluster_members(
        &self,
        cluster: &UnknownSpeakerCluster,
        chunks: &[UnknownSpeakerChunk],
    ) -> Result<(), StorageError> {
        self.replace_cluster_members(cluster, chunks)
    }

    /// Deletes a cluster, its members and clips atomically.
    ///
    /// Inputs: Match `NoteRepository::delete_unknown_speaker_cluster`.
    /// Outputs: Success or a portable storage error.
    /// Side effects: Deletes biometric BLOBs and temporary review audio.
    fn delete_unknown_speaker_cluster(&self, cluster_id: uuid::Uuid) -> Result<(), StorageError> {
        self.delete_cluster(cluster_id)
    }

    /// Records that `language` was used to transcribe part of a meeting.
    ///
    /// Inputs: Match `NoteRepository::record_transcription_language`.
    /// Outputs: Success (also when the language was already recorded, or when this repository
    /// has no meeting documents), or a portable storage error for an unknown meeting.
    /// Side effects: Updates the meeting's CRDT document and its indexed language list in one
    /// transaction, so paired devices receive it with the next sync.
    fn record_transcription_language(
        &self,
        meeting_id: &MeetingId,
        language: TranscriptionLanguage,
        first_used_at_ms: u64,
    ) -> Result<(), StorageError> {
        let Some(document_codec) = self.document_codec.as_deref() else {
            return Ok(());
        };
        let transaction = self
            .connection
            .write_transaction()
            .map_err(|error| sqlite_error("unable to begin recording a language", error))?;
        let document = read_document(&transaction, self.embedding_cipher.as_ref(), meeting_id)?;
        let updated_document =
            document_codec.with_transcription_language(&document, language, first_used_at_ms)?;
        if updated_document != document {
            write_document(
                &transaction,
                self.embedding_cipher.as_ref(),
                meeting_id,
                &updated_document,
            )?;
            apply_projection(
                &transaction,
                self.embedding_cipher.as_ref(),
                meeting_id,
                &document_codec.project(meeting_id, &updated_document)?,
            )?;
        }
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit recording a language", error))?;
        self.log_operation("record_transcription_language", meeting_id);
        Ok(())
    }

    /// Adds overlap spans to the meeting's CRDT document (#90).
    ///
    /// Inputs: Match `NoteRepository::record_overlap_spans`.
    /// Outputs: Success (also without meeting documents or spans), or a storage error for an
    /// unknown meeting.
    /// Side effects: Writes the meeting's document, which syncs to paired devices.
    fn record_overlap_spans(
        &self,
        meeting_id: &MeetingId,
        spans: &[OverlapSpan],
    ) -> Result<(), StorageError> {
        let Some(document_codec) = self.document_codec.as_deref() else {
            return Ok(());
        };
        if spans.is_empty() {
            return Ok(());
        }
        let transaction = self
            .connection
            .write_transaction()
            .map_err(|error| sqlite_error("unable to begin recording overlap", error))?;
        let document = read_document(&transaction, self.embedding_cipher.as_ref(), meeting_id)?;
        let updated_document = document_codec.with_overlap_spans(&document, spans)?;
        if updated_document != document {
            write_document(
                &transaction,
                self.embedding_cipher.as_ref(),
                meeting_id,
                &updated_document,
            )?;
        }
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit recording overlap", error))?;
        self.log_operation("record_overlap_spans", meeting_id);
        Ok(())
    }

    /// Reads the meeting's overlap spans from its CRDT document (#90).
    ///
    /// Inputs: Match `NoteRepository::load_overlap_spans`.
    /// Outputs: Disjoint spans in time order; none without meeting documents.
    /// Side effects: Reads the document.
    fn load_overlap_spans(&self, meeting_id: &MeetingId) -> Result<Vec<OverlapSpan>, StorageError> {
        let Some(document_codec) = self.document_codec.as_deref() else {
            return Ok(Vec::new());
        };
        let transaction = self
            .connection
            .write_transaction()
            .map_err(|error| sqlite_error("unable to begin reading overlap", error))?;
        let document = read_document(&transaction, self.embedding_cipher.as_ref(), meeting_id)?;
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to finish reading overlap", error))?;
        document_codec.overlap_spans(&document)
    }

    /// Deletes a meeting and everything it holds, leaving a deletion record.
    ///
    /// Inputs: Match `NoteRepository::delete_meeting`.
    /// Outputs: Whether the meeting existed, or a portable storage error.
    /// Side effects: Deletes its rows atomically. Never logs titles or transcripts.
    fn delete_meeting(&self, meeting_id: &MeetingId) -> Result<bool, StorageError> {
        let existed = self.delete_meeting_rows(meeting_id)?;
        self.log_operation("delete_meeting", meeting_id);
        Ok(existed)
    }

    /// Deletes an unknown voice, leaving a deletion record.
    ///
    /// Inputs: Match `NoteRepository::delete_unknown_voice`.
    /// Outputs: Whether the voice existed, or a portable storage error.
    /// Side effects: Deletes biometric BLOBs and temporary review audio atomically.
    fn delete_unknown_voice(&self, cluster_id: uuid::Uuid) -> Result<bool, StorageError> {
        self.delete_unknown_voice_rows(cluster_id)
    }

    /// Tells whether this device holds a deletion record for `meeting_id`.
    ///
    /// Inputs: Match `NoteRepository::is_meeting_deleted`.
    /// Outputs: The answer, or a portable storage error. Side effects: Reads one row.
    fn is_meeting_deleted(&self, meeting_id: &MeetingId) -> Result<bool, StorageError> {
        deletions::is_meeting_recorded(&self.connection, meeting_id)
    }

    /// Lists every deletion record.
    ///
    /// Outputs: The records, or a portable storage error. Side effects: Reads IDs only.
    fn deletion_records(&self) -> Result<crate::domain::models::DeletionRecords, StorageError> {
        self.read_deletion_records()
    }

    /// Applies a paired device's deletion records.
    ///
    /// Inputs: Match `NoteRepository::apply_deletion_records`.
    /// Outputs: The meetings and project resources removed here by this call, or a portable
    /// storage error.
    /// Side effects: Deletes rows and leaves records atomically.
    fn apply_deletion_records(
        &self,
        records: &crate::domain::models::DeletionRecords,
    ) -> Result<crate::domain::models::AppliedDeletions, StorageError> {
        self.apply_deletion_rows(records)
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

    /// Counts the chunks and clips of every unresolved cluster.
    fn unresolved_cluster_digests(&self) -> Result<Vec<ClusterDigest>, StorageError> {
        self.cluster_digests()
    }

    /// Loads and opens one unresolved cluster with its chunks and clips.
    fn load_cluster_share(
        &self,
        cluster_id: uuid::Uuid,
    ) -> Result<Option<ClusterShare>, StorageError> {
        self.cluster_share(cluster_id)
    }

    /// Re-seals and stores a share from a paired device.
    fn import_cluster_share(
        &self,
        share: &ClusterShare,
    ) -> Result<ClusterShareImport, StorageError> {
        self.import_share(share)
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
            "INSERT INTO task_metrics (task_id, device_id, task_type, model_name, duration_ms, latency_ms, estimated_power_mwh, timestamp_epoch_ms, audio_duration_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![metrics.task_id().to_string(), metrics.device_id().as_str(), metrics.task_type().as_str(), metrics.model_name(), metrics.duration_ms(), metrics.latency_ms(), metrics.estimated_power_mwh(), metrics.timestamp_epoch_ms(), metrics.audio_duration_ms()],
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
        let mut statement = self.connection.prepare("SELECT task_id, task_type, model_name, duration_ms, latency_ms, estimated_power_mwh, timestamp_epoch_ms, audio_duration_ms FROM task_metrics WHERE device_id = ?1 ORDER BY timestamp_epoch_ms").map_err(|error| sqlite_error("unable to prepare device metrics query", error))?;
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
                    row.get::<_, Option<u64>>(7)?,
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
                audio_duration_ms,
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
            )
            .with_audio_duration_ms(audio_duration_ms))
        })
        .collect()
    }

    /// Loads every completed metric so the local dashboard can form cross-device aggregates.
    ///
    /// Inputs: None.
    /// Outputs: Privacy-safe metric metadata in timestamp order or a portable storage error.
    /// Side effects: Reads SQLite rows only; it does not load AI inputs, outputs, or raw audio.
    fn get_all_metrics(&self) -> Result<Vec<TaskMetrics>, StorageError> {
        let mut statement = self.connection.prepare("SELECT task_id, device_id, task_type, model_name, duration_ms, latency_ms, estimated_power_mwh, timestamp_epoch_ms, audio_duration_ms FROM task_metrics ORDER BY timestamp_epoch_ms").map_err(|error| sqlite_error("unable to prepare all metrics query", error))?;
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
                    row.get::<_, Option<u64>>(8)?,
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
                audio_duration_ms,
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
            )
            .with_audio_duration_ms(audio_duration_ms))
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

/// Reads a meeting's serialized document.
///
/// Outputs: The document bytes, or a rejection for an unknown meeting. Side effects: None.
fn read_document(
    connection: &Connection,
    cipher: &dyn EmbeddingCipher,
    meeting_id: &MeetingId,
) -> Result<Vec<u8>, StorageError> {
    let stored = connection
        .query_row(
            "SELECT automerge_document FROM meetings WHERE meeting_id = ?1",
            params![meeting_id.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| sqlite_error("unable to load CRDT document", error))?
        .ok_or_else(|| missing_meeting_error(meeting_id))?;
    note_encryption::open_document(cipher, meeting_id, stored)
}

/// Replaces a meeting's serialized document, sealed (#92).
///
/// Outputs: Success, or a rejection for an unknown meeting. Side effects: Updates one row.
fn write_document(
    connection: &Connection,
    cipher: &dyn EmbeddingCipher,
    meeting_id: &MeetingId,
    document: &[u8],
) -> Result<(), StorageError> {
    let document = note_encryption::seal_field(
        cipher,
        note_encryption::NoteField::Document,
        meeting_id,
        document,
    )?;
    let updated_row_count = connection
        .execute(
            &format!(
                "UPDATE meetings SET automerge_document = ?1, document_revision = {NEXT_DOCUMENT_REVISION_SQL} WHERE meeting_id = ?2"
            ),
            params![document, meeting_id.as_str()],
        )
        .map_err(|error| sqlite_error("unable to save CRDT document", error))?;
    ensure_meeting_was_updated(updated_row_count, meeting_id)
}

/// Replaces a meeting's segment rows with `segments`.
///
/// Side effects: Deletes and inserts rows; callers provide the transaction.
fn write_segment_rows(
    connection: &Connection,
    cipher: &dyn EmbeddingCipher,
    meeting_id: &MeetingId,
    segments: &[TranscriptSegment],
) -> Result<(), StorageError> {
    connection
        .execute(
            "DELETE FROM transcript_segments WHERE meeting_id = ?1",
            params![meeting_id.as_str()],
        )
        .map_err(|error| sqlite_error("unable to clear transcript segments", error))?;
    for segment in segments {
        connection
            .execute(
                "INSERT INTO transcript_segments (meeting_id, start_time_ms, end_time_ms, speaker_identity_id, segment_text, unknown_cluster_id, confidence) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    meeting_id.as_str(),
                    sqlite_milliseconds(segment.start_time_ms())?,
                    sqlite_milliseconds(segment.end_time_ms())?,
                    segment.speaker_identity_id().map(|identity_id| identity_id.to_string()),
                    note_encryption::seal_text(
                        cipher,
                        note_encryption::NoteField::SegmentText,
                        meeting_id,
                        segment.text(),
                    )?,
                    segment.unknown_cluster_id().map(|cluster_id| cluster_id.to_string()),
                    confidence_column(segment.confidence()),
                ],
            )
            .map_err(|error| sqlite_error("unable to save transcript segment", error))?;
    }
    Ok(())
}

/// Rewrites the index fields the document holds; fields it lacks keep their rows, except the
/// project, which the document always decides (Issue #85).
///
/// Side effects: Updates the transcript and project columns and segment rows; marks each cluster the document
/// resolves as resolved (to the document's identity) and deletes its review clips (Issue #50).
/// Callers provide the transaction.
fn apply_projection(
    connection: &Connection,
    cipher: &dyn EmbeddingCipher,
    meeting_id: &MeetingId,
    projection: &MeetingProjection,
) -> Result<(), StorageError> {
    if let Some(transcript) = &projection.transcript {
        connection
            .execute(
                "UPDATE meetings SET transcript = ?1 WHERE meeting_id = ?2",
                params![
                    note_encryption::seal_text(
                        cipher,
                        note_encryption::NoteField::Transcript,
                        meeting_id,
                        transcript
                    )?,
                    meeting_id.as_str()
                ],
            )
            .map_err(|error| sqlite_error("unable to project the transcript", error))?;
    }
    if let Some(segments) = &projection.segments {
        write_segment_rows(connection, cipher, meeting_id, segments)?;
    }
    connection
        .execute(
            "UPDATE meetings SET project_id = ?1 WHERE meeting_id = ?2 AND project_id IS NOT ?1",
            params![
                projection
                    .project_id
                    .map(|project_id| project_id.to_string()),
                meeting_id.as_str()
            ],
        )
        .map_err(|error| sqlite_error("unable to project the meeting's project", error))?;
    connection
        .execute(
            "UPDATE meetings SET transcription_languages = ?1
             WHERE meeting_id = ?2 AND transcription_languages IS NOT ?1",
            params![
                stored_transcription_languages(&projection.transcription_languages),
                meeting_id.as_str()
            ],
        )
        .map_err(|error| sqlite_error("unable to project the transcription languages", error))?;
    combined_meetings::project_combination(connection, meeting_id, projection)?;
    for (cluster_id, identity_id) in &projection.cluster_resolutions {
        let cluster_key = cluster_id.to_string();
        connection
            .execute(
                "UPDATE unknown_speaker_clusters SET resolved_identity_id = ?1
                 WHERE cluster_id = ?2 AND meeting_id = ?3
                   AND resolved_identity_id IS NOT ?1",
                params![identity_id.to_string(), cluster_key, meeting_id.as_str()],
            )
            .map_err(|error| sqlite_error("unable to project a cluster resolution", error))?;
        connection
            .execute(
                "DELETE FROM unassigned_voice_clips WHERE cluster_id = ?1 AND meeting_id = ?2",
                params![cluster_key, meeting_id.as_str()],
            )
            .map_err(|error| sqlite_error("unable to delete a resolved cluster's clips", error))?;
    }
    Ok(())
}

/// Drops sub-second precision, which the `created_at_utc` column does not keep.
fn whole_seconds(instant: chrono::DateTime<chrono::Utc>) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(instant.timestamp(), 0).unwrap_or(instant)
}

/// Formats an instant like SQLite's `CURRENT_TIMESTAMP`, so meetings keep sorting by that column.
fn sqlite_timestamp_text(instant: chrono::DateTime<chrono::Utc>) -> String {
    instant.format("%Y-%m-%d %H:%M:%S").to_string()
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

/// Adds the audio length to telemetry of databases older than #68.
///
/// Inputs: An open connection. Outputs: Success or a storage rejection.
/// Side effects: May add one column; existing rows keep NULL, which proposals ignore.
fn add_task_audio_duration_column(sqlite_connection: &Connection) -> Result<(), StorageError> {
    let has_column: bool = sqlite_connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('task_metrics') WHERE name = 'audio_duration_ms')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| sqlite_error("unable to inspect the telemetry table", error))?;
    if has_column {
        return Ok(());
    }
    sqlite_connection
        .execute_batch("ALTER TABLE task_metrics ADD COLUMN audio_duration_ms INTEGER;")
        .map_err(|error| sqlite_error("unable to add the telemetry audio length", error))
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

/// Joins languages into the `meetings.transcription_languages` text: codes separated by commas
/// in first-use order, or `NULL` when none was recorded.
fn stored_transcription_languages(languages: &[TranscriptionLanguage]) -> Option<String> {
    if languages.is_empty() {
        return None;
    }
    Some(
        languages
            .iter()
            .map(|language| language.as_str())
            .collect::<Vec<_>>()
            .join(","),
    )
}

/// Splits the `meetings.transcription_languages` text back into languages.
///
/// Inputs: The meeting (for the log) and the column value, `None` when `NULL`.
/// Outputs: The valid languages in stored order; unknown codes are skipped.
/// Side effects: Logs a warning for each unknown code.
fn parse_stored_transcription_languages(
    meeting_id: &MeetingId,
    stored_languages: Option<&str>,
) -> Vec<TranscriptionLanguage> {
    stored_languages
        .unwrap_or_default()
        .split(',')
        .filter(|code| !code.is_empty())
        .filter_map(|code| {
            TranscriptionLanguage::parse(code)
                .inspect_err(|_| {
                    tracing::warn!(
                        meeting_id = %meeting_id.as_str(),
                        "Stored transcription language is unknown; skipping it"
                    );
                })
                .ok()
        })
        .collect()
}

/// Adds the nullable `meetings.transcription_languages` column to older databases.
///
/// Outputs: Success (also when the column exists) or a storage error.
/// Side effects: May run one `ALTER TABLE`; older meetings show no language.
fn add_meeting_transcription_languages_column(
    sqlite_connection: &Connection,
) -> Result<(), StorageError> {
    let language_column_count: i64 = sqlite_connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('meetings') WHERE name = 'transcription_languages'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| sqlite_error("unable to inspect meetings schema", error))?;
    if language_column_count == 0 {
        sqlite_connection
            .execute(
                "ALTER TABLE meetings ADD COLUMN transcription_languages TEXT",
                [],
            )
            .map_err(|error| {
                sqlite_error("unable to add the transcription languages column", error)
            })?;
        tracing::info!("Added the transcription languages column to an existing database");
    }
    Ok(())
}

/// Adds the nullable `transcript_segments.confidence` column (#90, #91) to older databases.
///
/// Outputs: Success (also when the column exists) or a storage error.
/// Side effects: May run one `ALTER TABLE`; existing segments carry no marks.
fn add_segment_confidence_column(sqlite_connection: &Connection) -> Result<(), StorageError> {
    let column_count: i64 = sqlite_connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('transcript_segments') WHERE name = 'confidence'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| sqlite_error("unable to inspect transcript segments schema", error))?;
    if column_count == 0 {
        sqlite_connection
            .execute(
                "ALTER TABLE transcript_segments ADD COLUMN confidence TEXT",
                [],
            )
            .map_err(|error| sqlite_error("unable to add the segment confidence column", error))?;
    }
    Ok(())
}

/// Encodes a segment's confidence marks for the index: `NULL` when plain, else
/// `{"o":bool,"s":bool,"w":[positions]}`.
fn confidence_column(confidence: &SegmentConfidence) -> Option<String> {
    (!confidence.is_plain()).then(|| {
        serde_json::json!({
            "o": confidence.overlapping_speech,
            "s": confidence.uncertain_speaker,
            "w": confidence.uncertain_words,
        })
        .to_string()
    })
}

/// Decodes `confidence_column`; anything malformed reads as plain.
fn confidence_from_column(column: Option<&str>) -> SegmentConfidence {
    let Some(value) = column.and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
    else {
        return SegmentConfidence::default();
    };
    SegmentConfidence {
        overlapping_speech: value["o"].as_bool().unwrap_or(false),
        uncertain_speaker: value["s"].as_bool().unwrap_or(false),
        uncertain_words: value["w"]
            .as_array()
            .map(|positions| {
                positions
                    .iter()
                    .filter_map(|position| position.as_u64())
                    .filter_map(|position| u16::try_from(position).ok())
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// Adds the nullable `attachments.meeting_time_ms` column (Issue #54) to older databases.
///
/// Outputs: Success (also when the column exists) or a storage error.
/// Side effects: May run one `ALTER TABLE`; existing attachments go at the end of their meeting.
fn add_attachment_meeting_time_column(sqlite_connection: &Connection) -> Result<(), StorageError> {
    let meeting_time_column_count: i64 = sqlite_connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('attachments') WHERE name = 'meeting_time_ms'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| sqlite_error("unable to inspect attachments schema", error))?;
    if meeting_time_column_count == 0 {
        sqlite_connection
            .execute(
                "ALTER TABLE attachments ADD COLUMN meeting_time_ms INTEGER",
                [],
            )
            .map_err(|error| {
                sqlite_error("unable to add the attachment meeting time column", error)
            })?;
    }
    Ok(())
}

/// Adds the `document_revision` column (Issue #43) to databases created before it, with its index.
///
/// Outputs: Success (also when the column exists) or a storage error.
/// Side effects: May run one `ALTER TABLE`; existing rows start at revision 0.
fn add_document_revision_column(sqlite_connection: &Connection) -> Result<(), StorageError> {
    let revision_column_count: i64 = sqlite_connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('meetings') WHERE name = 'document_revision'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| sqlite_error("unable to inspect meetings schema", error))?;
    if revision_column_count == 0 {
        sqlite_connection
            .execute(
                "ALTER TABLE meetings ADD COLUMN document_revision INTEGER NOT NULL DEFAULT 0",
                [],
            )
            .map_err(|error| sqlite_error("unable to add the document revision column", error))?;
    }
    sqlite_connection
        .execute(
            "CREATE INDEX IF NOT EXISTS meetings_document_revision_index ON meetings(document_revision)",
            [],
        )
        .map_err(|error| sqlite_error("unable to index document revisions", error))?;
    Ok(())
}

/// SQL for the revision a document write takes: one past the newest stored or deleted. SQLite
/// serializes writers, so concurrent connections never take the same revision. The floor keeps a
/// deleted meeting's revision from being reused, which watchers that saw it would miss.
const NEXT_DOCUMENT_REVISION_SQL: &str = "(SELECT MAX(
        COALESCE((SELECT MAX(document_revision) FROM meetings), 0),
        COALESCE((SELECT revision FROM document_revision_floor), 0)
    ) + 1)";

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

    /// The audio length survives storage, while a record made without one stays unknown.
    #[test]
    fn telemetry_keeps_the_audio_length_when_recorded() {
        use crate::{
            domain::models::{DeviceId, TaskMetrics, TaskType},
            ports::telemetry::MetricsRepository,
        };
        let repository = SqliteNoteRepository::open_in_memory(
            crate::application::crypto::voice_encryptor::ephemeral_cipher(),
        )
        .unwrap_or_else(|error| panic!("in-memory SQLite should initialize: {error}"));
        let device_id = DeviceId::new("device");
        let record = |audio_duration_ms, timestamp_epoch_ms| {
            TaskMetrics::new(
                uuid::Uuid::new_v4(),
                device_id.clone(),
                TaskType::Transcription,
                "ggml-base",
                500,
                500,
                0.1,
                timestamp_epoch_ms,
            )
            .with_audio_duration_ms(audio_duration_ms)
        };
        for metrics in [record(Some(15_000), 1), record(None, 2)] {
            repository
                .save_metrics(metrics)
                .unwrap_or_else(|error| panic!("metrics should save: {error}"));
        }

        let mut lengths: Vec<(u64, Option<u64>)> = repository
            .get_metrics_for_device("device")
            .unwrap_or_else(|error| panic!("metrics should load: {error}"))
            .iter()
            .map(|metric| (metric.timestamp_epoch_ms(), metric.audio_duration_ms()))
            .collect();
        lengths.sort_unstable();
        assert_eq!(lengths, vec![(1, Some(15_000)), (2, None)]);
    }

    /// A telemetry table from before the audio length gains the column and keeps its rows.
    #[test]
    fn older_telemetry_tables_gain_the_audio_length() {
        let connection = rusqlite::Connection::open_in_memory()
            .unwrap_or_else(|error| panic!("SQLite should open: {error}"));
        connection
            .execute_batch(
                "CREATE TABLE task_metrics (task_id TEXT PRIMARY KEY);
                 INSERT INTO task_metrics (task_id) VALUES ('old');",
            )
            .unwrap_or_else(|error| panic!("the old table should be created: {error}"));

        for _ in 0..2 {
            super::add_task_audio_duration_column(&connection)
                .unwrap_or_else(|error| panic!("the migration should run twice: {error}"));
        }

        let audio_duration_ms: Option<u64> = connection
            .query_row("SELECT audio_duration_ms FROM task_metrics", [], |row| {
                row.get(0)
            })
            .unwrap_or_else(|error| panic!("the old row should remain: {error}"));
        assert_eq!(audio_duration_ms, None);
    }

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

    /// Verifies a segment's confidence marks survive the index (#90, #91).
    #[test]
    fn segment_confidence_is_saved_and_loaded() {
        use crate::domain::transcript_confidence::SegmentConfidence;
        let repository = SqliteNoteRepository::open_in_memory(
            crate::application::crypto::voice_encryptor::ephemeral_cipher(),
        )
        .unwrap_or_else(|error| panic!("in-memory SQLite should initialize: {error}"));
        let meeting_id = MeetingId::new("meeting-confidence");
        repository
            .create_meeting(&meeting_id, &[1_u8])
            .unwrap_or_else(|error| panic!("meeting creation should succeed: {error}"));
        let marked = TranscriptSegment::new(meeting_id.clone(), 0, 1_000, None, "a b c")
            .with_confidence(SegmentConfidence {
                overlapping_speech: true,
                uncertain_speaker: false,
                uncertain_words: vec![0, 2],
            });
        let plain = TranscriptSegment::new(meeting_id.clone(), 1_000, 2_000, None, "d");

        repository
            .save_transcript_segments(&meeting_id, &[marked.clone(), plain.clone()])
            .unwrap_or_else(|error| panic!("segment save should succeed: {error}"));

        assert_eq!(
            repository.load_transcript_segments(&meeting_id).ok(),
            Some(vec![marked, plain])
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

    /// Verifies an attachment keeps its meeting time (Issue #54), and one without stays without.
    #[test]
    fn attachments_keep_their_meeting_time() {
        use crate::{domain::models::Attachment, ports::storage::AttachmentRepository};
        let repository = SqliteNoteRepository::open_in_memory(
            crate::application::crypto::voice_encryptor::ephemeral_cipher(),
        )
        .unwrap_or_else(|error| panic!("in-memory SQLite should initialize: {error}"));
        let meeting_id = MeetingId::new("meeting-photos");
        repository
            .create_meeting(&meeting_id, &[1_u8])
            .unwrap_or_else(|error| panic!("meeting creation should succeed: {error}"));
        let attachment = |meeting_time_ms: Option<u64>| {
            let attachment = Attachment::new(
                uuid::Uuid::new_v4(),
                Some(meeting_id.clone()),
                "/data/attachments/photo.jpg",
                "image/jpeg",
                chrono::Utc::now(),
                "device-test",
            );
            match meeting_time_ms {
                Some(meeting_time_ms) => attachment.with_meeting_time_ms(meeting_time_ms),
                None => attachment,
            }
        };
        for saved in [attachment(Some(42_000)), attachment(None)] {
            repository
                .save_attachment(&saved)
                .unwrap_or_else(|error| panic!("attachment save should succeed: {error}"));
        }

        let meeting_times: Vec<Option<u64>> = repository
            .get_attachments_for_meeting(&meeting_id)
            .unwrap_or_else(|error| panic!("attachments should load: {error}"))
            .iter()
            .map(Attachment::meeting_time_ms)
            .collect();
        assert_eq!(meeting_times, vec![Some(42_000), None]);
    }

    /// Verifies a database whose attachments table predates meeting times gains the column.
    #[test]
    fn migrates_attachments_without_a_meeting_time_column() {
        use crate::ports::storage::AttachmentRepository;
        let database_directory =
            std::env::temp_dir().join(format!("note-taker-attach-{}", uuid::Uuid::new_v4()));
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
                CREATE TABLE attachments (
                    attachment_id TEXT PRIMARY KEY NOT NULL,
                    meeting_id TEXT,
                    file_path TEXT NOT NULL,
                    mime_type TEXT NOT NULL,
                    original_timestamp_utc TEXT NOT NULL,
                    original_device_id TEXT NOT NULL
                );
                INSERT INTO meetings (meeting_id, acquisition_device_id, automerge_document)
                    VALUES ('legacy-meeting', 'old-device', X'01');
                INSERT INTO attachments VALUES ('6f1c1d4e-58f5-4c55-9a64-3b1f0c0b8d11',
                    'legacy-meeting', '/old/slide.png', 'image/png',
                    '2026-10-20T14:30:00+00:00', 'old-device');",
            )
            .unwrap_or_else(|error| panic!("legacy schema must be created: {error}"));
        drop(legacy_connection);

        let repository = SqliteNoteRepository::open(
            &database_path,
            crate::domain::models::DeviceId::new("migration-test"),
            crate::application::crypto::voice_encryptor::ephemeral_cipher(),
        )
        .unwrap_or_else(|error| panic!("legacy database must migrate: {error}"));
        let attachments = repository
            .get_attachments_for_meeting(&MeetingId::new("legacy-meeting"))
            .unwrap_or_else(|error| panic!("attachments should load: {error}"));
        drop(repository);
        let _ = std::fs::remove_dir_all(&database_directory);

        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].meeting_time_ms(), None);
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
