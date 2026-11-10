//! Defines portable error types returned through AI, audio, storage, and network ports.

use crate::domain::models::{AudioState, ExportFormat};

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
    /// The adapter refused audio exceeding the configured provider payload limit.
    #[error("transcription payload exceeds the provider limit of {maximum_bytes} bytes")]
    PayloadTooLarge { maximum_bytes: usize },
    /// The selected provider does not offer the requested transcription capability.
    #[error("selected transcription provider is unsupported")]
    ProviderUnsupported,
    /// The cloud provider rate limit was exhausted after bounded retries.
    #[error("cloud transcription rate limit exhausted")]
    RateLimited,
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

/// Describes why user-supplied text is not a valid meeting title.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum MeetingTitleError {
    /// The title is empty or contains only whitespace.
    #[error("meeting title must not be blank")]
    Blank,
    /// The trimmed title is longer than the allowed number of characters.
    #[error("meeting title must be at most {max_characters} characters")]
    TooLong {
        /// The maximum number of Unicode scalar values.
        max_characters: usize,
    },
    /// The title contains a control character such as a newline or terminal escape.
    #[error("meeting title must not contain control characters")]
    ControlCharacter,
}

/// Describes why user-supplied text is not a valid paired-device name.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DeviceNameError {
    /// The name is empty or contains only whitespace.
    #[error("device name must not be blank")]
    Blank,
    /// The trimmed name is longer than the allowed number of characters.
    #[error("device name must be at most {max_characters} characters")]
    TooLong {
        /// The maximum number of Unicode scalar values.
        max_characters: usize,
    },
    /// The name contains a control character such as a newline or terminal escape.
    #[error("device name must not contain control characters")]
    ControlCharacter,
}

/// Describes a meeting export that could not be selected, rendered, or written.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ExportError {
    /// The requested format name is not recognized by any interface.
    #[error("unknown export format `{value}`; expected markdown, pdf, or json")]
    UnknownFormat {
        /// The rejected user-supplied value.
        value: String,
    },
    /// The format is recognized but reserved for a future release.
    #[error("{format} export is not supported yet")]
    UnsupportedFormat {
        /// The reserved format.
        format: ExportFormat,
    },
    /// A batch date range ends before it starts or cannot be parsed.
    #[error("export date range is invalid; expected an inclusive start that is not after its end")]
    InvalidDateRange,
    /// The requested meeting is not present in the local repository.
    #[error("meeting {meeting_id} does not exist")]
    MeetingNotFound {
        /// The missing meeting identifier.
        meeting_id: String,
    },
    /// A batch selection matched no meetings, so there is nothing to export.
    #[error("no meetings matched the export selection")]
    NoMeetingsSelected,
    /// A filesystem operation on the export artifact or its staged attachments failed.
    #[error("export file operation failed: {reason}")]
    Io {
        /// A safe description that contains no meeting text.
        reason: String,
    },
    /// A renderer could not produce a valid document.
    #[error("export rendering failed: {reason}")]
    Rendering {
        /// A safe description that contains no meeting text.
        reason: String,
    },
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

/// Describes an at-rest encryption, decryption, or key-management failure.
///
/// Messages never contain key material, passphrases, plaintext embeddings, or ciphertext.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CryptoError {
    /// The stored value is too short, has an unknown format header, or decrypts to a length that
    /// is not a whole number of `f32` values.
    #[error("sealed embedding is malformed")]
    MalformedCiphertext,
    /// The authentication tag did not verify: the key is wrong, the data was tampered with, or the
    /// value belongs to another row.
    #[error("sealed embedding failed authentication")]
    AuthenticationFailed,
    /// The cipher refused to encrypt the embedding.
    #[error("embedding could not be encrypted")]
    EncryptionFailed,
    /// Argon2id parameters were invalid or derivation failed.
    #[error("encryption key derivation failed: {reason}")]
    KeyDerivation {
        /// A reason that contains no passphrase or key material.
        reason: String,
    },
    /// The random key file could not be located, created, read, or validated.
    #[error("encryption key file error: {reason}")]
    KeyFile {
        /// A reason that names paths but never key bytes.
        reason: String,
    },
    /// The platform keychain does not exist on this machine (for example headless Linux without a
    /// Secret Service). Unlocking fails with it; there is no key-file fallback.
    #[error("platform keychain unavailable: {reason}")]
    KeychainUnavailable {
        /// A reason from the platform that contains no key material.
        reason: String,
    },
    /// The platform keychain refused or failed an operation, or holds an entry that is not a
    /// valid key.
    #[error("platform keychain error: {reason}")]
    Keychain {
        /// A reason naming the keychain entry but never key bytes.
        reason: String,
    },
    /// An encrypted database's random key is in neither the keychain nor the key file.
    #[error("encryption key not found: {reason}")]
    KeyMissing {
        /// Names every place that was searched and how to recover.
        reason: String,
    },
    /// The configured key does not decrypt this database's key-check value.
    #[error("the configured encryption key does not match this database")]
    WrongKey,
    /// A paired device's X25519 public key has low order, so agreeing a pair key with it would
    /// yield a value an attacker can predict (Issue #50).
    #[error("the paired device's agreement key is unusable")]
    KeyAgreement,
    /// The database was set up with a different key source than the one configured now.
    #[error("this database is encrypted with a {stored}, but a {configured} is configured")]
    KeySourceMismatch {
        /// The key source recorded in the database (`passphrase` or `key file`).
        stored: &'static str,
        /// The key source selected by the current configuration.
        configured: &'static str,
    },
    /// Encryption metadata could not be read or written.
    #[error("encryption metadata is unavailable: {reason}")]
    Metadata {
        /// A reason that contains no key material.
        reason: String,
    },
}

/// Describes why local screen-share detection or capture could not complete (Issue #49).
///
/// Messages never contain screen contents.
#[derive(Debug, thiserror::Error)]
pub enum ScreenShareError {
    /// No adapter can detect or capture screen shares on this device.
    #[error("screen-share capture is unavailable on this device")]
    Unavailable,
    /// The adapter returned a frame that violates the frame contract.
    #[error("screen frame is invalid: {reason}")]
    InvalidFrame {
        /// A reason that contains no screen contents.
        reason: String,
    },
    /// The adapter could not detect a share or grab the screen.
    #[error("screen capture failed: {reason}")]
    CaptureFailed {
        /// A reason that contains no screen contents.
        reason: String,
    },
    /// The slide could not be stored on the meeting.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// Why a language model gave no answer. A provider's error body is never kept whole, since it
/// can echo meeting content; only the short reason of a rejection is, and only outside the
/// error's text, so logs (which use the text) never hold it.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LlmError {
    /// The server could not be reached or did not answer in time.
    #[error("the language model could not be reached")]
    Unreachable,
    /// The server refused the API key.
    #[error("the language model refused the API key")]
    Unauthorized,
    /// The server asked to slow down.
    #[error("the language model is busy or over its limit; try again later")]
    RateLimited,
    /// The server rejected the request, for example an unknown model.
    #[error("the language model rejected the request (HTTP {status})")]
    Rejected {
        /// The HTTP status.
        status: u16,
        /// The server's own reason, such as `invalid_request_error: Your credit balance is too
        /// low`: one line of at most 300 characters, shown to the owner and never logged.
        provider_reason: Option<String>,
    },
    /// The server failed.
    #[error("the language model failed (HTTP {status})")]
    ServerFailed {
        /// The HTTP status.
        status: u16,
    },
    /// The model declined to answer.
    #[error("the language model declined to answer")]
    Refused,
    /// The answer could not be read, or was empty.
    #[error("the language model's answer could not be read")]
    InvalidResponse,
    /// The connection needs an API key and has none.
    #[error("this connection needs an API key")]
    MissingKey,
    /// The command-line tool a connection runs (#105) is not installed on this computer.
    #[error("{tool} is not installed on this computer")]
    ToolMissing {
        /// The tool's name, such as `Claude Code`.
        tool: &'static str,
    },
    /// The command-line tool ran but gave no answer, for example when it is signed out or over
    /// its plan's limit (#105).
    #[error("the AI tool on this computer could not answer")]
    ToolFailed {
        /// What the tool said, one line of at most 300 characters; shown to the owner, never
        /// logged.
        reason: Option<String>,
    },
    /// The request cannot be sent as built.
    #[error("the request to the language model is invalid: {reason}")]
    InvalidRequest {
        /// What is wrong, free of meeting content.
        reason: &'static str,
    },
}
