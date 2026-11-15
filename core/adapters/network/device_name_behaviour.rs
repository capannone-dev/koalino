//! Defines the `/note-taker/device-name/1` request-response protocol (Issue #121): one device asks
//! a nearby one for the name its owner gave it, so the Devices screen lists "Laptop" instead of a
//! peer ID before pairing.
//!
//! It runs only over connections the peer gate admits, so an unpaired device answers only while
//! its owner keeps a pairing window open. The name is a claim, like the one a pairing request
//! carries: pairing still needs the code both devices show. Messages reuse the pairing framing
//! (length-prefixed versioned JSON, at most 4 KiB). A separate protocol, rather than a new field
//! on the pairing request, keeps older versions from taking the question for a pairing request.
//!
//! Paired devices use it too: each asks the other when they connect, and a renamed device asks
//! every connected paired device again, with its own name in the question, so both sides keep
//! the other's current name (#121).

use std::{io, time::Duration};

use futures::{AsyncRead, AsyncWrite};
use libp2p::request_response;
use serde::{Deserialize, Serialize};

use super::pairing_behaviour::{HasVersion, PAIRING_MESSAGE_VERSION, PairingCodec};

/// How long a name question waits; a device that keeps quiet is asked again later.
const DEVICE_NAME_TIMEOUT: Duration = Duration::from_secs(10);

/// Names the device-name protocol.
#[derive(Clone)]
pub struct DeviceNameProtocol;

impl AsRef<str> for DeviceNameProtocol {
    /// Returns the stable, versioned protocol identifier.
    fn as_ref(&self) -> &str {
        "/note-taker/device-name/1"
    }
}

/// Asks the remote device for its name.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DeviceNameQuestion {
    /// Wire-format version; must be the pairing messages' version.
    pub version: u32,
    /// The asking device's own name, sent only to paired devices so a rename reaches them
    /// (#121). Older versions leave it out and ignore it, unvalidated on receipt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asker_name: Option<String>,
}

impl Default for DeviceNameQuestion {
    /// Creates a current-version question that tells nothing about the asker.
    fn default() -> Self {
        Self {
            version: PAIRING_MESSAGE_VERSION,
            asker_name: None,
        }
    }
}

impl DeviceNameQuestion {
    /// Creates a question for a paired device that also tells it the asker's name.
    pub fn telling(asker_name: &str) -> Self {
        Self {
            version: PAIRING_MESSAGE_VERSION,
            asker_name: Some(asker_name.to_owned()),
        }
    }
}

/// Answers with the device's name.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DeviceNameAnswer {
    /// Wire-format version; must be the pairing messages' version.
    pub version: u32,
    /// The answering device's name, still unvalidated on receipt.
    pub device_name: String,
}

impl DeviceNameAnswer {
    /// Creates a current-version answer.
    pub fn new(device_name: &str) -> Self {
        Self {
            version: PAIRING_MESSAGE_VERSION,
            device_name: device_name.to_owned(),
        }
    }
}

impl HasVersion for DeviceNameQuestion {
    fn version(&self) -> u32 {
        self.version
    }
}

impl HasVersion for DeviceNameAnswer {
    fn version(&self) -> u32 {
        self.version
    }
}

/// Frames device-name messages as the pairing codec does.
#[derive(Clone, Default)]
pub struct DeviceNameCodec;

impl request_response::Codec for DeviceNameCodec {
    type Protocol = DeviceNameProtocol;
    type Request = DeviceNameQuestion;
    type Response = DeviceNameAnswer;

    async fn read_request<Stream>(
        &mut self,
        _: &Self::Protocol,
        stream: &mut Stream,
    ) -> io::Result<Self::Request>
    where
        Stream: AsyncRead + Unpin + Send,
    {
        PairingCodec::read_message(stream).await
    }

    async fn read_response<Stream>(
        &mut self,
        _: &Self::Protocol,
        stream: &mut Stream,
    ) -> io::Result<Self::Response>
    where
        Stream: AsyncRead + Unpin + Send,
    {
        PairingCodec::read_message(stream).await
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
        PairingCodec::write_message(stream, &request).await
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
        PairingCodec::write_message(stream, &response).await
    }
}

/// Creates the device-name behaviour.
///
/// Side effects: Allocates in-memory behaviour state only.
pub fn new_device_name_behaviour() -> request_response::Behaviour<DeviceNameCodec> {
    request_response::Behaviour::new(
        [(DeviceNameProtocol, request_response::ProtocolSupport::Full)],
        request_response::Config::default().with_request_timeout(DEVICE_NAME_TIMEOUT),
    )
}
