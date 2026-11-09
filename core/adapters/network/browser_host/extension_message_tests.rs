//! Specifies how the extension's JSON messages become Teams bridge frames.

use base64::Engine as _;
use protobuf::Message as _;
use serde_json::json;

use super::extension_message::{BridgeFrame, ExtensionMessageError, to_bridge_frame};
use crate::protocol::teams_bridge::{TeamsBridgeMessage, TrackKind, teams_bridge_message::Payload};

/// Base64 of `bytes`.
fn encoded(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Converts `message` and parses the resulting frame back.
fn converted(message: serde_json::Value) -> (BridgeFrame, Payload) {
    let frame =
        to_bridge_frame(message).unwrap_or_else(|error| panic!("message should convert: {error}"));
    let payload = TeamsBridgeMessage::parse_from_bytes(frame.bytes())
        .unwrap_or_else(|error| panic!("frame should parse: {error}"))
        .payload
        .unwrap_or_else(|| panic!("frame should carry a payload"));
    (frame, payload)
}

#[test]
fn a_meeting_becomes_metadata() {
    let (frame, payload) = converted(json!({
        "type": "meeting",
        "meetingId": "19:meeting_x@thread.v2",
        "title": "Weekly sync",
        "startTimeMs": 1_794_000_000_000_u64,
        "participants": [{"id": "8:orgid:abc", "name": "Ada Lovelace"}]
    }));
    assert_eq!(frame.audio_duration_ms(), None);
    let Payload::Metadata(metadata) = payload else {
        panic!("expected metadata");
    };
    assert_eq!(metadata.meeting_id, "19:meeting_x@thread.v2");
    assert_eq!(metadata.meeting_title, "Weekly sync");
    assert_eq!(metadata.start_time_ms, 1_794_000_000_000);
    assert_eq!(metadata.participants[0].external_id, "8:orgid:abc");
    assert_eq!(metadata.participants[0].display_name, "Ada Lovelace");
}

#[test]
fn microphone_audio_is_the_owners_participant_track() {
    let (frame, payload) = converted(json!({
        "type": "audio",
        "track": "microphone",
        "participant": {"id": "8:orgid:abc", "name": "Ada Lovelace"},
        "timestampMs": 4_000,
        "pcm": encoded(&[0_u8; 3_200])
    }));
    assert_eq!(frame.audio_duration_ms(), Some(100));
    let Payload::Audio(audio) = payload else {
        panic!("expected audio");
    };
    assert_eq!(audio.track.enum_value(), Ok(TrackKind::PARTICIPANT));
    assert_eq!(audio.sample_rate, 16_000);
    assert_eq!(audio.timestamp_ms, 4_000);
    assert_eq!(audio.audio_data.len(), 3_200);
    assert_eq!(audio.participant.external_id, "8:orgid:abc");
}

#[test]
fn remote_audio_is_the_mixed_track_with_a_fixed_participant() {
    let (_frame, payload) = converted(json!({
        "type": "audio",
        "track": "remote",
        "timestampMs": 0,
        "pcm": encoded(&[0_u8; 320])
    }));
    let Payload::Audio(audio) = payload else {
        panic!("expected audio");
    };
    assert_eq!(audio.track.enum_value(), Ok(TrackKind::MIXED_REMOTE));
    assert_eq!(audio.participant.external_id, "browser-remote-mix");
}

#[test]
fn a_slide_becomes_a_screen_share_frame() {
    let jpeg = [0xFF_u8, 0xD8, 0xFF, 0xE0];
    let (_frame, payload) = converted(json!({
        "type": "slide", "timestampMs": 61_000, "image": encoded(&jpeg)
    }));
    let Payload::Video(video) = payload else {
        panic!("expected video");
    };
    assert_eq!(video.source, "screen_share");
    assert_eq!(video.image_data, jpeg);
    assert_eq!(video.timestamp_ms, 61_000);
}

#[test]
fn a_document_and_a_meeting_end_are_passed_on() {
    let (_frame, payload) = converted(json!({
        "type": "document",
        "kind": "teams_transcript",
        "mediaType": "text/vtt",
        "text": "WEBVTT\n",
        "meetingId": "19:meeting_x@thread.v2"
    }));
    let Payload::Document(document) = payload else {
        panic!("expected a document");
    };
    assert_eq!(document.kind, "teams_transcript");
    assert_eq!(document.media_type, "text/vtt");
    assert_eq!(document.content, b"WEBVTT\n");
    assert_eq!(document.meeting_id, "19:meeting_x@thread.v2");

    let (_frame, payload) = converted(json!({"type": "ended", "meetingId": "19:m"}));
    let Payload::Ended(ended) = payload else {
        panic!("expected a meeting end");
    };
    assert_eq!(ended.meeting_id, "19:m");
}

#[test]
fn malformed_messages_are_rejected() {
    for (label, message) in [
        ("unknown type", json!({"type": "chat"})),
        (
            "missing fields",
            json!({"type": "audio", "track": "microphone"}),
        ),
        (
            "unknown track",
            json!({"type": "audio", "track": "camera", "timestampMs": 0, "pcm": ""}),
        ),
        (
            "invalid base64",
            json!({"type": "slide", "timestampMs": 0, "image": "@@@"}),
        ),
        (
            "empty audio",
            json!({"type": "audio", "track": "remote", "timestampMs": 0, "pcm": ""}),
        ),
        (
            "microphone without participant",
            json!({"type": "audio", "track": "microphone", "timestampMs": 0, "pcm": encoded(&[0, 0])}),
        ),
    ] {
        assert!(
            matches!(
                to_bridge_frame(message),
                Err(ExtensionMessageError::Invalid { .. })
            ),
            "{label} must be rejected"
        );
    }
}
