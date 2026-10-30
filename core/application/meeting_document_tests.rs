//! Specifies the meeting-document schema: transcript, origin, and segments round-trip, segment
//! replacement is minimal, and concurrent edits from two devices merge without loss. Cluster
//! resolutions (Issue #50) round-trip, decide the speaker of their segments, and converge when
//! two devices name the same cluster at once.

use automerge::{AutoCommit, ROOT, ReadDoc, transaction::Transactable};
use chrono::{TimeZone, Utc};
use uuid::Uuid;

use super::meeting_document::AutomergeMeetingDocumentCodec;
use crate::{
    domain::models::{DeviceId, MeetingId, TranscriptSegment},
    ports::meeting_document::{MeetingDocumentCodec, MeetingOrigin, MeetingProjection},
};

/// The meeting all fixtures belong to.
fn meeting() -> MeetingId {
    MeetingId::new("meeting-rome")
}

/// Builds a segment of the fixture meeting.
fn segment(start: u64, end: u64, speaker: Option<u128>, text: &str) -> TranscriptSegment {
    TranscriptSegment::new(meeting(), start, end, speaker.map(Uuid::from_u128), text)
}

/// Returns an empty serialized document.
fn empty_document() -> Vec<u8> {
    AutoCommit::new().save()
}

/// Projects `document`, panicking with context on failure.
fn project(document: &[u8]) -> MeetingProjection {
    AutomergeMeetingDocumentCodec
        .project(&meeting(), document)
        .unwrap_or_else(|error| panic!("document must project: {error}"))
}

/// Applies `segments`, panicking with context on failure.
fn with_segments(document: &[u8], segments: &[TranscriptSegment]) -> Vec<u8> {
    AutomergeMeetingDocumentCodec
        .with_segments(document, segments)
        .unwrap_or_else(|error| panic!("segments must be written: {error}"))
}

/// A document without the new fields projects to "nothing known", so the index is left alone.
#[test]
fn an_empty_document_projects_nothing() {
    assert_eq!(project(&empty_document()), MeetingProjection::default());
}

/// Origin, transcript, and segments (with speaker and cluster IDs) round-trip in start order.
#[test]
fn origin_transcript_and_segments_round_trip() {
    let origin = MeetingOrigin {
        created_at: Utc
            .timestamp_millis_opt(1_700_000_000_123)
            .single()
            .unwrap_or_default(),
        acquisition_device_id: DeviceId::new("device-laptop"),
    };
    let segments = vec![
        segment(5_000, 9_000, None, "Second").with_unknown_cluster_id(Some(Uuid::from_u128(7))),
        segment(0, 4_000, Some(1), "First"),
    ];

    let document = AutomergeMeetingDocumentCodec
        .with_origin(&empty_document(), &origin)
        .and_then(|document| {
            AutomergeMeetingDocumentCodec.with_transcript(&document, "First Second")
        })
        .unwrap_or_else(|error| panic!("document must be written: {error}"));
    let document = with_segments(&document, &segments);
    let projection = project(&document);

    assert_eq!(projection.origin, Some(origin));
    assert_eq!(projection.transcript.as_deref(), Some("First Second"));
    assert_eq!(
        projection.segments,
        Some(vec![segments[1].clone(), segments[0].clone()])
    );
}

/// Replacing the list removes segments that are gone and keeps identical spans apart.
#[test]
fn replacing_segments_removes_old_ones_and_keeps_duplicate_spans() {
    let document = with_segments(
        &empty_document(),
        &[
            segment(0, 1_000, None, "Old"),
            segment(2_000, 3_000, None, "Kept"),
        ],
    );
    let replacement = vec![
        segment(2_000, 3_000, None, "Kept"),
        segment(4_000, 5_000, None, "Same span A"),
        segment(4_000, 5_000, None, "Same span B"),
    ];

    let projection = project(&with_segments(&document, &replacement));

    assert_eq!(projection.segments, Some(replacement));
}

/// A speaker relink on one device and a text correction on another both survive the merge.
#[test]
fn concurrent_edits_to_different_segment_fields_merge() {
    let shared = with_segments(
        &empty_document(),
        &[
            segment(0, 1_000, None, "Helo"),
            segment(2_000, 3_000, None, "Two"),
        ],
    );
    let laptop_edit = with_segments(
        &shared,
        &[
            segment(0, 1_000, Some(9), "Helo"),
            segment(2_000, 3_000, None, "Two"),
        ],
    );
    let phone_edit = with_segments(
        &shared,
        &[
            segment(0, 1_000, None, "Hello"),
            segment(2_000, 3_000, None, "Two"),
        ],
    );

    let mut merged = AutoCommit::load(&laptop_edit)
        .unwrap_or_else(|error| panic!("laptop document must load: {error}"));
    let mut phone_document = AutoCommit::load(&phone_edit)
        .unwrap_or_else(|error| panic!("phone document must load: {error}"));
    merged
        .merge(&mut phone_document)
        .unwrap_or_else(|error| panic!("documents must merge: {error}"));

    assert_eq!(
        project(&merged.save()).segments,
        Some(vec![
            segment(0, 1_000, Some(9), "Hello"),
            segment(2_000, 3_000, None, "Two")
        ])
    );
}

/// The origin is written once; a later origin never replaces it.
#[test]
fn the_origin_is_written_once() {
    let first = MeetingOrigin {
        created_at: Utc.timestamp_millis_opt(1_000).single().unwrap_or_default(),
        acquisition_device_id: DeviceId::new("device-laptop"),
    };
    let second = MeetingOrigin {
        created_at: Utc.timestamp_millis_opt(2_000).single().unwrap_or_default(),
        acquisition_device_id: DeviceId::new("device-phone"),
    };

    let document = AutomergeMeetingDocumentCodec
        .with_origin(&empty_document(), &first)
        .and_then(|document| AutomergeMeetingDocumentCodec.with_origin(&document, &second))
        .unwrap_or_else(|error| panic!("origin must be written: {error}"));

    assert_eq!(project(&document).origin, Some(first));
}

/// A peer's malformed segment entry is skipped instead of failing the whole projection.
#[test]
fn malformed_segment_entries_are_skipped() {
    let mut document = AutoCommit::load(&with_segments(
        &empty_document(),
        &[segment(0, 1_000, None, "Valid")],
    ))
    .unwrap_or_else(|error| panic!("document must load: {error}"));
    let segments_object = document
        .get(ROOT, "segments")
        .ok()
        .flatten()
        .map(|(_, object_id)| object_id)
        .unwrap_or_else(|| panic!("segments map must exist"));
    let malformed = document
        .put_object(&segments_object, "garbage", automerge::ObjType::Map)
        .unwrap_or_else(|error| panic!("entry must be created: {error}"));
    document
        .put(&malformed, "start_ms", "not a number")
        .unwrap_or_else(|error| panic!("field must be written: {error}"));

    assert_eq!(
        project(&document.save()).segments,
        Some(vec![segment(0, 1_000, None, "Valid")])
    );
}

/// A corrupt document is an error, not an empty projection.
#[test]
fn a_corrupt_document_is_an_error() {
    assert!(
        AutomergeMeetingDocumentCodec
            .project(&meeting(), b"not automerge")
            .is_err()
    );
}

/// Records that `cluster` was named as `identity`, panicking with context on failure.
fn with_resolution(document: &[u8], cluster: u128, identity: u128) -> Vec<u8> {
    AutomergeMeetingDocumentCodec
        .with_cluster_resolution(
            document,
            Uuid::from_u128(cluster),
            Uuid::from_u128(identity),
        )
        .unwrap_or_else(|error| panic!("resolution must be written: {error}"))
}

/// A resolution round-trips and sets the speaker of every segment of its cluster, whatever the
/// segment's own speaker field says; other segments keep theirs.
#[test]
fn a_cluster_resolution_names_the_speaker_of_its_segments() {
    let cluster = Uuid::from_u128(0xC1);
    let document = with_segments(
        &empty_document(),
        &[
            segment(0, 1_000, None, "Hi").with_unknown_cluster_id(Some(cluster)),
            segment(2_000, 3_000, Some(5), "Bye").with_unknown_cluster_id(Some(cluster)),
            segment(4_000, 5_000, Some(6), "Other"),
        ],
    );

    let projection = project(&with_resolution(&document, 0xC1, 9));

    assert_eq!(
        projection.cluster_resolutions,
        vec![(cluster, Uuid::from_u128(9))]
    );
    assert_eq!(
        projection.segments,
        Some(vec![
            segment(0, 1_000, Some(9), "Hi").with_unknown_cluster_id(Some(cluster)),
            segment(2_000, 3_000, Some(9), "Bye").with_unknown_cluster_id(Some(cluster)),
            segment(4_000, 5_000, Some(6), "Other"),
        ])
    );
}

/// When two devices name the same cluster at once, both merge orders pick the same identity, and
/// every segment of the cluster follows it.
#[test]
fn concurrent_cluster_resolutions_converge() {
    let cluster = Uuid::from_u128(0xC1);
    let shared = with_segments(
        &empty_document(),
        &[
            segment(0, 1_000, None, "Hi").with_unknown_cluster_id(Some(cluster)),
            segment(2_000, 3_000, None, "Bye").with_unknown_cluster_id(Some(cluster)),
        ],
    );
    let laptop_edit = with_resolution(&shared, 0xC1, 1);
    let phone_edit = with_resolution(&shared, 0xC1, 2);
    let merge = |first: &[u8], second: &[u8]| {
        let mut merged = AutoCommit::load(first)
            .unwrap_or_else(|error| panic!("first document must load: {error}"));
        let mut other = AutoCommit::load(second)
            .unwrap_or_else(|error| panic!("second document must load: {error}"));
        merged
            .merge(&mut other)
            .unwrap_or_else(|error| panic!("documents must merge: {error}"));
        project(&merged.save())
    };

    let on_laptop = merge(&laptop_edit, &phone_edit);
    let on_phone = merge(&phone_edit, &laptop_edit);

    assert_eq!(on_laptop, on_phone);
    assert_eq!(on_laptop.cluster_resolutions.len(), 1);
    let winner = on_laptop.cluster_resolutions[0].1;
    assert!(winner == Uuid::from_u128(1) || winner == Uuid::from_u128(2));
    assert!(
        on_laptop
            .segments
            .unwrap_or_default()
            .iter()
            .all(|segment| segment.speaker_identity_id() == Some(winner))
    );
}

/// A resolution entry that is not a UUID pair is skipped.
#[test]
fn malformed_cluster_resolutions_are_skipped() {
    let mut document = AutoCommit::new();
    let resolutions = document
        .put_object(ROOT, "cluster_resolutions", automerge::ObjType::Map)
        .unwrap_or_else(|error| panic!("map must be written: {error}"));
    document
        .put(&resolutions, "not-a-uuid", Uuid::from_u128(1).to_string())
        .and_then(|()| document.put(&resolutions, Uuid::from_u128(2).to_string(), "not-a-uuid"))
        .unwrap_or_else(|error| panic!("entries must be written: {error}"));

    assert!(project(&document.save()).cluster_resolutions.is_empty());
}
