//! Defines portable error types returned through AI, audio, storage, and network ports.

use crate::domain::models::AudioState;

/// Identifies a lifecycle command rejected by an audio-capture state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioAction {
    /// Starts an idle recorder or resumes a paused recorder.
    Start,
    /// Pauses an active recorder while retaining its session.
    Pause,
    /// Records an operating-system suspension of an active audio stream.
    Interrupt,
    /// Closes an active or paused recorder permanently.
    Stop,
}

/// Describes an audio-capture operation that could not complete.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AudioError {
    /// The operating system interrupted an active audio stream; the session can be resumed.
    #[error("audio stream was interrupted by the operating system")]
    Interrupted,
    /// The requested lifecycle command is disallowed from the current state.
    #[error("cannot {action:?} audio capture while it is {current_state:?}")]
    InvalidState {
        /// The lifecycle command that the adapter rejected.
        action: AudioAction,
        /// The state from which the rejected command was issued.
        current_state: AudioState,
    },
    /// The requested format cannot be opened by the capture adapter.
    #[error("audio configuration is invalid: {reason}")]
    InvalidConfiguration {
        /// A privacy-safe explanation that contains no raw audio data.
        reason: String,
    },
    /// A subscriber cannot receive capture events because its channel is unavailable.
    #[error("audio subscriber is unavailable")]
    SubscriberUnavailable,
    /// The underlying capture device or stream reported an unrecoverable failure.
    #[error("audio capture is unavailable: {reason}")]
    Unavailable {
        /// A privacy-safe explanation of the adapter or device failure.
        reason: String,
    },
}

/// Describes a transcription operation that could not produce a transcript.
#[derive(Debug, thiserror::Error)]
pub enum TranscriptionError {
    /// The configured engine is unavailable on this device or selected peer.
    #[error("transcription engine is unavailable")]
    Unavailable,
    /// The adapter rejected the recording before inference started.
    #[error("recording cannot be transcribed: {reason}")]
    InvalidRecording { reason: String },
}

/// Describes a diarization operation that could not produce speaker segments.
#[derive(Debug, thiserror::Error)]
pub enum DiarizationError {
    /// The configured engine is unavailable on this device or selected peer.
    #[error("diarization engine is unavailable")]
    Unavailable,
    /// The adapter rejected the supplied audio before inference started.
    #[error("audio cannot be diarized: {reason}")]
    InvalidAudio {
        /// A privacy-safe reason that never contains raw audio or embeddings.
        reason: String,
    },
}

/// Describes an adapter-neutral external meeting bridge operation that could not complete.
#[derive(Debug, thiserror::Error)]
pub enum ExternalBridgeError {
    /// The configured bridge cannot currently subscribe to or deliver meeting data.
    #[error("external meeting bridge is unavailable")]
    Unavailable,
    /// The bridge rejected media or metadata that fails portable validation.
    #[error("external meeting data is invalid: {reason}")]
    InvalidData {
        /// A privacy-safe reason that excludes raw media and credentials.
        reason: String,
    },
}

/// Describes a persistence operation that could not complete.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// The configured repository cannot currently accept writes.
    #[error("note repository is unavailable")]
    Unavailable,
    /// The repository rejected a value because it violates its storage invariant.
    #[error("repository rejected data: {reason}")]
    Rejected { reason: String },
}

/// Describes a P2P operation that could not be completed without central routing.
#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    /// No encrypted peer connection is currently available.
    #[error("no encrypted peer connection is available")]
    Disconnected,
    /// The adapter refused to send data that did not meet its security policy.
    #[error("network security policy rejected the operation: {reason}")]
    SecurityPolicy { reason: String },
}

/// Describes a rejected attachment transfer without retaining file bytes.
#[derive(Debug, thiserror::Error)]
pub enum FileTransferError {
    /// The transfer metadata or stream violates the bounded protocol contract.
    #[error("invalid file transfer: {reason}")]
    Invalid { reason: String },
    /// The requested or received object is not authorized for its meeting.
    #[error("file transfer authorization denied")]
    Unauthorized,
    /// A network operation failed while queuing or serving transfer data.
    #[error(transparent)]
    Network(#[from] NetworkError),
    /// Content-addressed local persistence failed.
    #[error(transparent)]
    Storage(#[from] StorageError),
}
