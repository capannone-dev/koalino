//! Coordinates Automerge meeting documents through the domain-neutral storage port.

use automerge::{
    AutoCommit, ROOT, ReadDoc, ScalarValue, Value,
    sync::{self, SyncDoc},
    transaction::Transactable,
};

use crate::{
    domain::{errors::StorageError, models::MeetingId},
    ports::storage::NoteRepository,
};

/// Coordinates meeting note changes using Automerge while keeping CRDT types out of port contracts.
pub struct MeetingCrdtService<RepositoryPort> {
    note_repository: RepositoryPort,
}

impl<RepositoryPort> MeetingCrdtService<RepositoryPort>
where
    RepositoryPort: NoteRepository,
{
    /// Creates a service using an injected persistence-port implementation.
    ///
    /// Inputs: `note_repository` must durably store opaque CRDT document bytes.
    /// Outputs: A service ready to create and edit meeting documents.
    /// Side effects: None.
    pub fn new(note_repository: RepositoryPort) -> Self {
        Self { note_repository }
    }

    /// Creates an empty Automerge document for a meeting and persists it through the storage port.
    ///
    /// Inputs: `meeting_id` must be a stable unique meeting identifier.
    /// Outputs: Success or a portable storage error.
    /// Side effects: Serializes an Automerge document and writes it through the repository.
    pub fn create_meeting(&self, meeting_id: &MeetingId) -> Result<(), StorageError> {
        let mut new_document = AutoCommit::new();
        self.note_repository
            .create_meeting(meeting_id, &new_document.save())
    }

    /// Replaces the root note text in an existing meeting's Automerge document.
    ///
    /// Inputs: `meeting_id` must exist; `note_text` is user note content and is not logged.
    /// Outputs: Success or a portable storage error, including corrupt-document rejection.
    /// Side effects: Loads, changes, serializes, and persists the meeting's CRDT state.
    pub fn replace_note_text(
        &self,
        meeting_id: &MeetingId,
        note_text: &str,
    ) -> Result<(), StorageError> {
        let stored_document = self.note_repository.load_crdt_document(meeting_id)?;
        let mut automerge_document = load_document(&stored_document)?;
        automerge_document
            .put(ROOT, "note_text", note_text)
            .map_err(|error| crdt_error("unable to apply note text change", error))?;
        self.note_repository
            .save_crdt_document(meeting_id, &automerge_document.save())
    }

    /// Merges a serialized document received from an authenticated peer into local meeting state.
    ///
    /// Inputs: `meeting_id` must exist locally; `remote_document` must be a valid Automerge document from a trusted peer.
    /// Outputs: Success or a portable storage error, including corrupt-document rejection.
    /// Side effects: Reads local CRDT state, performs an Automerge conflict-free merge, and persists the merged state.
    pub fn merge_remote_document(
        &self,
        meeting_id: &MeetingId,
        remote_document: &[u8],
    ) -> Result<(), StorageError> {
        let local_document_bytes = self.note_repository.load_crdt_document(meeting_id)?;
        let mut local_document = load_document(&local_document_bytes)?;
        let mut received_document = load_document(remote_document)?;
        local_document
            .merge(&mut received_document)
            .map_err(|error| crdt_error("unable to merge remote CRDT document", error))?;
        self.note_repository
            .save_crdt_document(meeting_id, &local_document.save())
    }

    /// Generates one Automerge sync message for the current local meeting document.
    ///
    /// Inputs: `meeting_id` selects a stored document; `sync_state` tracks protocol state for one peer.
    /// Outputs: An optional Automerge message or a portable storage error.
    /// Side effects: Reads and deserializes persisted meeting state; updates the supplied sync state.
    pub fn generate_sync_message(
        &self,
        meeting_id: &MeetingId,
        sync_state: &mut sync::State,
    ) -> Result<Option<sync::Message>, StorageError> {
        let stored_document = self.note_repository.load_crdt_document(meeting_id)?;
        let mut automerge_document = load_document(&stored_document)?;
        Ok(automerge_document.sync().generate_sync_message(sync_state))
    }

    /// Applies one decoded Automerge sync message and persists the resulting meeting state.
    ///
    /// Inputs: `meeting_id` identifies an existing document; `sync_state` is per-peer protocol state;
    /// `sync_message` is decoded from an authenticated peer's envelope.
    /// Outputs: Success or a portable storage error when data cannot be merged or persisted.
    /// Side effects: Reads, updates, serializes, and persists the local Automerge document.
    pub fn apply_sync_message(
        &self,
        meeting_id: &MeetingId,
        sync_state: &mut sync::State,
        sync_message: sync::Message,
    ) -> Result<(), StorageError> {
        let stored_document = self.note_repository.load_crdt_document(meeting_id)?;
        let mut automerge_document = load_document(&stored_document)?;
        automerge_document
            .sync()
            .receive_sync_message(sync_state, sync_message)
            .map_err(|error| crdt_error("unable to apply remote sync message", error))?;
        self.note_repository
            .save_crdt_document(meeting_id, &automerge_document.save())
    }

    /// Loads the root note text from an existing meeting document.
    ///
    /// Inputs: `meeting_id` must identify a stored meeting document.
    /// Outputs: `Some` note text when present, `None` for a new empty document, or a storage error.
    /// Side effects: Reads and deserializes the stored CRDT document.
    pub fn load_note_text(&self, meeting_id: &MeetingId) -> Result<Option<String>, StorageError> {
        let stored_document = self.note_repository.load_crdt_document(meeting_id)?;
        let automerge_document = load_document(&stored_document)?;
        match automerge_document
            .get(ROOT, "note_text")
            .map_err(|error| crdt_error("unable to read note text", error))?
        {
            Some((Value::Scalar(scalar_value), _)) => match scalar_value.as_ref() {
                ScalarValue::Str(note_text) => Ok(Some(note_text.to_string())),
                _ => Err(StorageError::Rejected {
                    reason: String::from("meeting document contains a non-text note"),
                }),
            },
            Some(_) => Err(StorageError::Rejected {
                reason: String::from("meeting document contains a non-text note"),
            }),
            None => Ok(None),
        }
    }
}

/// Deserializes opaque repository bytes into an Automerge document.
///
/// Inputs: `serialized_document` must originate from a trusted local repository.
/// Outputs: A mutable Automerge document or a portable corruption error.
/// Side effects: None.
fn load_document(serialized_document: &[u8]) -> Result<AutoCommit, StorageError> {
    AutoCommit::load(serialized_document)
        .map_err(|error| crdt_error("stored CRDT document is corrupt", error))
}

/// Converts an Automerge failure into an infrastructure-independent storage error.
///
/// Inputs: `operation` describes the failed CRDT action; `error` is never persisted or logged here.
/// Outputs: A portable storage rejection.
/// Side effects: None.
fn crdt_error(operation: &str, error: automerge::AutomergeError) -> StorageError {
    StorageError::Rejected {
        reason: format!("{operation}: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use automerge::{AutoCommit, ROOT, transaction::Transactable};

    use crate::{
        adapters::storage::sqlite_adapter::SqliteNoteRepository,
        application::meeting_crdt::MeetingCrdtService,
        domain::{errors::StorageError, models::MeetingId},
        ports::storage::NoteRepository,
    };

    /// Verifies note text is created, changed, persisted, and reloaded through the storage port.
    #[test]
    fn creates_updates_and_loads_a_meeting_note() {
        let repository = SqliteNoteRepository::open_in_memory(
            crate::application::crypto::voice_encryptor::ephemeral_cipher(),
        )
        .unwrap_or_else(|error| panic!("in-memory SQLite should initialize: {error}"));
        let service = MeetingCrdtService::new(repository);
        let meeting_id = MeetingId::new("meeting-milan");

        service
            .create_meeting(&meeting_id)
            .unwrap_or_else(|error| panic!("CRDT meeting creation should succeed: {error}"));
        service
            .replace_note_text(&meeting_id, "Updated during an offline meeting.")
            .unwrap_or_else(|error| panic!("CRDT update should succeed: {error}"));

        assert_eq!(
            service.load_note_text(&meeting_id).ok(),
            Some(Some(String::from("Updated during an offline meeting.")))
        );
    }

    /// Verifies corrupt persisted CRDT bytes fail safely instead of producing a partial note.
    #[test]
    fn rejects_a_corrupted_crdt_document() {
        let repository = SqliteNoteRepository::open_in_memory(
            crate::application::crypto::voice_encryptor::ephemeral_cipher(),
        )
        .unwrap_or_else(|error| panic!("in-memory SQLite should initialize: {error}"));
        let meeting_id = MeetingId::new("meeting-corrupt");

        repository
            .create_meeting(&meeting_id, &[0_u8, 255_u8, 42_u8])
            .unwrap_or_else(|error| panic!("test fixture storage should succeed: {error}"));
        let service = MeetingCrdtService::new(repository);

        let storage_error = service
            .load_note_text(&meeting_id)
            .expect_err("corrupt CRDT bytes must be rejected");
        assert!(matches!(storage_error, StorageError::Rejected { .. }));
    }

    /// Verifies divergent documents merge offline and remain readable through the application service.
    #[test]
    fn merges_a_remote_offline_document_without_a_central_service() {
        let repository = SqliteNoteRepository::open_in_memory(
            crate::application::crypto::voice_encryptor::ephemeral_cipher(),
        )
        .unwrap_or_else(|error| panic!("in-memory SQLite should initialize: {error}"));
        let meeting_id = MeetingId::new("meeting-merge");
        let mut initial_document = AutoCommit::new();
        repository
            .create_meeting(&meeting_id, &initial_document.save())
            .unwrap_or_else(|error| panic!("meeting fixture should succeed: {error}"));

        let mut remote_document = AutoCommit::load(&initial_document.save())
            .unwrap_or_else(|error| panic!("initial CRDT state should deserialize: {error}"));
        remote_document
            .put(ROOT, "note_text", "Remote offline edit.")
            .unwrap_or_else(|error| panic!("remote CRDT edit should succeed: {error}"));
        let service = MeetingCrdtService::new(repository);
        service
            .replace_note_text(&meeting_id, "Local offline edit.")
            .unwrap_or_else(|error| panic!("local CRDT edit should succeed: {error}"));

        service
            .merge_remote_document(&meeting_id, &remote_document.save())
            .unwrap_or_else(|error| panic!("offline CRDT merge should succeed: {error}"));

        assert!(
            service
                .load_note_text(&meeting_id)
                .unwrap_or_else(|error| panic!("merged document should be readable: {error}"))
                .is_some()
        );
    }
}
