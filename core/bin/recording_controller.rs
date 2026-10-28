//! Drives one live recording from raw PCM to the meeting pipeline worker.
//!
//! Flow per recording: caller PCM → `LocalRecorder` (lifecycle state machine; drops audio unless
//! recording) → `VadChunker` (speech pauses, at most 15 s) → `ChunkAudioStore` (the engine's audio
//! source) plus a `VadAudioChunk` with meeting-relative offsets → `RecordingManager` (worker
//! thread running `MeetingSession`). Stop flushes the chunker, waits for every queued chunk, and
//! wipes any raw audio no engine consumed. The FFI and CLI share this driver, so both entry points
//! run the same flow. Audio and transcripts are never logged.
//!
//! With `with_screen_share`, each recording also runs a `ScreenShareWatcher` thread that attaches
//! new slides of a running screen share to the recorded meeting at the current meeting time
//! (Issue #49), only while audio is being captured.

use std::{
    sync::{Arc, mpsc::Receiver},
    time::Duration,
};

use uuid::Uuid;

use crate::{
    adapters::audio::{
        chunk_audio_store::{ChunkAudioStore, encode_wav_pcm16},
        local_recorder::LocalRecorder,
        vad_chunker::{FinalizedAudioChunk, VadChunker, VadConfig},
    },
    application::{
        meeting_crdt::MeetingCrdtService,
        pipeline::meeting_session::VadAudioChunk,
        recording_manager::{RecordingManager, RecordingManagerError, RecordingSummary},
        screen_share_monitor::ScreenShareMonitor,
    },
    domain::{
        errors::{AudioError, StorageError},
        models::{
            AudioChunk, AudioConfig, AudioRecording, AudioState, DeviceId, MeetingId, MeetingTitle,
            RecordingId,
        },
    },
    ports::audio::AudioCapture,
    screen_share_watcher::{RecordingClock, ScreenShareWatcher},
};

#[cfg(test)]
#[path = "recording_controller_tests.rs"]
mod tests;

/// Sample rate of the fixed capture format (16 kHz mono signed PCM).
const CAPTURE_SAMPLE_RATE_HZ: u32 = 16_000;

/// Selects how live audio is cut into pipeline chunks.
#[derive(Clone, Debug)]
pub enum ChunkingMode {
    /// Cut at speech pauses (needs the `vad` feature; otherwise falls back to time cuts).
    Vad(VadConfig),
    /// Cut near every 15 seconds without speech detection.
    TimeBased,
}

/// Reports why a recording command was rejected.
#[derive(Debug, thiserror::Error)]
pub enum RecordingControlError {
    /// A recording is already capturing audio.
    #[error("a recording is already in progress")]
    AlreadyRecording,
    /// No recording is active.
    #[error("no recording is in progress")]
    NotRecording,
    /// The capture state machine or chunker rejected the request.
    #[error(transparent)]
    Audio(#[from] AudioError),
    /// The pipeline worker rejected the request.
    #[error(transparent)]
    Pipeline(#[from] RecordingManagerError),
    /// The meeting title could not be stored.
    #[error("unable to store the meeting title: {0}")]
    Storage(#[from] StorageError),
}

/// Capture state of the recording in progress.
struct ActiveCapture {
    meeting_id: MeetingId,
    local_recorder: LocalRecorder,
    audio_receiver: Receiver<AudioChunk>,
    chunker: VadChunker,
    /// Unique per recording, so chunk IDs never collide across recordings.
    recording_key: Uuid,
    next_chunk_sequence: u64,
    /// Meeting time (ms) at which this recording's audio begins.
    meeting_start_offset_ms: u64,
    /// Distinct samples placed on the timeline so far (overlaps counted once).
    timeline_sample_count: u64,
    /// Samples captured so far, including those still buffered in the chunker.
    captured_sample_count: u64,
    /// Meeting time and capture state read by the screen-share watcher.
    recording_clock: Arc<RecordingClock>,
    /// Samples the shared screen during this recording; `None` without screen capture.
    screen_share_watcher: Option<ScreenShareWatcher>,
}

impl ActiveCapture {
    /// Publishes the current meeting time and capture state to the screen-share watcher.
    ///
    /// Side effects: Updates the shared clock.
    fn publish_clock(&self) {
        self.recording_clock.publish(
            self.meeting_start_offset_ms + samples_to_ms(self.captured_sample_count),
            self.local_recorder.state() == AudioState::Recording,
        );
    }
}

/// Local screen-share capture configured for every recording (Issue #49).
struct ScreenShareSetup {
    /// Between recordings; `None` while a watcher owns it, or after a watcher failed.
    idle_monitor: Option<ScreenShareMonitor>,
    sample_interval: Duration,
}

/// Owns the pipeline worker and the recording in progress, if any.
pub struct RecordingController {
    /// Shared with the Teams bridge pump, which feeds the worker's external meeting session.
    recording_manager: Arc<RecordingManager>,
    chunk_audio_store: Arc<ChunkAudioStore>,
    chunking_mode: ChunkingMode,
    device_id: DeviceId,
    active_capture: Option<ActiveCapture>,
    /// State reported while no recording is active (`Idle`, then `Stopped` after a stop).
    inactive_state: AudioState,
    /// `None` when the composition configured no screen capture.
    screen_share: Option<ScreenShareSetup>,
}

impl RecordingController {
    /// Creates an idle controller around a ready pipeline worker.
    ///
    /// Inputs: The worker, the audio store its transcription engine reads from, the chunking mode,
    /// and the device ID recorded as the acquiring node of each chunk.
    /// Outputs: The controller in the `Idle` state.
    /// Side effects: None.
    pub fn new(
        recording_manager: RecordingManager,
        chunk_audio_store: Arc<ChunkAudioStore>,
        chunking_mode: ChunkingMode,
        device_id: DeviceId,
    ) -> Self {
        Self {
            recording_manager: Arc::new(recording_manager),
            chunk_audio_store,
            chunking_mode,
            device_id,
            active_capture: None,
            inactive_state: AudioState::Idle,
            screen_share: None,
        }
    }

    /// Captures new slides of a running screen share during every recording (Issue #49).
    ///
    /// Inputs: The monitor (with the conferencing app's detector, a capturer and the slide store)
    /// and the time between samples (2 s in production).
    /// Outputs: The controller. Side effects: None until a recording starts.
    pub fn with_screen_share(
        mut self,
        monitor: ScreenShareMonitor,
        sample_interval: Duration,
    ) -> Self {
        self.screen_share = Some(ScreenShareSetup {
            idle_monitor: Some(monitor),
            sample_interval,
        });
        self
    }

    /// Shares the pipeline worker with another producer (the Teams bridge pump).
    ///
    /// Outputs: A handle to the same worker; the worker stops once every handle is dropped.
    /// Side effects: None.
    pub fn shared_recording_manager(&self) -> Arc<RecordingManager> {
        Arc::clone(&self.recording_manager)
    }

    /// Shares the audio store the worker's transcription engine reads chunks from.
    ///
    /// Outputs: The store; callers must key entries with IDs no local recording uses.
    /// Side effects: None.
    pub fn shared_chunk_audio_store(&self) -> Arc<ChunkAudioStore> {
        Arc::clone(&self.chunk_audio_store)
    }

    /// Borrows the pipeline worker, e.g. to read its availability or query its repository.
    pub fn recording_manager(&self) -> &RecordingManager {
        &self.recording_manager
    }

    /// Returns the portable lifecycle state of the current or last recording.
    pub fn state(&self) -> AudioState {
        self.active_capture
            .as_ref()
            .map_or(self.inactive_state, |capture| {
                capture.local_recorder.state()
            })
    }

    /// Returns the meeting being recorded, if any.
    pub fn active_meeting_id(&self) -> Option<&MeetingId> {
        self.active_capture
            .as_ref()
            .map(|capture| &capture.meeting_id)
    }

    /// Starts a new untitled recording, or resumes a paused or interrupted one.
    ///
    /// Inputs: `meeting_id` for a new recording (`None` generates `meeting-<uuid>`); ignored when
    /// resuming.
    /// Outputs: See `start_titled`.
    /// Side effects: See `start_titled`; an existing meeting keeps its title.
    pub fn start(
        &mut self,
        meeting_id: Option<MeetingId>,
    ) -> Result<MeetingId, RecordingControlError> {
        self.start_titled(meeting_id, None)
    }

    /// Starts a new recording with an optional title, or resumes a paused or interrupted one.
    ///
    /// Inputs: `meeting_id` for a new recording (`None` generates `meeting-<uuid>`) and `title`
    /// (`None` keeps the meeting's current title, which for a new meeting means untitled); both
    /// are ignored when resuming.
    /// Outputs: The recorded meeting, `AlreadyRecording`, or an audio/pipeline/storage error.
    /// Side effects: Opens a pipeline session (creating the meeting if needed), stores `title` on
    /// the meeting (replacing an earlier one), and reads the meeting's timeline so new segments
    /// follow existing ones; blocks until the worker answers. A failed title write ends the
    /// session again.
    pub fn start_titled(
        &mut self,
        meeting_id: Option<MeetingId>,
        title: Option<MeetingTitle>,
    ) -> Result<MeetingId, RecordingControlError> {
        if let Some(capture) = self.active_capture.as_mut() {
            if capture.local_recorder.state() == AudioState::Recording {
                return Err(RecordingControlError::AlreadyRecording);
            }
            capture.local_recorder.start()?;
            capture.publish_clock();
            tracing::info!(meeting_id = %capture.meeting_id.as_str(), "Recording resumed");
            return Ok(capture.meeting_id.clone());
        }

        let meeting_id =
            meeting_id.unwrap_or_else(|| MeetingId::new(format!("meeting-{}", Uuid::new_v4())));
        let mut local_recorder = LocalRecorder::new(capture_config())?;
        let audio_receiver = local_recorder.subscribe_audio()?;
        let chunker = match &self.chunking_mode {
            ChunkingMode::Vad(vad_config) => VadChunker::new(vad_config.clone())?,
            ChunkingMode::TimeBased => VadChunker::time_based(VadConfig::english()),
        };
        self.recording_manager.begin_recording(meeting_id.clone())?;
        if let Some(title) = title
            && let Err(error) = self.save_meeting_title(&meeting_id, title)
        {
            let _ = self.recording_manager.end_recording();
            return Err(error);
        }
        if let Err(error) = local_recorder.start() {
            let _ = self.recording_manager.end_recording();
            return Err(error.into());
        }
        let meeting_start_offset_ms = self.meeting_timeline_end_ms(&meeting_id);
        let recording_clock = Arc::new(RecordingClock::default());
        recording_clock.publish(meeting_start_offset_ms, true);
        let screen_share_watcher = self.start_screen_share_watcher(&meeting_id, &recording_clock);
        self.active_capture = Some(ActiveCapture {
            meeting_id: meeting_id.clone(),
            local_recorder,
            audio_receiver,
            chunker,
            recording_key: Uuid::new_v4(),
            next_chunk_sequence: 0,
            meeting_start_offset_ms,
            timeline_sample_count: 0,
            captured_sample_count: 0,
            recording_clock,
            screen_share_watcher,
        });
        Ok(meeting_id)
    }

    /// Feeds captured 16 kHz mono PCM into the recording.
    ///
    /// Inputs: Raw samples from the platform capture; they are never logged.
    /// Outputs: How many chunks were queued for the pipeline, or `NotRecording`. Audio received
    /// while paused or interrupted is accepted and discarded.
    /// Side effects: Buffers audio; stores each finished chunk's PCM until the engine reads it and
    /// queues the chunk on the worker without waiting for transcription.
    pub fn push_pcm(&mut self, samples: Vec<i16>) -> Result<usize, RecordingControlError> {
        let capture = self
            .active_capture
            .as_mut()
            .ok_or(RecordingControlError::NotRecording)?;
        capture
            .local_recorder
            .push_test_chunk(AudioChunk::new(samples, capture_config()))?;
        for routed_chunk in capture.audio_receiver.try_iter() {
            capture.captured_sample_count += routed_chunk.samples().len() as u64;
            capture.chunker.push_pcm(routed_chunk)?;
        }
        capture.publish_clock();
        let mut submitted_count = 0;
        for finalized_chunk in capture.chunker.drain_ready_chunks() {
            if submit_chunk(
                capture,
                finalized_chunk,
                &self.chunk_audio_store,
                &self.recording_manager,
                &self.device_id,
            )? {
                submitted_count += 1;
            }
        }
        Ok(submitted_count)
    }

    /// Pauses the recording; buffered audio is kept for the resumed recording.
    ///
    /// Inputs: None.
    /// Outputs: `Ok(())`, `NotRecording`, or an invalid-state error.
    /// Side effects: Later PCM is discarded until `start` resumes.
    pub fn pause(&mut self) -> Result<(), RecordingControlError> {
        let capture = self
            .active_capture
            .as_mut()
            .ok_or(RecordingControlError::NotRecording)?;
        capture.local_recorder.pause()?;
        capture.publish_clock();
        Ok(())
    }

    /// Marks the recording interrupted by the operating system and flushes buffered audio.
    ///
    /// Inputs: None.
    /// Outputs: `Ok(())` once interrupted, `NotRecording`, or an invalid-state error.
    /// Side effects: Queues the buffered audio as a chunk so nothing captured is lost; `start`
    /// resumes the same recording.
    pub fn handle_interruption(&mut self) -> Result<(), RecordingControlError> {
        let capture = self
            .active_capture
            .as_mut()
            .ok_or(RecordingControlError::NotRecording)?;
        match capture.local_recorder.handle_interruption() {
            Err(AudioError::Interrupted) | Ok(()) => {}
            Err(error) => return Err(error.into()),
        }
        capture.publish_clock();
        if let Some(flushed_chunk) = capture.chunker.finish() {
            submit_chunk(
                capture,
                flushed_chunk,
                &self.chunk_audio_store,
                &self.recording_manager,
                &self.device_id,
            )?;
        }
        Ok(())
    }

    /// Stops the recording after every captured chunk has been processed.
    ///
    /// Inputs: None.
    /// Outputs: The recording summary, `NotRecording`, or a pipeline error.
    /// Side effects: Flushes the chunker, blocks until the worker drains its queue (call it off
    /// the UI thread), deletes raw audio no engine consumed, and moves to `Stopped`. A later
    /// `start` begins a new recording.
    pub fn stop(&mut self) -> Result<RecordingSummary, RecordingControlError> {
        let mut capture = self
            .active_capture
            .take()
            .ok_or(RecordingControlError::NotRecording)?;
        // No slide may be captured once Stop was pressed.
        if let Some(watcher) = capture.screen_share_watcher.take()
            && let Some(screen_share) = self.screen_share.as_mut()
        {
            screen_share.idle_monitor = watcher.stop();
        }
        if let Err(error) = capture.local_recorder.stop() {
            tracing::warn!(error = %error, "Recorder rejected stop; ending the recording anyway");
        }
        for routed_chunk in capture.audio_receiver.try_iter() {
            if capture.chunker.push_pcm(routed_chunk).is_err() {
                tracing::warn!("Audio received at stop had an unexpected format and was dropped");
            }
        }
        let mut flushed_chunks = capture.chunker.drain_ready_chunks();
        flushed_chunks.extend(capture.chunker.finish());
        for finalized_chunk in flushed_chunks {
            if let Err(error) = submit_chunk(
                &mut capture,
                finalized_chunk,
                &self.chunk_audio_store,
                &self.recording_manager,
                &self.device_id,
            ) {
                tracing::warn!(error = %error, "Final audio chunk could not be queued");
            }
        }
        let summary = self.recording_manager.end_recording();
        let discarded_chunk_count = self
            .chunk_audio_store
            .discard_with_prefix(&chunk_id_prefix(capture.recording_key));
        if discarded_chunk_count > 0 {
            tracing::info!(
                discarded_chunk_count,
                "Deleted raw audio of chunks that were not transcribed"
            );
        }
        self.inactive_state = AudioState::Stopped;
        Ok(summary?)
    }

    /// Starts screen-share sampling for a new recording, when configured.
    ///
    /// Inputs: The recorded meeting and the recording's clock.
    /// Outputs: The running watcher, or `None` without screen capture or when its thread cannot
    /// start (the recording then continues without it).
    /// Side effects: Hands the idle monitor to a new watcher thread.
    fn start_screen_share_watcher(
        &mut self,
        meeting_id: &MeetingId,
        recording_clock: &Arc<RecordingClock>,
    ) -> Option<ScreenShareWatcher> {
        let screen_share = self.screen_share.as_mut()?;
        let monitor = screen_share.idle_monitor.take()?;
        match ScreenShareWatcher::start(
            monitor,
            meeting_id.clone(),
            Arc::clone(recording_clock),
            screen_share.sample_interval,
        ) {
            Ok(watcher) => Some(watcher),
            Err(error) => {
                tracing::warn!(error = %error, "Screen-share watcher could not start; recording without it");
                None
            }
        }
    }

    /// Stores `title` on an existing meeting through the worker's repository.
    ///
    /// Inputs: A meeting the worker has already created and its validated title.
    /// Outputs: `Ok(())`, a storage error, or `WorkerStopped`.
    /// Side effects: Writes the title into the meeting's CRDT document (so it replicates, Issue
    /// #39) and its SQLite projection on the worker thread; the title is never logged.
    fn save_meeting_title(
        &self,
        meeting_id: &MeetingId,
        title: MeetingTitle,
    ) -> Result<(), RecordingControlError> {
        let titled_meeting_id = meeting_id.clone();
        self.recording_manager
            .with_note_repository(move |note_repository| {
                MeetingCrdtService::new(note_repository)
                    .set_meeting_title(&titled_meeting_id, &title)
            })??;
        Ok(())
    }

    /// Finds where a meeting's existing timeline ends, so a new recording continues after it.
    ///
    /// Inputs: The meeting about to be recorded.
    /// Outputs: The latest segment end in ms, or 0 for an empty or unreadable timeline.
    /// Side effects: Reads the meeting's segments on the worker thread.
    fn meeting_timeline_end_ms(&self, meeting_id: &MeetingId) -> u64 {
        let queried_meeting_id = meeting_id.clone();
        let timeline_end = self
            .recording_manager
            .with_note_repository(move |note_repository| {
                note_repository
                    .load_transcript_segments(&queried_meeting_id)
                    .map(|segments| {
                        segments
                            .iter()
                            .map(|segment| segment.end_time_ms())
                            .max()
                            .unwrap_or(0)
                    })
            });
        match timeline_end {
            Ok(Ok(end_ms)) => end_ms,
            Ok(Err(error)) => {
                tracing::warn!(error = %error, "Meeting timeline unreadable; recording from 0 ms");
                0
            }
            Err(error) => {
                tracing::warn!(error = %error, "Meeting timeline unreadable; recording from 0 ms");
                0
            }
        }
    }
}

impl Drop for RecordingController {
    /// Stops an active recording so its audio is processed and then wiped.
    fn drop(&mut self) {
        if self.active_capture.is_some()
            && let Err(error) = self.stop()
        {
            tracing::warn!(error = %error, "Recording could not be stopped cleanly");
        }
        self.chunk_audio_store.clear();
    }
}

/// Returns the prefix of every chunk ID of the recording `recording_key`.
fn chunk_id_prefix(recording_key: Uuid) -> String {
    format!("{recording_key}-chunk-")
}

/// Returns the fixed capture format.
fn capture_config() -> AudioConfig {
    AudioConfig::new(CAPTURE_SAMPLE_RATE_HZ, 1)
}

/// Converts a sample count at the capture rate to milliseconds.
fn samples_to_ms(sample_count: u64) -> u64 {
    sample_count.saturating_mul(1_000) / u64::from(CAPTURE_SAMPLE_RATE_HZ)
}

/// Places one finished chunk on the meeting timeline and queues it for the pipeline, unless it
/// holds no speech.
///
/// Inputs: The active capture, the chunk, the engine audio store, the worker, and the device.
/// Outputs: `Ok(true)` once queued, `Ok(false)` for a chunk without speech (Whisper would invent
/// text for it; Issue #51), or the worker's error.
/// Side effects: Advances the meeting timeline either way. A queued chunk's PCM is stored under
/// a fresh recording ID (read and deleted by the engine) and a WAV copy is sent with it for
/// diarization and review clips; a skipped chunk's audio is dropped.
fn submit_chunk(
    capture: &mut ActiveCapture,
    finalized_chunk: FinalizedAudioChunk,
    chunk_audio_store: &ChunkAudioStore,
    recording_manager: &RecordingManager,
    device_id: &DeviceId,
) -> Result<bool, RecordingControlError> {
    let contains_speech = finalized_chunk.contains_speech();
    let samples = finalized_chunk.into_audio_chunk().into_samples();
    // Chunks are contiguous: each starts where the previous one ended.
    let chunk_start_sample = capture.timeline_sample_count;
    let chunk_end_sample = chunk_start_sample + samples.len() as u64;
    capture.timeline_sample_count = chunk_end_sample;
    if !contains_speech {
        tracing::debug!(
            meeting_id = %capture.meeting_id.as_str(),
            chunk_start_ms = samples_to_ms(chunk_start_sample),
            chunk_end_ms = samples_to_ms(chunk_end_sample),
            "Chunk without speech skipped"
        );
        return Ok(false);
    }

    let recording_id = RecordingId::new(format!(
        "{}{}",
        chunk_id_prefix(capture.recording_key),
        capture.next_chunk_sequence
    ));
    capture.next_chunk_sequence += 1;
    let audio_bytes = encode_wav_pcm16(&samples, CAPTURE_SAMPLE_RATE_HZ);
    chunk_audio_store.insert(&recording_id, samples);
    let vad_chunk = VadAudioChunk::new(
        AudioRecording::new(recording_id, capture.meeting_id.clone(), device_id.clone()),
        capture.meeting_start_offset_ms + samples_to_ms(chunk_start_sample),
        capture.meeting_start_offset_ms + samples_to_ms(chunk_end_sample),
        audio_bytes,
    )
    .map_err(|_| AudioError::InvalidConfiguration {
        reason: "chunk bounds are inverted".to_owned(),
    })?;
    recording_manager.submit_chunk(vad_chunk)?;
    Ok(true)
}
