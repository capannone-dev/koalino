//! Native messaging framing, as Firefox and Chromium-based browsers speak it with a host over
//! its stdin and stdout: each message is a 32-bit length in native byte order followed by that
//! many bytes of UTF-8 JSON.
//!
//! Incoming messages are capped at `MAXIMUM_INCOMING_MESSAGE_BYTES` (a 30 s audio chunk is about
//! 1.3 MB of base64), and the cap is checked before the body is read, so a bad length cannot make
//! the host allocate gigabytes. Message bodies are never logged: they carry meeting audio, slides
//! and transcripts.

use std::io::{ErrorKind, Read, Write};

use thiserror::Error;

/// Largest message the host accepts from the extension.
pub const MAXIMUM_INCOMING_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
/// Largest message a browser accepts from a host (Chrome's limit is 1 MB).
pub const MAXIMUM_OUTGOING_MESSAGE_BYTES: usize = 1024 * 1024;

/// Why a message could not be read or written.
#[derive(Debug, Error)]
pub enum NativeMessagingError {
    /// The length prefix announced more than the host accepts.
    #[error("native message of {length} bytes exceeds the limit")]
    TooLarge {
        /// The announced length.
        length: usize,
    },
    /// The body is not valid UTF-8 JSON.
    #[error("native message is not valid JSON")]
    InvalidJson,
    /// Reading or writing the stream failed (including a body cut short).
    #[error("native messaging stream failed: {0}")]
    Io(#[from] std::io::Error),
}

/// Reads one message.
///
/// Inputs: The stream (the host's stdin).
/// Outputs: The JSON value; `None` when the stream ended before a new message (the browser
/// closed the port); an error for an oversized length, a body cut short, or invalid JSON.
/// Side effects: Reads from the stream (blocking).
pub fn read_message(
    input: &mut impl Read,
) -> Result<Option<serde_json::Value>, NativeMessagingError> {
    let mut length_bytes = [0_u8; 4];
    match input.read_exact(&mut length_bytes) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let length = usize::try_from(u32::from_ne_bytes(length_bytes)).unwrap_or(usize::MAX);
    if length > MAXIMUM_INCOMING_MESSAGE_BYTES {
        return Err(NativeMessagingError::TooLarge { length });
    }
    let mut body = vec![0_u8; length];
    input.read_exact(&mut body)?;
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|_| NativeMessagingError::InvalidJson)
}

/// Writes one message and flushes it.
///
/// Inputs: The stream (the host's stdout) and the JSON value.
/// Outputs: Success, or an error when the encoded message exceeds the browser's 1 MB limit or
/// the stream fails.
/// Side effects: Writes to the stream.
pub fn write_message(
    output: &mut impl Write,
    message: &serde_json::Value,
) -> Result<(), NativeMessagingError> {
    let body = serde_json::to_vec(message).map_err(|_| NativeMessagingError::InvalidJson)?;
    if body.len() > MAXIMUM_OUTGOING_MESSAGE_BYTES {
        return Err(NativeMessagingError::TooLarge { length: body.len() });
    }
    let length = u32::try_from(body.len())
        .map_err(|_| NativeMessagingError::TooLarge { length: body.len() })?;
    output.write_all(&length.to_ne_bytes())?;
    output.write_all(&body)?;
    output.flush()?;
    Ok(())
}
