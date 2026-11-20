//! Encodes the `NTO1` messages that copy the owner's summary and chat prompts to a paired device
//! (#145).
//!
//! A message is `NTO1` followed by the JSON of `SharedPrompts` sealed with the pair key
//! (`seal_for_peer`, purpose `NTO1`, bound to the sender and receiver). Prompts are the owner's
//! words, so they travel only sealed and are never logged. The sender holds the pair key but is
//! still peer input, so `decode_shared_prompts_message` checks each prompt as the settings screen
//! would, and the receiver merges them by `SharedPrompts::merge`.

use crate::{
    application::crypto::pair_key::{open_from_peer, seal_for_peer},
    domain::{
        llm::PromptTemplate,
        models::DeviceId,
        peer_trust::PairKey,
        shared_prompts::{ChatPrompt, SharedPrompts},
    },
};

/// Marks a prompts message.
const SHARED_PROMPTS_HEADER: &[u8; 4] = b"NTO1";
/// The largest message accepted: two prompts of at most 8000 characters, in JSON.
const MAX_MESSAGE_BYTES: usize = 256 * 1024;

/// Why a prompts message could not be built or used.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SharedPromptsMessageError {
    /// The message exceeds the size limit.
    #[error("prompts message is too large")]
    TooLarge,
    /// The message or its body cannot be parsed.
    #[error("prompts message is malformed")]
    Malformed,
    /// The message does not open with the pair key in this direction, or was changed.
    #[error("prompts message failed authentication")]
    Unauthenticated,
    /// A prompt this device's settings would refuse.
    #[error("prompts message holds a prompt that cannot be used")]
    Invalid,
    /// Encryption failed.
    #[error("prompts could not be encrypted")]
    EncryptionFailed,
}

/// Reports whether `payload` is a prompts message.
///
/// Inputs: A received payload. Outputs: Whether it starts with the header. Side effects: None.
pub fn is_shared_prompts_message(payload: &[u8]) -> bool {
    payload.starts_with(SHARED_PROMPTS_HEADER)
}

/// Seals `prompts` for `receiver`.
///
/// Inputs: The prompts, the pair key of the two devices, and both peer IDs.
/// Outputs: The sealed message, or `Malformed`, `TooLarge` or `EncryptionFailed`.
/// Side effects: Draws a random nonce.
pub fn encode_shared_prompts_message(
    prompts: &SharedPrompts,
    pair_key: &PairKey,
    sender: &DeviceId,
    receiver: &DeviceId,
) -> Result<Vec<u8>, SharedPromptsMessageError> {
    let plaintext = zeroize::Zeroizing::new(
        serde_json::to_vec(prompts).map_err(|_| SharedPromptsMessageError::Malformed)?,
    );
    let sealed = seal_for_peer(
        pair_key,
        SHARED_PROMPTS_HEADER,
        sender,
        receiver,
        &plaintext,
    )
    .map_err(|_| SharedPromptsMessageError::EncryptionFailed)?;
    let mut message = Vec::with_capacity(SHARED_PROMPTS_HEADER.len() + sealed.len());
    message.extend_from_slice(SHARED_PROMPTS_HEADER);
    message.extend_from_slice(&sealed);
    if message.len() > MAX_MESSAGE_BYTES {
        return Err(SharedPromptsMessageError::TooLarge);
    }
    Ok(message)
}

/// Opens and checks a message `sender` sent to `receiver`.
///
/// Inputs: A payload for which `is_shared_prompts_message` holds, the pair key, and both peer
/// IDs. Outputs: The prompts, or why they were rejected (`Invalid` for a prompt
/// `PromptTemplate::parse` or `ChatPrompt::parse` refuses). Side effects: None; nothing is
/// logged.
pub fn decode_shared_prompts_message(
    payload: &[u8],
    pair_key: &PairKey,
    sender: &DeviceId,
    receiver: &DeviceId,
) -> Result<SharedPrompts, SharedPromptsMessageError> {
    if payload.len() > MAX_MESSAGE_BYTES {
        return Err(SharedPromptsMessageError::TooLarge);
    }
    let sealed = payload
        .strip_prefix(SHARED_PROMPTS_HEADER.as_slice())
        .ok_or(SharedPromptsMessageError::Malformed)?;
    let plaintext = zeroize::Zeroizing::new(
        open_from_peer(pair_key, SHARED_PROMPTS_HEADER, sender, receiver, sealed)
            .map_err(|_| SharedPromptsMessageError::Unauthenticated)?,
    );
    let prompts: SharedPrompts =
        serde_json::from_slice(&plaintext).map_err(|_| SharedPromptsMessageError::Malformed)?;
    let summary_ok = prompts
        .summary
        .text
        .as_deref()
        .is_none_or(|text| PromptTemplate::parse(text).is_ok());
    let chat_ok = prompts
        .chat
        .text
        .as_deref()
        .is_none_or(|text| ChatPrompt::parse(text).is_ok());
    if !(summary_ok && chat_ok) {
        return Err(SharedPromptsMessageError::Invalid);
    }
    Ok(prompts)
}
