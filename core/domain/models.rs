//! Defines immutable, strongly typed business values for meetings and recordings.

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
