//! Specifies the sealed `NTO1` messages that carry the owner's prompts to paired devices (#145).

use super::shared_prompts_exchange::{
    SharedPromptsMessageError, decode_shared_prompts_message, encode_shared_prompts_message,
    is_shared_prompts_message,
};
use crate::domain::{
    models::DeviceId,
    peer_trust::PairKey,
    shared_prompts::{PromptVersion, SharedPrompts},
};

fn key(byte: u8) -> PairKey {
    PairKey::from_bytes([byte; 32])
}

fn laptop() -> DeviceId {
    DeviceId::new("laptop")
}

fn phone() -> DeviceId {
    DeviceId::new("phone")
}

fn prompts(summary: &str, chat: &str) -> SharedPrompts {
    SharedPrompts {
        summary: PromptVersion {
            text: Some(summary.to_owned()),
            changed_at_ms: 1_800_000_000_000,
        },
        chat: PromptVersion {
            text: Some(chat.to_owned()),
            changed_at_ms: 1_800_000_000_500,
        },
    }
}

#[test]
fn prompts_round_trip_and_stay_hidden() {
    let sent = prompts(
        "Only the decisions of {{title}}.",
        "Be brief about {{scope}}.",
    );
    let message = encode_shared_prompts_message(&sent, &key(1), &laptop(), &phone())
        .unwrap_or_else(|error| panic!("{error}"));

    assert!(is_shared_prompts_message(&message));
    assert!(!message.windows(9).any(|window| window == b"decisions"));
    let received = decode_shared_prompts_message(&message, &key(1), &laptop(), &phone())
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(received, sent);
}

#[test]
fn a_message_for_another_pair_or_direction_is_refused() {
    let message = encode_shared_prompts_message(&prompts("A", "B"), &key(1), &laptop(), &phone())
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        decode_shared_prompts_message(&message, &key(2), &laptop(), &phone()),
        Err(SharedPromptsMessageError::Unauthenticated)
    );
    assert_eq!(
        decode_shared_prompts_message(&message, &key(1), &phone(), &laptop()),
        Err(SharedPromptsMessageError::Unauthenticated)
    );
}

#[test]
fn a_prompt_this_device_could_not_use_is_refused() {
    for bad in [
        prompts("Summary of {{nonsense}}", "Chat"),
        prompts("Summary", "Chat about {{title}}"),
        prompts("   ", "Chat"),
    ] {
        let message = encode_shared_prompts_message(&bad, &key(1), &laptop(), &phone())
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            decode_shared_prompts_message(&message, &key(1), &laptop(), &phone()),
            Err(SharedPromptsMessageError::Invalid)
        );
    }
}

#[test]
fn built_in_prompts_travel_as_none() {
    let sent = SharedPrompts {
        summary: PromptVersion {
            text: None,
            changed_at_ms: 7,
        },
        chat: PromptVersion::default(),
    };
    let message = encode_shared_prompts_message(&sent, &key(1), &laptop(), &phone())
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        decode_shared_prompts_message(&message, &key(1), &laptop(), &phone()),
        Ok(sent)
    );
}

/// The peer messages share one stream, so a reused header hands a message to the wrong reader:
/// `NTS1`, first chosen here, is the meeting sync envelope, and taking those for prompts stopped
/// every meeting from syncing.
#[test]
fn prompts_messages_are_never_taken_for_other_peer_messages() {
    use crate::application::{
        attachment_exchange::is_attachment_manifest,
        cluster_share_exchange::is_cluster_share_message,
        deletion_exchange::is_deletion_message,
        paired_chat_exchange::{is_paired_chat_request, is_paired_chat_response},
        project_library_exchange::is_project_library_message,
        speech_transfer_exchange::{is_speech_part, is_speech_receipt},
        transcription_exchange::{is_transcription_request, is_transcription_response},
        voice_library_exchange::is_voice_library_message,
    };
    let others: [fn(&[u8]) -> bool; 11] = [
        is_attachment_manifest,
        is_cluster_share_message,
        is_deletion_message,
        is_paired_chat_request,
        is_paired_chat_response,
        is_project_library_message,
        is_speech_part,
        is_speech_receipt,
        is_transcription_request,
        is_transcription_response,
        is_voice_library_message,
    ];
    let message = encode_shared_prompts_message(&prompts("A", "B"), &key(1), &laptop(), &phone())
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(others.iter().all(|is_other| !is_other(&message)));
    for taken in [b"NTS1", b"NTM1", b"NTK1"] {
        assert!(!message.starts_with(taken));
        assert!(!is_shared_prompts_message(taken));
    }
}
