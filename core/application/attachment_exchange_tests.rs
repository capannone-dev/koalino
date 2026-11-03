//! Specifies `NTA1` attachment manifests: they survive a round trip with every field, open only
//! with the pair key in the right direction, split when long, and refuse implausible values.

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use super::attachment_exchange::{
    AttachmentManifestError, MAX_ATTACHMENTS_PER_MESSAGE, decode_attachment_manifest,
    encode_attachment_manifests, is_attachment_manifest, is_content_hash,
};
use crate::{
    application::crypto::pair_key::seal_for_peer,
    domain::{
        models::{AttachmentOwner, DeviceId, MeetingId, SharedAttachment},
        peer_trust::PairKey,
        projects::ProjectId,
    },
    ports::file_transfer::MAX_FILE_TRANSFER_BYTES,
};

/// A pair key.
fn key() -> PairKey {
    PairKey::from_bytes([4; 32])
}

/// The sender.
fn laptop() -> DeviceId {
    DeviceId::new("laptop")
}

/// The receiver.
fn phone() -> DeviceId {
    DeviceId::new("phone")
}

/// An attachment with the given ID number.
fn attachment(number: u128) -> SharedAttachment {
    SharedAttachment {
        attachment_id: Uuid::from_u128(number),
        owner: AttachmentOwner::Meeting(MeetingId::new("meeting-1")),
        file_name: "whiteboard.png".to_owned(),
        mime_type: "image/png".to_owned(),
        content_hash: "ab".repeat(32),
        size_bytes: 123,
        original_timestamp: Utc
            .timestamp_opt(1_700_000_000, 5_000)
            .single()
            .unwrap_or_default(),
        original_device_id: "device-laptop".to_owned(),
        meeting_time_ms: Some(42_000),
    }
}

/// Seals one list into one message.
fn sealed(attachments: &[SharedAttachment]) -> Vec<u8> {
    encode_attachment_manifests(attachments, &key(), &laptop(), &phone())
        .unwrap_or_else(|error| panic!("the list must seal: {error}"))
        .remove(0)
}

#[test]
fn a_manifest_survives_the_round_trip_with_every_field() {
    let message = sealed(&[attachment(1)]);

    assert!(is_attachment_manifest(&message));
    assert_eq!(
        decode_attachment_manifest(&message, &key(), &laptop(), &phone()),
        Ok(vec![attachment(1)])
    );
}

#[test]
fn names_are_not_readable_in_the_message() {
    let message = sealed(&[attachment(1)]);

    assert!(!message.windows(10).any(|window| window == b"whiteboard"));
}

#[test]
fn a_manifest_opens_only_with_the_key_and_direction_it_was_sealed_for() {
    let message = sealed(&[attachment(1)]);

    for (pair_key, from, to) in [
        (PairKey::from_bytes([5; 32]), laptop(), phone()),
        (key(), phone(), laptop()),
    ] {
        assert_eq!(
            decode_attachment_manifest(&message, &pair_key, &from, &to),
            Err(AttachmentManifestError::Unauthenticated)
        );
    }
}

#[test]
fn a_long_list_is_split_and_an_empty_one_sends_nothing() {
    let attachments: Vec<_> = (0..=MAX_ATTACHMENTS_PER_MESSAGE as u128)
        .map(attachment)
        .collect();

    let messages = encode_attachment_manifests(&attachments, &key(), &laptop(), &phone());
    let empty = encode_attachment_manifests(&[], &key(), &laptop(), &phone());

    assert_eq!(messages.map(|messages| messages.len()), Ok(2));
    assert_eq!(empty, Ok(Vec::new()));
}

#[test]
fn implausible_values_are_refused() {
    let mut bad_hash = attachment(1);
    bad_hash.content_hash = "../../secret".to_owned();
    let mut upper_case_hash = attachment(1);
    upper_case_hash.content_hash = "AB".repeat(32);
    let mut too_large = attachment(1);
    too_large.size_bytes = MAX_FILE_TRANSFER_BYTES + 1;
    let mut long_name = attachment(1);
    long_name.file_name = "x".repeat(513);
    let mut no_name = attachment(1);
    no_name.file_name = String::new();

    for invalid in [bad_hash, upper_case_hash, too_large, long_name, no_name] {
        assert_eq!(
            decode_attachment_manifest(&sealed(&[invalid]), &key(), &laptop(), &phone()),
            Err(AttachmentManifestError::Invalid)
        );
    }
}

/// A project resource with the given ID number.
fn resource(number: u128) -> SharedAttachment {
    SharedAttachment {
        owner: AttachmentOwner::Project(ProjectId::from_uuid(Uuid::from_u128(85))),
        meeting_time_ms: None,
        ..attachment(number)
    }
}

/// Seals a hand-written JSON body, as a misbehaving peer could.
fn sealed_json(json: &str) -> Vec<u8> {
    let mut message = b"NTA1".to_vec();
    message.extend_from_slice(
        &seal_for_peer(&key(), b"NTA1", &laptop(), &phone(), json.as_bytes())
            .unwrap_or_else(|error| panic!("the body must seal: {error}")),
    );
    message
}

/// One JSON entry whose owner fields are `owner`.
fn entry_with_owner(owner: &str) -> String {
    format!(
        r#"[{{"attachment_id":"{}",{owner},"file_name":"a.png","mime_type":"image/png","content_hash":"{}","size_bytes":1,"original_timestamp":"2026-11-03T10:00:00Z","original_device_id":"laptop","meeting_time_ms":null}}]"#,
        Uuid::from_u128(1),
        "ab".repeat(32)
    )
}

#[test]
fn project_resources_round_trip_in_messages_of_their_own() {
    let mixed = [resource(1), attachment(2), resource(3)];

    let messages = encode_attachment_manifests(&mixed, &key(), &laptop(), &phone())
        .unwrap_or_else(|error| panic!("the list must seal: {error}"));
    let decoded: Vec<Vec<SharedAttachment>> = messages
        .iter()
        .map(|message| {
            decode_attachment_manifest(message, &key(), &laptop(), &phone())
                .unwrap_or_else(|error| panic!("the message must open: {error}"))
        })
        .collect();

    assert_eq!(
        decoded,
        vec![vec![attachment(2)], vec![resource(1), resource(3)]]
    );
}

#[test]
fn an_entry_must_name_exactly_one_owner() {
    let project = Uuid::from_u128(85);
    let both = format!(r#""meeting_id":"meeting-1","project_id":"{project}""#);
    let neither = String::from(r#""meeting_id":"""#);
    let only_project = format!(r#""meeting_id":"","project_id":"{project}""#);

    for owner in [both, neither] {
        assert_eq!(
            decode_attachment_manifest(
                &sealed_json(&entry_with_owner(&owner)),
                &key(),
                &laptop(),
                &phone()
            ),
            Err(AttachmentManifestError::Invalid)
        );
    }
    assert!(
        decode_attachment_manifest(
            &sealed_json(&entry_with_owner(&only_project)),
            &key(),
            &laptop(),
            &phone()
        )
        .is_ok()
    );
}

#[test]
fn content_hashes_are_lowercase_sha256_hex() {
    assert!(is_content_hash(&"0f".repeat(32)));
    assert!(!is_content_hash(&"0f".repeat(31)));
    assert!(!is_content_hash(&"0g".repeat(32)));
}
