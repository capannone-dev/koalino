//! Declares how a meeting's retained speech moves to the paired device that combines it (#88).
//!
//! When one meeting was recorded on two devices, the device that does not combine the recordings
//! sends its sealed speech (#87) to the one that does, in parts that each fit one P2P message. The
//! sender waits for each part's receipt before the next, and deletes its copy only after the last
//! receipt. Raw audio: never logged, wiped after use.

use zeroize::Zeroizing;

use crate::domain::{
    models::{DeviceId, MeetingId},
    transcription_language::TranscriptionLanguage,
};

/// One retained speech chunk in transit.
pub struct SpeechChunk {
    /// Start in its meeting's time, in milliseconds.
    pub start_ms: u64,
    /// End in its meeting's time; not before `start_ms`.
    pub end_ms: u64,
    /// The language it was recorded with, if any.
    pub language: Option<TranscriptionLanguage>,
    /// 16 kHz mono samples, wiped when dropped.
    pub samples: Zeroizing<Vec<i16>>,
}

/// One part of a meeting's speech.
pub struct SpeechPart {
    /// The meeting the speech belongs to.
    pub meeting_id: MeetingId,
    /// This part's position, from 0.
    pub part_index: u32,
    /// How many parts the meeting's speech was split into; at least 1.
    pub part_count: u32,
    /// The part's chunks in time order.
    pub chunks: Vec<SpeechChunk>,
}

/// Why a part was not delivered.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SpeechTransferError {
    /// The device is not connected, has no agreed pair key, or did not answer in time.
    #[error("the paired device is not reachable")]
    Unreachable,
    /// The device answered that it does not take this meeting's speech.
    #[error("the paired device refused the speech")]
    Refused,
    /// The part does not fit one message.
    #[error("the speech part is too large")]
    TooLarge,
}

/// Sends speech to a paired device.
pub trait SpeechSender: Send + Sync {
    /// Sends one part and waits for its receipt.
    ///
    /// Inputs: The device and the part. Outputs: Success once the device stored it, or why not.
    /// Side effects: Seals the audio for that device and sends it; blocks the calling thread.
    fn send_part(&self, to: &DeviceId, part: SpeechPart) -> Result<(), SpeechTransferError>;
}

/// Stores speech received from paired devices.
pub trait SpeechInbox: Send {
    /// Stores one received part.
    ///
    /// Inputs: The sending device and the part. Outputs: Whether it was stored (a part already
    /// stored counts as stored), or `Refused` for speech this device does not combine.
    /// Side effects: Writes the speech sealed under this device's key; a meeting's speech is
    /// available to the pass only once its last part is stored.
    fn receive_part(
        &mut self,
        from: &DeviceId,
        part: SpeechPart,
    ) -> Result<(), SpeechTransferError>;
}
