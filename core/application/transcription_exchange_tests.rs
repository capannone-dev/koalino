//! Specifies the `NTT1`/`NTT2` messages: they survive a round trip, open only with the pair key in
//! the right direction, hide their contents, and refuse a chunk that does not fit.

use uuid::Uuid;

use super::transcription_exchange::{
    TranscriptionMessageError, TranscriptionRequest, TranscriptionResponse,
    decode_transcription_request, decode_transcription_response, encode_transcription_request,
    encode_transcription_response, is_transcription_request, is_transcription_response,
};
use crate::domain::{models::DeviceId, peer_trust::PairKey};

/// A pair key.
fn key() -> PairKey {
    PairKey::from_bytes([6; 32])
}

/// The requester.
fn phone() -> DeviceId {
    DeviceId::new("phone")
}

/// The host.
fn laptop() -> DeviceId {
    DeviceId::new("laptop")
}

/// A small request.
fn request(previous_text: Option<&str>) -> TranscriptionRequest {
    TranscriptionRequest {
        job_id: Uuid::from_u128(7),
        previous_text: previous_text.map(str::to_owned),
        samples: vec![1, -2, 3_000, i16::MIN],
    }
}

/// Seals `request` from the phone to the laptop.
fn sealed(request: &TranscriptionRequest) -> Vec<u8> {
    encode_transcription_request(request, &key(), &phone(), &laptop())
        .unwrap_or_else(|error| panic!("the request must seal: {error}"))
}

#[test]
fn requests_survive_the_round_trip_with_or_without_context() {
    for previous_text in [Some("the last words"), None] {
        let message = sealed(&request(previous_text));

        assert!(is_transcription_request(&message));
        assert!(!is_transcription_response(&message));
        assert_eq!(
            decode_transcription_request(&message, &key(), &phone(), &laptop()),
            Ok(request(previous_text))
        );
    }
}

#[test]
fn responses_survive_the_round_trip_including_a_failure_and_empty_text() {
    for text in [Some("said aloud".to_owned()), Some(String::new()), None] {
        let response = TranscriptionResponse {
            job_id: Uuid::from_u128(8),
            text,
        };
        let message = encode_transcription_response(&response, &key(), &laptop(), &phone())
            .unwrap_or_else(|error| panic!("the response must seal: {error}"));

        assert!(is_transcription_response(&message));
        assert_eq!(
            decode_transcription_response(&message, &key(), &laptop(), &phone()),
            Ok(response)
        );
    }
}

#[test]
fn messages_open_only_with_the_key_and_direction_they_were_sealed_for() {
    let message = sealed(&request(None));

    for (pair_key, from, to) in [
        (PairKey::from_bytes([1; 32]), phone(), laptop()),
        (key(), laptop(), phone()),
    ] {
        assert_eq!(
            decode_transcription_request(&message, &pair_key, &from, &to),
            Err(TranscriptionMessageError::Unauthenticated)
        );
    }
}

#[test]
fn context_is_not_readable_in_the_message() {
    let message = sealed(&request(Some("confidential")));

    assert!(!message.windows(12).any(|window| window == b"confidential"));
}

#[test]
fn a_chunk_that_does_not_fit_one_message_is_refused() {
    let too_long = TranscriptionRequest {
        job_id: Uuid::from_u128(1),
        previous_text: None,
        samples: vec![0; 8 * 1024 * 1024],
    };

    assert_eq!(
        encode_transcription_request(&too_long, &key(), &phone(), &laptop()),
        Err(TranscriptionMessageError::TooLarge)
    );
}
