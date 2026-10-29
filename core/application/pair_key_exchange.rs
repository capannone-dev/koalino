//! Frames the X25519 public key a device sends each paired device (Issue #50).
//!
//! The message travels over the same encrypted libp2p stream as meeting sync: `NTK1` followed by
//! exactly 32 key bytes, told apart from `NTS1` sync envelopes and `NTM1` manifests by its first
//! four bytes. Devices that predate #50 reject it as an unknown envelope and carry on. The key is
//! public, but it is peer input, so its size is checked before use.

/// First four bytes of a pair-key message.
const PAIR_KEY_MESSAGE_HEADER: &[u8; 4] = b"NTK1";

/// Why a pair-key message could not be used.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PairKeyMessageError {
    /// The body is not exactly 32 bytes.
    #[error("pair-key message is malformed")]
    Malformed,
}

/// Frames this device's public key.
///
/// Inputs: The X25519 public key. Outputs: `NTK1` followed by the key. Side effects: None.
pub fn encode_pair_key_message(public_key: &[u8; 32]) -> Vec<u8> {
    let mut message = Vec::with_capacity(PAIR_KEY_MESSAGE_HEADER.len() + public_key.len());
    message.extend_from_slice(PAIR_KEY_MESSAGE_HEADER);
    message.extend_from_slice(public_key);
    message
}

/// Reads a pair-key message received from a peer.
///
/// Inputs: A payload from the encrypted transport.
/// Outputs: `None` when the payload is another kind of message; otherwise the public key, or
/// `Malformed`. Side effects: None.
pub fn decode_pair_key_message(payload: &[u8]) -> Option<Result<[u8; 32], PairKeyMessageError>> {
    let body = payload.strip_prefix(PAIR_KEY_MESSAGE_HEADER)?;
    Some(<[u8; 32]>::try_from(body).map_err(|_| PairKeyMessageError::Malformed))
}
