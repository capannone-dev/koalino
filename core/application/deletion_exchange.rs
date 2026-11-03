//! Encodes the `NTD1` messages that carry deletion records to paired devices.
//!
//! A message is `NTD1` followed by JSON listing deleted meeting IDs, unknown-voice cluster IDs and
//! project-resource attachment IDs (Issue #85; absent in messages from older devices, which in turn
//! ignore the field).
//! It holds identifiers only, travels on the encrypted transport like the meeting documents, and
//! comes only from paired devices, because the network adapter drops sync payloads from anyone
//! else. Records only ever grow, and applying one twice changes nothing.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::models::{DeletionRecords, MeetingId};

/// Marks a deletion message.
const DELETION_HEADER: &[u8; 4] = b"NTD1";
/// The most records of each kind in one message. Longer histories are split.
pub const MAX_RECORDS_PER_MESSAGE: usize = 4_096;
/// The longest meeting ID accepted, in bytes.
const MAX_MEETING_ID_BYTES: usize = 256;

/// Why a deletion message could not be used.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DeletionMessageError {
    /// The message or its body cannot be parsed.
    #[error("deletion message is malformed")]
    Malformed,
    /// The message holds too many records or an unusable meeting ID.
    #[error("deletion message holds implausible values")]
    Invalid,
}

/// The JSON body of a message.
#[derive(Serialize, Deserialize)]
struct DeletionBody {
    meeting_ids: Vec<String>,
    cluster_ids: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    attachment_ids: Vec<Uuid>,
}

/// Reports whether `payload` is a deletion message.
///
/// Inputs: A received payload. Outputs: Whether it starts with the header. Side effects: None.
pub fn is_deletion_message(payload: &[u8]) -> bool {
    payload.starts_with(DELETION_HEADER)
}

/// Encodes `records` in as many messages as they need.
///
/// Inputs: The records to send. Outputs: At least one message when any record exists, or none for
/// empty records. Side effects: None.
pub fn encode_deletion_messages(records: &DeletionRecords) -> Vec<Vec<u8>> {
    let meeting_parts = records.meeting_ids.chunks(MAX_RECORDS_PER_MESSAGE);
    let cluster_parts = records.cluster_ids.chunks(MAX_RECORDS_PER_MESSAGE);
    let attachment_parts = records.attachment_ids.chunks(MAX_RECORDS_PER_MESSAGE);
    let part_count = meeting_parts
        .len()
        .max(cluster_parts.len())
        .max(attachment_parts.len());
    let mut meeting_parts = meeting_parts;
    let mut cluster_parts = cluster_parts;
    let mut attachment_parts = attachment_parts;
    (0..part_count)
        .filter_map(|_| {
            let body = DeletionBody {
                meeting_ids: meeting_parts
                    .next()
                    .unwrap_or_default()
                    .iter()
                    .map(|meeting_id| meeting_id.as_str().to_owned())
                    .collect(),
                cluster_ids: cluster_parts.next().unwrap_or_default().to_vec(),
                attachment_ids: attachment_parts.next().unwrap_or_default().to_vec(),
            };
            let encoded_body = serde_json::to_vec(&body).ok()?;
            let mut message = Vec::with_capacity(DELETION_HEADER.len() + encoded_body.len());
            message.extend_from_slice(DELETION_HEADER);
            message.extend_from_slice(&encoded_body);
            Some(message)
        })
        .collect()
}

/// Reads and bounds a message from a paired device.
///
/// Inputs: A payload for which `is_deletion_message` holds.
/// Outputs: The records it carries, or why it was refused. Side effects: None.
pub fn decode_deletion_message(payload: &[u8]) -> Result<DeletionRecords, DeletionMessageError> {
    let encoded_body = payload
        .strip_prefix(DELETION_HEADER.as_slice())
        .ok_or(DeletionMessageError::Malformed)?;
    let body: DeletionBody =
        serde_json::from_slice(encoded_body).map_err(|_| DeletionMessageError::Malformed)?;
    let usable_id = |meeting_id: &String| {
        !meeting_id.trim().is_empty() && meeting_id.len() <= MAX_MEETING_ID_BYTES
    };
    if body.meeting_ids.len() > MAX_RECORDS_PER_MESSAGE
        || body.cluster_ids.len() > MAX_RECORDS_PER_MESSAGE
        || body.attachment_ids.len() > MAX_RECORDS_PER_MESSAGE
        || !body.meeting_ids.iter().all(usable_id)
    {
        return Err(DeletionMessageError::Invalid);
    }
    Ok(DeletionRecords {
        meeting_ids: body.meeting_ids.into_iter().map(MeetingId::new).collect(),
        cluster_ids: body.cluster_ids,
        attachment_ids: body.attachment_ids,
    })
}
