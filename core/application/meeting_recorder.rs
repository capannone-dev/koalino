//! Implements the recording-processing use case without knowledge of AI or storage adapters.

use crate::{
    domain::{
        errors::{StorageError, TranscriptionError},
        models::{AudioRecording, Transcript},
    },
    ports::{ai::TranscriptionEngine, storage::NoteRepository},
};

/// Reports a failure while coordinating transcription and local-first persistence.
#[derive(Debug, thiserror::Error)]
pub enum MeetingProcessingError {
    /// The transcription port could not produce a transcript.
    #[error(transparent)]
    Transcription(#[from] TranscriptionError),
    /// The storage port could not persist a produced transcript.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// Coordinates an injected transcription engine with an injected note repository.
pub struct MeetingRecorder<TranscriptionPort, RepositoryPort> {
    transcription_engine: TranscriptionPort,
    note_repository: RepositoryPort,
}

impl<TranscriptionPort, RepositoryPort> MeetingRecorder<TranscriptionPort, RepositoryPort>
where
    TranscriptionPort: TranscriptionEngine,
    RepositoryPort: NoteRepository,
{
    /// Creates the recording-processing use case with concrete port implementations.
    ///
    /// Inputs: Both values must implement their respective ports; adapters are composed at an entry point.
    /// Outputs: A ready-to-use application service.
    /// Side effects: None.
    pub fn new(transcription_engine: TranscriptionPort, note_repository: RepositoryPort) -> Self {
        Self {
            transcription_engine,
            note_repository,
        }
    }

    /// Transcribes a recording and persists the resulting transcript through injected ports.
    ///
    /// Inputs: `audio_recording` must identify a recording accessible to the selected transcription adapter.
    /// Outputs: The persisted transcript or a transcription/storage failure.
    /// Side effects: Delegates AI processing and encrypted local persistence to adapters.
    pub fn process_recording(
        &self,
        audio_recording: &AudioRecording,
    ) -> Result<Transcript, MeetingProcessingError> {
        let generated_transcript = self.transcription_engine.transcribe(audio_recording)?;
        self.note_repository
            .save_transcript(&generated_transcript)?;
        Ok(generated_transcript)
    }
}
