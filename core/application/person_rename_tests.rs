//! Specifies renaming a saved person from a meeting's transcript (#186): the new name is trimmed
//! and validated, an unknown person or a name another person already has is refused, and nothing
//! is written on refusal.

use uuid::Uuid;

use crate::{
    adapters::storage::in_memory_voice_vector_db::InMemoryVoiceVectorDB,
    application::person_rename::{PersonRenameError, rename_person},
    domain::models::SpeakerIdentity,
    ports::voice::VoiceVectorDB,
};

/// A voice store holding the people named, with their IDs in the same order.
fn store_with(names: &[&str]) -> (InMemoryVoiceVectorDB, Vec<Uuid>) {
    let store = InMemoryVoiceVectorDB::new();
    let ids = names
        .iter()
        .map(|name| {
            let id = Uuid::new_v4();
            store
                .save_speaker_identity(&SpeakerIdentity::new(id, *name))
                .unwrap_or_else(|error| panic!("identity should save: {error}"));
            id
        })
        .collect();
    (store, ids)
}

/// The stored name of `id`.
fn name_of(store: &InMemoryVoiceVectorDB, id: Uuid) -> Option<String> {
    store
        .list_speaker_identities()
        .unwrap_or_else(|error| panic!("identities should list: {error}"))
        .into_iter()
        .find(|identity| identity.identity_id() == id)
        .map(|identity| identity.display_name().to_owned())
}

#[test]
fn a_person_is_renamed_with_the_trimmed_name() {
    let (store, ids) = store_with(&["Speaker Ann"]);
    let renamed = rename_person(&store, ids[0], "  Anna Verdi ")
        .unwrap_or_else(|error| panic!("rename should succeed: {error}"));
    assert_eq!(renamed.previous_name(), "Speaker Ann");
    assert_eq!(renamed.new_name(), "Anna Verdi");
    assert_eq!(name_of(&store, ids[0]).as_deref(), Some("Anna Verdi"));
}

#[test]
fn a_change_of_case_only_is_allowed() {
    let (store, ids) = store_with(&["anna verdi"]);
    assert!(rename_person(&store, ids[0], "Anna Verdi").is_ok());
    assert_eq!(name_of(&store, ids[0]).as_deref(), Some("Anna Verdi"));
}

#[test]
fn a_name_another_person_has_is_refused() {
    let (store, ids) = store_with(&["Anna Verdi", "Bruno Neri"]);
    assert!(matches!(
        rename_person(&store, ids[1], " anna VERDI"),
        Err(PersonRenameError::NameTaken)
    ));
    assert_eq!(name_of(&store, ids[1]).as_deref(), Some("Bruno Neri"));
}

#[test]
fn blank_overlong_or_multiline_names_are_refused() {
    let (store, ids) = store_with(&["Anna Verdi"]);
    let overlong = "a".repeat(101);
    for name in ["", "   ", overlong.as_str(), "Anna\nVerdi"] {
        assert!(
            matches!(
                rename_person(&store, ids[0], name),
                Err(PersonRenameError::InvalidName)
            ),
            "{name:?} should be refused"
        );
    }
    assert_eq!(name_of(&store, ids[0]).as_deref(), Some("Anna Verdi"));
}

#[test]
fn an_unknown_person_is_refused() {
    let (store, _) = store_with(&["Anna Verdi"]);
    assert!(matches!(
        rename_person(&store, Uuid::new_v4(), "Carla Bianchi"),
        Err(PersonRenameError::PersonNotFound)
    ));
}
