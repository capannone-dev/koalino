//! Implements the note repository port with SQLite-backed transcript and Automerge BLOB storage.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    domain::{
        errors::StorageError,
        models::{DeviceId, MeetingId, Transcript, UnassignedVoiceClip},
    },
    ports::storage::NoteRepository,
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

impl NoteRepository for SqliteNoteRepository {
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
