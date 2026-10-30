//! Specifies the host: it runs jobs off the caller's thread, passes context, deletes the audio
//! whether the engine reads it or not, and reports a missing engine as a failed job.

use std::{
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use anyhow::anyhow;
use uuid::Uuid;

use super::ThreadedTranscriptionHost;
use crate::{
    adapters::audio::chunk_audio_store::ChunkAudioStore,
    domain::{
        errors::TranscriptionError,
        models::{AudioRecording, DeviceId, Transcript, TranscriptionContext},
        transcription_language::TranscriptionLanguage,
    },
    ports::{
        ai::TranscriptionEngine,
        remote_transcription::{FinishedTranscription, HostedTranscription, TranscriptionHost},
    },
};

/// Answers with the context it got and never reads the audio, the worst case for deletion.
struct EchoEngine;

impl TranscriptionEngine for EchoEngine {
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        Ok(Transcript::new(
            audio_recording.meeting_id().clone(),
            context
                .language()
                .map(|language| language.as_str())
                .or(context.previous_text())
                .unwrap_or("no context"),
        ))
    }
}

/// A job of two samples.
fn job(previous_text: Option<&str>) -> HostedTranscription {
    HostedTranscription {
        peer: DeviceId::new("phone"),
        job_id: Uuid::from_u128(5),
        samples: vec![1, 2],
        previous_text: previous_text.map(str::to_owned),
        language: None,
    }
}

/// Waits for a job to finish, for up to five seconds.
fn wait_finished(host: &mut ThreadedTranscriptionHost) -> FinishedTranscription {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        if let Some(finished) = host.poll_finished() {
            return finished;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the job should finish");
}

#[test]
fn a_job_is_transcribed_and_its_audio_deleted() {
    let seen_store: Arc<Mutex<Option<Arc<ChunkAudioStore>>>> = Arc::new(Mutex::new(None));
    let captured = Arc::clone(&seen_store);
    let mut host = ThreadedTranscriptionHost::spawn(move |store| {
        *captured.lock().unwrap_or_else(PoisonError::into_inner) = Some(Arc::clone(store));
        Ok(EchoEngine)
    })
    .unwrap_or_else(|error| panic!("the host must start: {error:#}"));

    assert!(host.submit(job(Some("earlier words"))));
    let finished = wait_finished(&mut host);

    assert_eq!(finished.peer, DeviceId::new("phone"));
    assert_eq!(finished.job_id, Uuid::from_u128(5));
    assert_eq!(finished.text.as_deref(), Some("earlier words"));
    let store = seen_store
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .unwrap_or_else(|| panic!("the engine must have been built"));
    assert!(store.is_empty());
}

#[test]
fn an_engine_that_cannot_be_built_fails_the_job_and_is_tried_again() {
    let attempts = Arc::new(Mutex::new(0));
    let counted = Arc::clone(&attempts);
    let mut host = ThreadedTranscriptionHost::spawn(move |_| -> anyhow::Result<EchoEngine> {
        let mut attempts = counted.lock().unwrap_or_else(PoisonError::into_inner);
        *attempts += 1;
        if *attempts == 1 {
            Err(anyhow!("no model"))
        } else {
            Ok(EchoEngine)
        }
    })
    .unwrap_or_else(|error| panic!("the host must start: {error:#}"));

    assert!(host.submit(job(None)));
    assert_eq!(wait_finished(&mut host).text, None);
    assert!(host.submit(job(None)));
    assert_eq!(wait_finished(&mut host).text.as_deref(), Some("no context"));
}

/// A hosted chunk is transcribed in the language the asking device chose.
#[test]
fn a_hosted_job_uses_the_requested_language() {
    let mut host = ThreadedTranscriptionHost::spawn(|_| Ok(EchoEngine))
        .unwrap_or_else(|error| panic!("the host must start: {error:#}"));
    assert!(host.submit(HostedTranscription {
        language: TranscriptionLanguage::parse("it").ok(),
        ..job(None)
    }));

    assert_eq!(wait_finished(&mut host).text.as_deref(), Some("it"));
}
