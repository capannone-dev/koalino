//! Specifies the sealed `NTP1` project-list messages (Issue #85).

use chrono::{TimeZone, Utc};
use uuid::Uuid;

use super::project_library_exchange::{
    MAX_PROJECTS, ProjectLibraryMessageError, decode_project_library_message,
    encode_project_library_message, is_project_library_message,
};
use crate::domain::{
    models::DeviceId,
    peer_trust::PairKey,
    projects::{Project, ProjectId, ProjectLibrary, ProjectName, ProjectTombstone},
};

/// A fixed pair key.
fn key(byte: u8) -> PairKey {
    PairKey::from_bytes([byte; 32])
}

fn laptop() -> DeviceId {
    DeviceId::new("laptop")
}

fn phone() -> DeviceId {
    DeviceId::new("phone")
}

/// A library with one project and one deletion record.
fn library() -> ProjectLibrary {
    let instant = Utc
        .timestamp_millis_opt(1_800_000_000_123)
        .single()
        .unwrap_or_else(|| panic!("the instant must exist"));
    ProjectLibrary {
        projects: vec![Project::new(
            ProjectId::from_uuid(Uuid::from_u128(1)),
            ProjectName::parse("Website relaunch").unwrap_or_else(|error| panic!("{error}")),
            instant,
            instant,
        )],
        tombstones: vec![ProjectTombstone {
            project_id: ProjectId::from_uuid(Uuid::from_u128(2)),
            deleted_at: instant,
        }],
    }
}

/// Seals the fixture library from the laptop to the phone.
fn sealed() -> Vec<u8> {
    encode_project_library_message(&library(), &key(1), &laptop(), &phone())
        .unwrap_or_else(|error| panic!("library must seal: {error}"))
}

#[test]
fn a_library_round_trips_and_hides_its_names() {
    let message = sealed();

    assert!(is_project_library_message(&message));
    assert!(
        !message
            .windows(b"Website".len())
            .any(|window| window == b"Website")
    );
    assert_eq!(
        decode_project_library_message(&message, &key(1), &laptop(), &phone()),
        Ok(library())
    );
}

#[test]
fn a_message_opens_only_with_its_key_and_direction() {
    let message = sealed();

    assert_eq!(
        decode_project_library_message(&message, &key(2), &laptop(), &phone()),
        Err(ProjectLibraryMessageError::Unauthenticated)
    );
    assert_eq!(
        decode_project_library_message(&message, &key(1), &phone(), &laptop()),
        Err(ProjectLibraryMessageError::Unauthenticated)
    );
}

#[test]
fn other_payloads_are_not_project_lists() {
    assert!(!is_project_library_message(b"NTV1 voice library"));
    assert_eq!(
        decode_project_library_message(b"NTV1xxxx", &key(1), &laptop(), &phone()),
        Err(ProjectLibraryMessageError::Malformed)
    );
}

#[test]
fn an_oversized_list_is_refused() {
    let mut oversized = library();
    let template = oversized.projects[0].clone();
    oversized.projects = (0..=MAX_PROJECTS as u128)
        .map(|index| {
            Project::new(
                ProjectId::from_uuid(Uuid::from_u128(index + 10)),
                template.name().clone(),
                template.created_at(),
                template.updated_at(),
            )
        })
        .collect();

    assert_eq!(
        encode_project_library_message(&oversized, &key(1), &laptop(), &phone()),
        Err(ProjectLibraryMessageError::Invalid)
    );
}

#[test]
fn a_nested_project_keeps_its_parent_and_move_time_across_the_message() {
    let moved_at = Utc
        .timestamp_millis_opt(1_800_000_100_456)
        .single()
        .unwrap_or_else(|| panic!("the instant must exist"));
    let mut nested = library();
    nested.projects[0] = nested.projects[0]
        .clone()
        .with_placement(Some(ProjectId::from_uuid(Uuid::from_u128(9))), moved_at);
    let message = encode_project_library_message(&nested, &key(1), &laptop(), &phone())
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(
        decode_project_library_message(&message, &key(1), &laptop(), &phone()),
        Ok(nested)
    );
}

#[test]
fn a_message_from_a_device_without_nesting_reads_as_top_level_projects() {
    // What a device from before #142 sends: no parent and no move time.
    let body = br#"{"projects":[{"project_id":"00000000-0000-0000-0000-000000000001",
        "name":"Website relaunch","created_at":"2027-01-15T08:00:00.123Z",
        "updated_at":"2027-01-15T08:00:00.123Z"}],"tombstones":[]}"#;
    let sealed = crate::application::crypto::pair_key::seal_for_peer(
        &key(1),
        b"NTP1",
        &laptop(),
        &phone(),
        body,
    )
    .unwrap_or_else(|error| panic!("{error:?}"));
    let message = [b"NTP1".as_slice(), &sealed].concat();

    let decoded = decode_project_library_message(&message, &key(1), &laptop(), &phone())
        .unwrap_or_else(|error| panic!("{error}"));

    let project = &decoded.projects[0];
    assert_eq!(project.parent(), None);
    assert_eq!(project.moved_at(), project.created_at());
}
