//! Specifies the `NTD1` messages that carry deletion records to paired devices.

use uuid::Uuid;

use super::deletion_exchange::{
    DeletionMessageError, MAX_RECORDS_PER_MESSAGE, decode_deletion_message,
    encode_deletion_messages, is_deletion_message,
};
use crate::domain::models::{DeletionRecords, MeetingId};

/// Records survive the trip, and only messages of this kind are recognised.
#[test]
fn records_round_trip() {
    let records = DeletionRecords {
        meeting_ids: vec![MeetingId::new("meeting-1")],
        cluster_ids: vec![Uuid::from_u128(1)],
    };

    let messages = encode_deletion_messages(&records);

    assert_eq!(messages.len(), 1);
    assert!(is_deletion_message(&messages[0]));
    assert!(!is_deletion_message(b"NTS1{}"));
    assert_eq!(decode_deletion_message(&messages[0]), Ok(records));
}

/// A long history is split, and every record arrives.
#[test]
fn many_records_split_across_messages() {
    let records = DeletionRecords {
        meeting_ids: (0..MAX_RECORDS_PER_MESSAGE + 1)
            .map(|index| MeetingId::new(format!("meeting-{index}")))
            .collect(),
        cluster_ids: vec![Uuid::from_u128(1)],
    };

    let messages = encode_deletion_messages(&records);

    assert_eq!(messages.len(), 2);
    let decoded: Vec<DeletionRecords> = messages
        .iter()
        .map(|message| {
            decode_deletion_message(message).unwrap_or_else(|error| panic!("decodes: {error}"))
        })
        .collect();
    let meeting_count: usize = decoded.iter().map(|part| part.meeting_ids.len()).sum();
    let cluster_count: usize = decoded.iter().map(|part| part.cluster_ids.len()).sum();
    assert_eq!(meeting_count, MAX_RECORDS_PER_MESSAGE + 1);
    assert_eq!(cluster_count, 1);
}

/// Peer input is bounded: damaged bodies, too many records, and unusable IDs are refused.
#[test]
fn implausible_messages_are_refused() {
    assert_eq!(
        decode_deletion_message(b"NTD1{broken"),
        Err(DeletionMessageError::Malformed)
    );
    assert_eq!(
        decode_deletion_message(b"NTS1{}"),
        Err(DeletionMessageError::Malformed)
    );
    for meeting_id in [String::new(), "   ".to_owned(), "x".repeat(300)] {
        let message = format!(
            "NTD1{}",
            serde_json::json!({"meeting_ids": [meeting_id], "cluster_ids": []})
        );
        assert_eq!(
            decode_deletion_message(message.as_bytes()),
            Err(DeletionMessageError::Invalid)
        );
    }
    let too_many: Vec<String> = (0..=MAX_RECORDS_PER_MESSAGE)
        .map(|index| format!("meeting-{index}"))
        .collect();
    let message = format!(
        "NTD1{}",
        serde_json::json!({"meeting_ids": too_many, "cluster_ids": []})
    );
    assert_eq!(
        decode_deletion_message(message.as_bytes()),
        Err(DeletionMessageError::Invalid)
    );
}
