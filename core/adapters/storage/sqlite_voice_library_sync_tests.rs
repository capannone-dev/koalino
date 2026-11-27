//! Specifies merging voice libraries between devices (Issue #56): variants union, names converge
//! on the newest rename whichever order messages arrive in, a deletion is never undone, and each
//! device reseals embeddings with its own key.

use std::sync::Arc;

use chrono::{DateTime, TimeZone, Utc};
use uuid::Uuid;

use super::sqlite_voice_vector_db::SqliteVoiceVectorDB;
use crate::{
    application::crypto::voice_encryptor::{EncryptionKey, VoiceEncryptor},
    domain::models::{
        IdentityTombstone, SharedSpeakerIdentity, SharedVoiceVariant, SpeakerIdentity,
        VoiceEmbedding, VoiceLibrary,
    },
    ports::voice::{VoiceLibrarySync, VoiceVectorDB},
};

const ALICE: Uuid = Uuid::from_u128(0xA1);
const VARIANT_ONE: Uuid = Uuid::from_u128(0xB1);
const VARIANT_TWO: Uuid = Uuid::from_u128(0xB2);

/// A store sealed with a key derived from `key_byte`.
fn store(key_byte: u8) -> SqliteVoiceVectorDB {
    SqliteVoiceVectorDB::open_in_memory(Arc::new(VoiceEncryptor::new(&EncryptionKey::from_bytes(
        [key_byte; 32],
    ))))
    .unwrap_or_else(|error| panic!("store must open: {error}"))
}

/// A fixed instant `second` seconds into the test's timeline.
fn at(second: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, second)
        .single()
        .unwrap_or_else(|| panic!("the instant must exist"))
}

/// Alice under `name`, updated at `updated_at`.
fn alice(name: &str, updated_at: DateTime<Utc>) -> SharedSpeakerIdentity {
    SharedSpeakerIdentity {
        identity_id: ALICE,
        display_name: name.to_owned(),
        created_at: at(0),
        updated_at,
    }
}

/// A variant of Alice.
fn variant(variant_id: Uuid, vector: Vec<f32>) -> SharedVoiceVariant {
    SharedVoiceVariant {
        identity_id: ALICE,
        variant_id,
        vector,
        created_at: at(0),
    }
}

/// A library holding `identities` and `variants`.
fn library(
    identities: Vec<SharedSpeakerIdentity>,
    variants: Vec<SharedVoiceVariant>,
) -> VoiceLibrary {
    VoiceLibrary {
        identities,
        variants,
        tombstones: Vec::new(),
    }
}

/// Imports `library`, which must be accepted.
fn import(store: &SqliteVoiceVectorDB, library: &VoiceLibrary) -> bool {
    store
        .import_voice_library(library)
        .unwrap_or_else(|error| panic!("import must succeed: {error}"))
}

/// The display names stored.
fn names(store: &SqliteVoiceVectorDB) -> Vec<String> {
    store
        .list_speaker_identities()
        .unwrap_or_else(|error| panic!("identities must list: {error}"))
        .iter()
        .map(|identity| identity.display_name().to_owned())
        .collect()
}

/// The library's digest.
fn digest(store: &SqliteVoiceVectorDB) -> [u8; 32] {
    store
        .voice_library_digest()
        .unwrap_or_else(|error| panic!("digest must compute: {error}"))
}

/// The library's export.
fn export(store: &SqliteVoiceVectorDB) -> VoiceLibrary {
    store
        .export_voice_library()
        .unwrap_or_else(|error| panic!("export must succeed: {error}"))
}

#[test]
fn variants_union_and_both_devices_converge() {
    let (laptop, phone) = (store(1), store(2));
    import(
        &laptop,
        &library(
            vec![alice("Alice", at(1))],
            vec![variant(VARIANT_ONE, vec![1.0, 0.0])],
        ),
    );
    import(
        &phone,
        &library(
            vec![alice("Alice", at(1))],
            vec![variant(VARIANT_TWO, vec![0.0, 1.0])],
        ),
    );
    assert_ne!(digest(&laptop), digest(&phone));

    assert!(import(&phone, &export(&laptop)));
    assert!(import(&laptop, &export(&phone)));

    assert_eq!(laptop.voice_variant_count(ALICE).ok(), Some(2));
    assert_eq!(phone.voice_variant_count(ALICE).ok(), Some(2));
    assert_eq!(digest(&laptop), digest(&phone));
    assert!(
        !import(&phone, &export(&laptop)),
        "a repeat changes nothing"
    );
}

/// The export opens with this store's key, and the receiving store reseals it with its own.
#[test]
fn embeddings_are_resealed_with_the_receivers_key() {
    let (laptop, phone) = (store(1), store(2));
    laptop
        .save_speaker_identity(&SpeakerIdentity::new(ALICE, "Alice"))
        .and_then(|()| {
            laptop.store_embedding(VoiceEmbedding::new(ALICE, VARIANT_ONE, vec![0.6, 0.8]))
        })
        .unwrap_or_else(|error| panic!("enrollment must succeed: {error}"));

    import(&phone, &export(&laptop));

    let found = phone
        .search_similar(&[0.6, 0.8], 1)
        .unwrap_or_else(|error| panic!("the phone must open what it stored: {error}"));
    assert_eq!(found[0].0.variant_id(), VARIANT_ONE);
    assert!(found[0].1 > 0.99);
}

#[test]
fn the_newest_rename_wins_in_either_order() {
    let (first, second) = (store(1), store(2));
    let (older, newer) = (
        library(vec![alice("Ali", at(5))], Vec::new()),
        library(vec![alice("Alice Rossi", at(9))], Vec::new()),
    );

    import(&first, &older);
    import(&first, &newer);
    import(&second, &newer);
    import(&second, &older);

    assert_eq!(names(&first), vec!["Alice Rossi".to_owned()]);
    assert_eq!(names(&second), vec!["Alice Rossi".to_owned()]);
    assert_eq!(digest(&first), digest(&second));
}

/// Two renames at the same instant still converge, without a device-specific tie-break.
#[test]
fn simultaneous_renames_converge() {
    let (first, second) = (store(1), store(2));
    let (one, other) = (
        library(vec![alice("Alice A", at(5))], Vec::new()),
        library(vec![alice("Alice B", at(5))], Vec::new()),
    );

    import(&first, &one);
    import(&first, &other);
    import(&second, &other);
    import(&second, &one);

    assert_eq!(names(&first), names(&second));
}

/// Deleting a voice is a privacy promise: a device that has not heard yet must not bring it back.
#[test]
fn a_deletion_reaches_other_devices_and_is_never_undone() {
    let (laptop, phone) = (store(1), store(2));
    let enrolled = library(
        vec![alice("Alice", at(1))],
        vec![variant(VARIANT_ONE, vec![1.0, 0.0])],
    );
    import(&laptop, &enrolled);
    import(&phone, &enrolled);
    let stale_phone_export = export(&phone);

    laptop
        .delete_identity(ALICE)
        .unwrap_or_else(|error| panic!("deletion must succeed: {error}"));
    assert!(!import(&laptop, &stale_phone_export));
    assert!(names(&laptop).is_empty());

    assert!(import(&phone, &export(&laptop)));
    assert!(names(&phone).is_empty());
    assert_eq!(phone.voice_variant_count(ALICE).ok(), Some(0));
    assert_eq!(digest(&laptop), digest(&phone));

    let later_rename = library(
        vec![alice("Alice", at(59))],
        vec![variant(VARIANT_TWO, vec![0.0, 1.0])],
    );
    assert!(!import(&phone, &later_rename));
    assert!(names(&phone).is_empty());
}

/// The first import of a deletion on a device that never had the identity still records it.
#[test]
fn a_deletion_of_an_unknown_identity_is_remembered() {
    let phone = store(2);
    let deletion = VoiceLibrary {
        identities: Vec::new(),
        variants: Vec::new(),
        tombstones: vec![IdentityTombstone {
            identity_id: ALICE,
            deleted_at: at(3),
        }],
    };
    assert!(import(&phone, &deletion));

    assert!(!import(
        &phone,
        &library(vec![alice("Alice", at(1))], Vec::new())
    ));
    assert!(names(&phone).is_empty());
}

/// A bad entry from a peer rejects the whole import, so nothing partial is stored.
#[test]
fn an_invalid_entry_stores_nothing() {
    let phone = store(2);
    let invalid = library(
        vec![alice("Alice", at(1))],
        vec![variant(VARIANT_ONE, vec![f32::NAN])],
    );
    assert!(phone.import_voice_library(&invalid).is_err());
    assert!(names(&phone).is_empty());

    let blank = library(vec![alice("  ", at(1))], Vec::new());
    assert!(phone.import_voice_library(&blank).is_err());
}

/// A variant of an identity the message does not carry and this device does not know is skipped.
#[test]
fn a_variant_without_its_identity_is_skipped() {
    let phone = store(2);
    assert!(!import(
        &phone,
        &library(Vec::new(), vec![variant(VARIANT_ONE, vec![1.0, 0.0])])
    ));
    assert_eq!(phone.voice_variant_count(ALICE).ok(), Some(0));
}

/// The digest depends only on metadata and changes with the library.
#[test]
fn the_digest_follows_the_library() {
    let laptop = store(1);
    let empty = digest(&laptop);
    laptop
        .save_speaker_identity(&SpeakerIdentity::new(ALICE, "Alice"))
        .unwrap_or_else(|error| panic!("identity must save: {error}"));
    let with_identity = digest(&laptop);
    assert_ne!(empty, with_identity);
    assert_eq!(with_identity, digest(&laptop));
    laptop
        .store_embedding(VoiceEmbedding::new(ALICE, VARIANT_ONE, vec![1.0, 0.0]))
        .unwrap_or_else(|error| panic!("variant must store: {error}"));
    assert_ne!(with_identity, digest(&laptop));
}

/// Merging two saved people (#188) moves the samples to the person kept, under new IDs so paired
/// devices add them, and deletes the other with a tombstone, so every device ends the same way.
#[test]
fn a_merge_moves_samples_and_converges_on_a_paired_device() {
    const BOB: Uuid = Uuid::from_u128(0xB0B);
    let (laptop, phone) = (store(1), store(2));
    laptop
        .save_speaker_identity(&SpeakerIdentity::new(ALICE, "Alice"))
        .and_then(|()| laptop.save_speaker_identity(&SpeakerIdentity::new(BOB, "Alice R.")))
        .and_then(|()| {
            laptop.store_embedding(VoiceEmbedding::new(ALICE, VARIANT_ONE, vec![1.0, 0.0]))
        })
        .and_then(|()| {
            laptop.store_embedding(VoiceEmbedding::new(BOB, VARIANT_TWO, vec![0.0, 1.0]))
        })
        .unwrap_or_else(|error| panic!("enrollment must succeed: {error}"));
    // The phone already has both people from an earlier sync.
    import(&phone, &export(&laptop));

    let moved = laptop
        .merge_identity(BOB, ALICE)
        .unwrap_or_else(|error| panic!("merge must succeed: {error}"));
    assert_eq!(moved, 1);
    assert_eq!(names(&laptop), vec!["Alice".to_owned()]);
    assert_eq!(laptop.voice_variant_count(ALICE).ok(), Some(2));
    let exported = export(&laptop);
    assert!(
        exported
            .tombstones
            .iter()
            .any(|tombstone| tombstone.identity_id == BOB)
    );
    assert!(
        exported
            .variants
            .iter()
            .all(|variant| variant.identity_id == ALICE && variant.variant_id != VARIANT_TWO),
        "moved samples get new IDs"
    );
    // Bob's voice still matches, now as Alice.
    let found = laptop
        .search_similar(&[0.0, 1.0], 1)
        .unwrap_or_else(|error| panic!("search must succeed: {error}"));
    assert_eq!(
        found.first().map(|(variant, _)| variant.identity_id()),
        Some(ALICE)
    );

    import(&phone, &exported);
    assert_eq!(names(&phone), vec!["Alice".to_owned()]);
    assert_eq!(phone.voice_variant_count(ALICE).ok(), Some(2));
    assert_eq!(digest(&phone), digest(&laptop));
}

#[test]
fn a_merge_needs_two_different_saved_people() {
    let laptop = store(1);
    laptop
        .save_speaker_identity(&SpeakerIdentity::new(ALICE, "Alice"))
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(laptop.merge_identity(ALICE, ALICE).is_err());
    assert!(
        laptop
            .merge_identity(Uuid::from_u128(0xDEAD), ALICE)
            .is_err()
    );
    assert!(
        laptop
            .merge_identity(ALICE, Uuid::from_u128(0xDEAD))
            .is_err()
    );
    assert_eq!(names(&laptop), vec!["Alice".to_owned()]);
}

/// A Teams user linked to the merged person stays recognized, as the person kept (#188).
#[test]
fn a_merge_moves_external_links_to_the_person_kept() {
    use crate::ports::external_bridge::ExternalIdentityRepository;
    const BOB: Uuid = Uuid::from_u128(0xB0B);
    let laptop = store(1);
    let bob = SpeakerIdentity::new(BOB, "Bob");
    laptop
        .save_speaker_identity(&SpeakerIdentity::new(ALICE, "Alice"))
        .and_then(|()| laptop.save_speaker_identity(&bob))
        .and_then(|()| laptop.save_external_mapping("teams-user-1", &bob))
        .unwrap_or_else(|error| panic!("{error}"));
    laptop
        .merge_identity(BOB, ALICE)
        .unwrap_or_else(|error| panic!("merge must succeed: {error}"));
    let linked = laptop
        .find_by_external_id("teams-user-1")
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(linked.map(|identity| identity.identity_id()), Some(ALICE));
}
