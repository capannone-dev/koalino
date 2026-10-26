//! Defines behavioral contracts for context propagation through compute dispatching.

use std::{cell::Cell, rc::Rc};

use crate::{
    application::compute_router::{ComputeDispatcher, ComputePreference},
    domain::{
        errors::TranscriptionError,
        models::{
            AudioRecording, DeviceId, DeviceManifest, MeetingId, RecordingId, Transcript,
            TranscriptionContext, TranscriptionModel,
        },
    },
    ports::ai::TranscriptionEngine,
};

/// Captures context received for each deterministic mock transcription call.
struct ContextCapturingTranscriptionEngine {
    invocation_count: Cell<usize>,
    received_previous_texts: Rc<std::cell::RefCell<Vec<Option<String>>>>,
}

impl TranscriptionEngine for ContextCapturingTranscriptionEngine {
    /// Records portable context and returns a distinct transcript for each invocation.
    ///
    /// Inputs: `audio_recording` supplies the meeting identity; `transcription_context` is the
    /// dispatcher-provided prior transcript context.
    /// Outputs: A deterministic next transcript or no error.
    /// Side effects: Appends context to in-memory test state only.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        self.received_previous_texts
            .borrow_mut()
            .push(transcription_context.previous_text().map(str::to_owned));
        let transcript_text = if self.invocation_count.get() == 0 {
            "first chunk transcript"
        } else {
            "second chunk transcript"
        };
        self.invocation_count
            .set(self.invocation_count.get().saturating_add(1));
        Ok(Transcript::new(
            audio_recording.meeting_id().clone(),
            transcript_text,
        ))
    }
}

/// Builds portable metadata for context-propagation dispatches.
fn context_recording() -> AudioRecording {
    AudioRecording::new(
        RecordingId::new("context-recording"),
        MeetingId::new("context-meeting"),
        DeviceId::new("capture-device"),
    )
}

/// Builds a local Whisper-capable manifest without accessing real hardware.
fn context_manifest() -> DeviceManifest {
    DeviceManifest::new(
        DeviceId::new("context-device"),
        8,
        0,
        vec![TranscriptionModel::Whisper],
    )
}

/// Verifies sequential dispatches send the prior transcript as the next chunk's context.
///
/// Inputs: One local dispatcher and two chunks from the same meeting.
/// Outputs: Confirms the first invocation receives no previous text and the second receives the
/// first chunk's transcript.
/// Side effects: Invokes only an in-memory mock engine; it neither performs AI work nor records
/// telemetry.
#[test]
fn compute_dispatcher_carries_previous_chunk_transcript_to_next_chunk_context() {
    let received_previous_texts = Rc::new(std::cell::RefCell::new(Vec::new()));
    let dispatcher = ComputeDispatcher::new(
        context_manifest(),
        ContextCapturingTranscriptionEngine {
            invocation_count: Cell::new(0),
            received_previous_texts: Rc::clone(&received_previous_texts),
        },
    );
    let audio_recording = context_recording();

    dispatcher
        .dispatch_transcription(
            &audio_recording,
            TranscriptionModel::Whisper,
            &ComputePreference::LocalOnly,
        )
        .unwrap_or_else(|error| panic!("first context dispatch should succeed: {error}"));
    dispatcher
        .dispatch_transcription(
            &audio_recording,
            TranscriptionModel::Whisper,
            &ComputePreference::LocalOnly,
        )
        .unwrap_or_else(|error| panic!("second context dispatch should succeed: {error}"));

    assert_eq!(
        received_previous_texts.borrow().as_slice(),
        [None, Some("first chunk transcript".to_owned())]
    );
}
