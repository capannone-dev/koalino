//! Specifies the manifest message paired devices exchange (Issue #43): a round trip keeps every
//! field, sync envelopes are told apart, and malformed or oversized peer input is rejected.

use crate::{
    application::device_manifest_exchange::{
        ManifestMessageError, decode_manifest_message, encode_manifest_message,
    },
    domain::models::{DeviceId, DeviceManifest, TranscriptionModel},
};

/// A manifest advertising local Whisper and OpenAI.
fn manifest() -> DeviceManifest {
    DeviceManifest::new(
        DeviceId::new("12D3KooWLaptop"),
        8,
        4_096,
        vec![
            TranscriptionModel::Whisper,
            TranscriptionModel::ThirdParty(String::from("openai")),
        ],
    )
}

/// Builds a manifest message around arbitrary JSON, as a misbehaving peer could.
fn message_with_body(body: &str) -> Vec<u8> {
    let mut message = b"NTM1".to_vec();
    message.extend_from_slice(body.as_bytes());
    message
}

#[test]
fn a_manifest_survives_a_round_trip() {
    let encoded = encode_manifest_message(&manifest())
        .unwrap_or_else(|error| panic!("manifest must encode: {error}"));

    let decoded = decode_manifest_message(&encoded);

    assert!(encoded.starts_with(b"NTM1"));
    assert_eq!(decoded, Some(Ok(manifest())));
}

#[test]
fn other_messages_are_not_manifests() {
    assert_eq!(decode_manifest_message(b"NTS1\0\0\0\x01mpayload"), None);
    assert_eq!(decode_manifest_message(b""), None);
}

#[test]
fn malformed_manifests_are_rejected() {
    assert_eq!(
        decode_manifest_message(&message_with_body("{not json")),
        Some(Err(ManifestMessageError::Malformed))
    );
}

#[test]
fn oversized_or_implausible_manifests_are_rejected() {
    let oversized = message_with_body(&" ".repeat(20_000));
    let too_many_models = DeviceManifest::new(
        DeviceId::new("12D3KooWLaptop"),
        8,
        0,
        vec![TranscriptionModel::Whisper; 17],
    );
    let long_label = DeviceManifest::new(
        DeviceId::new("12D3KooWLaptop"),
        8,
        0,
        vec![TranscriptionModel::ThirdParty("x".repeat(65))],
    );
    let control_character = DeviceManifest::new(
        DeviceId::new("12D3KooWLaptop"),
        8,
        0,
        vec![TranscriptionModel::ThirdParty(String::from("open\u{1b}ai"))],
    );

    assert_eq!(
        decode_manifest_message(&oversized),
        Some(Err(ManifestMessageError::TooLarge))
    );
    for invalid in [too_many_models, long_label, control_character] {
        assert_eq!(
            encode_manifest_message(&invalid),
            Err(ManifestMessageError::Invalid)
        );
        let body = serde_json::to_string(&invalid)
            .unwrap_or_else(|error| panic!("manifest must serialize: {error}"));
        assert_eq!(
            decode_manifest_message(&message_with_body(&body)),
            Some(Err(ManifestMessageError::Invalid))
        );
    }
}

#[test]
fn the_storage_id_travels_and_older_manifests_still_open() {
    let mut with_storage_id = manifest();
    with_storage_id.storage_device_id = Some(DeviceId::new("device-laptop"));
    let encoded = encode_manifest_message(&with_storage_id)
        .unwrap_or_else(|error| panic!("manifest must encode: {error}"));
    assert_eq!(
        decode_manifest_message(&encoded)
            .and_then(Result::ok)
            .and_then(|decoded| decoded.storage_device_id),
        Some(DeviceId::new("device-laptop"))
    );

    // A device from before #88 sends no storage ID.
    let older = encode_manifest_message(&manifest())
        .unwrap_or_else(|error| panic!("manifest must encode: {error}"));
    assert!(!String::from_utf8_lossy(&older).contains("storage_device_id"));
    assert_eq!(
        decode_manifest_message(&older)
            .and_then(Result::ok)
            .map(|decoded| decoded.storage_device_id),
        Some(None)
    );
}
