//! Specifies the live-capture driver: PCM → recorder → chunker → audio store → pipeline worker,
//! with meeting-relative offsets, pause/resume, graceful stop, and raw-audio cleanup. The
//! transcription engine is a mock reading from the real `ChunkAudioStore`; storage is in-memory.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use crate::{
    adapters::{
        ai::openai_whisper_adapter::AudioFileSource, audio::chunk_audio_store::ChunkAudioStore,
        storage::sqlite_adapter::SqliteNoteRepository,
    },
    application::{
        crypto::voice_encryptor::ephemeral_cipher, meeting_crdt::MeetingCrdtService,
        pipeline::pipeline_factory::PipelineFactory, recording_manager::RecordingManager,
        screen_share_monitor::ScreenShareMonitor,
    },
    domain::{
        errors::{ScreenShareError, StorageError, TranscriptionError},
        models::{
            AudioRecording, AudioState, DeviceId, MeetingId, MeetingTitle, SegmentSource,
            Transcript, TranscriptSegment, TranscriptionContext,
        },
        transcription_language::TranscriptionLanguage,
    },
    ports::{
        ai::TranscriptionEngine,
        screen_share::{
            ActiveScreenShare, MeetingSlideSink, ScreenCapturer, ScreenFrame, ScreenShareDetector,
        },
    },
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
        transcription_context: &TranscriptionContext,
    ) -> Result<Transcript, TranscriptionError> {
        if !self.reads_audio {
            return Err(TranscriptionError::Unavailable);
        }
        let wav_bytes = self.chunk_audio_store.load_wav(audio_recording)?;
        let duration_ms = (wav_bytes.len() - 44) / 2 * 1_000 / SAMPLES_PER_SECOND;
        Ok(Transcript::new(
            audio_recording.meeting_id().clone(),
            match transcription_context.language() {
                Some(language) => format!("chunk of {duration_ms} ms in {}", language.as_str()),
                None => format!("chunk of {duration_ms} ms"),
            },
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

/// Shared switches and records of the fake screen-share ports (Issue #49).
#[derive(Clone, Default)]
struct ScreenShareProbe {
    /// Whether the fake conferencing app is sharing.
    is_sharing: Arc<AtomicBool>,
    capture_count: Arc<AtomicUsize>,
    /// `(meeting ID, meeting time)` of every attached slide.
    slides: Arc<Mutex<Vec<(String, u64)>>>,
}

impl ScreenShareProbe {
    fn capture_count(&self) -> usize {
        self.capture_count.load(Ordering::SeqCst)
    }

    fn slides(&self) -> Vec<(String, u64)> {
        self.slides
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    /// Waits up to two seconds for a slide count, since the watcher runs on its own thread.
    fn wait_for_slides(&self, slide_count: usize) -> Vec<(String, u64)> {
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.slides().len() < slide_count && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        self.slides()
    }
}

impl ScreenShareDetector for ScreenShareProbe {
    fn active_share(&self) -> Result<Option<ActiveScreenShare>, ScreenShareError> {
        Ok(self
            .is_sharing
            .load(Ordering::SeqCst)
            .then(|| ActiveScreenShare::new("fake-app")))
    }
}

impl ScreenCapturer for ScreenShareProbe {
    fn capture_frame(&self, _share: &ActiveScreenShare) -> Result<ScreenFrame, ScreenShareError> {
        self.capture_count.fetch_add(1, Ordering::SeqCst);
        ScreenFrame::new(32, 18, vec![200; 32 * 18], b"\x89PNG".to_vec())
    }
}

impl MeetingSlideSink for ScreenShareProbe {
    fn attach_slide(
        &self,
        meeting_id: &MeetingId,
        _encoded_image: &[u8],
        meeting_time_ms: u64,
    ) -> Result<(), StorageError> {
        self.slides
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push((meeting_id.as_str().to_owned(), meeting_time_ms));
        Ok(())
    }
}

/// Builds a time-based controller that samples the fake screen share every 5 ms.
fn controller_with_screen_share() -> (RecordingController, ScreenShareProbe) {
    let probe = ScreenShareProbe::default();
    let (controller, _) = controller_with(true, |_| {});
    let monitor = ScreenShareMonitor::new(
        Box::new(probe.clone()),
        Box::new(probe.clone()),
        Box::new(probe.clone()),
    );
    (
        controller.with_screen_share(monitor, Duration::from_millis(5)),
        probe,
    )
}

/// Verifies a shared slide is attached to the meeting being recorded at the current meeting time.
#[test]
fn a_shared_slide_is_attached_to_the_active_meeting_at_the_meeting_time() {
    let (mut controller, probe) = controller_with_screen_share();
    let meeting_id = controller
        .start(Some(MeetingId::new("meeting-share")))
        .unwrap_or_else(|error| panic!("start should succeed: {error}"));
    controller
        .push_pcm(pcm_seconds(4))
        .unwrap_or_else(|error| panic!("PCM should be accepted: {error}"));

    probe.is_sharing.store(true, Ordering::SeqCst);
    let slides = probe.wait_for_slides(1);
    controller
        .stop()
        .unwrap_or_else(|error| panic!("stop should succeed: {error}"));

    assert_eq!(slides, vec![(meeting_id.as_str().to_owned(), 4_000)]);
}

/// Verifies nothing is captured while paused or after stop, and a later recording captures again.
#[test]
fn the_screen_is_captured_only_while_recording() {
    let (mut controller, probe) = controller_with_screen_share();
    probe.is_sharing.store(true, Ordering::SeqCst);
    controller
        .start(Some(MeetingId::new("meeting-first")))
        .unwrap_or_else(|error| panic!("start should succeed: {error}"));
    probe.wait_for_slides(1);

    controller
        .pause()
        .unwrap_or_else(|error| panic!("pause should succeed: {error}"));
    std::thread::sleep(Duration::from_millis(20));
    let captures_when_paused = probe.capture_count();
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(probe.capture_count(), captures_when_paused, "paused");

    controller
        .stop()
        .unwrap_or_else(|error| panic!("stop should succeed: {error}"));
    let captures_when_stopped = probe.capture_count();
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(probe.capture_count(), captures_when_stopped, "stopped");

    controller
        .start(Some(MeetingId::new("meeting-second")))
        .unwrap_or_else(|error| panic!("start should succeed: {error}"));
    let slides = probe.wait_for_slides(2);
    controller
        .stop()
        .unwrap_or_else(|error| panic!("stop should succeed: {error}"));
    let meetings: Vec<&str> = slides.iter().map(|(meeting, _)| meeting.as_str()).collect();
    assert_eq!(meetings, vec!["meeting-first", "meeting-second"]);
}

/// Verifies the controller reports the meeting time of the meeting being recorded, for photos
/// taken during the recording (Issue #54), and nothing for other meetings or after stop.
#[test]
fn the_recorded_meeting_reports_its_current_meeting_time() {
    let (mut controller, _) = controller_with(true, |_| {});
    let meeting_id = controller
        .start(Some(MeetingId::new("meeting-photo")))
        .unwrap_or_else(|error| panic!("start should succeed: {error}"));
    assert_eq!(controller.current_meeting_time_ms(&meeting_id), Some(0));

    controller
        .push_pcm(pcm_seconds(3))
        .unwrap_or_else(|error| panic!("PCM should be accepted: {error}"));
    assert_eq!(controller.current_meeting_time_ms(&meeting_id), Some(3_000));
    assert_eq!(
        controller.current_meeting_time_ms(&MeetingId::new("meeting-other")),
        None
    );

    controller
        .stop()
        .unwrap_or_else(|error| panic!("stop should succeed: {error}"));
    assert_eq!(controller.current_meeting_time_ms(&meeting_id), None);
}

/// Without a choice, chunks carry the configured default (`NOTE_TAKER_WHISPER_LANGUAGE`), so the
/// language travels with the audio and is recorded; a choice overrides it for one recording.
#[test]
fn chunks_without_a_choice_carry_the_configured_default() {
    let language = |code: &str| {
        TranscriptionLanguage::parse(code).unwrap_or_else(|error| panic!("{code}: {error}"))
    };
    let (controller, _) = controller_with(true, |_| {});
    let mut controller = controller.with_default_language(Some(language("de")));
    let default_meeting = controller
        .start(Some(MeetingId::new("meeting-default")))
        .unwrap_or_else(|error| panic!("recording should start: {error}"));
    controller
        .push_pcm(pcm_seconds(1))
        .unwrap_or_else(|error| panic!("PCM should be accepted: {error}"));
    controller
        .stop()
        .unwrap_or_else(|error| panic!("recording should stop: {error}"));
    controller.set_language(Some(language("it")));
    let chosen_meeting = controller
        .start(Some(MeetingId::new("meeting-chosen")))
        .unwrap_or_else(|error| panic!("recording should start: {error}"));
    controller
        .push_pcm(pcm_seconds(1))
        .unwrap_or_else(|error| panic!("PCM should be accepted: {error}"));
    controller
        .stop()
        .unwrap_or_else(|error| panic!("recording should stop: {error}"));

    let text_of = |meeting_id: &MeetingId| -> Vec<String> {
        stored_segments(&controller, meeting_id)
            .iter()
            .map(|segment| segment.text().to_owned())
            .collect()
    };
    assert_eq!(text_of(&default_meeting), ["chunk of 1000 ms in de"]);
    assert_eq!(text_of(&chosen_meeting), ["chunk of 1000 ms in it"]);
}

/// The language set before a start applies from the first chunk; one set during the recording
/// applies to chunks cut after it; and the next recording starts without either.
#[test]
fn the_language_applies_from_the_next_chunk_and_ends_with_the_recording() {
    let language = |code: &str| {
        TranscriptionLanguage::parse(code).unwrap_or_else(|error| panic!("{code}: {error}"))
    };
    let (mut controller, _) = controller_with(true, |_| {});
    controller.set_language(Some(language("it")));
    let meeting_id = controller
        .start(Some(MeetingId::new("meeting-languages")))
        .unwrap_or_else(|error| panic!("recording should start: {error}"));
    controller
        .push_pcm(pcm_seconds(16))
        .unwrap_or_else(|error| panic!("PCM should be accepted: {error}"));
    controller.set_language(Some(language("en")));
    controller
        .stop()
        .unwrap_or_else(|error| panic!("recording should stop: {error}"));

    let texts: Vec<String> = stored_segments(&controller, &meeting_id)
        .iter()
        .map(|segment| segment.text().to_owned())
        .collect();
    assert_eq!(texts, ["chunk of 15000 ms in it", "chunk of 1000 ms in en"]);
    assert_eq!(controller.language(), None);
}

/// Each segment's span and track.
fn spans_by_track(segments: &[TranscriptSegment]) -> Vec<(u64, u64, SegmentSource)> {
    let mut spans: Vec<(u64, u64, SegmentSource)> = segments
        .iter()
        .map(|segment| {
            (
                segment.start_time_ms(),
                segment.end_time_ms(),
                segment.source(),
            )
        })
        .collect();
    spans.sort_by_key(|(start, end, source)| (*start, *end, *source == SegmentSource::Microphone));
    spans
}

/// Verifies the computer's sound is cut, placed on the same meeting time, and marked (#108).
/// Both tracks arrive interleaved, as the capture drains them together.
#[test]
fn the_computer_sound_is_a_second_track_on_the_same_meeting_time() {
    let (mut controller, chunk_audio_store) = controller_with(true, |_| {});
    let meeting_id = controller
        .start(Some(MeetingId::new("meeting-two-tracks")))
        .unwrap_or_else(|error| panic!("{error}"));
    let mut submitted = 0;
    for _ in 0..16 {
        controller
            .push_pcm(pcm_seconds(1))
            .unwrap_or_else(|error| panic!("{error}"));
        submitted += controller
            .push_computer_sound_pcm(pcm_seconds(1))
            .unwrap_or_else(|error| panic!("{error}"));
    }
    assert_eq!(submitted, 1);
    controller.stop().unwrap_or_else(|error| panic!("{error}"));

    assert!(chunk_audio_store.is_empty());
    assert_eq!(
        spans_by_track(&stored_segments(&controller, &meeting_id)),
        vec![
            (0, 15_000, SegmentSource::ComputerSound),
            (0, 15_000, SegmentSource::Microphone),
            (15_000, 16_000, SegmentSource::ComputerSound),
            (15_000, 16_000, SegmentSource::Microphone),
        ]
    );
}

/// Verifies computer sound that starts late, or pauses while nothing plays, is placed by the
/// microphone's clock rather than by its own sample count (#108).
#[test]
fn the_computer_sound_follows_the_microphone_s_clock() {
    let (mut controller, _chunk_audio_store) = controller_with(true, |_| {});
    let meeting_id = controller
        .start(Some(MeetingId::new("meeting-late-sound")))
        .unwrap_or_else(|error| panic!("{error}"));
    // The loopback stream opens after ten seconds: its first second ends at the microphone's time.
    controller
        .push_pcm(pcm_seconds(10))
        .unwrap_or_else(|error| panic!("{error}"));
    controller
        .push_computer_sound_pcm(pcm_seconds(1))
        .unwrap_or_else(|error| panic!("{error}"));
    // Then nothing plays for four seconds, and it sends nothing meanwhile.
    controller
        .push_pcm(pcm_seconds(5))
        .unwrap_or_else(|error| panic!("{error}"));
    controller
        .push_computer_sound_pcm(pcm_seconds(1))
        .unwrap_or_else(|error| panic!("{error}"));
    controller.stop().unwrap_or_else(|error| panic!("{error}"));

    let computer_sound: Vec<(u64, u64, SegmentSource)> =
        spans_by_track(&stored_segments(&controller, &meeting_id))
            .into_iter()
            .filter(|(_, _, source)| *source == SegmentSource::ComputerSound)
            .collect();
    assert_eq!(
        computer_sound,
        vec![
            (9_000, 10_000, SegmentSource::ComputerSound),
            (14_000, 15_000, SegmentSource::ComputerSound)
        ]
    );
}

/// Verifies computer sound is refused without a recording and discarded while paused (#108).
#[test]
fn the_computer_sound_needs_a_running_recording() {
    let (mut controller, _chunk_audio_store) = controller_with(true, |_| {});
    assert!(matches!(
        controller.push_computer_sound_pcm(pcm_seconds(1)),
        Err(RecordingControlError::NotRecording)
    ));
    let meeting_id = controller
        .start(Some(MeetingId::new("meeting-paused-sound")))
        .unwrap_or_else(|error| panic!("{error}"));
    controller.pause().unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        controller
            .push_computer_sound_pcm(pcm_seconds(16))
            .unwrap_or_else(|error| panic!("{error}")),
        0
    );
    controller.stop().unwrap_or_else(|error| panic!("{error}"));
    assert!(stored_segments(&controller, &meeting_id).is_empty());
}
