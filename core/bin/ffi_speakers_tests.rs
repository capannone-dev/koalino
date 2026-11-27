//! Specifies how the meeting screen learns which voice a transcript segment belongs to, and the
//! JSON answer to renaming a saved person from the transcript (#186).

use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    adapters::storage::in_memory_voice_vector_db::InMemoryVoiceVectorDB,
    domain::models::{MeetingId, SpeakerIdentity, TranscriptSegment},
    ffi_speakers::{rename_person_json, segment_voice_json},
    ports::voice::VoiceVectorDB,
};

fn segment(identity: Option<Uuid>, cluster: Option<Uuid>) -> TranscriptSegment {
    TranscriptSegment::new(MeetingId::new("m-1"), 0, 1_000, identity, "Hello.")
        .with_unknown_cluster_id(cluster)
}

#[test]
fn a_saved_persons_segment_names_the_person() {
    let person = Uuid::new_v4();
    let voice = segment_voice_json(&segment(Some(person), Some(Uuid::new_v4())), |id| {
        id == person
    });
    assert_eq!(voice, json!({"kind": "person", "id": person.to_string()}));
}

#[test]
fn an_unknown_voice_names_its_cluster() {
    let cluster = Uuid::new_v4();
    let voice = segment_voice_json(&segment(None, Some(cluster)), |_| false);
    assert_eq!(voice, json!({"kind": "unknown", "id": cluster.to_string()}));
}

#[test]
fn an_identity_that_is_not_saved_here_falls_back_to_its_cluster_or_nothing() {
    let cluster = Uuid::new_v4();
    let gone = Uuid::new_v4();
    assert_eq!(
        segment_voice_json(&segment(Some(gone), Some(cluster)), |_| false),
        json!({"kind": "unknown", "id": cluster.to_string()})
    );
    assert_eq!(
        segment_voice_json(&segment(Some(gone), None), |_| false),
        Value::Null
    );
    assert_eq!(
        segment_voice_json(&segment(None, None), |_| true),
        Value::Null
    );
}

fn parsed(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|error| panic!("{error}: {text}"))
}

#[test]
fn renaming_answers_with_the_new_name_or_an_error_code() {
    let store = InMemoryVoiceVectorDB::new();
    let anna = Uuid::new_v4();
    let bruno = Uuid::new_v4();
    for (id, name) in [(anna, "Anna"), (bruno, "Bruno")] {
        store
            .save_speaker_identity(&SpeakerIdentity::new(id, name))
            .unwrap_or_else(|error| panic!("{error}"));
    }
    let renamed = parsed(&rename_person_json(
        &store,
        &anna.to_string(),
        " Anna Verdi ",
    ));
    assert_eq!(renamed["ok"], true);
    assert_eq!(renamed["identity_id"], anna.to_string());
    assert_eq!(renamed["identity_name"], "Anna Verdi");

    let error_code =
        |id: &str, name: &str| parsed(&rename_person_json(&store, id, name))["error_code"].clone();
    assert_eq!(error_code("not-a-uuid", "X"), "invalid_argument");
    assert_eq!(error_code(&bruno.to_string(), "  "), "invalid_name");
    assert_eq!(error_code(&bruno.to_string(), "anna verdi"), "name_taken");
    assert_eq!(
        error_code(&Uuid::new_v4().to_string(), "Carla"),
        "person_not_found"
    );
}

#[test]
fn merging_answers_with_the_person_kept_or_an_error_code() {
    use crate::{
        adapters::storage::sqlite_adapter::SqliteNoteRepository,
        application::crypto::voice_encryptor::ephemeral_cipher, ffi_speakers::merge_people_json,
    };
    let notes = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
        .unwrap_or_else(|error| panic!("{error}"));
    let store = InMemoryVoiceVectorDB::new();
    let (anna, twin) = (Uuid::new_v4(), Uuid::new_v4());
    for (id, name) in [(anna, "Anna"), (twin, "Anna V.")] {
        store
            .save_speaker_identity(&SpeakerIdentity::new(id, name))
            .unwrap_or_else(|error| panic!("{error}"));
    }
    let error_code = |source: &str, target: &str| {
        parsed(&merge_people_json(&notes, &store, source, target))["error_code"].clone()
    };
    assert_eq!(error_code("x", &anna.to_string()), "invalid_argument");
    assert_eq!(
        error_code(&anna.to_string(), &anna.to_string()),
        "same_person"
    );
    assert_eq!(
        error_code(&Uuid::new_v4().to_string(), &anna.to_string()),
        "person_not_found"
    );

    let merged = parsed(&merge_people_json(
        &notes,
        &store,
        &twin.to_string(),
        &anna.to_string(),
    ));
    assert_eq!(merged["ok"], true, "{merged}");
    assert_eq!(merged["identity_id"], anna.to_string());
    assert_eq!(merged["identity_name"], "Anna");
    assert_eq!(merged["segments_updated"], 0);
    assert_eq!(merged["variants_moved"], 0);
}
