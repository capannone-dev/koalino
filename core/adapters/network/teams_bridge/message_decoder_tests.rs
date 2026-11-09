//! Specifies decoding of Teams bridge frames into domain values, against golden bytes produced by
//! `protoc --encode` from `core/protocol/teams_bridge.proto` (see `core/protocol/fixtures/`).

use protobuf::Message;

use crate::{
    adapters::network::teams_bridge::message_decoder::{
        BridgeEvent, MAXIMUM_AUDIO_CHUNK_MS, MAXIMUM_DOCUMENT_BYTES, decode_frame,
        local_meeting_uuid,
    },
    domain::{
        errors::ExternalBridgeError,
        models::{ExternalTrackKind, MeetingDocumentKind, MeetingDocumentMediaType, MeetingId},
    },
    protocol::teams_bridge::{
        AudioChunk, ExternalParticipant, MeetingDocument, MeetingEnded, TeamsBridgeMessage,
        TrackKind, VideoFrame,
    },
};

/// Golden frame: one 3-sample audio chunk from "teams-user-1" at 12 s.
const GOLDEN_AUDIO: &[u8] = include_bytes!("../../../protocol/fixtures/audio.bin");
/// Golden frame: one screen-share PNG frame at 34 s.
const GOLDEN_VIDEO: &[u8] = include_bytes!("../../../protocol/fixtures/video.bin");
/// Golden frame: metadata for "19:meeting_abc@thread.v2" with two participants.
const GOLDEN_METADATA: &[u8] = include_bytes!("../../../protocol/fixtures/metadata.bin");
/// The Teams meeting in the golden metadata.
const GOLDEN_TEAMS_MEETING: &str = "19:meeting_abc@thread.v2";

/// Decodes `frame` for the golden meeting, panicking on errors.
fn decode_for_golden_meeting(frame: &[u8]) -> BridgeEvent {
    decode_frame(frame, Some(local_meeting_uuid(GOLDEN_TEAMS_MEETING)))
        .unwrap_or_else(|error| panic!("frame should decode: {error}"))
}

/// Serializes an audio message with the given payload and sample rate.
fn audio_frame(audio_data: Vec<u8>, sample_rate: u32) -> Vec<u8> {
    let mut participant = ExternalParticipant::new();
    participant.external_id = "teams-user-9".to_owned();
    participant.display_name = "Grace Hopper".to_owned();
    let mut audio = AudioChunk::new();
    audio.participant = protobuf::MessageField::some(participant);
    audio.audio_data = audio_data;
    audio.timestamp_ms = 5_000;
    audio.sample_rate = sample_rate;
    let mut message = TeamsBridgeMessage::new();
    message.set_audio(audio);
    message
        .write_to_bytes()
        .unwrap_or_else(|error| panic!("message should encode: {error}"))
}

/// Serializes a message, panicking on errors.
fn encode(message: &TeamsBridgeMessage) -> Vec<u8> {
    message
        .write_to_bytes()
        .unwrap_or_else(|error| panic!("message should encode: {error}"))
}

/// Serializes a meeting document.
fn document_frame(kind: &str, media_type: &str, content: Vec<u8>) -> Vec<u8> {
    let mut document = MeetingDocument::new();
    document.kind = kind.to_owned();
    document.media_type = media_type.to_owned();
    document.content = content;
    document.timestamp_ms = 0;
    let mut message = TeamsBridgeMessage::new();
    message.set_document(document);
    encode(&message)
}

/// Serializes a meeting end for `teams_meeting_id`.
fn ended_frame(teams_meeting_id: &str) -> Vec<u8> {
    let mut ended = MeetingEnded::new();
    ended.meeting_id = teams_meeting_id.to_owned();
    let mut message = TeamsBridgeMessage::new();
    message.set_ended(ended);
    encode(&message)
}

/// Verifies the golden audio frame becomes an `ExternalMediaChunk` with participant and timing.
#[test]
fn golden_audio_decodes_into_an_external_media_chunk() {
    match decode_for_golden_meeting(GOLDEN_AUDIO) {
        BridgeEvent::Audio(bridge_audio) => {
            let chunk = bridge_audio.media_chunk();
            assert_eq!(chunk.external_participant().external_id(), "teams-user-1");
            assert_eq!(chunk.external_participant().display_name(), "Ada Lovelace");
            assert_eq!(
                chunk.external_participant().email(),
                Some("ada@example.com")
            );
            assert_eq!(chunk.audio_bytes(), &[1, 0, 2, 0, 0xFF, 0xFF]);
            assert_eq!(chunk.timestamp_ms(), 12_000);
            assert_eq!(bridge_audio.duration_ms(), 0);
            assert_eq!(bridge_audio.sample_count(), 3);
        }
        other => panic!("expected audio, got {other:?}"),
    }
}

/// Verifies the golden screen-share frame becomes a `SlideCapture` of the active meeting.
#[test]
fn golden_video_decodes_into_a_slide_capture() {
    match decode_for_golden_meeting(GOLDEN_VIDEO) {
        BridgeEvent::Slide(slide) => {
            assert_eq!(slide.meeting_id(), local_meeting_uuid(GOLDEN_TEAMS_MEETING));
            assert_eq!(slide.image_bytes(), &[0x89, b'P', b'N', b'G']);
            assert_eq!(slide.timestamp_ms(), 34_000);
            assert_eq!(slide.source(), "screen_share");
        }
        other => panic!("expected a slide, got {other:?}"),
    }
}

/// Verifies golden metadata decodes without an active meeting and names the local meeting.
#[test]
fn golden_metadata_decodes_into_a_meeting_start() {
    match decode_frame(GOLDEN_METADATA, None) {
        Ok(BridgeEvent::MeetingStarted(meeting)) => {
            assert_eq!(meeting.teams_meeting_id(), GOLDEN_TEAMS_MEETING);
            assert_eq!(meeting.title(), "Weekly sync");
            assert_eq!(meeting.start_time_ms(), 1_793_000_000_000);
            assert_eq!(meeting.participants().len(), 2);
            assert_eq!(meeting.participants()[1].email(), Some("alan@example.com"));
            assert_eq!(meeting.participants()[0].email(), None);
            assert_eq!(
                meeting.meeting_id(),
                &MeetingId::new(local_meeting_uuid(GOLDEN_TEAMS_MEETING).to_string())
            );
        }
        other => panic!("expected a meeting start, got {other:?}"),
    }
}

/// Verifies the local meeting UUID is stable per Teams meeting (reconnects map to one meeting).
#[test]
fn local_meeting_uuids_are_stable_and_distinct() {
    assert_eq!(
        local_meeting_uuid("meeting-a"),
        local_meeting_uuid("meeting-a")
    );
    assert_ne!(
        local_meeting_uuid("meeting-a"),
        local_meeting_uuid("meeting-b")
    );
}

/// Verifies camera frames are recognized but not turned into slides.
#[test]
fn camera_frames_are_ignored() {
    let mut video = VideoFrame::new();
    video.image_data = vec![0xFF, 0xD8];
    video.source = "camera".to_owned();
    let mut message = TeamsBridgeMessage::new();
    message.set_video(video);
    let frame = message
        .write_to_bytes()
        .unwrap_or_else(|error| panic!("message should encode: {error}"));

    assert!(matches!(
        decode_for_golden_meeting(&frame),
        BridgeEvent::IgnoredVideo
    ));
}

/// Verifies frames that the pipeline cannot use are rejected with a reason.
#[test]
fn invalid_frames_are_rejected() {
    let ten_milliseconds = vec![0_u8; 320];
    let over_limit = vec![0_u8; usize::try_from(MAXIMUM_AUDIO_CHUNK_MS * 32 + 32).unwrap_or(0)];
    for (label, frame) in [
        ("malformed bytes", vec![0xFF, 0xFF, 0xFF]),
        ("empty envelope", Vec::new()),
        (
            "44.1 kHz audio",
            audio_frame(ten_milliseconds.clone(), 44_100),
        ),
        ("odd byte count", audio_frame(vec![0, 1, 2], 16_000)),
        ("empty audio", audio_frame(Vec::new(), 16_000)),
        ("over 30 s of audio", audio_frame(over_limit, 16_000)),
    ] {
        assert!(
            matches!(
                decode_frame(&frame, Some(local_meeting_uuid("m"))),
                Err(ExternalBridgeError::InvalidData { .. })
            ),
            "{label} must be rejected"
        );
    }
    assert!(
        decode_frame(
            &audio_frame(ten_milliseconds, 16_000),
            Some(local_meeting_uuid("m"))
        )
        .is_ok()
    );
}

/// Verifies audio and video before any `MeetingMetadata` are rejected (no meeting to attach to).
#[test]
fn media_before_meeting_metadata_is_rejected() {
    for frame in [GOLDEN_AUDIO, GOLDEN_VIDEO] {
        let error = decode_frame(frame, None)
            .err()
            .unwrap_or_else(|| panic!("media without a meeting must be rejected"));
        assert!(error.to_string().contains("MeetingMetadata"), "{error}");
    }
}

/// Verifies audio without a track kind (the bot, older senders) is a participant's track.
#[test]
fn audio_without_a_track_kind_is_a_participant_track() {
    match decode_for_golden_meeting(GOLDEN_AUDIO) {
        BridgeEvent::Audio(audio) => {
            assert_eq!(
                audio.media_chunk().track_kind(),
                ExternalTrackKind::Participant
            );
        }
        other => panic!("expected audio, got {other:?}"),
    }
}

/// Verifies a browser's mixed remote audio is marked as such (Issue #102).
#[test]
fn mixed_remote_audio_is_marked() {
    let mut message = TeamsBridgeMessage::parse_from_bytes(&audio_frame(vec![0; 320], 16_000))
        .unwrap_or_else(|error| panic!("frame should parse: {error}"));
    message.mut_audio().track = TrackKind::MIXED_REMOTE.into();
    match decode_for_golden_meeting(&encode(&message)) {
        BridgeEvent::Audio(audio) => {
            assert_eq!(
                audio.media_chunk().track_kind(),
                ExternalTrackKind::MixedRemote
            );
        }
        other => panic!("expected audio, got {other:?}"),
    }
}

/// Verifies a Teams transcript becomes a document of the active meeting.
#[test]
fn a_teams_transcript_decodes_into_a_meeting_document() {
    let content = b"WEBVTT\n\n00:00:01.000 --> 00:00:02.000\n<v Ada>Hello</v>\n".to_vec();
    match decode_for_golden_meeting(&document_frame(
        "teams_transcript",
        "text/vtt",
        content.clone(),
    )) {
        BridgeEvent::Document(document) => {
            assert_eq!(
                document.meeting_id(),
                local_meeting_uuid(GOLDEN_TEAMS_MEETING)
            );
            assert_eq!(document.kind(), MeetingDocumentKind::TeamsTranscript);
            assert_eq!(document.media_type(), MeetingDocumentMediaType::WebVtt);
            assert_eq!(document.text().as_bytes(), content.as_slice());
        }
        other => panic!("expected a document, got {other:?}"),
    }
}

/// Verifies the live transcript and captions kinds are accepted as plain text or WebVTT.
#[test]
fn live_transcript_and_captions_are_accepted() {
    for (kind, expected) in [
        (
            "teams_live_transcript",
            MeetingDocumentKind::TeamsLiveTranscript,
        ),
        ("teams_captions", MeetingDocumentKind::TeamsCaptions),
    ] {
        match decode_for_golden_meeting(&document_frame(kind, "text/plain", b"x".to_vec())) {
            BridgeEvent::Document(document) => {
                assert_eq!(document.kind(), expected);
                assert_eq!(document.media_type(), MeetingDocumentMediaType::PlainText);
            }
            other => panic!("expected a document, got {other:?}"),
        }
    }
}

/// Verifies documents the core cannot store safely are rejected.
#[test]
fn invalid_documents_are_rejected() {
    let too_large = vec![b'a'; MAXIMUM_DOCUMENT_BYTES + 1];
    for (label, frame) in [
        (
            "unknown kind",
            document_frame("chat_export", "text/vtt", b"x".to_vec()),
        ),
        (
            "unknown media type",
            document_frame("teams_transcript", "text/html", b"x".to_vec()),
        ),
        (
            "empty content",
            document_frame("teams_transcript", "text/vtt", Vec::new()),
        ),
        (
            "not UTF-8",
            document_frame("teams_transcript", "text/vtt", vec![0xFF, 0xFE]),
        ),
        (
            "over the size limit",
            document_frame("teams_transcript", "text/vtt", too_large),
        ),
    ] {
        assert!(
            matches!(
                decode_frame(&frame, Some(local_meeting_uuid("m"))),
                Err(ExternalBridgeError::InvalidData { .. })
            ),
            "{label} must be rejected"
        );
    }
    assert!(
        decode_frame(
            &document_frame("teams_transcript", "text/vtt", b"x".to_vec()),
            None
        )
        .is_err(),
        "a document needs a meeting"
    );
}

/// Verifies a document naming its meeting needs no active meeting (a transcript published after
/// the meeting ended).
#[test]
fn a_document_naming_its_meeting_needs_no_active_meeting() {
    let mut message = TeamsBridgeMessage::parse_from_bytes(&document_frame(
        "teams_transcript",
        "text/vtt",
        b"WEBVTT".to_vec(),
    ))
    .unwrap_or_else(|error| panic!("frame should parse: {error}"));
    message.mut_document().meeting_id = GOLDEN_TEAMS_MEETING.to_owned();
    match decode_frame(&encode(&message), None) {
        Ok(BridgeEvent::Document(document)) => {
            assert_eq!(
                document.meeting_id(),
                local_meeting_uuid(GOLDEN_TEAMS_MEETING)
            );
        }
        other => panic!("expected a document, got {other:?}"),
    }
}

/// Verifies a meeting end names the active meeting, and is rejected for any other meeting.
#[test]
fn a_meeting_end_must_name_the_active_meeting() {
    match decode_for_golden_meeting(&ended_frame(GOLDEN_TEAMS_MEETING)) {
        BridgeEvent::MeetingEnded(meeting_uuid) => {
            assert_eq!(meeting_uuid, local_meeting_uuid(GOLDEN_TEAMS_MEETING));
        }
        other => panic!("expected a meeting end, got {other:?}"),
    }
    assert!(
        decode_frame(
            &ended_frame("19:other@thread.v2"),
            Some(local_meeting_uuid(GOLDEN_TEAMS_MEETING))
        )
        .is_err()
    );
    assert!(decode_frame(&ended_frame(GOLDEN_TEAMS_MEETING), None).is_err());
}
