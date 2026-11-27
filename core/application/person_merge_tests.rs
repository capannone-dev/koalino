//! Specifies merging one saved person into another (#188): every transcript passage of the
//! person merged now names the person kept, their voice samples move, and they are deleted.

use uuid::Uuid;

use crate::{
    adapters::storage::{
        in_memory_voice_vector_db::InMemoryVoiceVectorDB, sqlite_adapter::SqliteNoteRepository,
    },
    application::{
        crypto::voice_encryptor::ephemeral_cipher,
        person_merge::{PersonMergeError, merge_people},
    },
    domain::models::{MeetingId, SpeakerIdentity, TranscriptSegment, VoiceEmbedding},
    ports::{storage::NoteRepository, voice::VoiceVectorDB},
};

struct World {
    notes: SqliteNoteRepository,
    voices: InMemoryVoiceVectorDB,
    anna: Uuid,
    anna_twin: Uuid,
    bruno: Uuid,
}

/// Two meetings where Anna was saved twice, plus Bruno.
fn world() -> World {
    let notes = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .unwrap_or_else(|error| panic!("{error}"));
    let voices = InMemoryVoiceVectorDB::new();
    let (anna, anna_twin, bruno) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    for (id, name) in [(anna, "Anna"), (anna_twin, "Anna Verdi"), (bruno, "Bruno")] {
        voices
            .save_speaker_identity(&SpeakerIdentity::new(id, name))
            .and_then(|()| {
                voices.store_embedding(VoiceEmbedding::new(id, Uuid::new_v4(), vec![1.0, 0.5]))
            })
            .unwrap_or_else(|error| panic!("{error}"));
    }
    for (meeting, speakers) in [("m-1", [anna, anna_twin]), ("m-2", [anna_twin, bruno])] {
        let meeting_id = MeetingId::new(meeting);
        notes
            .create_meeting(&meeting_id, &[])
            .unwrap_or_else(|error| panic!("{error}"));
        let segments: Vec<TranscriptSegment> = speakers
            .iter()
            .enumerate()
            .map(|(index, speaker)| {
                let start = u64::try_from(index).unwrap_or(0) * 1_000;
                TranscriptSegment::new(
                    meeting_id.clone(),
                    start,
                    start + 1_000,
                    Some(*speaker),
                    "Hi.",
                )
            })
            .collect();
        notes
            .save_transcript_segments(&meeting_id, &segments)
            .unwrap_or_else(|error| panic!("{error}"));
    }
    World {
        notes,
        voices,
        anna,
        anna_twin,
        bruno,
    }
}

fn speakers(world: &World, meeting: &str) -> Vec<Option<Uuid>> {
    world
        .notes
        .load_transcript_segments(&MeetingId::new(meeting))
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .map(TranscriptSegment::speaker_identity_id)
        .collect()
}

#[test]
fn every_passage_and_sample_moves_to_the_person_kept() {
    let world = world();
    let merged = merge_people(&world.notes, &world.voices, world.anna_twin, world.anna)
        .unwrap_or_else(|error| panic!("merge should succeed: {error}"));
    assert_eq!(merged.identity_id(), world.anna);
    assert_eq!(merged.identity_name(), "Anna");
    assert_eq!(merged.segments_updated(), 2);
    assert_eq!(merged.variants_moved(), 1);

    assert_eq!(
        speakers(&world, "m-1"),
        vec![Some(world.anna), Some(world.anna)]
    );
    assert_eq!(
        speakers(&world, "m-2"),
        vec![Some(world.anna), Some(world.bruno)]
    );
    assert_eq!(world.voices.voice_variant_count(world.anna).ok(), Some(2));
    let names: Vec<String> = world
        .voices
        .list_speaker_identities()
        .unwrap_or_else(|error| panic!("{error}"))
        .iter()
        .map(|identity| identity.display_name().to_owned())
        .collect();
    assert_eq!(names, vec!["Anna".to_owned(), "Bruno".to_owned()]);
}

#[test]
fn a_person_cannot_be_merged_into_themselves_or_someone_unknown() {
    let world = world();
    assert!(matches!(
        merge_people(&world.notes, &world.voices, world.anna, world.anna),
        Err(PersonMergeError::SamePerson)
    ));
    assert!(matches!(
        merge_people(&world.notes, &world.voices, Uuid::new_v4(), world.anna),
        Err(PersonMergeError::PersonNotFound)
    ));
    assert!(matches!(
        merge_people(&world.notes, &world.voices, world.anna, Uuid::new_v4()),
        Err(PersonMergeError::PersonNotFound)
    ));
    assert_eq!(
        speakers(&world, "m-1"),
        vec![Some(world.anna), Some(world.anna_twin)]
    );
}
