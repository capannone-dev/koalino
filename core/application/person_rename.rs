//! Renames a saved person (#186), as when the owner double-clicks or long-presses a speaker's
//! label in a meeting's transcript.
//!
//! People are shared by every meeting and paired device, so the new name shows wherever the
//! person speaks. Saving the identity refreshes its `updated_at`, which is how the voice library's
//! sync (#56) picks the newest name. Two people with the same name would be indistinguishable in
//! transcripts, and merging them (their voices and segments) is a separate job, so a name another
//! person already has is refused. Names are personal data and are never logged.

use uuid::Uuid;

use crate::{
    domain::{errors::StorageError, models::SpeakerIdentity},
    ports::voice::VoiceVectorDB,
};

/// The longest accepted name, in characters: room for any full name, well under what the voice
/// library's sync accepts.
const MAX_NAME_CHARACTERS: usize = 100;

/// Why a person could not be renamed.
#[derive(Debug, thiserror::Error)]
pub enum PersonRenameError {
    /// The name is blank, longer than `MAX_NAME_CHARACTERS`, or contains control characters.
    #[error("the name must be 1 to 100 characters on one line")]
    InvalidName,
    /// No saved person has this ID.
    #[error("no saved person has this ID")]
    PersonNotFound,
    /// Another saved person already has this name, compared without case.
    #[error("another saved person already has this name")]
    NameTaken,
    /// The voice store could not be read or written.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// A completed rename.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonRename {
    identity_id: Uuid,
    previous_name: String,
    new_name: String,
}

impl PersonRename {
    /// The renamed person's ID.
    pub fn identity_id(&self) -> Uuid {
        self.identity_id
    }

    /// The name before the rename.
    pub fn previous_name(&self) -> &str {
        &self.previous_name
    }

    /// The stored, trimmed new name.
    pub fn new_name(&self) -> &str {
        &self.new_name
    }
}

/// Renames the saved person `identity_id` to `raw_name`.
///
/// Inputs: The voice store; the person's ID; the name the owner typed (surrounding whitespace is
/// trimmed).
/// Outputs: The rename, or `InvalidName`, `PersonNotFound`, `NameTaken` (another person has the
/// name, ignoring case; the same person changing only the case is allowed), or `Storage`.
/// Side effects: Validates before any write; on success saves the identity under the new name,
/// which refreshes its `updated_at` so paired devices take the new name. Names are not logged.
pub fn rename_person<VectorDatabase>(
    voice_store: &VectorDatabase,
    identity_id: Uuid,
    raw_name: &str,
) -> Result<PersonRename, PersonRenameError>
where
    VectorDatabase: VoiceVectorDB + ?Sized,
{
    let new_name = raw_name.trim();
    if new_name.is_empty()
        || new_name.chars().count() > MAX_NAME_CHARACTERS
        || new_name.chars().any(char::is_control)
    {
        return Err(PersonRenameError::InvalidName);
    }
    let people = voice_store.list_speaker_identities()?;
    let person = people
        .iter()
        .find(|person| person.identity_id() == identity_id)
        .ok_or(PersonRenameError::PersonNotFound)?;
    let wanted = new_name.to_lowercase();
    if people.iter().any(|other| {
        other.identity_id() != identity_id && other.display_name().trim().to_lowercase() == wanted
    }) {
        return Err(PersonRenameError::NameTaken);
    }
    let previous_name = person.display_name().to_owned();
    voice_store.save_speaker_identity(&SpeakerIdentity::new(identity_id, new_name))?;
    tracing::info!(identity_id = %identity_id, "saved person renamed");
    Ok(PersonRename {
        identity_id,
        previous_name,
        new_name: new_name.to_owned(),
    })
}
