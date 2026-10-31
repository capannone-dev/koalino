//! Tests falling back from the cloud engine to the local one.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use super::cloud_fallback::CloudFallbackEngine;
use crate::{
    adapters::audio::chunk_audio_store::ChunkAudioStore,
    domain::{
        errors::TranscriptionError,
        models::{
            AudioRecording, DeviceId, MeetingId, RecordingId, Transcript, TranscriptionContext,
        },
    },
    ports::{ai::TranscriptionEngine, remote_transcription::ChunkSamples},
};

/// A cloud engine that consumes the chunk, as the real one does, and fails or answers.
struct Cloud {
    store: Arc<ChunkAudioStore>,
    error: Option<fn() -> TranscriptionError>,
}
impl TranscriptionEngine for Cloud {
    fn transcribe(
        &self,
        recording: &AudioRecording,
        _: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        self.store.discard(recording.recording_id());
        match self.error {
            Some(error) => Err(error()),
            None => Ok(Transcript::new(recording.meeting_id().clone(), "cloud")),
        }
    }
}

/// A local engine that needs the chunk to still be there.
struct Local(Arc<ChunkAudioStore>);
impl TranscriptionEngine for Local {
    fn transcribe(
        &self,
        recording: &AudioRecording,
        _: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        self.0
            .copy_samples(recording.recording_id())
            .ok_or(TranscriptionError::Unavailable)?;
        Ok(Transcript::new(recording.meeting_id().clone(), "local"))
    }
}

fn recording() -> AudioRecording {
    AudioRecording::new(
        RecordingId::new("chunk"),
        MeetingId::new("meeting"),
        DeviceId::new("device"),
    )
}

fn run(
    error: Option<fn() -> TranscriptionError>,
    loads: &'static AtomicUsize,
) -> Result<Transcript, TranscriptionError> {
    let store = Arc::new(ChunkAudioStore::new());
    store.insert(&RecordingId::new("chunk"), vec![1, 2, 3]);
    let loader_store = Arc::clone(&store);
    let engine = CloudFallbackEngine::new(
        Cloud {
            store: Arc::clone(&store),
            error,
        },
        Box::new(move || {
            loads.fetch_add(1, Ordering::Relaxed);
            Ok(Local(Arc::clone(&loader_store)))
        }),
        store,
    );
    engine.transcribe(&recording(), &TranscriptionContext::default())
}

/// Verifies a working cloud never loads the local model.
#[test]
fn a_working_cloud_never_loads_the_local_model() {
    static LOADS: AtomicUsize = AtomicUsize::new(0);
    let transcript = run(None, &LOADS).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(transcript.content(), "cloud");
    assert_eq!(LOADS.load(Ordering::Relaxed), 0);
}

/// Verifies no network means the same chunk is transcribed locally, and the app says so.
#[test]
fn no_network_transcribes_the_chunk_locally() {
    static LOADS: AtomicUsize = AtomicUsize::new(0);
    let transcript =
        run(Some(|| TranscriptionError::Unavailable), &LOADS).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(transcript.content(), "local");
    assert_eq!(LOADS.load(Ordering::Relaxed), 1);
}

/// Verifies a bad recording is not retried on the local model.
#[test]
fn an_invalid_recording_does_not_fall_back() {
    static LOADS: AtomicUsize = AtomicUsize::new(0);
    let result = run(
        Some(|| TranscriptionError::InvalidRecording {
            reason: "bad".to_owned(),
        }),
        &LOADS,
    );
    assert!(result.is_err());
    assert_eq!(LOADS.load(Ordering::Relaxed), 0);
}
