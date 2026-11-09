//! Specifies how the session credits an online-meeting participant's track (#103): one voice goes
//! to the participant, a track with several voices (a meeting room) has each voice matched against
//! the gallery, and a participant's voice is offered for the gallery only after enough clean
//! speech of one voice that does not sound like someone else.

use std::cell::RefCell;

use uuid::Uuid;

use super::{MEETING_ID, MockTranscriptionEngine, session_fixture};
use crate::{
    application::pipeline::meeting_session::SpeakerAttribution,
    domain::{
        errors::StorageError,
        models::{
            AudioRecording, DeviceId, ExternalMediaChunk, ExternalParticipant, ExternalTrackKind,
            MeetingId, RecordingId, SpeakerIdentity,
        },
    },
    ports::{ai::SpeakerSegment, external_bridge::IdentityMapper},
};

/// Maps every participant to one identity and records the voices offered to it.
struct OfferRecordingMapper {
    identity: SpeakerIdentity,
    offered: RefCell<Vec<Vec<f32>>>,
    mapped: RefCell<usize>,
}

impl OfferRecordingMapper {
    /// A mapper for a fresh participant identity.
    fn new() -> Self {
        Self {
            identity: SpeakerIdentity::new(Uuid::new_v4(), "Meeting Room 4"),
            offered: RefCell::new(Vec::new()),
            mapped: RefCell::new(0),
        }
    }

    /// The participant's attribution.
    fn participant(&self) -> SpeakerAttribution {
        SpeakerAttribution::Known {
            identity_id: self.identity.identity_id(),
        }
    }
}

impl IdentityMapper for OfferRecordingMapper {
    /// Returns the fixed identity.
    fn map_external_to_local(
        &self,
        _external: ExternalParticipant,
    ) -> Result<SpeakerIdentity, StorageError> {
        *self.mapped.borrow_mut() += 1;
        Ok(self.identity.clone())
    }

    /// Records the offer and reports it kept.
    fn keep_participant_voice(
        &self,
        _identity: &SpeakerIdentity,
        embedding: Vec<f32>,
    ) -> Result<bool, StorageError> {
        self.offered.borrow_mut().push(embedding);
        Ok(true)
    }
}

/// A chunk of the room participant's track starting at `start_ms`.
fn track_chunk(start_ms: u64) -> (ExternalMediaChunk, AudioRecording) {
    (
        ExternalMediaChunk::new(
            ExternalParticipant::new("teams-room-4", "Meeting Room 4", None),
            vec![0; 32],
            start_ms,
        ),
        AudioRecording::new(
            RecordingId::new(format!("external-teams-room-4-{start_ms}")),
            MeetingId::new(MEETING_ID),
            DeviceId::new("device-test"),
        ),
    )
}

/// A voice the mock matcher finds in no one (its first value is neither 1.0 nor 0.5).
const UNKNOWN_VOICE: [f32; 2] = [0.2, 0.9];
/// A voice unlike `UNKNOWN_VOICE` and `[1.0, 0.0]`, also matched to no one.
const OTHER_UNKNOWN_VOICE: [f32; 2] = [0.1, -0.9];

#[tokio::test]
async fn a_one_voice_track_is_credited_to_the_participant() {
    let mut fixture = session_fixture(
        MockTranscriptionEngine::returning(&["good morning"]),
        vec![Ok(vec![SpeakerSegment::new(
            0,
            2_500,
            UNKNOWN_VOICE.to_vec(),
        )])],
    );
    let mapper = OfferRecordingMapper::new();
    let (media_chunk, recording) = track_chunk(10_000);

    let produced = fixture
        .session
        .process_external_chunk(&mapper, &media_chunk, &recording, 2_500)
        .await
        .unwrap_or_else(|error| panic!("chunk should process: {error}"));

    assert_eq!(produced.len(), 1);
    assert_eq!(produced[0].attribution(), &mapper.participant());
    assert!(mapper.offered.borrow().is_empty(), "too little speech");
}

#[tokio::test]
async fn a_room_track_matches_each_voice_against_the_gallery() {
    let mut fixture = session_fixture(
        MockTranscriptionEngine::returning(&["welcome everyone and thanks"]),
        vec![Ok(vec![
            SpeakerSegment::new(0, 4_000, vec![1.0, 0.0]),
            SpeakerSegment::new(4_000, 8_000, OTHER_UNKNOWN_VOICE.to_vec()),
        ])],
    );
    let known_identity_id = fixture.known_identity_id;
    let mapper = OfferRecordingMapper::new();
    let (media_chunk, recording) = track_chunk(0);

    let produced = fixture
        .session
        .process_external_chunk(&mapper, &media_chunk, &recording, 8_000)
        .await
        .unwrap_or_else(|error| panic!("chunk should process: {error}"));

    assert!(!produced.is_empty());
    assert!(
        produced
            .iter()
            .all(|segment| segment.attribution() != &mapper.participant()),
        "nothing said in the room is credited to the room"
    );
    assert!(produced.iter().any(|segment| segment.attribution()
        == &SpeakerAttribution::Known {
            identity_id: known_identity_id
        }));
    assert!(mapper.offered.borrow().is_empty());
}

#[tokio::test]
async fn people_taking_turns_in_a_room_are_told_apart_from_then_on() {
    let mut fixture = session_fixture(
        MockTranscriptionEngine::returning(&["first person", "second person", "first again"]),
        vec![
            Ok(vec![SpeakerSegment::new(0, 10_000, UNKNOWN_VOICE.to_vec())]),
            Ok(vec![SpeakerSegment::new(
                0,
                10_000,
                OTHER_UNKNOWN_VOICE.to_vec(),
            )]),
            Ok(vec![SpeakerSegment::new(0, 10_000, UNKNOWN_VOICE.to_vec())]),
        ],
    );
    let mapper = OfferRecordingMapper::new();
    let mut attributions = Vec::new();
    for start_ms in [0, 10_000, 20_000] {
        let (media_chunk, recording) = track_chunk(start_ms);
        let produced = fixture
            .session
            .process_external_chunk(&mapper, &media_chunk, &recording, 10_000)
            .await
            .unwrap_or_else(|error| panic!("chunk should process: {error}"));
        attributions.push(produced[0].attribution().clone());
    }

    assert_eq!(attributions[0], mapper.participant());
    assert!(matches!(
        attributions[1],
        SpeakerAttribution::Unknown { .. }
    ));
    assert!(matches!(
        attributions[2],
        SpeakerAttribution::Unknown { .. }
    ));
}

#[tokio::test]
async fn thirty_seconds_of_one_voice_are_offered_once() {
    let mut fixture = session_fixture(
        MockTranscriptionEngine::returning(&["one", "two", "three", "four"]),
        (0..4)
            .map(|_| Ok(vec![SpeakerSegment::new(0, 10_000, UNKNOWN_VOICE.to_vec())]))
            .collect(),
    );
    let mapper = OfferRecordingMapper::new();
    for start_ms in [0, 10_000, 20_000, 30_000] {
        let (media_chunk, recording) = track_chunk(start_ms);
        fixture
            .session
            .process_external_chunk(&mapper, &media_chunk, &recording, 10_000)
            .await
            .unwrap_or_else(|error| panic!("chunk should process: {error}"));
    }

    let offered = mapper.offered.borrow();
    assert_eq!(offered.len(), 1, "offered once, after 30 s");
    assert!((offered[0][0] - UNKNOWN_VOICE[0]).abs() < 1e-6);
}

#[tokio::test]
async fn a_voice_that_sounds_like_someone_else_is_not_offered() {
    // The mock matcher finds the known gallery person for a first value of 1.0.
    let mut fixture = session_fixture(
        MockTranscriptionEngine::returning(&["one", "two", "three"]),
        (0..3)
            .map(|_| Ok(vec![SpeakerSegment::new(0, 10_000, vec![1.0, 0.1])]))
            .collect(),
    );
    let mapper = OfferRecordingMapper::new();
    for start_ms in [0, 10_000, 20_000] {
        let (media_chunk, recording) = track_chunk(start_ms);
        let produced = fixture
            .session
            .process_external_chunk(&mapper, &media_chunk, &recording, 10_000)
            .await
            .unwrap_or_else(|error| panic!("chunk should process: {error}"));
        assert_eq!(produced[0].attribution(), &mapper.participant());
    }

    assert!(mapper.offered.borrow().is_empty());
}

#[tokio::test]
async fn a_mixed_remote_track_has_its_voices_matched_from_the_first_chunk() {
    // A browser receives every remote participant mixed into one track (#102): even a single
    // voice in it is matched against the gallery, never credited to the track's participant.
    let mut fixture = session_fixture(
        MockTranscriptionEngine::returning(&["hello there", "good morning all"]),
        vec![
            Ok(vec![SpeakerSegment::new(0, 10_000, vec![1.0, 0.0])]),
            Ok(vec![SpeakerSegment::new(0, 10_000, UNKNOWN_VOICE.to_vec())]),
        ],
    );
    let known_identity_id = fixture.known_identity_id;
    let mapper = OfferRecordingMapper::new();
    let mut attributions = Vec::new();
    for start_ms in [0, 10_000] {
        let (media_chunk, recording) = track_chunk(start_ms);
        let media_chunk = media_chunk.with_track_kind(ExternalTrackKind::MixedRemote);
        let produced = fixture
            .session
            .process_external_chunk(&mapper, &media_chunk, &recording, 10_000)
            .await
            .unwrap_or_else(|error| panic!("chunk should process: {error}"));
        attributions.push(produced[0].attribution().clone());
    }

    assert_eq!(
        attributions[0],
        SpeakerAttribution::Known {
            identity_id: known_identity_id
        }
    );
    assert!(matches!(
        attributions[1],
        SpeakerAttribution::Unknown { .. }
    ));
    assert_eq!(
        *mapper.mapped.borrow(),
        0,
        "a mixed track maps no participant"
    );
    assert!(
        mapper.offered.borrow().is_empty(),
        "a mixed track is never kept"
    );
}
