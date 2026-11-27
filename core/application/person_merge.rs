//! Merges one saved person into another (#188), when the same person was saved twice (for
//! example from two meetings before their voices were matched).
//!
//! Every transcript passage that names the person merged is relinked to the person kept, first,
//! in every meeting; then the voice store moves their samples and links and deletes them. In that
//! order a failure part-way never leaves passages naming a deleted person: at worst both people
//! still exist and a retry finishes the job. Meetings sync their passages to paired devices as
//! usual, and the voice library sync carries the moved samples and the deletion. Names and
//! embeddings are never logged.

use uuid::Uuid;

use crate::{
    domain::errors::StorageError,
    ports::{storage::NoteRepository, voice::VoiceVectorDB},
};

/// Why two people could not be merged.
#[derive(Debug, thiserror::Error)]
pub enum PersonMergeError {
    /// The person merged and the person kept are the same.
    #[error("a person cannot be merged into themselves")]
    SamePerson,
    /// One of the two is not a saved person.
    #[error("no saved person has this ID")]
    PersonNotFound,
    /// Meetings or the voice store could not be read or written.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// A completed merge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonMerge {
    identity_id: Uuid,
    identity_name: String,
    segments_updated: usize,
    variants_moved: usize,
}

impl PersonMerge {
    /// The person kept.
    pub fn identity_id(&self) -> Uuid {
        self.identity_id
    }

    /// The kept person's name.
    pub fn identity_name(&self) -> &str {
        &self.identity_name
    }

    /// How many passages now name the person kept instead.
    pub fn segments_updated(&self) -> usize {
        self.segments_updated
    }

    /// How many voice samples moved to the person kept.
    pub fn variants_moved(&self) -> usize {
        self.variants_moved
    }
}

/// Merges `source` into `target`.
///
/// Inputs: The meetings, the voice store, the person to merge and the person to keep.
/// Outputs: The merge, or `SamePerson`, `PersonNotFound` (checked before any write) or `Storage`.
/// Side effects: Rewrites the transcript segments of every meeting that names `source`, then
/// moves `source`'s voice samples and external links to `target` and deletes `source`, leaving
/// a record that deletes them on paired devices too.
pub fn merge_people<Notes, Voices>(
    notes: &Notes,
    voices: &Voices,
    source: Uuid,
    target: Uuid,
) -> Result<PersonMerge, PersonMergeError>
where
    Notes: NoteRepository + ?Sized,
    Voices: VoiceVectorDB + ?Sized,
{
    if source == target {
        return Err(PersonMergeError::SamePerson);
    }
    let people = voices.list_speaker_identities()?;
    let kept = people
        .iter()
        .find(|person| person.identity_id() == target)
        .ok_or(PersonMergeError::PersonNotFound)?;
    if !people.iter().any(|person| person.identity_id() == source) {
        return Err(PersonMergeError::PersonNotFound);
    }
    let mut segments_updated = 0_usize;
    for meeting_id in notes.list_meeting_ids()? {
        let segments = notes.load_transcript_segments(&meeting_id)?;
        if !segments
            .iter()
            .any(|segment| segment.speaker_identity_id() == Some(source))
        {
            continue;
        }
        let relinked: Vec<_> = segments
            .into_iter()
            .map(|segment| {
                if segment.speaker_identity_id() == Some(source) {
                    segments_updated += 1;
                    segment.with_speaker_identity_id(Some(target))
                } else {
                    segment
                }
            })
            .collect();
        notes.save_transcript_segments(&meeting_id, &relinked)?;
    }
    let variants_moved = voices.merge_identity(source, target)?;
    tracing::info!(
        source = %source,
        target = %target,
        segments_updated,
        variants_moved,
        "saved people merged"
    );
    Ok(PersonMerge {
        identity_id: target,
        identity_name: kept.display_name().to_owned(),
        segments_updated,
        variants_moved,
    })
}
