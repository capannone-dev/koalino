//! Encodes and validates the `DeviceManifest` messages paired devices exchange (Issue #43,
//! REQUIREMENTS 3.4).
//!
//! A manifest travels over the same encrypted libp2p stream as meeting sync, framed as `NTM1`
//! followed by the manifest's JSON, so it is told apart from `NTS1` sync envelopes by its first
//! four bytes. Devices that predate manifests reject it as an unknown envelope and carry on.
//! Manifests come from paired devices but are still peer input, so size, model count, and model
//! labels are bounded before the value is used. A manifest carries no credentials, model weights,
//! raw audio, or hardware serial numbers.

use crate::domain::models::{DeviceManifest, TranscriptionModel};

/// First four bytes of a manifest message.
const MANIFEST_MESSAGE_HEADER: &[u8; 4] = b"NTM1";
/// Largest accepted manifest message, header included.
const MAX_MANIFEST_MESSAGE_BYTES: usize = 16 * 1024;
/// Most transcription models one manifest may list.
const MAX_ADVERTISED_MODELS: usize = 16;
/// Longest accepted third-party model label, in characters.
const MAX_MODEL_LABEL_CHARACTERS: usize = 64;

/// Why a manifest message could not be used.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ManifestMessageError {
    /// The message exceeds `MAX_MANIFEST_MESSAGE_BYTES`.
    #[error("manifest message is too large")]
    TooLarge,
    /// The body is not a manifest in JSON.
    #[error("manifest message is malformed")]
    Malformed,
    /// The manifest lists too many models, or a model label that is empty, too long, or contains
    /// control characters.
    #[error("manifest advertises implausible capabilities")]
    Invalid,
}

/// Frames a manifest for sending to paired devices.
///
/// Inputs: This device's manifest.
/// Outputs: `NTM1` followed by the manifest's JSON, or `Invalid`/`TooLarge` for a manifest that
/// receivers would reject.
/// Side effects: None.
pub fn encode_manifest_message(manifest: &DeviceManifest) -> Result<Vec<u8>, ManifestMessageError> {
    validate(manifest)?;
    let body = serde_json::to_vec(manifest).map_err(|_| ManifestMessageError::Malformed)?;
    let mut message = Vec::with_capacity(MANIFEST_MESSAGE_HEADER.len() + body.len());
    message.extend_from_slice(MANIFEST_MESSAGE_HEADER);
    message.extend_from_slice(&body);
    if message.len() > MAX_MANIFEST_MESSAGE_BYTES {
        return Err(ManifestMessageError::TooLarge);
    }
    Ok(message)
}

/// Reads a manifest message received from a peer.
///
/// Inputs: A payload from the encrypted transport.
/// Outputs: `None` when the payload is not a manifest message (for example a sync envelope);
/// otherwise the validated manifest, or why it was rejected.
/// Side effects: None; the payload is never logged.
pub fn decode_manifest_message(
    payload: &[u8],
) -> Option<Result<DeviceManifest, ManifestMessageError>> {
    let body = payload.strip_prefix(MANIFEST_MESSAGE_HEADER)?;
    if payload.len() > MAX_MANIFEST_MESSAGE_BYTES {
        return Some(Err(ManifestMessageError::TooLarge));
    }
    Some(
        serde_json::from_slice::<DeviceManifest>(body)
            .map_err(|_| ManifestMessageError::Malformed)
            .and_then(|manifest| validate(&manifest).map(|()| manifest)),
    )
}

/// Bounds the parts of a manifest a peer controls.
///
/// Outputs: Success, or `Invalid` for too many models or an unusable model label.
/// Side effects: None.
fn validate(manifest: &DeviceManifest) -> Result<(), ManifestMessageError> {
    let models = manifest.available_transcription_models();
    let labels_are_plausible = models.iter().all(|model| match model {
        TranscriptionModel::Whisper => true,
        TranscriptionModel::ThirdParty(label) => {
            !label.is_empty()
                && label.chars().count() <= MAX_MODEL_LABEL_CHARACTERS
                && !label.chars().any(char::is_control)
        }
    });
    if models.len() > MAX_ADVERTISED_MODELS || !labels_are_plausible {
        return Err(ManifestMessageError::Invalid);
    }
    Ok(())
}
