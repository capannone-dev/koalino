//! Specifies the sealed `NTV1` voice-library messages (Issue #56).

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use super::voice_library_exchange::{
    VoiceLibraryMessageError, decode_voice_library_message, encode_voice_library_messages,
    is_voice_library_message,
};
use crate::domain::{
    models::{
        DeviceId, IdentityTombstone, SharedSpeakerIdentity, SharedVoiceVariant, VoiceLibrary,
    },
    peer_trust::PairKey,
};

const ALICE: Uuid = Uuid::from_u128(0xA1);

/// Fixed pair keys and peers.
fn key(byte: u8) -> PairKey {
    PairKey::from_bytes([byte; 32])
}
fn laptop() -> DeviceId {
    DeviceId::new("laptop")
}
fn phone() -> DeviceId {
    DeviceId::new("phone")
}

/// A library with one identity and `variant_count` variants of `dimensions`.
fn library(variant_count: u128, dimensions: usize) -> VoiceLibrary {
    let now = Utc
        .with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
        .single()
        .unwrap_or_else(|| panic!("the instant must exist"));
    VoiceLibrary {
        identities: vec![SharedSpeakerIdentity {
            identity_id: ALICE,
            display_name: "Alice".to_owned(),
            created_at: now,
            updated_at: now,
        }],
        variants: (0..variant_count)
            .map(|index| SharedVoiceVariant {
                identity_id: ALICE,
                variant_id: Uuid::from_u128(0x1000 + index),
                vector: vec![0.25; dimensions],
                created_at: now,
            })
            .collect(),
        tombstones: vec![IdentityTombstone {
            identity_id: Uuid::from_u128(0xD1),
            deleted_at: now,
        }],
    }
}

#[test]
fn a_message_round_trips_and_is_marked() {
    let original = library(2, 192);
    let messages = encode_voice_library_messages(&original, &key(7), &laptop(), &phone())
        .unwrap_or_else(|error| panic!("encoding must succeed: {error}"));

    assert_eq!(messages.len(), 1);
    assert!(is_voice_library_message(&messages[0]));
    let decoded = decode_voice_library_message(&messages[0], &key(7), &laptop(), &phone())
        .unwrap_or_else(|error| panic!("decoding must succeed: {error}"));
    assert_eq!(decoded, original);
}

/// The embeddings are not readable in the message.
#[test]
fn a_message_hides_its_contents() {
    let messages = encode_voice_library_messages(&library(1, 4), &key(7), &laptop(), &phone())
        .unwrap_or_else(|error| panic!("encoding must succeed: {error}"));
    let text = String::from_utf8_lossy(&messages[0]);
    assert!(!text.contains("Alice") && !text.contains("0.25"));
}

#[test]
fn only_the_paired_direction_opens_a_message() {
    let messages = encode_voice_library_messages(&library(1, 4), &key(7), &laptop(), &phone())
        .unwrap_or_else(|error| panic!("encoding must succeed: {error}"));
    for (pair_key, sender, receiver) in [
        (key(8), laptop(), phone()),
        (key(7), phone(), laptop()),
        (key(7), laptop(), DeviceId::new("other")),
    ] {
        assert_eq!(
            decode_voice_library_message(&messages[0], &pair_key, &sender, &receiver),
            Err(VoiceLibraryMessageError::Unauthenticated)
        );
    }
}

/// A large library is split; each message stands alone with the identities its variants need.
#[test]
fn a_large_library_is_split_into_messages_that_stand_alone() {
    let messages =
        encode_voice_library_messages(&library(1_000, 1_024), &key(7), &laptop(), &phone())
            .unwrap_or_else(|error| panic!("encoding must succeed: {error}"));
    assert!(messages.len() > 1);

    let mut variants = 0;
    for message in &messages {
        assert!(message.len() <= 16 * 1024 * 1024);
        let decoded = decode_voice_library_message(message, &key(7), &laptop(), &phone())
            .unwrap_or_else(|error| panic!("decoding must succeed: {error}"));
        assert_eq!(decoded.identities.len(), 1);
        variants += decoded.variants.len();
    }
    assert_eq!(variants, 1_000);
}

/// Values a peer controls are bounded even with the right key.
#[test]
fn implausible_values_are_refused() {
    let mut oversized = library(1, 4);
    oversized.variants[0].vector = vec![0.5; 4_097];
    let mut non_finite = library(1, 4);
    non_finite.variants[0].vector[0] = f32::INFINITY;
    let mut empty_vector = library(1, 4);
    empty_vector.variants[0].vector.clear();
    let mut blank_name = library(0, 4);
    blank_name.identities[0].display_name = "  ".to_owned();
    let mut long_name = library(0, 4);
    long_name.identities[0].display_name = "x".repeat(257);

    for invalid in [oversized, non_finite, empty_vector, blank_name, long_name] {
        assert_eq!(
            encode_voice_library_messages(&invalid, &key(7), &laptop(), &phone()),
            Err(VoiceLibraryMessageError::Invalid)
        );
    }
}
