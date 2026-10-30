//! Defines the isolated libp2p behaviour and bounded framing used only for binary attachments,
//! and the two frames it carries: a request for one chunk (`NTFQ`) and a chunk (`NTFC`).

use std::io;

use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use libp2p::request_response;

use crate::{domain::models::FileChunk, ports::file_transfer::FILE_TRANSFER_CHUNK_BYTES};

/// Bounds one encoded file-transfer protocol message; files are split into 64 KiB frames above this layer.
pub const MAX_FILE_TRANSFER_MESSAGE_BYTES: usize = 128 * 1024;

/// Marks a request for one chunk.
const CHUNK_REQUEST_HEADER: &[u8; 4] = b"NTFQ";
/// Marks a chunk.
const CHUNK_HEADER: &[u8; 4] = b"NTFC";
/// The length of a content hash in hexadecimal.
const HASH_LENGTH: usize = 64;

/// One decoded frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileTransferFrame {
    /// A request for the chunk of `hash` starting at `offset`.
    ChunkRequest {
        /// The file's content address.
        hash: String,
        /// Where the chunk starts.
        offset: u64,
    },
    /// A chunk.
    Chunk(FileChunk),
}

/// Encodes a chunk request.
///
/// Inputs: The content hash and the offset. Outputs: The frame. Side effects: None.
pub fn encode_chunk_request(hash: &str, offset: u64) -> Vec<u8> {
    let mut frame = CHUNK_REQUEST_HEADER.to_vec();
    frame.extend_from_slice(hash.as_bytes());
    frame.extend_from_slice(&offset.to_be_bytes());
    frame
}

/// Encodes a chunk.
///
/// Inputs: The chunk. Outputs: The frame. Side effects: None.
pub fn encode_chunk(chunk: &FileChunk) -> Vec<u8> {
    let mut frame = CHUNK_HEADER.to_vec();
    frame.extend_from_slice(chunk.hash().as_bytes());
    frame.extend_from_slice(&chunk.total_size().to_be_bytes());
    frame.extend_from_slice(&chunk.offset().to_be_bytes());
    frame.extend_from_slice(chunk.data());
    frame
}

/// Decodes a frame from a paired device.
///
/// Inputs: The received bytes. Outputs: The frame, or `None` for anything malformed or for a
/// chunk larger than the limit. Side effects: None.
pub fn decode_frame(frame: &[u8]) -> Option<FileTransferFrame> {
    let (header, rest) = frame.split_first_chunk::<4>()?;
    let (hash, rest) = rest.split_at_checked(HASH_LENGTH)?;
    let hash = std::str::from_utf8(hash).ok()?.to_owned();
    let (first_number, rest) = rest.split_first_chunk::<8>()?;
    let first_number = u64::from_be_bytes(*first_number);
    if header == CHUNK_REQUEST_HEADER && rest.is_empty() {
        return Some(FileTransferFrame::ChunkRequest {
            hash,
            offset: first_number,
        });
    }
    if header == CHUNK_HEADER {
        let (offset, data) = rest.split_first_chunk::<8>()?;
        if data.len() > FILE_TRANSFER_CHUNK_BYTES {
            return None;
        }
        return Some(FileTransferFrame::Chunk(FileChunk::new(
            hash,
            first_number,
            u64::from_be_bytes(*offset),
            data.to_vec(),
        )));
    }
    None
}

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

#[cfg(test)]
mod frame_tests {
    //! Specifies the frames: both kinds survive a round trip, and malformed ones are refused.

    use super::{FileTransferFrame, decode_frame, encode_chunk, encode_chunk_request};
    use crate::{domain::models::FileChunk, ports::file_transfer::FILE_TRANSFER_CHUNK_BYTES};

    #[test]
    fn frames_survive_the_round_trip() {
        let hash = "a".repeat(64);
        let chunk = FileChunk::new(hash.clone(), 9, 4, vec![1, 2, 3]);

        assert_eq!(
            decode_frame(&encode_chunk_request(&hash, 77)),
            Some(FileTransferFrame::ChunkRequest {
                hash: hash.clone(),
                offset: 77
            })
        );
        assert_eq!(
            decode_frame(&encode_chunk(&chunk)),
            Some(FileTransferFrame::Chunk(chunk))
        );
    }

    #[test]
    fn malformed_frames_are_refused() {
        let hash = "a".repeat(64);
        let mut request_with_extra_bytes = encode_chunk_request(&hash, 1);
        request_with_extra_bytes.push(0);
        let oversized = FileChunk::new(hash, 1, 0, vec![0; FILE_TRANSFER_CHUNK_BYTES + 1]);

        for frame in [
            Vec::new(),
            b"NTFQ".to_vec(),
            b"XXXX".repeat(30),
            request_with_extra_bytes,
            encode_chunk(&oversized),
        ] {
            assert_eq!(decode_frame(&frame), None);
        }
    }
}
