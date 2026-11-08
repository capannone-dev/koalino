//! Specifies the engine wrapper that transcribes with the owner's glossary: every request is
//! prompted with it after any earlier text, and an echo of it is removed from the result.

use std::sync::Mutex;

use super::glossary_transcription::GlossaryTranscription;
use crate::{
    domain::{
        errors::TranscriptionError,
        models::{
            AudioRecording, DeviceId, MeetingId, RecordingId, Transcript, TranscriptionContext,
        },
        transcription_glossary::TranscriptionGlossary,
    },
    ports::ai::TranscriptionEngine,
};

/// Answers with fixed text and remembers each prompt it was given.
struct Echoing {
    text: &'static str,
    prompts: Mutex<Vec<Option<String>>>,
}

impl TranscriptionEngine for Echoing {
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        self.prompts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(transcription_context.prompt());
        Ok(Transcript::new(
            audio_recording.meeting_id().clone(),
            self.text,
        ))
    }
}

fn recording() -> AudioRecording {
    AudioRecording::new(
        RecordingId::new("window-1"),
        MeetingId::new("meeting"),
        DeviceId::new("device"),
    )
}

#[test]
fn requests_carry_the_glossary_and_lose_its_echo() {
    let inner = Echoing {
        text: "Marco Rossi, OKR. Let's start.",
        prompts: Mutex::new(Vec::new()),
    };
    let glossary = TranscriptionGlossary::parse(["Marco Rossi", "OKR"])
        .unwrap_or_else(|error| panic!("{error}"));
    let engine = GlossaryTranscription::new(&inner, Some(glossary));

    let transcript = engine
        .transcribe(
            &recording(),
            &TranscriptionContext::new(Some("Earlier window.".to_owned())),
        )
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(transcript.content(), "Let's start.");
    assert_eq!(
        *inner
            .prompts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        [Some("Earlier window. Marco Rossi, OKR.".to_owned())]
    );
}

#[test]
fn without_a_glossary_requests_pass_through_unchanged() {
    let inner = Echoing {
        text: "Marco Rossi, OKR.",
        prompts: Mutex::new(Vec::new()),
    };
    let engine = GlossaryTranscription::new(&inner, None);

    let transcript = engine
        .transcribe(&recording(), &TranscriptionContext::new(None))
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(transcript.content(), "Marco Rossi, OKR.");
    assert_eq!(
        *inner
            .prompts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        [None]
    );
}
