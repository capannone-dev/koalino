//! Specifies the sealed messages that ask a paired computer to answer a chat (#107).

use uuid::Uuid;

use super::paired_chat_exchange::{
    MAX_HISTORY_TURNS, PairedChatAnswer, PairedChatError, PairedChatOutcome, PairedChatRequest,
    PairedChatResponse, PairedChatTurn, decode_paired_chat_request, decode_paired_chat_response,
    encode_paired_chat_request, encode_paired_chat_response, is_paired_chat_request,
    is_paired_chat_response,
};
use crate::domain::{
    chat::{ChatRole, ChatScope, ChatSource, ChatTarget},
    models::{DeviceId, MeetingId},
    peer_trust::PairKey,
};

fn key(byte: u8) -> PairKey {
    PairKey::from_bytes([byte; 32])
}

fn phone() -> DeviceId {
    DeviceId::new("12D3KooWPhone")
}

fn laptop() -> DeviceId {
    DeviceId::new("12D3KooWLaptop")
}

fn request() -> PairedChatRequest {
    PairedChatRequest {
        job_id: Uuid::from_u128(7),
        scope: ChatScope::Meeting(MeetingId::new("meeting-1")),
        question: "What did we decide?".to_owned(),
        history: vec![
            PairedChatTurn {
                role: ChatRole::User,
                text: "Who came?".to_owned(),
            },
            PairedChatTurn {
                role: ChatRole::Assistant,
                text: "Anna and Luca [1].".to_owned(),
            },
        ],
    }
}

fn answer() -> PairedChatAnswer {
    PairedChatAnswer {
        text: "To ship on Friday [1].".to_owned(),
        included: vec!["transcript".to_owned()],
        omitted: vec!["photos".to_owned()],
        sources: vec![ChatSource {
            number: 1,
            label: "Budget review".to_owned(),
            target: Some(ChatTarget::Meeting {
                meeting_id: "meeting-1".to_owned(),
            }),
            at_ms: Some(61_000),
        }],
        connection_label: "Claude Code on Laptop".to_owned(),
        truncated: false,
        images_sent: 0,
    }
}

#[test]
fn a_request_opens_only_with_the_pair_key_in_its_direction() {
    let message = encode_paired_chat_request(&request(), &key(1), &phone(), &laptop())
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(is_paired_chat_request(&message));
    assert!(!is_paired_chat_response(&message));
    let opened = decode_paired_chat_request(&message, &key(1), &phone(), &laptop());
    assert_eq!(opened, Ok(request()));
    assert_eq!(
        decode_paired_chat_request(&message, &key(2), &phone(), &laptop()),
        Err(PairedChatError::Unauthenticated)
    );
    assert_eq!(
        decode_paired_chat_request(&message, &key(1), &laptop(), &phone()),
        Err(PairedChatError::Unauthenticated)
    );
}

#[test]
fn the_question_is_not_readable_on_the_wire() {
    let message = encode_paired_chat_request(&request(), &key(1), &phone(), &laptop())
        .unwrap_or_else(|error| panic!("{error}"));
    let needle = b"What did we decide";
    assert!(!message.windows(needle.len()).any(|window| window == needle));
}

#[test]
fn every_scope_round_trips() {
    for scope in [
        ChatScope::All,
        ChatScope::Meeting(MeetingId::new("m-2")),
        ChatScope::parse("project", Some("6f1c1c62-0f43-4c1e-9c64-2b1f2b3c4d5e"))
            .unwrap_or(ChatScope::All),
    ] {
        let sent = PairedChatRequest {
            scope: scope.clone(),
            ..request()
        };
        let message = encode_paired_chat_request(&sent, &key(1), &phone(), &laptop())
            .unwrap_or_else(|error| panic!("{error}"));
        let opened = decode_paired_chat_request(&message, &key(1), &phone(), &laptop());
        assert_eq!(opened.map(|request| request.scope), Ok(scope));
    }
}

#[test]
fn a_long_history_keeps_only_its_latest_turns() {
    let turns: Vec<PairedChatTurn> = (0..MAX_HISTORY_TURNS + 5)
        .map(|index| PairedChatTurn {
            role: ChatRole::User,
            text: format!("turn {index}"),
        })
        .collect();
    let sent = PairedChatRequest {
        history: turns,
        ..request()
    };
    let message = encode_paired_chat_request(&sent, &key(1), &phone(), &laptop())
        .unwrap_or_else(|error| panic!("{error}"));
    let opened = decode_paired_chat_request(&message, &key(1), &phone(), &laptop())
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(opened.history.len(), MAX_HISTORY_TURNS);
    assert_eq!(
        opened.history.last().map(|turn| turn.text.as_str()),
        Some(format!("turn {}", MAX_HISTORY_TURNS + 4).as_str())
    );
}

#[test]
fn a_blank_or_overlong_question_is_refused() {
    for question in [String::from("  "), "x".repeat(4_001)] {
        let sent = PairedChatRequest {
            question,
            ..request()
        };
        assert!(encode_paired_chat_request(&sent, &key(1), &phone(), &laptop()).is_err());
    }
}

#[test]
fn every_outcome_round_trips() {
    for outcome in [
        PairedChatOutcome::Accepted,
        PairedChatOutcome::Answered(answer()),
        PairedChatOutcome::Declined {
            error_code: "not_offered".to_owned(),
        },
    ] {
        let response = PairedChatResponse {
            job_id: Uuid::from_u128(9),
            outcome,
        };
        let message = encode_paired_chat_response(&response, &key(1), &laptop(), &phone())
            .unwrap_or_else(|error| panic!("{error}"));
        assert!(is_paired_chat_response(&message));
        assert_eq!(
            decode_paired_chat_response(&message, &key(1), &laptop(), &phone()),
            Ok(response)
        );
    }
}

#[test]
fn an_implausible_error_code_is_malformed() {
    let response = PairedChatResponse {
        job_id: Uuid::from_u128(9),
        outcome: PairedChatOutcome::Declined {
            error_code: "Not offered!".to_owned(),
        },
    };
    let message = encode_paired_chat_response(&response, &key(1), &laptop(), &phone())
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        decode_paired_chat_response(&message, &key(1), &laptop(), &phone()),
        Err(PairedChatError::Malformed)
    );
}

#[test]
fn a_cut_message_is_refused() {
    let message = encode_paired_chat_request(&request(), &key(1), &phone(), &laptop())
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(
        decode_paired_chat_request(&message[..message.len() - 3], &key(1), &phone(), &laptop())
            .is_err()
    );
    assert_eq!(
        decode_paired_chat_request(b"NTQ1", &key(1), &phone(), &laptop()),
        Err(PairedChatError::Unauthenticated)
    );
}

#[test]
fn chat_messages_are_never_taken_for_other_peer_messages() {
    // The headers share one stream, so a reused one would hand a message to the wrong reader
    // (`NTC1` was already the cluster shares').
    use crate::application::{
        attachment_exchange::is_attachment_manifest,
        cluster_share_exchange::is_cluster_share_message,
        deletion_exchange::is_deletion_message,
        project_library_exchange::is_project_library_message,
        speech_transfer_exchange::{is_speech_part, is_speech_receipt},
        transcription_exchange::{is_transcription_request, is_transcription_response},
        voice_library_exchange::is_voice_library_message,
    };
    let others: [fn(&[u8]) -> bool; 9] = [
        is_attachment_manifest,
        is_cluster_share_message,
        is_deletion_message,
        is_project_library_message,
        is_speech_part,
        is_speech_receipt,
        is_transcription_request,
        is_transcription_response,
        is_voice_library_message,
    ];
    let question = encode_paired_chat_request(&request(), &key(1), &phone(), &laptop())
        .unwrap_or_else(|error| panic!("{error}"));
    let reply = encode_paired_chat_response(
        &PairedChatResponse {
            job_id: Uuid::from_u128(1),
            outcome: PairedChatOutcome::Accepted,
        },
        &key(1),
        &laptop(),
        &phone(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    for message in [&question, &reply] {
        assert!(others.iter().all(|is_other| !is_other(message)));
        assert!(
            !message.starts_with(b"NTM1")
                && !message.starts_with(b"NTK1")
                && !message.starts_with(b"NTS1")
        );
    }
}
