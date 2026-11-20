//! Lets a stopped after-meeting pass resume from the windows it finished (#87).
//!
//! A phone runs the pass from a background job that Android stops after 10 to 30 minutes, and a
//! pass that is stopped is run again from the start. This engine wraps the real one: before a
//! window is transcribed, it hashes the window's audio with its meeting, name, language and the
//! previous window's text, and returns the transcript kept under that hash in the meeting's
//! sealed progress file (`RetainedAudioStore::finished_window`); otherwise it transcribes and
//! keeps the result. As the previous text is part of the hash, a window is reused only after the
//! same chain of windows, so the result is the one an uninterrupted pass gives.

use sha2::{Digest, Sha256};

use crate::{
    adapters::audio::{
        chunk_audio_store::ChunkAudioStore, retained_audio_store::RetainedAudioStore,
    },
    domain::{
        errors::TranscriptionError,
        models::{AudioRecording, Transcript, TranscriptionContext},
    },
    ports::ai::TranscriptionEngine,
};

/// Wraps the pass's engine so finished windows survive a stop.
pub struct ResumablePassEngine<'pass> {
    inner: &'pass dyn TranscriptionEngine,
    audio: &'pass ChunkAudioStore,
    progress: &'pass RetainedAudioStore,
}

impl<'pass> ResumablePassEngine<'pass> {
    /// Creates the wrapper.
    ///
    /// Inputs: The real engine, the store it reads window audio from, and the retained-speech
    /// store holding the sealed progress. Outputs: The wrapper. Side effects: None.
    pub fn new(
        inner: &'pass dyn TranscriptionEngine,
        audio: &'pass ChunkAudioStore,
        progress: &'pass RetainedAudioStore,
    ) -> Self {
        Self {
            inner,
            audio,
            progress,
        }
    }

    /// Hashes what decides a window's transcript, or `None` when its audio is not held.
    fn window_key(
        &self,
        audio_recording: &AudioRecording,
        transcription_context: &TranscriptionContext,
    ) -> Option<[u8; 32]> {
        let audio_hash = self.audio.fingerprint(audio_recording.recording_id())?;
        let mut hasher = Sha256::new();
        for part in [
            audio_recording.meeting_id().as_str(),
            audio_recording.recording_id().as_str(),
            transcription_context.previous_text().unwrap_or(""),
            transcription_context
                .language()
                .map_or("", |language| language.as_str()),
        ] {
            hasher.update(u64::try_from(part.len()).unwrap_or(u64::MAX).to_le_bytes());
            hasher.update(part.as_bytes());
        }
        hasher.update(audio_hash);
        Some(hasher.finalize().into())
    }
}

impl TranscriptionEngine for ResumablePassEngine<'_> {
    /// Returns a finished window's transcript, or transcribes the window and keeps the result.
    ///
    /// Inputs: As the wrapped engine. Outputs: The transcript, or the wrapped engine's error.
    /// Side effects: A reused window's audio is wiped from the store, as reading it would; a new
    /// one is transcribed and appended, sealed, to the meeting's progress file. A failure to keep
    /// it is logged and only costs the work again.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        let meeting_id = audio_recording.meeting_id();
        let key = self.window_key(audio_recording, transcription_context);
        if let Some(key) = &key
            && let Some(finished) = self.progress.finished_window(meeting_id, key)
        {
            self.audio.discard(audio_recording.recording_id());
            return Ok(finished);
        }
        let transcript = self
            .inner
            .transcribe(audio_recording, transcription_context)?;
        if let Some(key) = &key
            && let Err(error) = self
                .progress
                .record_finished_window(meeting_id, key, &transcript)
        {
            tracing::warn!(meeting_id = %meeting_id.as_str(), error = %error, "a finished pass window could not be kept");
        }
        Ok(transcript)
    }
}
