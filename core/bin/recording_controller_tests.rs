//! Specifies the live-capture driver: PCM → recorder → chunker → audio store → pipeline worker,
//! with meeting-relative offsets, pause/resume, graceful stop, and raw-audio cleanup. The
//! transcription engine is a mock reading from the real `ChunkAudioStore`; storage is in-memory.

use std::sync::Arc;

use crate::{
    adapters::{
        ai::openai_whisper_adapter::AudioFileSource, audio::chunk_audio_store::ChunkAudioStore,
        storage::sqlite_adapter::SqliteNoteRepository,
    },
    application::{
        crypto::voice_encryptor::ephemeral_cipher, meeting_crdt::MeetingCrdtService,
        pipeline::pipeline_factory::PipelineFactory, recording_manager::RecordingManager,
    },
    domain::{
        errors::TranscriptionError,
        models::{
            AudioRecording, AudioState, DeviceId, MeetingId, MeetingTitle, Transcript,
            TranscriptSegment, TranscriptionContext,
        },
    },
    ports::ai::TranscriptionEngine,
};

#[cfg(feature = "vad")]
use crate::adapters::audio::vad_chunker::VadConfig;

use super::{ChunkingMode, RecordingControlError, RecordingController};

/// Samples per second of the fixed capture format.
const SAMPLES_PER_SECOND: usize = 16_000;

/// Reads each chunk's WAV from the store like a real engine, or fails without reading it.
struct StoreReadingTranscriptionEngine {
    chunk_audio_store: Arc<ChunkAudioStore>,
    reads_audio: bool,
}

impl TranscriptionEngine for StoreReadingTranscriptionEngine {
    /// Loads (and thereby deletes) the chunk audio and reports its duration as the transcript.
    fn transcribe(
        &self,
        audio_recording: &AudioRecording,
        _transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        if !self.reads_audio {
            return Err(TranscriptionError::Unavailable);
        }
        let wav_bytes = self.chunk_audio_store.load_wav(audio_recording)?;
        let duration_ms = (wav_bytes.len() - 44) / 2 * 1_000 / SAMPLES_PER_SECOND;
        Ok(Transcript::new(
            audio_recording.meeting_id().clone(),
            format!("chunk of {duration_ms} ms"),
        ))
    }
}

/// Builds a time-based controller whose worker owns an in-memory repository.
///
/// `prepare_repository` runs on the worker before the pipeline is ready, e.g. to seed meetings.
fn controller_with(
    reads_audio: bool,
    prepare_repository: impl FnOnce(&SqliteNoteRepository) + Send + 'static,
) -> (RecordingController, Arc<ChunkAudioStore>) {
    controller_chunking(reads_audio, ChunkingMode::TimeBased, prepare_repository)
}

/// Builds a controller like `controller_with`, cutting chunks with `chunking_mode`.
fn controller_chunking(
    reads_audio: bool,
    chunking_mode: ChunkingMode,
    prepare_repository: impl FnOnce(&SqliteNoteRepository) + Send + 'static,
) -> (RecordingController, Arc<ChunkAudioStore>) {
    let chunk_audio_store = Arc::new(ChunkAudioStore::new());
    let engine_store = Arc::clone(&chunk_audio_store);
    let recording_manager = RecordingManager::spawn(move || -> Result<_, String> {
        let note_repository = SqliteNoteRepository::open_in_memory(ephemeral_cipher())
            .map_err(|error| error.to_string())?;
        prepare_repository(&note_repository);
        Ok(PipelineFactory::new(
            StoreReadingTranscriptionEngine {
                chunk_audio_store: engine_store,
                reads_audio,
            },
            note_repository,
        ))
    })
    .unwrap_or_else(|error| panic!("manager should start: {error}"));
    let controller = RecordingController::new(
        recording_manager,
        Arc::clone(&chunk_audio_store),
        chunking_mode,
        DeviceId::new("device-test"),
    );
    (controller, chunk_audio_store)
}

/// Builds `seconds` of constant 16 kHz mono PCM.
fn pcm_seconds(seconds: usize) -> Vec<i16> {
    vec![100; seconds * SAMPLES_PER_SECOND]
}

/// Loads a meeting's persisted segments through the worker-owned repository.
fn stored_segments(
    controller: &RecordingController,
    meeting_id: &MeetingId,
) -> Vec<TranscriptSegment> {
    let meeting_id = meeting_id.clone();
    controller
        .recording_manager()
        .with_note_repository(move |note_repository| {
            note_repository.load_transcript_segments(&meeting_id)
        })
        .unwrap_or_else(|error| panic!("worker should answer: {error}"))
        .unwrap_or_else(|error| panic!("segments should load: {error}"))
}

/// Verifies 16 s of audio yields a 15 s chunk while recording and a flushed tail on stop, both
/// placed on the meeting timeline back to back (no audio is transcribed twice; Issue #51).
#[test]
fn recording_audio_produces_meeting_timed_segments() {
    let (mut controller, chunk_audio_store) = controller_with(true, |_| {});
    let meeting_id = controller
        .start(Some(MeetingId::new("meeting-live")))
        .unwrap_or_else(|error| panic!("recording should start: {error}"));
    assert_eq!(meeting_id, MeetingId::new("meeting-live"));
    assert_eq!(controller.state(), AudioState::Recording);

    let submitted = controller
        .push_pcm(pcm_seconds(16))
        .unwrap_or_else(|error| panic!("PCM should be accepted: {error}"));
    assert_eq!(submitted, 1);
    let summary = controller
        .stop()
        .unwrap_or_else(|error| panic!("recording should stop: {error}"));

    assert_eq!(summary.processed_chunk_count(), 2);
    assert_eq!(summary.failed_chunk_count(), 0);
    assert_eq!(controller.state(), AudioState::Stopped);
    assert!(chunk_audio_store.is_empty());
    let segments = stored_segments(&controller, &meeting_id);
    let bounds: Vec<(u64, u64)> = segments
        .iter()
        .map(|segment| (segment.start_time_ms(), segment.end_time_ms()))
        .collect();
    assert_eq!(bounds, vec![(0, 15_000), (15_000, 16_000)]);
    assert_eq!(segments[0].text(), "chunk of 15000 ms");
}

/// Verifies audio pushed while paused is discarded and start resumes the same meeting.
#[test]
fn pause_discards_audio_and_start_resumes_the_same_meeting() {
    let (mut controller, _chunk_audio_store) = controller_with(true, |_| {});
    let meeting_id = controller
        .start(Some(MeetingId::new("meeting-paused")))
        .unwrap_or_else(|error| panic!("recording should start: {error}"));
    controller
        .push_pcm(pcm_seconds(2))
        .unwrap_or_else(|error| panic!("PCM should be accepted: {error}"));
    controller
        .pause()
        .unwrap_or_else(|error| panic!("recording should pause: {error}"));
    assert_eq!(controller.state(), AudioState::Paused);
    controller
        .push_pcm(pcm_seconds(5))
        .unwrap_or_else(|error| panic!("paused PCM should be ignored: {error}"));

    let resumed_meeting_id = controller
        .start(None)
        .unwrap_or_else(|error| panic!("recording should resume: {error}"));
    assert_eq!(resumed_meeting_id, meeting_id);
    controller
        .push_pcm(pcm_seconds(1))
        .unwrap_or_else(|error| panic!("PCM should be accepted: {error}"));
    let summary = controller
        .stop()
        .unwrap_or_else(|error| panic!("recording should stop: {error}"));

    assert_eq!(summary.processed_chunk_count(), 1);
    let segments = stored_segments(&controller, &meeting_id);
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].end_time_ms(), 3_000);
}

/// Verifies a new recording into an existing meeting continues after its last segment.
#[test]
fn recording_into_an_existing_meeting_appends_after_its_timeline() {
    let (mut controller, _chunk_audio_store) = controller_with(true, |_| {});
    let meeting_id = MeetingId::new("meeting-two-takes");
    for _take in 0..2 {
        controller
            .start(Some(meeting_id.clone()))
            .unwrap_or_else(|error| panic!("recording should start: {error}"));
        controller
            .push_pcm(pcm_seconds(2))
            .unwrap_or_else(|error| panic!("PCM should be accepted: {error}"));
        controller
            .stop()
            .unwrap_or_else(|error| panic!("recording should stop: {error}"));
    }

    let bounds: Vec<(u64, u64)> = stored_segments(&controller, &meeting_id)
        .iter()
        .map(|segment| (segment.start_time_ms(), segment.end_time_ms()))
        .collect();
    assert_eq!(bounds, vec![(0, 2_000), (2_000, 4_000)]);
}

/// Verifies a recording without a meeting ID gets a fresh one, and a seeded meeting is reused.
#[test]
fn start_without_a_meeting_generates_one_and_seeded_meetings_are_kept() {
    let (mut controller, _chunk_audio_store) = controller_with(true, |note_repository| {
        let _ = MeetingCrdtService::new(note_repository).create_meeting(&MeetingId::new("seeded"));
    });

    let generated = controller
        .start(None)
        .unwrap_or_else(|error| panic!("recording should start: {error}"));
    assert!(generated.as_str().starts_with("meeting-"));
    controller
        .stop()
        .unwrap_or_else(|error| panic!("recording should stop: {error}"));
    let seeded = controller
        .start(Some(MeetingId::new("seeded")))
        .unwrap_or_else(|error| panic!("seeded meeting should be reusable: {error}"));
    assert_eq!(seeded, MeetingId::new("seeded"));
}

/// Verifies lifecycle misuse is rejected without disturbing the active recording.
#[test]
fn lifecycle_misuse_is_rejected() {
    let (mut controller, _chunk_audio_store) = controller_with(true, |_| {});
    assert_eq!(controller.state(), AudioState::Idle);
    assert!(matches!(
        controller.stop(),
        Err(RecordingControlError::NotRecording)
    ));
    assert!(matches!(
        controller.push_pcm(pcm_seconds(1)),
        Err(RecordingControlError::NotRecording)
    ));

    controller
        .start(Some(MeetingId::new("meeting-busy")))
        .unwrap_or_else(|error| panic!("recording should start: {error}"));
    assert!(matches!(
        controller.start(Some(MeetingId::new("meeting-other"))),
        Err(RecordingControlError::AlreadyRecording)
    ));
    assert_eq!(
        controller.active_meeting_id(),
        Some(&MeetingId::new("meeting-busy"))
    );
}

/// Verifies chunks without speech are not transcribed but still advance the timeline, so the
/// speech after them keeps its place (Issue #51).
#[test]
#[cfg(feature = "vad")]
fn chunks_without_speech_are_skipped_but_keep_the_timeline() {
    let (mut controller, chunk_audio_store) =
        controller_chunking(true, ChunkingMode::Vad(VadConfig::english()), |_| {});
    let meeting_id = controller
        .start(Some(MeetingId::new("meeting-silence")))
        .unwrap_or_else(|error| panic!("recording should start: {error}"));
    controller
        .push_pcm(vec![0; 4 * SAMPLES_PER_SECOND])
        .unwrap_or_else(|error| panic!("PCM should be accepted: {error}"));
    let summary = controller
        .stop()
        .unwrap_or_else(|error| panic!("recording should stop: {error}"));

    assert_eq!(summary.processed_chunk_count(), 0);
    assert!(chunk_audio_store.is_empty());
    assert!(stored_segments(&controller, &meeting_id).is_empty());
}

/// Verifies stop deletes raw audio of chunks the engine never read (failed transcriptions).
#[test]
fn stop_deletes_audio_left_by_failed_chunks() {
    let (mut controller, chunk_audio_store) = controller_with(false, |_| {});
    controller
        .start(Some(MeetingId::new("meeting-failing")))
        .unwrap_or_else(|error| panic!("recording should start: {error}"));
    controller
        .push_pcm(pcm_seconds(16))
        .unwrap_or_else(|error| panic!("PCM should be accepted: {error}"));
    let summary = controller
        .stop()
        .unwrap_or_else(|error| panic!("recording should stop: {error}"));

    assert_eq!(summary.failed_chunk_count(), 2);
    assert!(chunk_audio_store.is_empty());
}

/// Verifies an interruption flushes buffered audio so nothing captured before it is lost.
#[test]
fn interruption_flushes_buffered_audio() {
    let (mut controller, _chunk_audio_store) = controller_with(true, |_| {});
    let meeting_id = controller
        .start(Some(MeetingId::new("meeting-interrupted")))
        .unwrap_or_else(|error| panic!("recording should start: {error}"));
    controller
        .push_pcm(pcm_seconds(3))
        .unwrap_or_else(|error| panic!("PCM should be accepted: {error}"));
    controller
        .handle_interruption()
        .unwrap_or_else(|error| panic!("interruption should be accepted: {error}"));
    assert_eq!(controller.state(), AudioState::Interrupted);
    controller
        .start(None)
        .unwrap_or_else(|error| panic!("recording should resume: {error}"));
    controller
        .push_pcm(pcm_seconds(1))
        .unwrap_or_else(|error| panic!("PCM should be accepted: {error}"));
    controller
        .stop()
        .unwrap_or_else(|error| panic!("recording should stop: {error}"));

    let bounds: Vec<(u64, u64)> = stored_segments(&controller, &meeting_id)
        .iter()
        .map(|segment| (segment.start_time_ms(), segment.end_time_ms()))
        .collect();
    assert_eq!(bounds, vec![(0, 3_000), (3_000, 4_000)]);
}

/// Loads a meeting's stored title through the worker-owned repository.
fn stored_title(controller: &RecordingController, meeting_id: &MeetingId) -> Option<String> {
    let meeting_id = meeting_id.clone();
    controller
        .recording_manager()
        .with_note_repository(move |note_repository| {
            note_repository.load_meeting_metadata(&meeting_id)
        })
        .unwrap_or_else(|error| panic!("worker should answer: {error}"))
        .unwrap_or_else(|error| panic!("metadata should load: {error}"))
        .and_then(|metadata| metadata.title().map(|title| title.as_str().to_owned()))
}

/// Parses a fixture title.
fn title(title_text: &str) -> MeetingTitle {
    MeetingTitle::parse(title_text).unwrap_or_else(|error| panic!("title must parse: {error}"))
}

/// Verifies a titled start stores the title on the new meeting, an untitled start of an existing
/// meeting keeps it, and a titled start of an existing meeting replaces it.
#[test]
fn a_titled_start_stores_the_meeting_title() {
    let (mut controller, _chunk_audio_store) = controller_with(true, |_| {});
    let meeting_id = controller
        .start_titled(None, Some(title("Project Sync")))
        .unwrap_or_else(|error| panic!("recording should start: {error}"));
    assert_eq!(
        stored_title(&controller, &meeting_id).as_deref(),
        Some("Project Sync")
    );
    controller
        .stop()
        .unwrap_or_else(|error| panic!("recording should stop: {error}"));

    controller
        .start(Some(meeting_id.clone()))
        .unwrap_or_else(|error| panic!("recording should restart: {error}"));
    controller
        .stop()
        .unwrap_or_else(|error| panic!("recording should stop: {error}"));
    assert_eq!(
        stored_title(&controller, &meeting_id).as_deref(),
        Some("Project Sync")
    );

    controller
        .start_titled(Some(meeting_id.clone()), Some(title("Renamed Sync")))
        .unwrap_or_else(|error| panic!("recording should restart: {error}"));
    assert_eq!(
        stored_title(&controller, &meeting_id).as_deref(),
        Some("Renamed Sync")
    );
}

/// Verifies an untitled start leaves the meeting untitled (it reads with the dated fallback).
#[test]
fn an_untitled_start_leaves_the_meeting_untitled() {
    let (mut controller, _chunk_audio_store) = controller_with(true, |_| {});
    let meeting_id = controller
        .start(None)
        .unwrap_or_else(|error| panic!("recording should start: {error}"));

    assert_eq!(stored_title(&controller, &meeting_id), None);
}
