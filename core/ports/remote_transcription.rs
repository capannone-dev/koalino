//! Declares what routing transcription to a paired device needs (Issue #59): the host that runs
//! other devices' jobs, the stored routing choice, and access to a chunk's samples that does not
//! consume them, so a failed remote attempt can still fall back to the local engine.

use uuid::Uuid;

use crate::domain::{
    errors::StorageError,
    models::{DeviceId, RecordingId},
    transcription_language::TranscriptionLanguage,
};

/// One transcription a paired device asked this device to run.
pub struct HostedTranscription {
    /// The device asking.
    pub peer: DeviceId,
    /// Identifies the job on that device.
    pub job_id: Uuid,
    /// 16 kHz mono samples. Raw audio: never logged, and gone once the job ends.
    pub samples: Vec<i16>,
    /// The previous chunk's text, as transcription context.
    pub previous_text: Option<String>,
    /// The language the asking device chose; `None` uses this device's configuration.
    pub language: Option<TranscriptionLanguage>,
}

/// A hosted job's result.
pub struct FinishedTranscription {
    /// The device that asked.
    pub peer: DeviceId,
    /// Identifies the job on that device.
    pub job_id: Uuid,
    /// The text, or `None` when this device could not transcribe it.
    pub text: Option<String>,
}

/// Runs other devices' transcriptions away from the P2P thread.
pub trait TranscriptionHost {
    /// Queues a job without waiting.
    ///
    /// Outputs: `false` when the queue is full, and the job's audio is dropped.
    /// Side effects: Holds the audio until the job ends.
    fn submit(&mut self, job: HostedTranscription) -> bool;

    /// Takes one finished job, if any. Side effects: Removes it from the queue.
    fn poll_finished(&mut self) -> Option<FinishedTranscription>;
}

/// Keeps the owner's routing choice across restarts.
pub trait TranscriptionRouteStore {
    /// Reads the stored choice's text. Outputs: The text, `None` if never set, or an error.
    fn load_transcription_route(&self) -> Result<Option<String>, StorageError>;

    /// Stores the choice's text. Side effects: Writes one row.
    fn save_transcription_route(&self, route: &str) -> Result<(), StorageError>;
}

/// Reads and deletes a recorded chunk's samples.
pub trait ChunkSamples {
    /// Copies a chunk's samples, leaving them stored.
    ///
    /// Outputs: The samples, or `None` for an unknown chunk. Side effects: None.
    fn copy_samples(&self, recording_id: &RecordingId) -> Option<Vec<i16>>;

    /// Deletes and wipes a chunk's samples. Side effects: Removes them from the store.
    fn delete_samples(&self, recording_id: &RecordingId);
}
