//! Defines portable error types returned through AI, storage, and network ports.

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
