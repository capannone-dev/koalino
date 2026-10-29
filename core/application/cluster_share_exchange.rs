//! Encodes the `NTC1` messages that copy an unresolved unknown-speaker cluster to a paired device
//! (Issue #50), so its speakers can be named there.
//!
//! A message is `NTC1` followed by a value sealed with the pair key (`seal_for_peer`, purpose
//! `NTC1`, bound to the sender and receiver). Inside:
//!
//! ```text
//! JSON length (u32, big-endian) ‖ JSON body ‖ clip audio, concatenated in clip order
//! ```
//!
//! The JSON body holds the cluster (meeting ID, cluster ID, creation time, total duration,
//! representative embedding), every member chunk with its embedding, and the clips' metadata
//! with each clip's audio length. Raw audio rides outside the JSON so it is not inflated.
//!
//! Clips are split across messages so each carries at most the caller's audio budget; every
//! message repeats the cluster and its chunks, so the receiver can import each one alone. A clip
//! larger than the whole budget is left out (and logged without content).
//!
//! The sender holds the pair key but is still peer input, so the body is bounded and checked:
//! finite embeddings of bounded size, bounded chunk and clip counts, short IDs without control
//! characters, and clip spans that end after they start. Nothing here logs payload contents.

use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    application::crypto::pair_key::{open_from_peer, seal_for_peer},
    domain::{
        errors::CryptoError,
        models::{
            ClusterShare, DeviceId, MeetingId, UnassignedVoiceClip, UnknownSpeakerChunk,
            UnknownSpeakerCluster, WeakVoiceMatchCandidate,
        },
        peer_trust::PairKey,
    },
};

/// First four bytes of a cluster-share message; also the sealing purpose.
const CLUSTER_SHARE_HEADER: &[u8; 4] = b"NTC1";
/// Largest message accepted, matching the transport's payload limit.
const MAX_CLUSTER_SHARE_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
/// Most dimensions accepted in one embedding.
const MAX_EMBEDDING_DIMENSIONS: usize = 4_096;
/// Most member chunks accepted in one cluster.
const MAX_CHUNKS: usize = 4_096;
/// Most clips one message may carry.
const MAX_CLIPS_PER_MESSAGE: usize = 256;
/// Longest accepted meeting or chunk ID, in characters.
const MAX_ID_CHARACTERS: usize = 128;

/// Why a cluster-share message could not be built or used.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ClusterShareMessageError {
    /// The message exceeds the transport's payload limit.
    #[error("cluster-share message is too large")]
    TooLarge,
    /// The message or its body cannot be parsed.
    #[error("cluster-share message is malformed")]
    Malformed,
    /// The message does not open with the pair key in this direction, or was changed.
    #[error("cluster-share message failed authentication")]
    Unauthenticated,
    /// The share holds implausible values (non-finite or oversized embeddings, too many members,
    /// unusable IDs, or clips that end before they start).
    #[error("cluster share holds implausible values")]
    Invalid,
    /// Encryption failed.
    #[error("cluster share could not be encrypted")]
    EncryptionFailed,
}

/// The JSON body of a message.
#[derive(Serialize, Deserialize)]
struct ShareBody {
    meeting_id: String,
    cluster_id: String,
    created_at_ms: i64,
    total_duration_ms: u64,
    representative_embedding: Vec<f32>,
    chunks: Vec<ChunkBody>,
    clips: Vec<ClipBody>,
}

/// One member chunk in the body.
#[derive(Serialize, Deserialize)]
struct ChunkBody {
    chunk_id: String,
    duration_ms: u64,
    embedding: Vec<f32>,
}

/// One clip's metadata in the body; its audio follows the body.
#[derive(Serialize, Deserialize)]
struct ClipBody {
    clip_id: String,
    start_time_ms: u64,
    end_time_ms: u64,
    audio_length: usize,
    derived_embedding: Vec<f32>,
    weak_match: Option<WeakMatchBody>,
}

/// A clip's weak-match candidate in the body.
#[derive(Serialize, Deserialize)]
struct WeakMatchBody {
    identity_id: String,
    confidence_score: f32,
}

/// Reports whether `payload` is a cluster-share message, before its pair key is looked up.
pub fn is_cluster_share_message(payload: &[u8]) -> bool {
    payload.starts_with(CLUSTER_SHARE_HEADER)
}

/// Builds the messages that copy `share` from `sender` to `receiver`.
///
/// Inputs: The share, the pair key of the two devices, both peer IDs, and the most clip audio
/// one message may carry, in bytes.
/// Outputs: One or more sealed messages (one when the share has no clips), or `Invalid`,
/// `TooLarge`, or `EncryptionFailed`.
/// Side effects: Draws random nonces; logs a warning (without content) for each clip left out.
pub fn encode_cluster_share_messages(
    share: &ClusterShare,
    pair_key: &PairKey,
    sender: &DeviceId,
    receiver: &DeviceId,
    audio_budget_bytes: usize,
) -> Result<Vec<Vec<u8>>, ClusterShareMessageError> {
    validate_share(share)?;
    let mut clip_groups: Vec<Vec<&UnassignedVoiceClip>> = vec![Vec::new()];
    let mut group_audio_bytes = 0_usize;
    for clip in share.clips() {
        let clip_bytes = clip.audio_bytes().len();
        if clip_bytes > audio_budget_bytes {
            tracing::warn!(
                clip_id = %clip.clip_id(),
                "a review clip exceeds the share budget and is not copied"
            );
            continue;
        }
        let needs_new_group = clip_groups
            .last()
            .is_some_and(|group| group.len() >= MAX_CLIPS_PER_MESSAGE)
            || group_audio_bytes + clip_bytes > audio_budget_bytes;
        if needs_new_group {
            clip_groups.push(Vec::new());
            group_audio_bytes = 0;
        }
        if let Some(group) = clip_groups.last_mut() {
            group.push(clip);
        }
        group_audio_bytes += clip_bytes;
    }
    clip_groups
        .iter()
        .filter(|group| !group.is_empty() || clip_groups.len() == 1)
        .map(|group| encode_message(share, group, pair_key, sender, receiver))
        .collect()
}

/// Opens and validates a message `sender` sent to `receiver`.
///
/// Inputs: A payload for which `is_cluster_share_message` holds, the pair key, and both peer IDs.
/// Outputs: The share (its clips linked to the cluster), or why it was rejected.
/// Side effects: None; the payload is never logged.
pub fn decode_cluster_share_message(
    payload: &[u8],
    pair_key: &PairKey,
    sender: &DeviceId,
    receiver: &DeviceId,
) -> Result<ClusterShare, ClusterShareMessageError> {
    if payload.len() > MAX_CLUSTER_SHARE_MESSAGE_BYTES {
        return Err(ClusterShareMessageError::TooLarge);
    }
    let sealed = payload
        .strip_prefix(CLUSTER_SHARE_HEADER)
        .ok_or(ClusterShareMessageError::Malformed)?;
    let plaintext = open_from_peer(pair_key, CLUSTER_SHARE_HEADER, sender, receiver, sealed)
        .map_err(|error| match error {
            CryptoError::AuthenticationFailed => ClusterShareMessageError::Unauthenticated,
            _ => ClusterShareMessageError::Malformed,
        })?;
    let (length_bytes, rest) = plaintext
        .split_first_chunk::<4>()
        .ok_or(ClusterShareMessageError::Malformed)?;
    let body_length = usize::try_from(u32::from_be_bytes(*length_bytes))
        .map_err(|_| ClusterShareMessageError::Malformed)?;
    if body_length > rest.len() {
        return Err(ClusterShareMessageError::Malformed);
    }
    let (body_bytes, mut audio) = rest.split_at(body_length);
    let body: ShareBody =
        serde_json::from_slice(body_bytes).map_err(|_| ClusterShareMessageError::Malformed)?;
    if body.clips.len() > MAX_CLIPS_PER_MESSAGE || body.chunks.len() > MAX_CHUNKS {
        return Err(ClusterShareMessageError::Invalid);
    }
    let cluster_id = parse_uuid(&body.cluster_id)?;
    let meeting_id = MeetingId::new(body.meeting_id);
    let created_at = Utc
        .timestamp_millis_opt(body.created_at_ms)
        .single()
        .ok_or(ClusterShareMessageError::Invalid)?;
    let chunks: Vec<UnknownSpeakerChunk> = body
        .chunks
        .into_iter()
        .map(|chunk| UnknownSpeakerChunk::new(chunk.chunk_id, chunk.embedding, chunk.duration_ms))
        .collect();
    let mut clips = Vec::with_capacity(body.clips.len());
    for clip in body.clips {
        if clip.audio_length > audio.len() {
            return Err(ClusterShareMessageError::Malformed);
        }
        let (clip_audio, remaining_audio) = audio.split_at(clip.audio_length);
        audio = remaining_audio;
        let weak_match = clip
            .weak_match
            .map(|candidate| {
                parse_uuid(&candidate.identity_id).map(|identity_id| {
                    WeakVoiceMatchCandidate::new(identity_id, candidate.confidence_score)
                })
            })
            .transpose()?;
        clips.push(
            UnassignedVoiceClip::with_review_context(
                parse_uuid(&clip.clip_id)?,
                meeting_id.clone(),
                clip.start_time_ms,
                clip.end_time_ms,
                clip_audio.to_vec(),
                clip.derived_embedding,
                weak_match,
            )
            .with_cluster_id(Some(cluster_id)),
        );
    }
    if !audio.is_empty() {
        return Err(ClusterShareMessageError::Malformed);
    }
    let cluster = UnknownSpeakerCluster::new(
        cluster_id,
        meeting_id,
        body.representative_embedding,
        chunks
            .iter()
            .map(|chunk| chunk.chunk_id().to_owned())
            .collect(),
        body.total_duration_ms,
        created_at,
        None,
    );
    let share = ClusterShare::new(cluster, chunks, clips);
    validate_share(&share)?;
    Ok(share)
}

/// Seals one message carrying the cluster, all its chunks, and `clips`.
///
/// Outputs: The message, or `TooLarge`/`EncryptionFailed`. Side effects: Draws a random nonce.
fn encode_message(
    share: &ClusterShare,
    clips: &[&UnassignedVoiceClip],
    pair_key: &PairKey,
    sender: &DeviceId,
    receiver: &DeviceId,
) -> Result<Vec<u8>, ClusterShareMessageError> {
    let cluster = share.cluster();
    let body = ShareBody {
        meeting_id: cluster.meeting_id().as_str().to_owned(),
        cluster_id: cluster.cluster_id().to_string(),
        created_at_ms: cluster.created_at().timestamp_millis(),
        total_duration_ms: cluster.total_duration_ms(),
        representative_embedding: cluster.representative_embedding().to_vec(),
        chunks: share
            .chunks()
            .iter()
            .map(|chunk| ChunkBody {
                chunk_id: chunk.chunk_id().to_owned(),
                duration_ms: chunk.duration_ms(),
                embedding: chunk.embedding().to_vec(),
            })
            .collect(),
        clips: clips
            .iter()
            .map(|clip| ClipBody {
                clip_id: clip.clip_id().to_string(),
                start_time_ms: clip.start_time_ms(),
                end_time_ms: clip.end_time_ms(),
                audio_length: clip.audio_bytes().len(),
                derived_embedding: clip.derived_embedding().to_vec(),
                weak_match: clip.weak_match_candidate().map(|candidate| WeakMatchBody {
                    identity_id: candidate.identity_id().to_string(),
                    confidence_score: candidate.confidence_score(),
                }),
            })
            .collect(),
    };
    let body_bytes = zeroize::Zeroizing::new(
        serde_json::to_vec(&body).map_err(|_| ClusterShareMessageError::Malformed)?,
    );
    let body_length =
        u32::try_from(body_bytes.len()).map_err(|_| ClusterShareMessageError::TooLarge)?;
    let audio_length: usize = clips.iter().map(|clip| clip.audio_bytes().len()).sum();
    let mut plaintext =
        zeroize::Zeroizing::new(Vec::with_capacity(4 + body_bytes.len() + audio_length));
    plaintext.extend_from_slice(&body_length.to_be_bytes());
    plaintext.extend_from_slice(&body_bytes);
    for clip in clips {
        plaintext.extend_from_slice(clip.audio_bytes());
    }
    let sealed = seal_for_peer(pair_key, CLUSTER_SHARE_HEADER, sender, receiver, &plaintext)
        .map_err(|_| ClusterShareMessageError::EncryptionFailed)?;
    let mut message = Vec::with_capacity(CLUSTER_SHARE_HEADER.len() + sealed.len());
    message.extend_from_slice(CLUSTER_SHARE_HEADER);
    message.extend_from_slice(&sealed);
    if message.len() > MAX_CLUSTER_SHARE_MESSAGE_BYTES {
        return Err(ClusterShareMessageError::TooLarge);
    }
    Ok(message)
}

/// Bounds every value of a share that a peer controls.
///
/// Outputs: Success, or `Invalid`. Side effects: None.
fn validate_share(share: &ClusterShare) -> Result<(), ClusterShareMessageError> {
    let cluster = share.cluster();
    let embeddings_valid = std::iter::once(cluster.representative_embedding())
        .chain(share.chunks().iter().map(UnknownSpeakerChunk::embedding))
        .chain(
            share
                .clips()
                .iter()
                .map(UnassignedVoiceClip::derived_embedding),
        )
        .all(|embedding| {
            embedding.len() <= MAX_EMBEDDING_DIMENSIONS
                && embedding.iter().all(|value| value.is_finite())
        });
    let ids_valid = std::iter::once(cluster.meeting_id().as_str())
        .chain(share.chunks().iter().map(UnknownSpeakerChunk::chunk_id))
        .all(is_usable_id);
    let clips_valid = share.clips().iter().all(|clip| {
        clip.end_time_ms() >= clip.start_time_ms()
            && clip.cluster_id() == Some(cluster.cluster_id())
            && clip.meeting_id() == cluster.meeting_id()
            && clip
                .weak_match_candidate()
                .is_none_or(|candidate| candidate.confidence_score().is_finite())
    });
    if embeddings_valid
        && ids_valid
        && clips_valid
        && share.chunks().len() <= MAX_CHUNKS
        && !cluster.is_resolved()
    {
        Ok(())
    } else {
        Err(ClusterShareMessageError::Invalid)
    }
}

/// Reports whether an ID is non-empty, short, and free of control characters.
fn is_usable_id(id: &str) -> bool {
    !id.is_empty() && id.chars().count() <= MAX_ID_CHARACTERS && !id.chars().any(char::is_control)
}

/// Parses a UUID from the body.
fn parse_uuid(text: &str) -> Result<Uuid, ClusterShareMessageError> {
    Uuid::parse_str(text).map_err(|_| ClusterShareMessageError::Malformed)
}
