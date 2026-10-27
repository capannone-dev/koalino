//! Specifies the bounded, drop-oldest buffer between the WebSocket reader and the meeting pump.

use std::time::Duration;

use uuid::Uuid;

use crate::{
    adapters::network::teams_bridge::{
        bridge_buffer::{BridgeBuffer, MAXIMUM_BUFFERED_SLIDES},
        log_capture::capture_logs,
        message_decoder::{BridgeEvent, decode_frame, local_meeting_uuid},
    },
    domain::models::SlideCapture,
    protocol::teams_bridge::{AudioChunk, ExternalParticipant, TeamsBridgeMessage},
};

/// A short wait for tests that expect an event to already be queued.
const QUEUED_EVENT_WAIT: Duration = Duration::from_millis(50);

/// Builds a decoded audio event of `duration_ms` (16 kHz s16le) stamped `timestamp_ms`.
fn audio_event(duration_ms: u64, timestamp_ms: u64) -> BridgeEvent {
    let mut participant = ExternalParticipant::new();
    participant.external_id = "teams-user-1".to_owned();
    participant.display_name = "Ada Lovelace".to_owned();
    let mut audio = AudioChunk::new();
    audio.participant = protobuf::MessageField::some(participant);
    audio.audio_data = vec![0; usize::try_from(duration_ms * 32).unwrap_or(0)];
    audio.timestamp_ms = timestamp_ms;
    audio.sample_rate = 16_000;
    let mut message = TeamsBridgeMessage::new();
    message.set_audio(audio);
    let frame = protobuf::Message::write_to_bytes(&message)
        .unwrap_or_else(|error| panic!("message should encode: {error}"));
    decode_frame(&frame, Some(local_meeting_uuid("meeting")))
        .unwrap_or_else(|error| panic!("frame should decode: {error}"))
}

/// Builds a slide event stamped `timestamp_ms`.
fn slide_event(timestamp_ms: u64) -> BridgeEvent {
    BridgeEvent::Slide(SlideCapture::new(
        Uuid::nil(),
        vec![0x89],
        timestamp_ms,
        "screen_share".to_owned(),
    ))
}

/// Returns the timestamp of a queued audio or slide event.
fn timestamp_of(event: &BridgeEvent) -> u64 {
    match event {
        BridgeEvent::Audio(audio) => audio.media_chunk().timestamp_ms(),
        BridgeEvent::Slide(slide) => slide.timestamp_ms(),
        other => panic!("unexpected event {other:?}"),
    }
}

/// Drains every queued event without waiting.
fn drain(buffer: &BridgeBuffer) -> Vec<BridgeEvent> {
    std::iter::from_fn(|| buffer.next_event(Duration::ZERO)).collect()
}

/// Verifies events come out in arrival order and the buffered duration is tracked.
#[test]
fn events_are_delivered_in_arrival_order() {
    let buffer = BridgeBuffer::new(Duration::from_secs(10));
    buffer.push(audio_event(1_000, 0));
    buffer.push(slide_event(500));
    buffer.push(audio_event(2_000, 1_000));
    assert_eq!(buffer.buffered_audio_ms(), 3_000);

    let timestamps: Vec<u64> = drain(&buffer).iter().map(timestamp_of).collect();
    assert_eq!(timestamps, vec![0, 500, 1_000]);
    assert_eq!(buffer.buffered_audio_ms(), 0);
}

/// Verifies overflow drops the oldest audio, keeps newer audio, and logs a warning.
#[test]
fn audio_overflow_drops_the_oldest_chunks_and_logs_a_warning() {
    let captured_logs = capture_logs();
    let buffer = BridgeBuffer::new(Duration::from_secs(10));
    for chunk_index in 0..3 {
        let outcome = buffer.push(audio_event(3_000, chunk_index * 3_000));
        assert_eq!(outcome.dropped_audio_chunks, 0);
    }

    let outcome = buffer.push(audio_event(6_000, 9_000));

    assert_eq!(outcome.dropped_audio_chunks, 2);
    assert_eq!(buffer.buffered_audio_ms(), 9_000);
    let timestamps: Vec<u64> = drain(&buffer).iter().map(timestamp_of).collect();
    assert_eq!(timestamps, vec![6_000, 9_000]);
    let logs = captured_logs.text();
    assert!(logs.contains("WARN"), "expected a warning, got: {logs}");
    assert!(
        logs.contains("dropped"),
        "expected a drop warning, got: {logs}"
    );
}

/// Verifies a single chunk longer than the whole buffer is still kept (newest data wins).
#[test]
fn a_chunk_longer_than_the_buffer_replaces_everything_older() {
    let buffer = BridgeBuffer::new(Duration::from_secs(1));
    buffer.push(audio_event(500, 0));
    let outcome = buffer.push(audio_event(2_000, 500));
    assert_eq!(outcome.dropped_audio_chunks, 1);
    let timestamps: Vec<u64> = drain(&buffer).iter().map(timestamp_of).collect();
    assert_eq!(timestamps, vec![500]);
}

/// Verifies meeting starts are never dropped by audio overflow.
#[test]
fn meeting_starts_survive_audio_overflow() {
    let buffer = BridgeBuffer::new(Duration::from_secs(1));
    buffer.push(BridgeEvent::IgnoredVideo);
    let metadata = {
        let mut metadata = crate::protocol::teams_bridge::MeetingMetadata::new();
        metadata.meeting_id = "meeting".to_owned();
        let mut message = TeamsBridgeMessage::new();
        message.set_metadata(metadata);
        let frame = protobuf::Message::write_to_bytes(&message)
            .unwrap_or_else(|error| panic!("message should encode: {error}"));
        decode_frame(&frame, None).unwrap_or_else(|error| panic!("should decode: {error}"))
    };
    buffer.push(metadata);
    buffer.push(audio_event(800, 0));
    buffer.push(audio_event(800, 800));

    let events = drain(&buffer);
    assert!(matches!(events[0], BridgeEvent::MeetingStarted(_)));
    assert_eq!(events.len(), 2, "camera frames are not queued: {events:?}");
    assert_eq!(timestamp_of(&events[1]), 800);
}

/// Verifies slides have their own cap and drop the oldest slide.
#[test]
fn slide_overflow_drops_the_oldest_slide() {
    let buffer = BridgeBuffer::new(Duration::from_secs(10));
    let slide_cap = u64::try_from(MAXIMUM_BUFFERED_SLIDES).unwrap_or(u64::MAX);
    for slide_index in 0..=slide_cap {
        buffer.push(slide_event(slide_index));
    }
    let timestamps: Vec<u64> = drain(&buffer).iter().map(timestamp_of).collect();
    assert_eq!(timestamps.len(), MAXIMUM_BUFFERED_SLIDES);
    assert_eq!(timestamps.first(), Some(&1));
}

/// Verifies the consumer times out on an empty buffer and wakes up when an event arrives.
#[test]
fn next_event_waits_for_a_producer() {
    let buffer = BridgeBuffer::new(Duration::from_secs(10));
    assert!(buffer.next_event(QUEUED_EVENT_WAIT).is_none());

    let producer = buffer.clone();
    let producer_thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        producer.push(audio_event(100, 42));
    });
    let event = buffer.next_event(Duration::from_secs(5));
    producer_thread
        .join()
        .unwrap_or_else(|_| panic!("producer should not panic"));
    assert_eq!(event.as_ref().map(timestamp_of), Some(42));
}

/// Verifies closing wakes a waiting consumer and rejects later pushes.
#[test]
fn close_wakes_the_consumer_and_stops_accepting_events() {
    let buffer = BridgeBuffer::new(Duration::from_secs(10));
    let closer = buffer.clone();
    let closer_thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        closer.close();
    });
    assert!(buffer.next_event(Duration::from_secs(5)).is_none());
    closer_thread
        .join()
        .unwrap_or_else(|_| panic!("closer should not panic"));
    assert!(buffer.is_closed());
    buffer.push(audio_event(100, 0));
    assert_eq!(buffer.buffered_audio_ms(), 0);
}
