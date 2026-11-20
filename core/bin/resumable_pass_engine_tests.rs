//! Specifies how a stopped after-meeting pass resumes from the windows it finished (#87).

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::{
    adapters::audio::{
        chunk_audio_store::ChunkAudioStore, retained_audio_store::RetainedAudioStore,
    },
    application::crypto::voice_encryptor::ephemeral_cipher,
    domain::{
        errors::TranscriptionError,
        models::{
            AudioRecording, DeviceId, MeetingId, RecordingId, TimedWord, Transcript,
            TranscriptionContext,
        },
    },
    ports::ai::TranscriptionEngine,
    resumable_pass_engine::ResumablePassEngine,
};

/// Counts its calls and consumes the window's audio, as the real engine does.
struct CountingEngine {
    calls: AtomicUsize,
    audio: Arc<ChunkAudioStore>,
}

impl TranscriptionEngine for CountingEngine {
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        _transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert!(
            self.audio.discard(audio_recording.recording_id()),
            "the engine reads the window's audio"
        );
        Ok(
            Transcript::new(audio_recording.meeting_id().clone(), "parola")
                .with_timed_words(vec![TimedWord::new("parola", 0, 300)]),
        )
    }
}

fn folder() -> PathBuf {
    let path = std::env::temp_dir().join(format!("nt-resumable-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&path).unwrap_or_else(|error| panic!("{error}"));
    path
}

fn window(name: &str) -> AudioRecording {
    AudioRecording::new(
        RecordingId::new(name),
        MeetingId::new("meeting-1"),
        DeviceId::new("phone"),
    )
}

#[test]
fn a_window_finished_before_a_stop_is_not_transcribed_again() {
    let path = folder();
    let store = RetainedAudioStore::new(path.clone(), ephemeral_cipher());
    let audio = Arc::new(ChunkAudioStore::new());
    let inner = CountingEngine {
        calls: AtomicUsize::new(0),
        audio: Arc::clone(&audio),
    };
    let engine = ResumablePassEngine::new(&inner, &audio, &store);
    let context = TranscriptionContext::new(None);

    audio.insert(&RecordingId::new("refine-m-0"), vec![5; 1_600]);
    let first = engine
        .transcribe(&window("refine-m-0"), &context)
        .unwrap_or_else(|error| panic!("{error}"));
    // The job stopped; the next run feeds the same window again.
    audio.insert(&RecordingId::new("refine-m-0"), vec![5; 1_600]);
    let again = engine
        .transcribe(&window("refine-m-0"), &context)
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(first, again);
    assert_eq!(inner.calls.load(Ordering::SeqCst), 1);
    assert!(audio.is_empty(), "the reused window's audio is wiped too");

    // Other audio, or the same audio after other text, is transcribed.
    audio.insert(&RecordingId::new("refine-m-0"), vec![6; 1_600]);
    engine
        .transcribe(&window("refine-m-0"), &context)
        .unwrap_or_else(|error| panic!("{error}"));
    audio.insert(&RecordingId::new("refine-m-0"), vec![5; 1_600]);
    engine
        .transcribe(
            &window("refine-m-0"),
            &TranscriptionContext::new(Some("prima".to_owned())),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(inner.calls.load(Ordering::SeqCst), 3);
    let _ = std::fs::remove_dir_all(&path);
}
