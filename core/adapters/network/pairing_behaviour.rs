//! Defines the `/note-taker/pairing/1` request-response protocol used to pair two devices.
//!
//! The initiator sends its device name, and the responder replies, after its owner compared the
//! pairing codes, with acceptance and its own name, or with a rejection. The messages are small
//! versioned JSON documents in a length-prefixed frame of at most 4 KiB. Responses may take as long
//! as a person needs to compare codes, so the request timeout is two minutes.

use std::{io, time::Duration};

use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use libp2p::request_response;
use serde::{Deserialize, Serialize};

/// Largest encoded pairing message.
const MAX_PAIRING_MESSAGE_BYTES: usize = 4 * 1024;

/// How long the initiator waits for the owner of the other device to answer.
pub const PAIRING_RESPONSE_TIMEOUT: Duration = Duration::from_secs(120);

/// Wire-format version of both messages.
const PAIRING_MESSAGE_VERSION: u32 = 1;

/// Names the pairing protocol.
#[derive(Clone)]
pub struct PairingProtocol;

impl AsRef<str> for PairingProtocol {
    /// Returns the stable, versioned protocol identifier.
    fn as_ref(&self) -> &str {
        "/note-taker/pairing/1"
    }
}

/// Asks the remote device to pair, naming the requesting device.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PairingRequest {
    /// Wire-format version; must be `PAIRING_MESSAGE_VERSION`.
    pub version: u32,
    /// The requesting device's name, still unvalidated on receipt.
    pub device_name: String,
}

impl PairingRequest {
    /// Creates a current-version request.
    pub fn new(device_name: &str) -> Self {
        Self {
            version: PAIRING_MESSAGE_VERSION,
            device_name: device_name.to_owned(),
        }
    }
}

/// Answers a pairing request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PairingResponse {
    /// Wire-format version; must be `PAIRING_MESSAGE_VERSION`.
    pub version: u32,
    /// The answering device's name when its owner accepted; `None` for a rejection.
    pub accepted_as: Option<String>,
}

impl PairingResponse {
    /// Creates a current-version answer.
    pub fn new(accepted_as: Option<&str>) -> Self {
        Self {
            version: PAIRING_MESSAGE_VERSION,
            accepted_as: accepted_as.map(str::to_owned),
        }
    }
}

/// Frames pairing messages as length-prefixed JSON.
#[derive(Clone, Default)]
pub struct PairingCodec;

impl PairingCodec {
    /// Reads one frame and decodes a message of the current version.
    ///
    /// Outputs: The message, or `InvalidData` for an oversized frame, malformed JSON, or another
    /// version. Side effects: Consumes bytes from the stream.
    async fn read_message<Message, Stream>(stream: &mut Stream) -> io::Result<Message>
    where
        Message: for<'de> Deserialize<'de> + HasVersion,
        Stream: AsyncRead + Unpin + Send,
    {
        let mut encoded_length = [0_u8; 4];
        stream.read_exact(&mut encoded_length).await?;
        let length = u32::from_be_bytes(encoded_length) as usize;
        if length > MAX_PAIRING_MESSAGE_BYTES {
            return Err(invalid_data("pairing message exceeds the size limit"));
        }
        let mut encoded_message = vec![0_u8; length];
        stream.read_exact(&mut encoded_message).await?;
        let message: Message = serde_json::from_slice(&encoded_message)
            .map_err(|_| invalid_data("pairing message is not valid JSON"))?;
        if message.version() != PAIRING_MESSAGE_VERSION {
            return Err(invalid_data("unsupported pairing message version"));
        }
        Ok(message)
    }

    /// Encodes and writes one frame, then closes the write side.
    ///
    /// Outputs: Success, or `InvalidInput` for a message over the size limit.
    /// Side effects: Writes to the stream.
    async fn write_message<Message, Stream>(
        stream: &mut Stream,
        message: &Message,
    ) -> io::Result<()>
    where
        Message: Serialize,
        Stream: AsyncWrite + Unpin + Send,
    {
        let encoded_message = serde_json::to_vec(message)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "unencodable message"))?;
        let length = u32::try_from(encoded_message.len())
            .ok()
            .filter(|length| *length as usize <= MAX_PAIRING_MESSAGE_BYTES)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "pairing message exceeds the size limit",
                )
            })?;
        stream.write_all(&length.to_be_bytes()).await?;
        stream.write_all(&encoded_message).await?;
        stream.close().await
    }
}

/// Exposes a message's wire-format version for validation.
trait HasVersion {
    /// The message's declared version.
    fn version(&self) -> u32;
}

impl HasVersion for PairingRequest {
    fn version(&self) -> u32 {
        self.version
    }
}

impl HasVersion for PairingResponse {
    fn version(&self) -> u32 {
        self.version
    }
}

/// Builds an `InvalidData` error with a static description.
fn invalid_data(description: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, description)
}

impl request_response::Codec for PairingCodec {
    type Protocol = PairingProtocol;
    type Request = PairingRequest;
    type Response = PairingResponse;

    async fn read_request<Stream>(
        &mut self,
        _: &Self::Protocol,
        stream: &mut Stream,
    ) -> io::Result<Self::Request>
    where
        Stream: AsyncRead + Unpin + Send,
    {
        Self::read_message(stream).await
    }

    async fn read_response<Stream>(
        &mut self,
        _: &Self::Protocol,
        stream: &mut Stream,
    ) -> io::Result<Self::Response>
    where
        Stream: AsyncRead + Unpin + Send,
    {
        Self::read_message(stream).await
    }

    async fn write_request<Stream>(
        &mut self,
        _: &Self::Protocol,
        stream: &mut Stream,
        request: Self::Request,
    ) -> io::Result<()>
    where
        Stream: AsyncWrite + Unpin + Send,
    {
        Self::write_message(stream, &request).await
    }

    async fn write_response<Stream>(
        &mut self,
        _: &Self::Protocol,
        stream: &mut Stream,
        response: Self::Response,
    ) -> io::Result<()>
    where
        Stream: AsyncWrite + Unpin + Send,
    {
        Self::write_message(stream, &response).await
    }
}

/// Creates the pairing behaviour with its long response timeout.
///
/// Side effects: Allocates in-memory behaviour state only.
pub fn new_pairing_behaviour() -> request_response::Behaviour<PairingCodec> {
    request_response::Behaviour::new(
        [(PairingProtocol, request_response::ProtocolSupport::Full)],
        request_response::Config::default().with_request_timeout(PAIRING_RESPONSE_TIMEOUT),
    )
}
