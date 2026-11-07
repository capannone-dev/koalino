//! Specifies the sealed messages that carry a meeting's speech to the combining device (#88).

use uuid::Uuid;
use zeroize::Zeroizing;

use super::speech_transfer_exchange::{
    SpeechMessageError, SpeechReceipt, decode_speech_part, decode_speech_receipt,
    encode_speech_part, encode_speech_receipt, is_speech_part, is_speech_receipt, split_into_parts,
};
use crate::{
    domain::{
        models::{DeviceId, MeetingId},
        peer_trust::PairKey,
        transcription_language::TranscriptionLanguage,
    },
    ports::speech_transfer::{SpeechChunk, SpeechPart},
};

fn key(byte: u8) -> PairKey {
    PairKey::from_bytes([byte; 32])
}

fn phone() -> DeviceId {
    DeviceId::new("phone")
}

fn laptop() -> DeviceId {
    DeviceId::new("laptop")
}

fn chunk(start_ms: u64, samples: usize) -> SpeechChunk {
    SpeechChunk {
        start_ms,
        end_ms: start_ms + samples as u64 / 16,
        language: TranscriptionLanguage::parse("it").ok(),
        samples: Zeroizing::new((0..samples).map(|index| index as i16).collect()),
    }
}

fn part(chunks: Vec<SpeechChunk>) -> SpeechPart {
    SpeechPart {
        meeting_id: MeetingId::new("meeting-phone"),
        part_index: 1,
        part_count: 3,
        chunks,
    }
}

#[test]
fn a_part_round_trips_between_the_two_devices() {
    let job = Uuid::from_u128(7);
    let message = encode_speech_part(
        job,
        &part(vec![chunk(0, 16_000), chunk(2_000, 800)]),
        &key(1),
        &phone(),
        &laptop(),
    )
    .unwrap_or_else(|error| panic!("{error}"));

    assert!(is_speech_part(&message));
    let (decoded_job, decoded) = decode_speech_part(&message, &key(1), &phone(), &laptop())
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(decoded_job, job);
    assert_eq!(decoded.meeting_id, MeetingId::new("meeting-phone"));
    assert_eq!((decoded.part_index, decoded.part_count), (1, 3));
    assert_eq!(decoded.chunks.len(), 2);
    assert_eq!(decoded.chunks[1].start_ms, 2_000);
    assert_eq!(decoded.chunks[1].end_ms, 2_050);
    assert_eq!(
        decoded.chunks[0].language.map(|language| language.as_str()),
        Some("it")
    );
    assert_eq!(decoded.chunks[0].samples.len(), 16_000);
    assert_eq!(decoded.chunks[1].samples[799], 799);
}

#[test]
fn a_part_opens_only_with_the_pair_key_in_its_direction() {
    let message = encode_speech_part(
        Uuid::nil(),
        &part(vec![chunk(0, 160)]),
        &key(1),
        &phone(),
        &laptop(),
    )
    .unwrap_or_else(|error| panic!("{error}"));

    assert!(matches!(
        decode_speech_part(&message, &key(2), &phone(), &laptop()),
        Err(SpeechMessageError::Unauthenticated)
    ));
    assert!(matches!(
        decode_speech_part(&message, &key(1), &laptop(), &phone()),
        Err(SpeechMessageError::Unauthenticated)
    ));
}

#[test]
fn a_receipt_round_trips() {
    let receipt = SpeechReceipt {
        job_id: Uuid::from_u128(9),
        stored: false,
    };
    let message = encode_speech_receipt(&receipt, &key(1), &laptop(), &phone())
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(is_speech_receipt(&message));
    assert!(!is_speech_part(&message));
    assert_eq!(
        decode_speech_receipt(&message, &key(1), &laptop(), &phone()).ok(),
        Some(receipt)
    );
}

#[test]
fn chunks_are_split_into_parts_that_fit_a_message() {
    // Ten 15 s chunks (480 kB each) with a 1 MB budget: two chunks per part.
    let chunks: Vec<SpeechChunk> = (0..10)
        .map(|index| chunk(index * 20_000, 240_000))
        .collect();

    let parts = split_into_parts(&MeetingId::new("meeting-phone"), chunks, 1_000_000);

    assert_eq!(parts.len(), 5);
    assert!(
        parts
            .iter()
            .enumerate()
            .all(|(index, part)| part.part_index as usize == index
                && part.part_count == 5
                && part.chunks.len() == 2)
    );
}

#[test]
fn a_meeting_without_speech_is_one_empty_part() {
    let parts = split_into_parts(&MeetingId::new("meeting-phone"), Vec::new(), 1_000_000);

    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].part_count, 1);
    assert!(parts[0].chunks.is_empty());
}

#[test]
fn a_part_with_impossible_values_is_refused() {
    let mut bad = part(vec![chunk(0, 160)]);
    bad.part_index = 3;
    let message = encode_speech_part(Uuid::nil(), &bad, &key(1), &phone(), &laptop())
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(matches!(
        decode_speech_part(&message, &key(1), &phone(), &laptop()),
        Err(SpeechMessageError::Malformed)
    ));
}
