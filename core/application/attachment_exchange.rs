//! Encodes the `NTA1` messages that tell a paired device which attachments exist (Issue #57).
//!
//! A message is `NTA1` followed by a JSON list sealed with the pair key (`seal_for_peer`, purpose
//! `NTA1`, bound to the sender and receiver). A long list is split so each message fits the
//! transport, and each message can be applied alone. File names and meeting IDs are private, so
//! they travel only inside the sealed value and are never logged. The sender holds the pair key
//! but is still peer input, so `decode_attachment_manifest` bounds every value.
//!
//! Project resources (Issue #85) carry a `project_id` and an empty `meeting_id`, and always travel
//! in messages of their own: a device from before projects refuses an entry with an empty meeting
//! ID, which drops its whole message, so keeping them apart means it still accepts every meeting
//! attachment.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    application::{
        crypto::pair_key::{open_from_peer, seal_for_peer},
        voice_library_exchange::rfc3339,
    },
    domain::{
        models::{AttachmentOwner, DeviceId, MeetingId, SharedAttachment},
        peer_trust::PairKey,
        projects::ProjectId,
    },
    ports::file_transfer::MAX_FILE_TRANSFER_BYTES,
};

/// Marks an attachment manifest.
const ATTACHMENT_MANIFEST_HEADER: &[u8; 4] = b"NTA1";
/// How many attachments one message lists. Each takes well under 1 KiB of JSON, so a message
/// stays far inside the transport's 16 MiB limit.
pub const MAX_ATTACHMENTS_PER_MESSAGE: usize = 4_096;
/// The longest text value accepted, in bytes.
const MAX_TEXT_BYTES: usize = 512;

/// Why a manifest could not be built or used.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AttachmentManifestError {
    /// The message or its body cannot be parsed.
    #[error("attachment manifest is malformed")]
    Malformed,
    /// The message does not open with the pair key in this direction, or was changed.
    #[error("attachment manifest failed authentication")]
    Unauthenticated,
    /// The manifest holds implausible values (bad hash, oversized file or text, too many entries).
    #[error("attachment manifest holds implausible values")]
    Invalid,
    /// Encryption failed.
    #[error("attachment manifest could not be encrypted")]
    EncryptionFailed,
}

/// One attachment in the body.
#[derive(Serialize, Deserialize)]
struct AttachmentBody {
    attachment_id: Uuid,
    /// The meeting, or empty for a project resource.
    meeting_id: String,
    /// The project of a resource; absent for a meeting attachment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    project_id: Option<Uuid>,
    file_name: String,
    mime_type: String,
    content_hash: String,
    size_bytes: u64,
    #[serde(with = "rfc3339")]
    original_timestamp: DateTime<Utc>,
    original_device_id: String,
    meeting_time_ms: Option<u64>,
}

/// Reports whether `payload` is an attachment manifest.
///
/// Inputs: A received payload. Outputs: Whether it starts with the header. Side effects: None.
pub fn is_attachment_manifest(payload: &[u8]) -> bool {
    payload.starts_with(ATTACHMENT_MANIFEST_HEADER)
}

/// Seals `attachments` for `to`, split into messages that each fit the transport.
///
/// Meeting attachments come first; project resources follow in messages of their own.
///
/// Inputs: The list, the pair key, and the two device IDs the messages are bound to.
/// Outputs: The messages (none for an empty list), or an error. Side effects: None.
pub fn encode_attachment_manifests(
    attachments: &[SharedAttachment],
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<Vec<Vec<u8>>, AttachmentManifestError> {
    let (meeting_attachments, project_resources): (Vec<&SharedAttachment>, Vec<&SharedAttachment>) =
        attachments
            .iter()
            .partition(|attachment| matches!(attachment.owner, AttachmentOwner::Meeting(_)));
    meeting_attachments
        .chunks(MAX_ATTACHMENTS_PER_MESSAGE)
        .chain(project_resources.chunks(MAX_ATTACHMENTS_PER_MESSAGE))
        .map(|part| {
            let body: Vec<AttachmentBody> = part.iter().copied().map(to_body).collect();
            let plaintext =
                serde_json::to_vec(&body).map_err(|_| AttachmentManifestError::Malformed)?;
            let sealed = seal_for_peer(pair_key, ATTACHMENT_MANIFEST_HEADER, from, to, &plaintext)
                .map_err(|_| AttachmentManifestError::EncryptionFailed)?;
            let mut message = ATTACHMENT_MANIFEST_HEADER.to_vec();
            message.extend_from_slice(&sealed);
            Ok(message)
        })
        .collect()
}

/// Opens and validates a manifest.
///
/// Inputs: The received message, the pair key, and the sender and this device's IDs.
/// Outputs: The attachments, or why the message was refused. Side effects: None.
pub fn decode_attachment_manifest(
    message: &[u8],
    pair_key: &PairKey,
    from: &DeviceId,
    to: &DeviceId,
) -> Result<Vec<SharedAttachment>, AttachmentManifestError> {
    let sealed = message
        .strip_prefix(ATTACHMENT_MANIFEST_HEADER)
        .ok_or(AttachmentManifestError::Malformed)?;
    let plaintext = open_from_peer(pair_key, ATTACHMENT_MANIFEST_HEADER, from, to, sealed)
        .map_err(|_| AttachmentManifestError::Unauthenticated)?;
    let body: Vec<AttachmentBody> =
        serde_json::from_slice(&plaintext).map_err(|_| AttachmentManifestError::Malformed)?;
    if body.len() > MAX_ATTACHMENTS_PER_MESSAGE {
        return Err(AttachmentManifestError::Invalid);
    }
    body.into_iter().map(from_body).collect()
}

/// Reports whether `hash` is a lowercase hexadecimal SHA-256.
///
/// Inputs: Peer-supplied text. Outputs: Whether it has the right form. Side effects: None.
pub fn is_content_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// Copies an attachment into its wire form.
fn to_body(attachment: &SharedAttachment) -> AttachmentBody {
    let (meeting_id, project_id) = match &attachment.owner {
        AttachmentOwner::Meeting(meeting_id) => (meeting_id.as_str().to_owned(), None),
        AttachmentOwner::Project(project_id) => (String::new(), Some(project_id.as_uuid())),
    };
    AttachmentBody {
        attachment_id: attachment.attachment_id,
        meeting_id,
        project_id,
        file_name: attachment.file_name.clone(),
        mime_type: attachment.mime_type.clone(),
        content_hash: attachment.content_hash.clone(),
        size_bytes: attachment.size_bytes,
        original_timestamp: attachment.original_timestamp,
        original_device_id: attachment.original_device_id.clone(),
        meeting_time_ms: attachment.meeting_time_ms,
    }
}

/// Validates one wire entry.
///
/// Outputs: The attachment, or `Invalid` for values outside the limits or an entry that names both
/// or neither of a meeting and a project. Side effects: None.
fn from_body(body: AttachmentBody) -> Result<SharedAttachment, AttachmentManifestError> {
    let owner = match body.project_id {
        Some(project_id) if body.meeting_id.is_empty() => {
            AttachmentOwner::Project(ProjectId::from_uuid(project_id))
        }
        Some(_) => return Err(AttachmentManifestError::Invalid),
        None if body.meeting_id.is_empty() || body.meeting_id.len() > MAX_TEXT_BYTES => {
            return Err(AttachmentManifestError::Invalid);
        }
        None => AttachmentOwner::Meeting(MeetingId::new(body.meeting_id)),
    };
    let texts = [&body.file_name, &body.mime_type, &body.original_device_id];
    if !is_content_hash(&body.content_hash)
        || body.size_bytes > MAX_FILE_TRANSFER_BYTES
        || texts
            .iter()
            .any(|text| text.is_empty() || text.len() > MAX_TEXT_BYTES)
    {
        return Err(AttachmentManifestError::Invalid);
    }
    Ok(SharedAttachment {
        attachment_id: body.attachment_id,
        owner,
        file_name: body.file_name,
        mime_type: body.mime_type,
        content_hash: body.content_hash,
        size_bytes: body.size_bytes,
        original_timestamp: body.original_timestamp,
        original_device_id: body.original_device_id,
        meeting_time_ms: body.meeting_time_ms,
    })
}
