//! Transcribes with the owner's glossary of names and terms, around any transcription engine.
//!
//! `GlossaryTranscription` adds the glossary to each request's context (after any earlier text,
//! see `TranscriptionContext::prompt`) and removes an echo of it from the transcript, the known
//! failure of prompting Whisper. Live recordings do the same inside `MeetingSession`; this wrapper
//! serves the after-meeting pass, whose windows already carry the previous window's text. The
//! glossary is user content and is never logged.

use crate::{
    domain::{
        errors::TranscriptionError,
        models::{AudioRecording, Transcript, TranscriptionContext},
        transcription_glossary::TranscriptionGlossary,
    },
    ports::ai::TranscriptionEngine,
};

/// An engine that transcribes with a glossary.
pub struct GlossaryTranscription<Engine> {
    inner: Engine,
    /// `None` passes requests through unchanged.
    glossary: Option<TranscriptionGlossary>,
}

impl<Engine> GlossaryTranscription<Engine> {
    /// Wraps an engine.
    ///
    /// Inputs: The engine and the owner's glossary (`None` or empty for none).
    /// Outputs: The wrapper. Side effects: None.
    pub fn new(inner: Engine, glossary: Option<TranscriptionGlossary>) -> Self {
        Self {
            inner,
            glossary: glossary.filter(|glossary| !glossary.is_empty()),
        }
    }
}

impl<Engine> TranscriptionEngine for GlossaryTranscription<Engine>
where
    Engine: TranscriptionEngine,
{
    /// Transcribes with the glossary added to the request's context.
    ///
    /// Inputs: As `TranscriptionEngine::transcribe`. Outputs: The inner engine's transcript
    /// without an echo of the glossary, or its error. Side effects: Those of the inner engine.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        let Some(glossary) = &self.glossary else {
            return self
                .inner
                .transcribe(audio_recording, transcription_context);
        };
        let context = transcription_context
            .clone()
            .with_glossary(Some(glossary.clone()));
        let transcript = self.inner.transcribe(audio_recording, &context)?;
        Ok(glossary.without_echo(transcript))
    }
}
