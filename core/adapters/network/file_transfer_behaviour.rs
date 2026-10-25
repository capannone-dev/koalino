//! Defines the isolated libp2p behaviour and bounded framing used only for binary attachments.

use std::io;

use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use libp2p::request_response;

/// Bounds one encoded file-transfer protocol message; files are split into 64 KiB frames above this layer.
pub const MAX_FILE_TRANSFER_MESSAGE_BYTES: usize = 128 * 1024;

/// Names the attachment protocol independently from the CRDT sync-message protocol.
#[derive(Clone)]
pub struct FileTransferProtocol;
impl AsRef<str> for FileTransferProtocol {
    /// Returns the stable attachment-only protocol identifier.
    fn as_ref(&self) -> &str {
        "/note-taker/file-transfer/1"
    }
}

/// Wraps opaque adapter-private request bytes so the swarm distinguishes file events from CRDT events.
#[derive(Clone, Debug)]
pub struct FileTransferRequestPayload(pub Vec<u8>);
/// Wraps opaque adapter-private response bytes so the swarm distinguishes file events from CRDT events.
#[derive(Clone, Debug)]
pub struct FileTransferResponsePayload(pub Vec<u8>);

/// Frames only attachment protocol messages on their own authenticated request-response substreams.
#[derive(Clone, Default)]
pub struct FileTransferCodec;

impl FileTransferCodec {
    async fn read_frame<Stream>(&mut self, stream: &mut Stream) -> io::Result<Vec<u8>>
    where
        Stream: AsyncRead + Unpin + Send,
    {
        let mut encoded_length = [0_u8; 4];
        stream.read_exact(&mut encoded_length).await?;
        let length = u32::from_be_bytes(encoded_length) as usize;
        if length > MAX_FILE_TRANSFER_MESSAGE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "file transfer frame exceeds limit",
            ));
        }
        let mut payload = vec![0_u8; length];
        stream.read_exact(&mut payload).await?;
        Ok(payload)
    }
    async fn write_frame<Stream>(&mut self, stream: &mut Stream, payload: Vec<u8>) -> io::Result<()>
    where
        Stream: AsyncWrite + Unpin + Send,
    {
        if payload.len() > MAX_FILE_TRANSFER_MESSAGE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "file transfer frame exceeds limit",
            ));
        }
        let length = u32::try_from(payload.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "file transfer frame length overflow",
            )
        })?;
        stream.write_all(&length.to_be_bytes()).await?;
        stream.write_all(&payload).await?;
        stream.close().await
    }
}

#[async_trait::async_trait]
impl request_response::Codec for FileTransferCodec {
    type Protocol = FileTransferProtocol;
    type Request = FileTransferRequestPayload;
    type Response = FileTransferResponsePayload;
    async fn read_request<Stream>(
        &mut self,
        _: &Self::Protocol,
        stream: &mut Stream,
    ) -> io::Result<Self::Request>
    where
        Stream: AsyncRead + Unpin + Send,
    {
        Ok(FileTransferRequestPayload(self.read_frame(stream).await?))
    }
    async fn read_response<Stream>(
        &mut self,
        _: &Self::Protocol,
        stream: &mut Stream,
    ) -> io::Result<Self::Response>
    where
        Stream: AsyncRead + Unpin + Send,
    {
        Ok(FileTransferResponsePayload(self.read_frame(stream).await?))
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
        self.write_frame(stream, request.0).await
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
        self.write_frame(stream, response.0).await
    }
}

/// Creates the independent attachment `NetworkBehaviour` attached alongside CRDT sync behaviour.
pub fn new_file_transfer_behaviour() -> request_response::Behaviour<FileTransferCodec> {
    request_response::Behaviour::new(
        [(
            FileTransferProtocol,
            request_response::ProtocolSupport::Full,
        )],
        request_response::Config::default(),
    )
}
