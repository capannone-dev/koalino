//! Defines immutable, strongly typed business values for meetings and recordings.

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
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TranscriptionModel {
    /// The node can execute a Whisper-compatible transcription workload.
    Whisper,
    /// The node can execute a named provider or future model integration.
    ThirdParty(String),
}

/// Advertises a node's non-sensitive AI capacity for decentralized workload selection.
///
/// A future encrypted P2P adapter broadcasts this portable value. It intentionally contains no
/// credentials, model weights, raw audio, or hardware serial numbers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceManifest {
    device_id: DeviceId,
    cpu_cores: u16,
    gpu_vram_megabytes: u32,
    available_transcription_models: Vec<TranscriptionModel>,
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
