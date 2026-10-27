//! Defines immutable, strongly typed business values for meetings and recordings.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Describes the lifecycle of an audio-capture session without exposing a platform stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioState {
    /// The capture session has not started and accepts a start command.
    Idle,
    /// The capture session routes audio chunks to subscribers.
    Recording,
    /// The capture session is retained but temporarily discards captured chunks.
    Paused,
    /// The OS suspended the active stream; a later start command may safely recover it.
    Interrupted,
    /// The capture session is closed and cannot be restarted.
    Stopped,
    /// The capture session encountered an unrecoverable adapter failure.
    Error,
}

/// Defines the portable PCM format requested from an audio-capture adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioConfig {
    sample_rate_hz: u32,
    channels: u16,
}

impl AudioConfig {
    /// Creates a PCM capture configuration.
    ///
    /// Inputs: `sample_rate_hz` and `channels` describe the requested stream format; validation occurs
    /// when an adapter opens a stream.
    /// Outputs: A portable configuration that contains no operating-system device details.
    /// Side effects: None.
    pub fn new(sample_rate_hz: u32, channels: u16) -> Self {
        Self {
            sample_rate_hz,
            channels,
        }
    }

    /// Returns the requested number of PCM samples captured per second.
    ///
    /// Inputs: None.
    /// Outputs: The requested sample rate in hertz.
    /// Side effects: None.
    pub fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    /// Returns the requested number of interleaved PCM channels.
    ///
    /// Inputs: None.
    /// Outputs: The requested channel count.
    /// Side effects: None.
    pub fn channels(&self) -> u16 {
        self.channels
    }
}

/// Carries one adapter-produced signed 16-bit PCM chunk and its format metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioChunk {
    samples: Vec<i16>,
    config: AudioConfig,
}

/// Carries optional prior transcript text for one context-aware transcription request.
///
/// This portable value intentionally contains text only. Adapter-owned audio lookback handling and
/// chunk-boundary metadata remain separate concerns so the transcription port never exposes raw
/// audio outside the adapter boundary.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TranscriptionContext {
    previous_text: Option<String>,
}

impl TranscriptionContext {
    /// Creates context for one transcription request.
    ///
    /// Inputs: `previous_text` is the immediately preceding successful chunk transcript for the
    /// same meeting, when available. It must not contain raw audio or provider-specific values.
    /// Outputs: An immutable portable context for a transcription adapter.
    /// Side effects: Takes ownership of the optional transcript text.
    pub fn new(previous_text: Option<String>) -> Self {
        Self { previous_text }
    }

    /// Borrows the previous chunk transcript when context is available.
    ///
    /// Inputs: None.
    /// Outputs: The immediately preceding transcript text, or `None` for a meeting's first chunk.
    /// Side effects: None.
    pub fn previous_text(&self) -> Option<&str> {
        self.previous_text.as_deref()
    }
}

impl AudioChunk {
    /// Creates a portable signed 16-bit PCM chunk.
    ///
    /// Inputs: `samples` contains interleaved PCM values matching `config`.
    /// Outputs: An immutable chunk suitable for local processing or future P2P streaming adapters.
    /// Side effects: Takes ownership of the raw samples; callers must not log the returned value.
    pub fn new(samples: Vec<i16>, config: AudioConfig) -> Self {
        Self { samples, config }
    }

    /// Borrows the PCM samples without copying the raw audio content.
    ///
    /// Inputs: None.
    /// Outputs: The interleaved signed 16-bit PCM samples.
    /// Side effects: None.
    pub fn samples(&self) -> &[i16] {
        &self.samples
    }

    /// Returns the format metadata associated with this audio chunk.
    ///
    /// Inputs: None.
    /// Outputs: The chunk's portable PCM configuration.
    /// Side effects: None.
    pub fn config(&self) -> AudioConfig {
        self.config
    }
}

/// Identifies a meeting across offline replicas.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct MeetingId(String);

impl MeetingId {
    /// Creates a meeting identifier from an application-generated stable value.
    ///
    /// Inputs: `value` must be unique within the chosen identifier scheme.
    /// Outputs: A typed meeting identifier.
    /// Side effects: None.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrows the identifier text for adapter boundary metadata.
    ///
    /// Inputs: None.
    /// Outputs: The meeting identifier text.
    /// Side effects: None.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Represents the recorded time range used to associate externally acquired attachments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeetingWindow {
    meeting_id: MeetingId,
    meeting_start_time: DateTime<Utc>,
    meeting_end_time: DateTime<Utc>,
}

impl MeetingWindow {
    /// Creates a meeting window from recording lifecycle timestamps.
    ///
    /// Inputs: `meeting_id` identifies the meeting; timestamps must be UTC and end no earlier than start.
    /// Outputs: An immutable time window for repository matching.
    /// Side effects: None.
    pub fn new(
        meeting_id: MeetingId,
        meeting_start_time: DateTime<Utc>,
        meeting_end_time: DateTime<Utc>,
    ) -> Self {
        Self {
            meeting_id,
            meeting_start_time,
            meeting_end_time,
        }
    }

    /// Returns the meeting identifier owning this time window.
    ///
    /// Inputs: None.
    /// Outputs: A borrow of the stable meeting identifier.
    /// Side effects: None.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns the inclusive UTC start of this meeting window.
    ///
    /// Inputs: None.
    /// Outputs: The recorded meeting start timestamp.
    /// Side effects: None.
    pub fn meeting_start_time(&self) -> DateTime<Utc> {
        self.meeting_start_time
    }

    /// Returns the inclusive UTC end of this meeting window.
    ///
    /// Inputs: None.
    /// Outputs: The recorded meeting end timestamp.
    /// Side effects: None.
    pub fn meeting_end_time(&self) -> DateTime<Utc> {
        self.meeting_end_time
    }

    /// Reports whether a timestamp lies within this inclusive meeting time range.
    ///
    /// Inputs: `timestamp` is an attachment capture timestamp in UTC.
    /// Outputs: `true` when the timestamp is between the recorded start and end, inclusive.
    /// Side effects: None.
    pub fn contains(&self, timestamp: DateTime<Utc>) -> bool {
        self.meeting_start_time <= timestamp && timestamp <= self.meeting_end_time
    }
}

/// Describes a user-added file and its safe-to-persist acquisition metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attachment {
    attachment_id: Uuid,
    meeting_id: Option<MeetingId>,
    file_path: String,
    mime_type: String,
    original_timestamp: DateTime<Utc>,
    original_device_id: String,
}

/// Identifies one meeting attachment stored by SHA-256 content address rather than a mutable path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileReference {
    hash: String,
    size_bytes: u64,
    mime_type: String,
    meeting_id: MeetingId,
    filename: String,
}

impl FileReference {
    /// Creates immutable attachment-transfer metadata without retaining file contents.
    ///
    /// Inputs: `hash` is the lowercase SHA-256 content address; `size_bytes` is the exact file size.
    /// Outputs: A reference suitable for CRDT metadata or availability announcements.
    /// Side effects: Takes ownership of metadata only and never reads file content.
    pub fn new(
        hash: impl Into<String>,
        size_bytes: u64,
        mime_type: impl Into<String>,
        meeting_id: MeetingId,
        filename: impl Into<String>,
    ) -> Self {
        Self {
            hash: hash.into(),
            size_bytes,
            mime_type: mime_type.into(),
            meeting_id,
            filename: filename.into(),
        }
    }
    /// Borrows the expected SHA-256 content address.
    pub fn hash(&self) -> &str {
        &self.hash
    }
    /// Returns the exact expected byte count.
    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
    /// Borrows the media type declared for display and handling.
    pub fn mime_type(&self) -> &str {
        &self.mime_type
    }
    /// Borrows the meeting that authorizes this attachment.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }
    /// Borrows the original safe display filename.
    pub fn filename(&self) -> &str {
        &self.filename
    }
}

/// Requests an authorized content-addressed attachment from one authenticated portable peer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileTransferRequest {
    hash: String,
    requester_peer: DeviceId,
}

impl FileTransferRequest {
    /// Creates one request for an exact SHA-256 hash.
    pub fn new(hash: impl Into<String>, requester_peer: DeviceId) -> Self {
        Self {
            hash: hash.into(),
            requester_peer,
        }
    }
    /// Borrows the requested SHA-256 hash.
    pub fn hash(&self) -> &str {
        &self.hash
    }
    /// Borrows the authenticated requester identity.
    pub fn requester_peer(&self) -> &DeviceId {
        &self.requester_peer
    }
}

/// Carries one explicitly bounded binary file-transfer frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileChunk {
    hash: String,
    total_size: u64,
    chunk_size: u32,
    offset: u64,
    is_final: bool,
    data: Vec<u8>,
}

impl FileChunk {
    /// Creates an explicit-header transfer frame; validation is performed by the receiver.
    pub fn new(
        hash: impl Into<String>,
        total_size: u64,
        chunk_size: u32,
        offset: u64,
        is_final: bool,
        data: Vec<u8>,
    ) -> Self {
        Self {
            hash: hash.into(),
            total_size,
            chunk_size,
            offset,
            is_final,
            data,
        }
    }
    /// Borrows the expected content address.
    pub fn hash(&self) -> &str {
        &self.hash
    }
    /// Returns the announced complete file size.
    pub fn total_size(&self) -> u64 {
        self.total_size
    }
    /// Returns the declared maximum chunk size.
    pub fn chunk_size(&self) -> u32 {
        self.chunk_size
    }
    /// Returns this frame's zero-based byte offset.
    pub fn offset(&self) -> u64 {
        self.offset
    }
    /// Reports whether this explicitly marks the final frame.
    pub fn is_final(&self) -> bool {
        self.is_final
    }
    /// Borrows the file bytes for this frame.
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl Attachment {
    /// Creates an attachment record without reading the file or assigning it to a meeting.
    ///
    /// Inputs: `attachment_id` must be unique; `meeting_id` remains absent until user confirmation.
    /// Outputs: An immutable attachment suitable for an `AttachmentRepository`.
    /// Side effects: Takes ownership of metadata only; it does not access or log the file.
    pub fn new(
        attachment_id: Uuid,
        meeting_id: Option<MeetingId>,
        file_path: impl Into<String>,
        mime_type: impl Into<String>,
        original_timestamp: DateTime<Utc>,
        original_device_id: impl Into<String>,
    ) -> Self {
        Self {
            attachment_id,
            meeting_id,
            file_path: file_path.into(),
            mime_type: mime_type.into(),
            original_timestamp,
            original_device_id: original_device_id.into(),
        }
    }

    /// Returns the attachment's globally unique identifier.
    /// Inputs: None.
    /// Outputs: The attachment UUID.
    /// Side effects: None.
    pub fn attachment_id(&self) -> Uuid {
        self.attachment_id
    }

    /// Returns the assigned meeting, if the attachment has been confirmed or linked.
    /// Inputs: None.
    /// Outputs: An optional borrow of the owning meeting identifier.
    /// Side effects: None.
    pub fn meeting_id(&self) -> Option<&MeetingId> {
        self.meeting_id.as_ref()
    }

    /// Borrows the local path used by an adapter to locate this attachment.
    /// Inputs: None.
    /// Outputs: The caller-supplied local file path.
    /// Side effects: None.
    pub fn file_path(&self) -> &str {
        &self.file_path
    }

    /// Borrows the attachment MIME type.
    /// Inputs: None.
    /// Outputs: The declared MIME type.
    /// Side effects: None.
    pub fn mime_type(&self) -> &str {
        &self.mime_type
    }

    /// Returns the original UTC timestamp obtained through a metadata adapter.
    /// Inputs: None.
    /// Outputs: The attachment acquisition timestamp.
    /// Side effects: None.
    pub fn original_timestamp(&self) -> DateTime<Utc> {
        self.original_timestamp
    }

    /// Borrows the source device identifier recorded with this attachment.
    /// Inputs: None.
    /// Outputs: The source device identifier.
    /// Side effects: None.
    pub fn original_device_id(&self) -> &str {
        &self.original_device_id
    }
}

/// Ranks one meeting as a possible owner of an attachment.
#[derive(Clone, Debug, PartialEq)]
pub struct MeetingSuggestion {
    meeting_id: MeetingId,
    meeting_start_time: DateTime<Utc>,
    meeting_end_time: DateTime<Utc>,
    confidence_score: f32,
}

impl MeetingSuggestion {
    /// Creates a meeting suggestion with a normalized confidence score.
    ///
    /// Inputs: timestamps identify the matching window; score must be between zero and one.
    /// Outputs: An immutable suggestion for presentation or user confirmation.
    /// Side effects: None.
    pub fn new(
        meeting_id: MeetingId,
        meeting_start_time: DateTime<Utc>,
        meeting_end_time: DateTime<Utc>,
        confidence_score: f32,
    ) -> Self {
        Self {
            meeting_id,
            meeting_start_time,
            meeting_end_time,
            confidence_score,
        }
    }

    /// Returns the candidate meeting identifier.
    /// Inputs: None.
    /// Outputs: A borrow of the candidate meeting identifier.
    /// Side effects: None.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns the candidate meeting start timestamp.
    /// Inputs: None.
    /// Outputs: The UTC meeting start timestamp.
    /// Side effects: None.
    pub fn meeting_start_time(&self) -> DateTime<Utc> {
        self.meeting_start_time
    }

    /// Returns the candidate meeting end timestamp.
    /// Inputs: None.
    /// Outputs: The UTC meeting end timestamp.
    /// Side effects: None.
    pub fn meeting_end_time(&self) -> DateTime<Utc> {
        self.meeting_end_time
    }

    /// Returns the normalized time-window confidence score.
    /// Inputs: None.
    /// Outputs: A score from zero at an edge to one at the midpoint.
    /// Side effects: None.
    pub fn confidence_score(&self) -> f32 {
        self.confidence_score
    }
}

/// Identifies an audio recording belonging to a meeting.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RecordingId(String);

impl RecordingId {
    /// Creates a recording identifier from an application-generated stable value.
    ///
    /// Inputs: `value` must be unique within the chosen identifier scheme.
    /// Outputs: A typed recording identifier.
    /// Side effects: None.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

/// Identifies the node that acquired data or emitted an operational event.
#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DeviceId(String);

impl DeviceId {
    /// Creates a device identifier from the node's stable local identity.
    ///
    /// Inputs: `value` should be a non-secret, stable node identifier.
    /// Outputs: A typed device identifier.
    /// Side effects: None.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrows the stable identifier value for metadata fields.
    ///
    /// Inputs: None.
    /// Outputs: The identifier text.
    /// Side effects: None.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Carries one serialized Automerge sync-protocol message for a specific meeting document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncMessage {
    meeting_id: MeetingId,
    automerge_sync_bytes: Vec<u8>,
}

impl SyncMessage {
    /// Creates a sync envelope for one meeting document.
    ///
    /// Inputs: `meeting_id` identifies the target document and `automerge_sync_bytes` contains an
    /// Automerge sync message encoded by the application CRDT service.
    /// Outputs: A transport-neutral envelope ready for encrypted P2P transmission.
    /// Side effects: Takes ownership of protocol bytes without inspecting or logging note content.
    pub fn new(meeting_id: MeetingId, automerge_sync_bytes: Vec<u8>) -> Self {
        Self {
            meeting_id,
            automerge_sync_bytes,
        }
    }

    /// Returns the meeting document targeted by this protocol message.
    ///
    /// Inputs: None.
    /// Outputs: A borrow of the target meeting identifier.
    /// Side effects: None.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns the opaque Automerge sync-protocol bytes without copying them.
    ///
    /// Inputs: None.
    /// Outputs: The encoded Automerge sync message.
    /// Side effects: None.
    pub fn sync_bytes(&self) -> &[u8] {
        &self.automerge_sync_bytes
    }

    /// Serializes this envelope with a versioned, length-delimited binary framing.
    ///
    /// Inputs: None.
    /// Outputs: Bytes suitable only for the encrypted P2P transport, or a framing error.
    /// Side effects: Allocates an output buffer; it does not log or persist sync content.
    pub fn to_bytes(&self) -> Result<Vec<u8>, SyncMessageEncodeError> {
        const SYNC_ENVELOPE_HEADER: [u8; 4] = *b"NTS1";

        let meeting_id_bytes = self.meeting_id.as_str().as_bytes();
        let meeting_id_length = u32::try_from(meeting_id_bytes.len()).map_err(|_| {
            SyncMessageEncodeError::MeetingIdentifierTooLong {
                byte_length: meeting_id_bytes.len(),
            }
        })?;
        let mut serialized_message = Vec::with_capacity(
            SYNC_ENVELOPE_HEADER
                .len()
                .saturating_add(std::mem::size_of::<u32>())
                .saturating_add(meeting_id_bytes.len())
                .saturating_add(self.automerge_sync_bytes.len()),
        );
        serialized_message.extend_from_slice(&SYNC_ENVELOPE_HEADER);
        serialized_message.extend_from_slice(&meeting_id_length.to_be_bytes());
        serialized_message.extend_from_slice(meeting_id_bytes);
        serialized_message.extend_from_slice(&self.automerge_sync_bytes);
        Ok(serialized_message)
    }

    /// Deserializes a versioned sync envelope received through encrypted transport.
    ///
    /// Inputs: `serialized_message` must contain the framing produced by `to_bytes`.
    /// Outputs: A typed envelope or a validation error for malformed peer-controlled bytes.
    /// Side effects: Allocates only decoded metadata and payload bytes; it does not log content.
    pub fn from_bytes(serialized_message: &[u8]) -> Result<Self, SyncMessageDecodeError> {
        const SYNC_ENVELOPE_HEADER: [u8; 4] = *b"NTS1";
        let header_and_length_size = SYNC_ENVELOPE_HEADER
            .len()
            .saturating_add(std::mem::size_of::<u32>());
        if serialized_message.len() < header_and_length_size {
            return Err(SyncMessageDecodeError::InvalidEnvelope {
                reason: String::from("sync envelope is shorter than its header"),
            });
        }
        if serialized_message[..SYNC_ENVELOPE_HEADER.len()] != SYNC_ENVELOPE_HEADER {
            return Err(SyncMessageDecodeError::InvalidEnvelope {
                reason: String::from("sync envelope has an unknown protocol header"),
            });
        }

        let meeting_id_length_bytes: [u8; 4] = serialized_message
            [SYNC_ENVELOPE_HEADER.len()..header_and_length_size]
            .try_into()
            .map_err(|_| SyncMessageDecodeError::InvalidEnvelope {
                reason: String::from("sync envelope contains an invalid meeting identifier length"),
            })?;
        let meeting_id_length = usize::try_from(u32::from_be_bytes(meeting_id_length_bytes))
            .map_err(|_| SyncMessageDecodeError::InvalidEnvelope {
                reason: String::from("sync envelope meeting identifier length is unsupported"),
            })?;
        let meeting_id_end = header_and_length_size
            .checked_add(meeting_id_length)
            .ok_or_else(|| SyncMessageDecodeError::InvalidEnvelope {
                reason: String::from("sync envelope meeting identifier length overflows"),
            })?;
        if serialized_message.len() < meeting_id_end {
            return Err(SyncMessageDecodeError::InvalidEnvelope {
                reason: String::from("sync envelope is truncated before its meeting identifier"),
            });
        }
        let meeting_id =
            std::str::from_utf8(&serialized_message[header_and_length_size..meeting_id_end])
                .map_err(|_| SyncMessageDecodeError::InvalidEnvelope {
                    reason: String::from("sync envelope meeting identifier is not UTF-8"),
                })?;

        Ok(Self::new(
            MeetingId::new(meeting_id),
            serialized_message[meeting_id_end..].to_vec(),
        ))
    }
}

/// Describes malformed peer-controlled sync-envelope bytes.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SyncMessageDecodeError {
    /// The envelope does not satisfy the versioned binary framing contract.
    #[error("invalid sync message envelope: {reason}")]
    InvalidEnvelope {
        /// Explains the failed validation without including sync payload bytes.
        reason: String,
    },
}

/// Describes a local sync-envelope serialization failure.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SyncMessageEncodeError {
    /// The meeting identifier cannot be represented by the protocol's 32-bit length field.
    #[error("meeting identifier is too long for a sync message: {byte_length} bytes")]
    MeetingIdentifierTooLong {
        /// The unrepresentable UTF-8 identifier length.
        byte_length: usize,
    },
}

/// Identifies a transcription model that a device can execute without exposing its implementation.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TranscriptionModel {
    /// The node can execute a Whisper-compatible transcription workload.
    Whisper,
    /// The node can execute a named provider or future model integration.
    ThirdParty(String),
}

impl TranscriptionModel {
    /// Returns a stable non-sensitive model label for telemetry persistence.
    ///
    /// Inputs: None.
    /// Outputs: A borrowed label; provider names retain their configured text.
    /// Side effects: None.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Whisper => "whisper",
            Self::ThirdParty(model_name) => model_name,
        }
    }
}

/// Advertises a node's non-sensitive AI capacity for decentralized workload selection.
///
/// A future encrypted P2P adapter broadcasts this portable value. It intentionally contains no
/// credentials, model weights, raw audio, or hardware serial numbers.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DeviceManifest {
    device_id: DeviceId,
    cpu_cores: u16,
    gpu_vram_megabytes: u32,
    available_transcription_models: Vec<TranscriptionModel>,
    /// Reports whether this device can run feature-gated local VAD chunking.
    pub supports_vad: bool,
}

impl DeviceManifest {
    /// Creates a portable capability manifest for one decentralized node.
    ///
    /// Inputs: `device_id` identifies the node; CPU and GPU values describe available capacity;
    /// `available_transcription_models` lists workloads that the node can execute.
    /// Outputs: An immutable manifest suitable for encrypted P2P broadcast and routing.
    /// Side effects: Takes ownership of model capability metadata only; it performs no hardware scan.
    pub fn new(
        device_id: DeviceId,
        cpu_cores: u16,
        gpu_vram_megabytes: u32,
        available_transcription_models: Vec<TranscriptionModel>,
    ) -> Self {
        Self {
            device_id,
            cpu_cores,
            gpu_vram_megabytes,
            available_transcription_models,
            supports_vad: cfg!(feature = "vad"),
        }
    }

    /// Returns the stable identifier of the device that advertised this manifest.
    ///
    /// Inputs: None.
    /// Outputs: A borrow of the originating device identifier.
    /// Side effects: None.
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }

    /// Returns advertised GPU VRAM for telemetry's coarse CPU/GPU power heuristic.
    ///
    /// Inputs: None.
    /// Outputs: Megabytes of GPU VRAM advertised by the device manifest.
    /// Side effects: None.
    pub fn gpu_vram_megabytes(&self) -> u32 {
        self.gpu_vram_megabytes
    }

    /// Reports whether this node advertises support for a requested transcription model.
    ///
    /// Inputs: `transcription_model` is the model required by a workload.
    /// Outputs: `true` only when the model is present in this manifest.
    /// Side effects: None.
    pub fn supports_transcription_model(&self, transcription_model: &TranscriptionModel) -> bool {
        self.available_transcription_models
            .iter()
            .any(|available_model| available_model == transcription_model)
    }

    /// Produces a deterministic capacity score used only by the automatic routing policy.
    ///
    /// Inputs: None.
    /// Outputs: A score prioritizing GPU memory, then CPU cores, without identifying hardware.
    /// Side effects: None.
    pub fn transcription_capacity_score(&self) -> u64 {
        u64::from(self.gpu_vram_megabytes) * 1_000 + u64::from(self.cpu_cores)
    }
}

#[cfg(test)]
mod device_manifest_tests {
    //! Defines feature-sensitive serialization contracts for the device VAD capability flag.

    use super::{DeviceId, DeviceManifest, TranscriptionModel};

    fn manifest() -> DeviceManifest {
        DeviceManifest::new(
            DeviceId::new("vad-capability-test-device"),
            4,
            0,
            vec![TranscriptionModel::Whisper],
        )
    }

    #[test]
    #[cfg(feature = "vad")]
    fn smart_node_manifest_has_supports_vad_when_feature_enabled() {
        assert!(manifest().supports_vad);
    }

    #[test]
    #[cfg(not(feature = "vad"))]
    fn dumb_node_manifest_has_supports_vad_when_feature_disabled() {
        assert!(!manifest().supports_vad);
    }

    #[test]
    fn manifest_serialization_includes_supports_vad_field() {
        let serialized = serde_json::to_value(manifest())
            .unwrap_or_else(|error| panic!("manifest should serialize: {error}"));
        assert_eq!(
            serialized
                .get("supports_vad")
                .and_then(|value| value.as_bool()),
            Some(cfg!(feature = "vad"))
        );
        let round_tripped: DeviceManifest = serde_json::from_value(serialized)
            .unwrap_or_else(|error| panic!("manifest should deserialize: {error}"));
        assert_eq!(round_tripped.supports_vad, cfg!(feature = "vad"));
    }
}

/// Represents a captured recording without retaining raw audio content in the domain model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioRecording {
    recording_id: RecordingId,
    meeting_id: MeetingId,
    acquisition_device_id: DeviceId,
}

impl AudioRecording {
    /// Builds recording metadata for work passed to a transcription port.
    ///
    /// Inputs: Identifiers must refer to the captured recording, owning meeting, and acquiring node.
    /// Outputs: Immutable recording metadata; raw audio remains adapter-owned.
    /// Side effects: None.
    pub fn new(
        recording_id: RecordingId,
        meeting_id: MeetingId,
        acquisition_device_id: DeviceId,
    ) -> Self {
        Self {
            recording_id,
            meeting_id,
            acquisition_device_id,
        }
    }

    /// Returns the owning meeting identifier.
    ///
    /// Inputs: None.
    /// Outputs: A borrow of the meeting identifier.
    /// Side effects: None.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }
}

/// Represents text produced by a transcription engine for a meeting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transcript {
    meeting_id: MeetingId,
    content: String,
}

/// Categorizes AI work for privacy-safe telemetry grouping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskType {
    /// Converts a meeting recording into transcript text.
    Transcription,
    /// Separates speakers and derives segment embeddings.
    Diarization,
    /// Transfers a verified content-addressed attachment over encrypted P2P transport.
    FileTransfer,
    /// Executes transcription through a cloud API provider.
    CloudTranscription,
}

impl TaskType {
    /// Returns the stable storage label for this task category.
    ///
    /// Inputs: None.
    /// Outputs: A lowercase identifier safe for telemetry grouping.
    /// Side effects: None.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Transcription => "transcription",
            Self::Diarization => "diarization",
            Self::FileTransfer => "file_transfer",
            Self::CloudTranscription => "cloud_transcription",
        }
    }
}

/// Records one completed AI task without retaining inputs, outputs, or biometric data.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskMetrics {
    task_id: Uuid,
    device_id: DeviceId,
    task_type: TaskType,
    model_name: String,
    duration_ms: u64,
    latency_ms: u64,
    estimated_power_mwh: f32,
    timestamp_epoch_ms: u64,
}

impl TaskMetrics {
    /// Creates one completed-task telemetry record.
    ///
    /// Inputs: IDs and model label identify the execution; timing is milliseconds; power is an
    /// estimated milli-watt-hour value; timestamp is UTC Unix epoch milliseconds.
    /// Outputs: An immutable metrics value suitable for `MetricsRepository` persistence.
    /// Side effects: Takes ownership of non-sensitive model metadata; never records task payloads.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        task_id: Uuid,
        device_id: DeviceId,
        task_type: TaskType,
        model_name: impl Into<String>,
        duration_ms: u64,
        latency_ms: u64,
        estimated_power_mwh: f32,
        timestamp_epoch_ms: u64,
    ) -> Self {
        Self {
            task_id,
            device_id,
            task_type,
            model_name: model_name.into(),
            duration_ms,
            latency_ms,
            estimated_power_mwh,
            timestamp_epoch_ms,
        }
    }

    /// Returns the unique task execution ID.
    pub fn task_id(&self) -> Uuid {
        self.task_id
    }
    /// Returns the device that executed the AI task.
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }
    /// Returns the completed AI task category.
    pub fn task_type(&self) -> TaskType {
        self.task_type
    }
    /// Returns the model label without exposing task data.
    pub fn model_name(&self) -> &str {
        &self.model_name
    }
    /// Returns elapsed engine execution time in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }
    /// Returns observed end-to-end task latency in milliseconds.
    pub fn latency_ms(&self) -> u64 {
        self.latency_ms
    }
    /// Returns the heuristic energy estimate in milli-watt-hours.
    pub fn estimated_power_mwh(&self) -> f32 {
        self.estimated_power_mwh
    }
    /// Returns the UTC Unix epoch completion timestamp in milliseconds.
    pub fn timestamp_epoch_ms(&self) -> u64 {
        self.timestamp_epoch_ms
    }
}

/// Captures a device's non-sensitive instantaneous compute utilization.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceLoadStats {
    device_id: DeviceId,
    current_cpu_load_percent: f32,
    current_gpu_vram_used_megabytes: u32,
    active_tasks_count: u32,
}

impl DeviceLoadStats {
    /// Creates one device-load observation.
    ///
    /// Inputs: `current_cpu_load_percent` is normalized 0–100; VRAM and task count are snapshots.
    /// Outputs: An immutable load value for optional local telemetry persistence.
    /// Side effects: None.
    pub fn new(
        device_id: DeviceId,
        current_cpu_load_percent: f32,
        current_gpu_vram_used_megabytes: u32,
        active_tasks_count: u32,
    ) -> Self {
        Self {
            device_id,
            current_cpu_load_percent,
            current_gpu_vram_used_megabytes,
            active_tasks_count,
        }
    }
    /// Returns the observed device identifier.
    pub fn device_id(&self) -> &DeviceId {
        &self.device_id
    }
    /// Returns current CPU utilization as a percentage.
    pub fn current_cpu_load_percent(&self) -> f32 {
        self.current_cpu_load_percent
    }
    /// Returns current GPU VRAM use in megabytes.
    pub fn current_gpu_vram_used_megabytes(&self) -> u32 {
        self.current_gpu_vram_used_megabytes
    }
    /// Returns the number of active AI tasks.
    pub fn active_tasks_count(&self) -> u32 {
        self.active_tasks_count
    }
}

/// Summarizes persisted AI task metrics across devices and task categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AggregatedStats {
    task_count: u64,
    total_duration_ms: u64,
    average_latency_ms: u64,
}

impl AggregatedStats {
    /// Creates an aggregate summary returned by the telemetry repository.
    pub fn new(task_count: u64, total_duration_ms: u64, average_latency_ms: u64) -> Self {
        Self {
            task_count,
            total_duration_ms,
            average_latency_ms,
        }
    }
    /// Returns completed metric records included in the aggregate.
    pub fn task_count(&self) -> u64 {
        self.task_count
    }
    /// Returns cumulative AI processing time in milliseconds.
    pub fn total_duration_ms(&self) -> u64 {
        self.total_duration_ms
    }
    /// Returns integer-truncated average end-to-end latency in milliseconds.
    pub fn average_latency_ms(&self) -> u64 {
        self.average_latency_ms
    }
}

/// Identifies a person whose voice signatures may have multiple acoustic variants.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeakerIdentity {
    identity_id: Uuid,
    display_name: String,
}

impl SpeakerIdentity {
    /// Creates a speaker identity with an application-generated UUID and user-visible name.
    ///
    /// Inputs: `identity_id` must uniquely identify the person across replicas; `display_name` is
    /// a user-provided label and must not be treated as an authentication credential.
    /// Outputs: An immutable speaker identity without biometric data.
    /// Side effects: Takes ownership of the display name; it does not persist or log it.
    pub fn new(identity_id: Uuid, display_name: impl Into<String>) -> Self {
        Self {
            identity_id,
            display_name: display_name.into(),
        }
    }

    /// Returns the stable UUID associated with this person.
    ///
    /// Inputs: None.
    /// Outputs: The identity UUID by value.
    /// Side effects: None.
    pub fn identity_id(&self) -> Uuid {
        self.identity_id
    }

    /// Borrows the user-visible speaker label.
    ///
    /// Inputs: None.
    /// Outputs: The display name without copying it.
    /// Side effects: None.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
}

/// Identifies a participant supplied by an external meeting integration without provider coupling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalParticipant {
    external_id: String,
    display_name: String,
    email: Option<String>,
}

impl ExternalParticipant {
    /// Creates externally supplied identity metadata for stable local mapping and UI naming.
    ///
    /// Inputs: `external_id` must be provider-stable; `display_name` is the initial UI label;
    /// `email` is optional contact metadata and must not be logged by default.
    /// Outputs: An immutable external participant value with no authentication credential.
    /// Side effects: Takes ownership of identifier and display metadata only.
    pub fn new(
        external_id: impl Into<String>,
        display_name: impl Into<String>,
        email: Option<String>,
    ) -> Self {
        Self {
            external_id: external_id.into(),
            display_name: display_name.into(),
            email,
        }
    }

    /// Borrows the provider-stable identifier used for durable external-to-local mappings.
    ///
    /// Inputs: None.
    /// Outputs: A non-secret stable external identifier.
    /// Side effects: None.
    pub fn external_id(&self) -> &str {
        &self.external_id
    }

    /// Borrows the user-visible participant name used when auto-creating a local identity.
    ///
    /// Inputs: None.
    /// Outputs: The integration-provided display name.
    /// Side effects: None.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Borrows optional contact metadata without treating it as an identity key.
    ///
    /// Inputs: None.
    /// Outputs: An optional integration-provided email address.
    /// Side effects: None.
    pub fn email(&self) -> Option<&str> {
        self.email.as_deref()
    }
}

/// Carries an isolated external-speaker audio buffer with guaranteed participant metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalMediaChunk {
    external_participant: ExternalParticipant,
    audio_bytes: Vec<u8>,
    timestamp_ms: u64,
}

impl ExternalMediaChunk {
    /// Creates one opaque external audio chunk from a bridge subscription.
    ///
    /// Inputs: `external_participant` identifies the isolated speaker; `audio_bytes` must never be
    /// logged; `timestamp_ms` is relative to the external meeting stream.
    /// Outputs: An immutable bridge-neutral media value.
    /// Side effects: Takes ownership of raw audio bytes without persisting them.
    pub fn new(
        external_participant: ExternalParticipant,
        audio_bytes: Vec<u8>,
        timestamp_ms: u64,
    ) -> Self {
        Self {
            external_participant,
            audio_bytes,
            timestamp_ms,
        }
    }

    /// Borrows the guaranteed participant metadata associated with this isolated audio stream.
    ///
    /// Inputs: None.
    /// Outputs: The immutable external participant.
    /// Side effects: None.
    pub fn external_participant(&self) -> &ExternalParticipant {
        &self.external_participant
    }

    /// Borrows opaque audio bytes for approved AI ports only.
    ///
    /// Inputs: None.
    /// Outputs: Raw bridge-provided audio that callers must never log.
    /// Side effects: None.
    pub fn audio_bytes(&self) -> &[u8] {
        &self.audio_bytes
    }

    /// Returns the bridge timestamp for deterministic ordering and recording metadata.
    ///
    /// Inputs: None.
    /// Outputs: Milliseconds relative to the bridge meeting stream.
    /// Side effects: None.
    pub fn timestamp_ms(&self) -> u64 {
        self.timestamp_ms
    }
}

/// Carries an external screen-share frame for attachment workflow processing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlideCapture {
    meeting_id: Uuid,
    image_bytes: Vec<u8>,
    timestamp_ms: u64,
    source: String,
}

impl SlideCapture {
    /// Creates one opaque external slide frame associated with a meeting UUID.
    ///
    /// Inputs: `meeting_id` identifies the external meeting; `image_bytes` are sensitive capture
    /// data; `timestamp_ms` orders the capture; `source` identifies the adapter-neutral origin.
    /// Outputs: An immutable slide-capture value.
    /// Side effects: Takes ownership of image bytes without logging or persisting them.
    pub fn new(
        meeting_id: Uuid,
        image_bytes: Vec<u8>,
        timestamp_ms: u64,
        source: impl Into<String>,
    ) -> Self {
        Self {
            meeting_id,
            image_bytes,
            timestamp_ms,
            source: source.into(),
        }
    }

    /// Returns the source meeting UUID from the external bridge.
    ///
    /// Inputs: None.
    /// Outputs: The meeting UUID.
    /// Side effects: None.
    pub fn meeting_id(&self) -> Uuid {
        self.meeting_id
    }

    /// Borrows image bytes for the attachment workflow without copying them.
    ///
    /// Inputs: None.
    /// Outputs: Opaque frame bytes that callers must never log.
    /// Side effects: None.
    pub fn image_bytes(&self) -> &[u8] {
        &self.image_bytes
    }

    /// Returns the source-relative capture timestamp.
    ///
    /// Inputs: None.
    /// Outputs: Milliseconds relative to the external meeting stream.
    /// Side effects: None.
    pub fn timestamp_ms(&self) -> u64 {
        self.timestamp_ms
    }

    /// Borrows the adapter-neutral source label.
    ///
    /// Inputs: None.
    /// Outputs: The source label supplied by the bridge adapter.
    /// Side effects: None.
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// Carries one derived voice-signature variant without retaining raw audio.
#[derive(Clone, Debug, PartialEq)]
pub struct VoiceEmbedding {
    identity_id: Uuid,
    variant_id: Uuid,
    vector: Vec<f32>,
}

impl VoiceEmbedding {
    /// Creates a derived biometric signature variant for an existing speaker identity.
    ///
    /// Inputs: `identity_id` owns the variant; `variant_id` uniquely identifies this acoustic
    /// environment; `vector` contains model-derived numeric features and never raw audio.
    /// Outputs: An immutable embedding value suitable only for the voice-vector storage port.
    /// Side effects: Takes ownership of the biometric vector; callers must not log the value.
    pub fn new(identity_id: Uuid, variant_id: Uuid, vector: Vec<f32>) -> Self {
        Self {
            identity_id,
            variant_id,
            vector,
        }
    }

    /// Returns the UUID of the identity that owns this signature variant.
    ///
    /// Inputs: None.
    /// Outputs: The owning identity UUID by value.
    /// Side effects: None.
    pub fn identity_id(&self) -> Uuid {
        self.identity_id
    }

    /// Returns the UUID that distinguishes this acoustic signature from sibling variants.
    ///
    /// Inputs: None.
    /// Outputs: The variant UUID by value.
    /// Side effects: None.
    pub fn variant_id(&self) -> Uuid {
        self.variant_id
    }

    /// Borrows the model-derived vector without copying sensitive biometric values.
    ///
    /// Inputs: None.
    /// Outputs: The derived embedding dimensions.
    /// Side effects: None.
    pub fn vector(&self) -> &[f32] {
        &self.vector
    }
}

/// Reports the confidence category returned after a voice-vector similarity search.
#[derive(Clone, Debug, PartialEq)]
pub enum VoiceMatchResult {
    /// A high-confidence candidate can be attributed without user confirmation.
    StrongMatch(Uuid),
    /// A plausible candidate requires user confirmation and reports its adapter-provided score.
    WeakMatch(Uuid, f32),
    /// No candidate is sufficiently similar to attribute safely.
    NoMatch,
}

/// Retains the candidate metadata needed to safely confirm a deferred weak match.
#[derive(Clone, Debug, PartialEq)]
pub struct WeakVoiceMatchCandidate {
    identity_id: Uuid,
    confidence_score: f32,
}

impl WeakVoiceMatchCandidate {
    /// Creates a weak candidate returned by the existing identity-match policy.
    ///
    /// Inputs: `identity_id` identifies the candidate and `confidence_score` is the adapter score.
    /// Outputs: Immutable metadata that contains no embedding or raw audio.
    /// Side effects: None.
    pub fn new(identity_id: Uuid, confidence_score: f32) -> Self {
        Self {
            identity_id,
            confidence_score,
        }
    }

    /// Returns the candidate identity UUID.
    ///
    /// Inputs: None.
    /// Outputs: The suggested identity by value.
    /// Side effects: None.
    pub fn identity_id(&self) -> Uuid {
        self.identity_id
    }

    /// Returns the adapter-provided weak-match confidence.
    ///
    /// Inputs: None.
    /// Outputs: The score in the matching adapter's normalized range.
    /// Side effects: None.
    pub fn confidence_score(&self) -> f32 {
        self.confidence_score
    }
}

/// Retains a temporary, reviewable audio payload for a diarized segment without exposing storage details.
#[derive(Clone, Debug, PartialEq)]
pub struct UnassignedVoiceClip {
    clip_id: Uuid,
    meeting_id: MeetingId,
    start_time_ms: u64,
    end_time_ms: u64,
    audio_bytes: Vec<u8>,
    derived_embedding: Vec<f32>,
    weak_match_candidate: Option<WeakVoiceMatchCandidate>,
}

impl UnassignedVoiceClip {
    /// Creates a temporary review clip for a segment that could not be attributed automatically.
    ///
    /// Inputs: `clip_id` uniquely identifies the review item; `meeting_id` owns it; the time range
    /// identifies the segment inside `audio_bytes`, which is retained only until the user resolves it.
    /// Outputs: An immutable clip value suitable for persistence through `NoteRepository`.
    /// Side effects: Takes ownership of raw audio bytes; callers must not log the returned value.
    pub fn new(
        clip_id: Uuid,
        meeting_id: MeetingId,
        start_time_ms: u64,
        end_time_ms: u64,
        audio_bytes: Vec<u8>,
    ) -> Self {
        Self {
            clip_id,
            meeting_id,
            start_time_ms,
            end_time_ms,
            audio_bytes,
            derived_embedding: Vec::new(),
            weak_match_candidate: None,
        }
    }

    /// Creates a review clip retaining derived confirmation context from diarization.
    ///
    /// Inputs: `derived_embedding` is a model feature vector and `weak_match_candidate` is the
    /// optional policy result; neither value may be logged or exposed through FFI.
    /// Outputs: A clip that permits later application-level assignment or confirmation.
    /// Side effects: Takes ownership of temporary audio and sensitive derived embedding data.
    pub fn with_review_context(
        clip_id: Uuid,
        meeting_id: MeetingId,
        start_time_ms: u64,
        end_time_ms: u64,
        audio_bytes: Vec<u8>,
        derived_embedding: Vec<f32>,
        weak_match_candidate: Option<WeakVoiceMatchCandidate>,
    ) -> Self {
        Self {
            clip_id,
            meeting_id,
            start_time_ms,
            end_time_ms,
            audio_bytes,
            derived_embedding,
            weak_match_candidate,
        }
    }

    /// Returns the UUID assigned to this temporary manual-review item.
    ///
    /// Inputs: None.
    /// Outputs: The globally unique clip identifier.
    /// Side effects: None.
    pub fn clip_id(&self) -> Uuid {
        self.clip_id
    }

    /// Returns the meeting that owns this temporary review clip.
    ///
    /// Inputs: None.
    /// Outputs: A borrow of the owning meeting identifier.
    /// Side effects: None.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns the segment start timestamp relative to the original recording.
    ///
    /// Inputs: None.
    /// Outputs: Milliseconds from the start of the processed audio.
    /// Side effects: None.
    pub fn start_time_ms(&self) -> u64 {
        self.start_time_ms
    }

    /// Returns the segment end timestamp relative to the original recording.
    ///
    /// Inputs: None.
    /// Outputs: Milliseconds from the start of the processed audio.
    /// Side effects: None.
    pub fn end_time_ms(&self) -> u64 {
        self.end_time_ms
    }

    /// Borrows the temporarily retained source audio without copying it.
    ///
    /// Inputs: None.
    /// Outputs: Opaque raw audio bytes for a storage adapter or manual-review playback workflow.
    /// Side effects: None; callers must not log the returned bytes.
    pub fn audio_bytes(&self) -> &[u8] {
        &self.audio_bytes
    }

    /// Borrows the derived biometric embedding retained solely for user-confirmed assignment.
    ///
    /// Inputs: None.
    /// Outputs: Model-derived dimensions or an empty slice for legacy clips.
    /// Side effects: None; callers must never log this biometric value.
    pub fn derived_embedding(&self) -> &[f32] {
        &self.derived_embedding
    }

    /// Borrows weak-match candidate metadata, if diarization produced a candidate.
    ///
    /// Inputs: None.
    /// Outputs: Candidate UUID and score without biometric vector data.
    /// Side effects: None.
    pub fn weak_match_candidate(&self) -> Option<&WeakVoiceMatchCandidate> {
        self.weak_match_candidate.as_ref()
    }
}

impl Transcript {
    /// Creates transcript content associated with its owning meeting.
    ///
    /// Inputs: `content` is normalized text returned by an AI adapter.
    /// Outputs: An immutable transcript ready for persistence.
    /// Side effects: None.
    pub fn new(meeting_id: MeetingId, content: impl Into<String>) -> Self {
        Self {
            meeting_id,
            content: content.into(),
        }
    }

    /// Returns the meeting that owns this transcript.
    ///
    /// Inputs: None.
    /// Outputs: A borrow of the meeting identifier.
    /// Side effects: None.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns the transcript's normalized text.
    ///
    /// Inputs: None.
    /// Outputs: A borrow of transcript content.
    /// Side effects: None.
    pub fn content(&self) -> &str {
        &self.content
    }
}

/// Represents one persisted, time-bounded transcript interval with optional speaker attribution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptSegment {
    meeting_id: MeetingId,
    start_time_ms: u64,
    end_time_ms: u64,
    /// `None` when diarization could not attribute the interval to a known identity.
    speaker_identity_id: Option<Uuid>,
    text: String,
}

impl TranscriptSegment {
    /// Creates a transcript segment owned by one meeting.
    ///
    /// Inputs: `start_time_ms`/`end_time_ms` are offsets from the meeting start (end should not
    /// precede start); `speaker_identity_id` references a `SpeakerIdentity` when attributed.
    /// Outputs: An immutable segment ready for persistence or export.
    /// Side effects: None.
    pub fn new(
        meeting_id: MeetingId,
        start_time_ms: u64,
        end_time_ms: u64,
        speaker_identity_id: Option<Uuid>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            meeting_id,
            start_time_ms,
            end_time_ms,
            speaker_identity_id,
            text: text.into(),
        }
    }

    /// Borrows the owning meeting identifier.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns the start offset from the meeting start in milliseconds.
    pub fn start_time_ms(&self) -> u64 {
        self.start_time_ms
    }

    /// Returns the end offset from the meeting start in milliseconds.
    pub fn end_time_ms(&self) -> u64 {
        self.end_time_ms
    }

    /// Returns the attributed speaker identity, if diarization produced a strong match.
    pub fn speaker_identity_id(&self) -> Option<Uuid> {
        self.speaker_identity_id
    }

    /// Borrows the segment's transcript text.
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// Describes repository-level facts about a meeting that are independent of its content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeetingMetadata {
    meeting_id: MeetingId,
    created_at: DateTime<Utc>,
}

impl MeetingMetadata {
    /// Creates meeting metadata from persisted repository values.
    ///
    /// Inputs: `meeting_id` identifies the meeting; `created_at` is its UTC creation instant.
    /// Outputs: Immutable metadata.
    /// Side effects: None.
    pub fn new(meeting_id: MeetingId, created_at: DateTime<Utc>) -> Self {
        Self {
            meeting_id,
            created_at,
        }
    }

    /// Borrows the meeting identifier.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns the UTC instant at which the meeting record was created.
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
}
