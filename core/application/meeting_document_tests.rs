//! Specifies the meeting-document schema: transcript, origin, and segments round-trip, segment
//! replacement is minimal, and concurrent edits from two devices merge without loss. Cluster
//! resolutions (Issue #50) round-trip, decide the speaker of their segments, and converge when
//! two devices name the same cluster at once. A meeting's project (Issue #85) round-trips, can be
//! cleared, and converges when two devices move the meeting at once. The languages a meeting was
//! transcribed in are kept in the order they were first used, and an unknown code is skipped. A
//! meeting folded into another recording of it (#88) round-trips, converges, and can be split.

use automerge::{AutoCommit, ROOT, ReadDoc, transaction::Transactable};
use chrono::{TimeZone, Utc};
use uuid::Uuid;

use super::meeting_document::AutomergeMeetingDocumentCodec;
use crate::{
    domain::{
        models::{DeviceId, MeetingId, TranscriptSegment},
        projects::ProjectId,
        transcription_language::TranscriptionLanguage,
    },
    ports::meeting_document::{
        MeetingCombination, MeetingDocumentCodec, MeetingOrigin, MeetingProjection,
    },
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

/// Moves the fixture meeting into `project_id` (or out of any project), panicking on failure.
fn with_project(document: &[u8], project_id: Option<ProjectId>) -> Vec<u8> {
    AutomergeMeetingDocumentCodec
        .with_project(document, project_id)
        .unwrap_or_else(|error| panic!("the project must be written: {error}"))
}

/// A meeting moved into a project projects that project, and moving it out clears it.
#[test]
fn a_meetings_project_round_trips_and_clears() {
    let project_id = ProjectId::from_uuid(Uuid::from_u128(85));

    let moved_in = with_project(&empty_document(), Some(project_id));
    assert_eq!(project(&moved_in).project_id, Some(project_id));

    let moved_out = with_project(&moved_in, None);
    assert_eq!(project(&moved_out).project_id, None);
    assert_eq!(with_project(&moved_out, None), moved_out);
}

/// Two devices moving one meeting into different projects at once settle on the same project.
#[test]
fn concurrent_moves_converge_on_one_project() {
    let shared = with_project(&empty_document(), None);
    let first = with_project(&shared, Some(ProjectId::from_uuid(Uuid::from_u128(1))));
    let second = with_project(&shared, Some(ProjectId::from_uuid(Uuid::from_u128(2))));

    let mut merged_one_way = AutoCommit::load(&first).unwrap_or_else(|error| panic!("{error}"));
    let mut other = AutoCommit::load(&second).unwrap_or_else(|error| panic!("{error}"));
    merged_one_way
        .merge(&mut other)
        .unwrap_or_else(|error| panic!("{error}"));
    let mut merged_other_way = AutoCommit::load(&second).unwrap_or_else(|error| panic!("{error}"));
    let mut first_copy = AutoCommit::load(&first).unwrap_or_else(|error| panic!("{error}"));
    merged_other_way
        .merge(&mut first_copy)
        .unwrap_or_else(|error| panic!("{error}"));

    let winner = project(&merged_one_way.save()).project_id;
    assert!(winner.is_some());
    assert_eq!(winner, project(&merged_other_way.save()).project_id);
}

/// A peer's project value that is not a project ID leaves the meeting outside any project.
#[test]
fn a_malformed_project_value_is_ignored() {
    let mut document = AutoCommit::new();
    document
        .put(ROOT, "project_id", "not a project")
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(project(&document.save()).project_id, None);
}

/// Records `language` as used from `offset_ms`, panicking with context on failure.
fn with_language(document: &[u8], language: &str, offset_ms: u64) -> Vec<u8> {
    let language = TranscriptionLanguage::parse(language)
        .unwrap_or_else(|error| panic!("{language} must be a language: {error}"));
    AutomergeMeetingDocumentCodec
        .with_transcription_language(document, language, offset_ms)
        .unwrap_or_else(|error| panic!("the language must be recorded: {error}"))
}

/// The codes of a projection's languages.
fn language_codes(projection: &MeetingProjection) -> Vec<&'static str> {
    projection
        .transcription_languages
        .iter()
        .map(|language| language.as_str())
        .collect()
}

/// Languages project in the order they were first used; recording one again changes nothing,
/// and an earlier first use moves it forward.
#[test]
fn transcription_languages_keep_the_order_of_first_use() {
    let document = with_language(&empty_document(), "it", 0);
    let document = with_language(&document, "en", 30_000);
    let unchanged = with_language(&document, "it", 60_000);
    assert_eq!(unchanged, document);
    assert_eq!(language_codes(&project(&document)), vec!["it", "en"]);

    let document = with_language(&document, "auto", 10_000);
    assert_eq!(
        language_codes(&project(&document)),
        vec!["it", "auto", "en"]
    );
}

/// A code another device wrote that this build does not know is skipped, not fatal.
#[test]
fn an_unknown_transcription_language_is_skipped() {
    let mut automerge_document = AutoCommit::new();
    let languages = automerge_document
        .put_object(ROOT, "transcription_languages", automerge::ObjType::Map)
        .unwrap_or_else(|error| panic!("{error}"));
    automerge_document
        .put(&languages, "xx", 0_u64)
        .and_then(|()| automerge_document.put(&languages, "de", 5_u64))
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(
        language_codes(&project(&automerge_document.save())),
        vec!["de"]
    );
}

/// Two devices recording different languages into copies of one document merge both.
#[test]
fn transcription_languages_from_two_devices_merge() {
    let base = with_language(&empty_document(), "it", 0);
    let mut laptop = AutoCommit::load(&with_language(&base, "fr", 20_000))
        .unwrap_or_else(|error| panic!("{error}"));
    let mut phone = AutoCommit::load(&with_language(&base, "es", 10_000))
        .unwrap_or_else(|error| panic!("{error}"));
    laptop
        .merge(&mut phone)
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(
        language_codes(&project(&laptop.save())),
        vec!["it", "es", "fr"]
    );
}

/// Folds the fixture meeting into `combination`'s meeting (or unfolds it), panicking on failure.
fn with_combination(document: &[u8], combination: Option<&MeetingCombination>) -> Vec<u8> {
    AutomergeMeetingDocumentCodec
        .with_combination(document, combination)
        .unwrap_or_else(|error| panic!("the combination must be written: {error}"))
}

/// The fixture meeting folded into the laptop's recording, combined on `device`.
fn combination(device: &str) -> MeetingCombination {
    MeetingCombination {
        into: MeetingId::new("meeting-laptop"),
        offset_ms: -2_500,
        combine_on: DeviceId::new(device),
    }
}

/// A meeting folded into another recording of it projects the fold, and Split clears it (#88).
#[test]
fn a_meetings_combination_round_trips_and_clears() {
    let folded = with_combination(&empty_document(), Some(&combination("laptop")));
    assert_eq!(project(&folded).combined_into, Some(combination("laptop")));
    assert_eq!(
        with_combination(&folded, Some(&combination("laptop"))),
        folded
    );

    let split = with_combination(&folded, None);
    assert_eq!(project(&split).combined_into, None);
}

/// Meetings the owner split from this one are remembered, each once, so they are not joined again.
#[test]
fn meetings_kept_apart_accumulate() {
    let codec = AutomergeMeetingDocumentCodec;
    let document = codec
        .with_kept_apart(&empty_document(), &MeetingId::new("meeting-b"))
        .and_then(|document| codec.with_kept_apart(&document, &MeetingId::new("meeting-a")))
        .and_then(|document| codec.with_kept_apart(&document, &MeetingId::new("meeting-b")))
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(
        project(&document).kept_apart_from,
        vec![MeetingId::new("meeting-a"), MeetingId::new("meeting-b")]
    );
}

/// Two devices that both found the same pair, each choosing who combines, settle on one choice.
#[test]
fn concurrent_combinations_converge_on_one_device() {
    let shared = empty_document();
    let first = with_combination(&shared, Some(&combination("laptop")));
    let second = with_combination(&shared, Some(&combination("phone")));

    let mut one_way = AutoCommit::load(&first).unwrap_or_else(|error| panic!("{error}"));
    let mut other = AutoCommit::load(&second).unwrap_or_else(|error| panic!("{error}"));
    one_way
        .merge(&mut other)
        .unwrap_or_else(|error| panic!("{error}"));
    let mut other_way = AutoCommit::load(&second).unwrap_or_else(|error| panic!("{error}"));
    let mut first_copy = AutoCommit::load(&first).unwrap_or_else(|error| panic!("{error}"));
    other_way
        .merge(&mut first_copy)
        .unwrap_or_else(|error| panic!("{error}"));

    let winner = project(&one_way.save()).combined_into;
    assert!(winner.is_some());
    assert_eq!(winner, project(&other_way.save()).combined_into);
}

/// A peer's combination missing a field is ignored, so the meeting stays listed.
#[test]
fn an_incomplete_combination_is_ignored() {
    let mut document = AutoCommit::new();
    let combination = document
        .put_object(ROOT, "combined_into", automerge::ObjType::Map)
        .unwrap_or_else(|error| panic!("{error}"));
    document
        .put(&combination, "meeting_id", "meeting-laptop")
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(project(&document.save()).combined_into, None);
}

/// Verifies a segment's confidence marks round-trip and clear again (#90, #91).
#[test]
fn segment_confidence_round_trips_and_clears() {
    use crate::domain::transcript_confidence::SegmentConfidence;
    let marked =
        segment(0, 1_000, None, "we agreed on Friday").with_confidence(SegmentConfidence {
            overlapping_speech: true,
            uncertain_speaker: true,
            uncertain_words: vec![1, 3],
        });
    let document = with_segments(&empty_document(), std::slice::from_ref(&marked));

    let projected = project(&document).segments.unwrap_or_default();
    assert_eq!(projected, vec![marked.clone()]);

    let plain = marked.with_confidence(SegmentConfidence::default());
    let cleared = with_segments(&document, std::slice::from_ref(&plain));
    assert_eq!(project(&cleared).segments.unwrap_or_default(), vec![plain]);
}

/// Verifies a segment heard on the computer's sound keeps that source, and loses it again (#108).
#[test]
fn a_computer_sound_segment_keeps_its_source() {
    use crate::domain::models::SegmentSource;
    let heard =
        segment(0, 1_000, None, "can you hear me").with_source(SegmentSource::ComputerSound);
    let document = with_segments(&empty_document(), std::slice::from_ref(&heard));
    assert_eq!(
        project(&document).segments.unwrap_or_default(),
        vec![heard.clone()]
    );

    let spoken = heard.with_source(SegmentSource::Microphone);
    let cleared = with_segments(&document, std::slice::from_ref(&spoken));
    assert_eq!(project(&cleared).segments.unwrap_or_default(), vec![spoken]);
}

/// Verifies overlap spans are kept with the meeting, merged, and joined across devices (#90).
#[test]
fn overlap_spans_are_kept_merged_and_joined_across_devices() {
    use crate::domain::transcript_confidence::OverlapSpan;
    let codec = AutomergeMeetingDocumentCodec;
    let document = codec
        .with_overlap_spans(&empty_document(), &[OverlapSpan::new(1_000, 2_000)])
        .unwrap_or_else(|error| panic!("{error}"));
    let mut laptop = AutoCommit::load(&document).unwrap_or_else(|error| panic!("{error}"));
    let mut phone = laptop.fork();
    let laptop_bytes = codec
        .with_overlap_spans(&laptop.save(), &[OverlapSpan::new(2_100, 3_000)])
        .unwrap_or_else(|error| panic!("{error}"));
    let phone_bytes = codec
        .with_overlap_spans(&phone.save(), &[OverlapSpan::new(8_000, 9_000)])
        .unwrap_or_else(|error| panic!("{error}"));
    laptop = AutoCommit::load(&laptop_bytes).unwrap_or_else(|error| panic!("{error}"));
    phone = AutoCommit::load(&phone_bytes).unwrap_or_else(|error| panic!("{error}"));
    laptop
        .merge(&mut phone)
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(
        codec
            .overlap_spans(&laptop.save())
            .unwrap_or_else(|error| panic!("{error}")),
        vec![
            OverlapSpan::new(1_000, 3_000),
            OverlapSpan::new(8_000, 9_000)
        ]
    );
    assert_eq!(
        codec
            .overlap_spans(&empty_document())
            .unwrap_or_else(|error| panic!("{error}")),
        Vec::new()
    );
}
