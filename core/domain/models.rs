//! Defines immutable, strongly typed business values for meetings and recordings.

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
