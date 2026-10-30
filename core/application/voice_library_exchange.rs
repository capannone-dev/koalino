//! Encodes the `NTV1` messages that copy the voice library to a paired device (Issue #56).
//!
//! A message is `NTV1` followed by a JSON body sealed with the pair key (`seal_for_peer`, purpose
//! `NTV1`, bound to the sender and receiver). The body holds the identities, the deletion records,
//! and some of the voice variants: a large library is split so each message fits the transport,
//! and every message repeats the identities and deletions so the receiver can apply each alone.
//!
//! Embeddings are biometric PII, so they travel only inside the sealed value and are never logged;
//! the plaintext is wiped after use. The sender holds the pair key but is still peer input, so
//! `decode_voice_library_message` bounds every value.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    application::crypto::pair_key::{open_from_peer, seal_for_peer},
    domain::{
        models::{
            DeviceId, IdentityTombstone, SharedSpeakerIdentity, SharedVoiceVariant, VoiceLibrary,
        },
        peer_trust::PairKey,
    },
};

/// Marks a voice-library message.
const VOICE_LIBRARY_HEADER: &[u8; 4] = b"NTV1";
/// The largest message, matching the transport's payload limit.
const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
/// How many embedding values one message carries. JSON spends up to about 16 bytes on a value, so
/// this leaves room within `MAX_MESSAGE_BYTES` for the identities and deletions.
const MAX_EMBEDDING_VALUES_PER_MESSAGE: usize = 512 * 1024;
/// The longest embedding accepted.
const MAX_EMBEDDING_DIMENSIONS: usize = 4_096;
/// The longest display name accepted, in characters.
const MAX_NAME_CHARACTERS: usize = 256;
/// The most identities or deletion records one message may hold.
const MAX_IDENTITIES: usize = 4_096;

/// Why a voice-library message could not be built or used.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum VoiceLibraryMessageError {
    /// The message exceeds the transport's payload limit.
    #[error("voice-library message is too large")]
    TooLarge,
    /// The message or its body cannot be parsed.
    #[error("voice-library message is malformed")]
    Malformed,
    /// The message does not open with the pair key in this direction, or was changed.
    #[error("voice-library message failed authentication")]
    Unauthenticated,
    /// The library holds implausible values (non-finite or oversized embeddings, unusable names,
    /// too many identities).
    #[error("voice library holds implausible values")]
    Invalid,
    /// Encryption failed.
    #[error("voice library could not be encrypted")]
    EncryptionFailed,
}

/// The JSON body of a message.
#[derive(Serialize, Deserialize)]
struct LibraryBody {
    identities: Vec<IdentityBody>,
    variants: Vec<VariantBody>,
    tombstones: Vec<TombstoneBody>,
}

/// One identity in the body. Times keep full precision, so digests converge across devices.
#[derive(Serialize, Deserialize)]
struct IdentityBody {
    identity_id: Uuid,
    display_name: String,
    #[serde(with = "rfc3339")]
    created_at: DateTime<Utc>,
    #[serde(with = "rfc3339")]
    updated_at: DateTime<Utc>,
}

/// One voice variant in the body.
#[derive(Serialize, Deserialize)]
struct VariantBody {
    identity_id: Uuid,
    variant_id: Uuid,
    vector: Vec<f32>,
    #[serde(with = "rfc3339")]
    created_at: DateTime<Utc>,
}

/// One deletion record in the body.
#[derive(Serialize, Deserialize)]
struct TombstoneBody {
    identity_id: Uuid,
    #[serde(with = "rfc3339")]
    deleted_at: DateTime<Utc>,
}

/// Writes times as RFC 3339 text, keeping the full precision that library digests depend on.
mod rfc3339 {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    /// Writes `time` as text.
    pub(super) fn serialize<S: Serializer>(
        time: &DateTime<Utc>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&time.to_rfc3339())
    }

    /// Reads a time from text.
    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<DateTime<Utc>, D::Error> {
        let text = String::deserialize(deserializer)?;
        DateTime::parse_from_rfc3339(&text)
            .map(|time| time.with_timezone(&Utc))
            .map_err(D::Error::custom)
    }
}

/// Reports whether `payload` is a voice-library message.
///
/// Inputs: A received payload. Outputs: Whether it starts with the header. Side effects: None.
pub fn is_voice_library_message(payload: &[u8]) -> bool {
    payload.starts_with(VOICE_LIBRARY_HEADER)
}

/// Seals `library` for `receiver`, in as many messages as it needs.
///
/// Inputs: The library, the pair key of the two devices, and both peer IDs.
/// Outputs: One or more sealed messages, or `Invalid`, `TooLarge`, or `EncryptionFailed`.
/// Side effects: Draws random nonces.
pub fn encode_voice_library_messages(
    library: &VoiceLibrary,
    pair_key: &PairKey,
    sender: &DeviceId,
    receiver: &DeviceId,
) -> Result<Vec<Vec<u8>>, VoiceLibraryMessageError> {
    validate_library(library)?;
    let mut variant_groups: Vec<Vec<&SharedVoiceVariant>> = vec![Vec::new()];
    let mut group_values = 0_usize;
    for variant in &library.variants {
        if group_values + variant.vector.len() > MAX_EMBEDDING_VALUES_PER_MESSAGE {
            variant_groups.push(Vec::new());
            group_values = 0;
        }
        if let Some(group) = variant_groups.last_mut() {
            group.push(variant);
        }
        group_values += variant.vector.len();
    }
    variant_groups
        .iter()
        .map(|group| encode_message(library, group, pair_key, sender, receiver))
        .collect()
}

/// Opens and validates a message `sender` sent to `receiver`.
///
/// Inputs: A payload for which `is_voice_library_message` holds, the pair key, and both peer IDs.
/// Outputs: The library the message carries, or why it was rejected.
/// Side effects: None; nothing is logged.
pub fn decode_voice_library_message(
    payload: &[u8],
    pair_key: &PairKey,
    sender: &DeviceId,
    receiver: &DeviceId,
) -> Result<VoiceLibrary, VoiceLibraryMessageError> {
    if payload.len() > MAX_MESSAGE_BYTES {
        return Err(VoiceLibraryMessageError::TooLarge);
    }
    let sealed = payload
        .strip_prefix(VOICE_LIBRARY_HEADER.as_slice())
        .ok_or(VoiceLibraryMessageError::Malformed)?;
    let plaintext = zeroize::Zeroizing::new(
        open_from_peer(pair_key, VOICE_LIBRARY_HEADER, sender, receiver, sealed)
            .map_err(|_| VoiceLibraryMessageError::Unauthenticated)?,
    );
    let body: LibraryBody =
        serde_json::from_slice(&plaintext).map_err(|_| VoiceLibraryMessageError::Malformed)?;
    let library = VoiceLibrary {
        identities: body
            .identities
            .into_iter()
            .map(|identity| SharedSpeakerIdentity {
                identity_id: identity.identity_id,
                display_name: identity.display_name,
                created_at: identity.created_at,
                updated_at: identity.updated_at,
            })
            .collect(),
        variants: body
            .variants
            .into_iter()
            .map(|variant| SharedVoiceVariant {
                identity_id: variant.identity_id,
                variant_id: variant.variant_id,
                vector: variant.vector,
                created_at: variant.created_at,
            })
            .collect(),
        tombstones: body
            .tombstones
            .into_iter()
            .map(|tombstone| IdentityTombstone {
                identity_id: tombstone.identity_id,
                deleted_at: tombstone.deleted_at,
            })
            .collect(),
    };
    validate_library(&library)?;
    Ok(library)
}

/// Seals one message holding `variants` and the whole library's identities and deletions.
///
/// Outputs: The message, or `TooLarge` or `EncryptionFailed`. Side effects: Draws a nonce.
fn encode_message(
    library: &VoiceLibrary,
    variants: &[&SharedVoiceVariant],
    pair_key: &PairKey,
    sender: &DeviceId,
    receiver: &DeviceId,
) -> Result<Vec<u8>, VoiceLibraryMessageError> {
    let body = LibraryBody {
        identities: library
            .identities
            .iter()
            .map(|identity| IdentityBody {
                identity_id: identity.identity_id,
                display_name: identity.display_name.clone(),
                created_at: identity.created_at,
                updated_at: identity.updated_at,
            })
            .collect(),
        variants: variants
            .iter()
            .map(|variant| VariantBody {
                identity_id: variant.identity_id,
                variant_id: variant.variant_id,
                vector: variant.vector.clone(),
                created_at: variant.created_at,
            })
            .collect(),
        tombstones: library
            .tombstones
            .iter()
            .map(|tombstone| TombstoneBody {
                identity_id: tombstone.identity_id,
                deleted_at: tombstone.deleted_at,
            })
            .collect(),
    };
    let plaintext = zeroize::Zeroizing::new(
        serde_json::to_vec(&body).map_err(|_| VoiceLibraryMessageError::Malformed)?,
    );
    let sealed = seal_for_peer(pair_key, VOICE_LIBRARY_HEADER, sender, receiver, &plaintext)
        .map_err(|_| VoiceLibraryMessageError::EncryptionFailed)?;
    let mut message = Vec::with_capacity(VOICE_LIBRARY_HEADER.len() + sealed.len());
    message.extend_from_slice(VOICE_LIBRARY_HEADER);
    message.extend_from_slice(&sealed);
    if message.len() > MAX_MESSAGE_BYTES {
        return Err(VoiceLibraryMessageError::TooLarge);
    }
    Ok(message)
}

/// Bounds every value of a library that a peer controls.
///
/// Outputs: Success, or `Invalid`. Side effects: None.
fn validate_library(library: &VoiceLibrary) -> Result<(), VoiceLibraryMessageError> {
    let usable_name = |name: &str| {
        !name.trim().is_empty()
            && name.chars().count() <= MAX_NAME_CHARACTERS
            && !name.chars().any(char::is_control)
    };
    let usable_vector = |vector: &[f32]| {
        !vector.is_empty()
            && vector.len() <= MAX_EMBEDDING_DIMENSIONS
            && vector.iter().all(|value| value.is_finite())
    };
    let plausible = library.identities.len() <= MAX_IDENTITIES
        && library.tombstones.len() <= MAX_IDENTITIES
        && library
            .identities
            .iter()
            .all(|identity| usable_name(&identity.display_name))
        && library
            .variants
            .iter()
            .all(|variant| usable_vector(&variant.vector));
    if plausible {
        Ok(())
    } else {
        Err(VoiceLibraryMessageError::Invalid)
    }
}
