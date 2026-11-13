//! Drives one live recording from raw PCM to the meeting pipeline worker.
//!
//! Flow per recording: caller PCM → `LocalRecorder` (lifecycle state machine; drops audio unless
//! recording) → `VadChunker` (speech pauses, at most 15 s) → `ChunkAudioStore` (the engine's audio
//! source) plus a `VadAudioChunk` with meeting-relative offsets → `RecordingManager` (worker
//! thread running `MeetingSession`). Stop flushes the chunker, waits for every queued chunk, and
//! wipes any raw audio no engine consumed. The FFI and CLI share this driver, so both entry points
//! run the same flow. Audio and transcripts are never logged.
//!
//! The computer's own sound (#108) can be pushed beside the microphone with
//! `push_computer_sound_pcm`. It has its own chunker, is placed by the microphone's clock (a
//! stream that opens late or sends nothing while nothing plays jumps ahead to it), and its chunks
//! are marked so the pipeline keeps them apart; it is not kept for the after-meeting pass, which
//! leaves its segments as they are.
//!
//! With `with_screen_share`, each recording also runs a `ScreenShareWatcher` thread that attaches
//! new slides of a running screen share to the recorded meeting at the current meeting time
//! (Issue #49), only while audio is being captured.
//!
//! With `with_speaker_names`, each recording also runs a `SpeakerNameWatcher` thread that reads
//! who call windows show as speaking and attaches the readings to the meeting at stop (#113).

use crate::domain::transcription_language::TranscriptionLanguage;
use std::{
    sync::{Arc, mpsc::Receiver},
    time::Duration,
};

use uuid::Uuid;

use crate::{
    adapters::audio::{
        chunk_audio_store::{ChunkAudioStore, encode_wav_pcm16},
        local_recorder::LocalRecorder,
        retained_speech_writer::RetainedSpeechWriter,
        vad_chunker::{FinalizedAudioChunk, VadChunker, VadConfig},
    },
    application::{
        meeting_crdt::MeetingCrdtService,
        pipeline::meeting_session::VadAudioChunk,
        recording_manager::{RecordingManager, RecordingManagerError, RecordingSummary},
        screen_share_monitor::ScreenShareMonitor,
        transcript_refinement::ChunkSpan,
    },
    domain::{
        errors::{AudioError, StorageError},
        models::{
            AudioChunk, AudioConfig, AudioRecording, AudioState, DeviceId, MeetingId, MeetingTitle,
            RecordingId, SegmentSource,
        },
    },
    ports::audio::AudioCapture,
    refinement_worker::RefinementControl,
    screen_share_watcher::{RecordingClock, ScreenShareWatcher},
    speaker_name_watcher::{SpeakerNameWatcher, SpeakerNamesSetup},
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
    /// The computer's sound (#108), from its first samples; `None` until then.
    computer_sound: Option<ComputerSoundTrack>,
    /// Reads speaking names from call windows (#113); `None` when off or not configured.
    speaker_name_watcher: Option<SpeakerNameWatcher>,
}

/// The computer's sound recorded beside the microphone (#108).
struct ComputerSoundTrack {
    chunker: VadChunker,
    /// Where its next chunk starts on this recording's timeline, in samples.
    timeline_sample_count: u64,
    /// Where its last received sample ends on that timeline, buffered samples included.
    received_end_sample: u64,
}

/// The computer's sound behind the microphone by more than this (0.2 s) jumps to it. Both tracks
/// are drained every 100 ms, so their batches differ by less; a larger lag would push the echo
/// detector's 0–300 ms search off the leak.
const COMPUTER_SOUND_GAP_SAMPLES: u64 = 3_200;

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
    /// The language for chunks cut from now on; `None` falls back to `default_language`.
    language: Option<TranscriptionLanguage>,
    /// The configured language (`NOTE_TAKER_WHISPER_LANGUAGE`) for recordings without a choice;
    /// `None` leaves such chunks unlabelled, to the engine's configuration.
    default_language: Option<TranscriptionLanguage>,
    /// State reported while no recording is active (`Idle`, then `Stopped` after a stop).
    inactive_state: AudioState,
    /// `None` when the composition configured no screen capture.
    screen_share: Option<ScreenShareSetup>,
    /// Where speech is kept for the after-meeting pass (#87); `None` keeps nothing.
    transcript_pass: Option<TranscriptPass>,
    /// Reading speaking names from call windows (#113); `None` when not configured.
    speaker_names: Option<SpeakerNamesSetup>,
}

/// The after-meeting pass's store and control, as the recorder sees them (#87).
#[derive(Clone)]
pub struct TranscriptPass {
    /// Compresses and keeps each speech chunk in the pass's store, off the capture path (#88).
    pub writer: Arc<RetainedSpeechWriter>,
    /// Told when recordings start and stop, so the pass never runs during one.
    pub control: Arc<RefinementControl>,
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
            language: None,
            default_language: None,
            inactive_state: AudioState::Idle,
            screen_share: None,
            transcript_pass: None,
            speaker_names: None,
        }
    }

    /// Reads who call windows show as speaking during every recording the owner allows (#113).
    ///
    /// Inputs: The reader factory, the owner's switch, the store and the reading interval.
    /// Outputs: The controller. Side effects: None until a recording starts.
    #[must_use]
    pub fn with_speaker_names(mut self, speaker_names: SpeakerNamesSetup) -> Self {
        self.speaker_names = Some(speaker_names);
        self
    }

    /// Keeps each recording's speech sealed for the after-meeting pass (#87).
    ///
    /// Inputs: The pass's store and control. Outputs: The controller.
    /// Side effects: None until a recording starts.
    #[must_use]
    pub fn with_transcript_pass(mut self, transcript_pass: TranscriptPass) -> Self {
        self.transcript_pass = Some(transcript_pass);
        self
    }

    /// Returns the after-meeting pass's control, so the platform can report charging.
    pub fn transcript_pass_control(&self) -> Option<Arc<RefinementControl>> {
        self.transcript_pass
            .as_ref()
            .map(|transcript_pass| Arc::clone(&transcript_pass.control))
    }

    /// Sets the language for recordings without a chosen one.
    ///
    /// Inputs: The configured language, or `None`.
    /// Outputs: The controller, which labels such chunks with it so the language travels with the
    /// audio (also to a paired device) and is recorded in the meeting's metadata.
    /// Side effects: None.
    pub fn with_default_language(
        mut self,
        default_language: Option<TranscriptionLanguage>,
    ) -> Self {
        self.default_language = default_language;
        self
    }

    /// The language chunks cut now are labelled with: the choice, else the configured default.
    fn chunk_language(&self) -> Option<TranscriptionLanguage> {
        self.language.or(self.default_language)
    }

    /// Chooses the language for the next recording, or for the rest of the current one.
    ///
    /// Inputs: The language; `None` leaves it to the engine's configuration.
    /// Outputs: None. Side effects: Applies to chunks cut from now on, not those already queued.
    /// A stop clears the choice.
    pub fn set_language(&mut self, language: Option<TranscriptionLanguage>) {
        self.language = language;
    }

    /// Returns the language chosen for chunks cut from now on, if any.
    pub fn language(&self) -> Option<TranscriptionLanguage> {
        self.language
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

    /// Returns the current meeting time of `meeting_id` while it is being recorded.
    ///
    /// Inputs: Any meeting.
    /// Outputs: Milliseconds of meeting time of the audio captured so far (paused or not), or
    /// `None` when that meeting is not the one being recorded. Photos taken now go there in the
    /// transcript (Issue #54).
    /// Side effects: None.
    pub fn current_meeting_time_ms(&self, meeting_id: &MeetingId) -> Option<u64> {
        self.active_capture
            .as_ref()
            .filter(|capture| &capture.meeting_id == meeting_id)
            .map(|capture| {
                capture.meeting_start_offset_ms + samples_to_ms(capture.captured_sample_count)
            })
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
        if let Some(transcript_pass) = &self.transcript_pass {
            transcript_pass.control.set_recording(true);
        }
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
        let chunker = new_chunker(&self.chunking_mode)?;
        if let Err(error) = self.recording_manager.begin_recording(meeting_id.clone()) {
            self.end_transcript_pass_hold();
            return Err(error.into());
        }
        if let Some(title) = title
            && let Err(error) = self.save_meeting_title(&meeting_id, title)
        {
            let _ = self.recording_manager.end_recording();
            self.end_transcript_pass_hold();
            return Err(error);
        }
        if let Err(error) = local_recorder.start() {
            let _ = self.recording_manager.end_recording();
            self.end_transcript_pass_hold();
            return Err(error.into());
        }
        let meeting_start_offset_ms = self.meeting_timeline_end_ms(&meeting_id);
        let recording_clock = Arc::new(RecordingClock::default());
        recording_clock.publish(meeting_start_offset_ms, true);
        let screen_share_watcher = self.start_screen_share_watcher(&meeting_id, &recording_clock);
        let speaker_name_watcher = self.speaker_names.as_ref().and_then(|setup| {
            SpeakerNameWatcher::start(setup, Arc::clone(&recording_clock)).unwrap_or_else(|error| {
                tracing::warn!(error = %error, "Speaker-name watcher could not start; recording without it");
                None
            })
        });
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
            computer_sound: None,
            speaker_name_watcher,
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
                SegmentSource::Microphone,
                finalized_chunk,
                &self.chunk_audio_store,
                &self.recording_manager,
                &self.device_id,
                // Field by field: `capture` borrows `self.active_capture`.
                self.language.or(self.default_language),
                self.transcript_pass.as_ref(),
            )? {
                submitted_count += 1;
            }
        }
        Ok(submitted_count)
    }

    /// Feeds the computer's own sound, as 16 kHz mono PCM, into the recording (#108).
    ///
    /// Inputs: Raw samples from the system-sound capture; they are never logged.
    /// Outputs: How many chunks were queued for the pipeline, or `NotRecording`. Audio received
    /// while paused or interrupted is discarded.
    /// Side effects: Buffers audio. The samples are placed to end at the microphone's captured
    /// time: the first ones, however late the stream opened, and any after the track fell behind
    /// (nothing played, so nothing was sent), which first flushes its buffer; stores each
    /// finished chunk's PCM until the engine reads it and queues it on the worker.
    pub fn push_computer_sound_pcm(
        &mut self,
        samples: Vec<i16>,
    ) -> Result<usize, RecordingControlError> {
        let capture = self
            .active_capture
            .as_mut()
            .ok_or(RecordingControlError::NotRecording)?;
        if capture.local_recorder.state() != AudioState::Recording {
            return Ok(0);
        }
        if capture.computer_sound.is_none() {
            // The first samples end at the microphone's time, however late the stream opened.
            let first_sample = capture
                .captured_sample_count
                .saturating_sub(samples.len() as u64);
            capture.computer_sound = Some(ComputerSoundTrack {
                chunker: new_chunker(&self.chunking_mode)?,
                timeline_sample_count: first_sample,
                received_end_sample: first_sample,
            });
        }
        let language = self.language.or(self.default_language);
        // Both tracks are drained together, so these samples end about where the microphone's
        // captured audio ends.
        let expected_start_sample = capture
            .captured_sample_count
            .saturating_sub(samples.len() as u64);
        let mut finished = Vec::new();
        if let Some(track) = capture.computer_sound.as_mut()
            && expected_start_sample > track.received_end_sample + COMPUTER_SOUND_GAP_SAMPLES
        {
            finished.extend(track.chunker.drain_ready_chunks());
            finished.extend(track.chunker.finish());
        }
        let mut submitted_count = 0;
        for finalized_chunk in finished {
            if submit_chunk(
                capture,
                SegmentSource::ComputerSound,
                finalized_chunk,
                &self.chunk_audio_store,
                &self.recording_manager,
                &self.device_id,
                language,
                None,
            )? {
                submitted_count += 1;
            }
        }
        let ready = match capture.computer_sound.as_mut() {
            Some(track) => {
                if expected_start_sample > track.received_end_sample + COMPUTER_SOUND_GAP_SAMPLES {
                    track.timeline_sample_count = expected_start_sample;
                    track.received_end_sample = expected_start_sample;
                }
                track.received_end_sample += samples.len() as u64;
                track
                    .chunker
                    .push_pcm(AudioChunk::new(samples, capture_config()))?;
                track.chunker.drain_ready_chunks()
            }
            None => Vec::new(),
        };
        for finalized_chunk in ready {
            if submit_chunk(
                capture,
                SegmentSource::ComputerSound,
                finalized_chunk,
                &self.chunk_audio_store,
                &self.recording_manager,
                &self.device_id,
                language,
                None,
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
                SegmentSource::Microphone,
                flushed_chunk,
                &self.chunk_audio_store,
                &self.recording_manager,
                &self.device_id,
                // Field by field: `capture` borrows `self.active_capture`.
                self.language.or(self.default_language),
                self.transcript_pass.as_ref(),
            )?;
        }
        for flushed_chunk in flush_computer_sound(capture) {
            submit_chunk(
                capture,
                SegmentSource::ComputerSound,
                flushed_chunk,
                &self.chunk_audio_store,
                &self.recording_manager,
                &self.device_id,
                self.language.or(self.default_language),
                None,
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
        // Audio still buffered keeps the choice; the next recording starts without it.
        let language = self.chunk_language();
        self.language = None;
        // No slide may be captured once Stop was pressed.
        if let Some(watcher) = capture.screen_share_watcher.take()
            && let Some(screen_share) = self.screen_share.as_mut()
        {
            screen_share.idle_monitor = watcher.stop();
        }
        if let Some(watcher) = capture.speaker_name_watcher.take()
            && let Some(setup) = self.speaker_names.as_ref()
        {
            watcher.finish(setup, &capture.meeting_id, capture.meeting_start_offset_ms);
        }
        if let Err(error) = capture.local_recorder.stop() {
            tracing::warn!(error = %error, "Recorder rejected stop; ending the recording anyway");
        }
        for routed_chunk in capture.audio_receiver.try_iter() {
            if capture.chunker.push_pcm(routed_chunk).is_err() {
                tracing::warn!("Audio received at stop had an unexpected format and was dropped");
            }
        }
        let mut flushed_chunks: Vec<(SegmentSource, FinalizedAudioChunk)> = capture
            .chunker
            .drain_ready_chunks()
            .into_iter()
            .chain(capture.chunker.finish())
            .map(|chunk| (SegmentSource::Microphone, chunk))
            .collect();
        flushed_chunks.extend(
            flush_computer_sound(&mut capture)
                .into_iter()
                .map(|chunk| (SegmentSource::ComputerSound, chunk)),
        );
        for (track, finalized_chunk) in flushed_chunks {
            let transcript_pass = match track {
                SegmentSource::Microphone => self.transcript_pass.as_ref(),
                SegmentSource::ComputerSound => None,
            };
            if let Err(error) = submit_chunk(
                &mut capture,
                track,
                finalized_chunk,
                &self.chunk_audio_store,
                &self.recording_manager,
                &self.device_id,
                language,
                transcript_pass,
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
        // After the final speaker pass, so the after-meeting pass reads the final speakers.
        self.end_transcript_pass_hold();
        Ok(summary?)
    }

    /// Lets the after-meeting pass run again once no recording is in progress.
    ///
    /// Side effects: First waits until every speech chunk queued for keeping is written, so the
    /// pass reads the whole meeting.
    fn end_transcript_pass_hold(&self) {
        if let Some(transcript_pass) = &self.transcript_pass {
            transcript_pass.writer.flush();
            transcript_pass.control.set_recording(false);
        }
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
/// Inputs: The active capture, the track the chunk was captured on (#108), the chunk, the engine
/// audio store, the worker, the device, the language, and the after-meeting pass's store (`None`
/// keeps nothing, as for the computer's sound).
/// Outputs: `Ok(true)` once queued, `Ok(false)` for a chunk without speech (Whisper would invent
/// text for it; Issue #51), or the worker's error.
/// Side effects: Advances the track's timeline either way. A queued chunk is also sealed for the
/// after-meeting pass when one is configured (#87). A queued chunk's PCM is stored under
/// a fresh recording ID (read and deleted by the engine) and a WAV copy is sent with it for
/// diarization and review clips; a skipped chunk's audio is dropped.
// Each argument is a distinct piece of the capture the chunk is placed in; a struct would only
// rename them.
#[allow(clippy::too_many_arguments)]
fn submit_chunk(
    capture: &mut ActiveCapture,
    track: SegmentSource,
    finalized_chunk: FinalizedAudioChunk,
    chunk_audio_store: &ChunkAudioStore,
    recording_manager: &RecordingManager,
    device_id: &DeviceId,
    language: Option<TranscriptionLanguage>,
    transcript_pass: Option<&TranscriptPass>,
) -> Result<bool, RecordingControlError> {
    let contains_speech = finalized_chunk.contains_speech();
    let samples = finalized_chunk.into_audio_chunk().into_samples();
    // A track's chunks are contiguous: each starts where its previous one ended.
    let timeline_sample_count = match track {
        SegmentSource::Microphone => &mut capture.timeline_sample_count,
        SegmentSource::ComputerSound => match capture.computer_sound.as_mut() {
            Some(computer_sound) => &mut computer_sound.timeline_sample_count,
            None => return Ok(false),
        },
    };
    let chunk_start_sample = *timeline_sample_count;
    let chunk_end_sample = chunk_start_sample + samples.len() as u64;
    *timeline_sample_count = chunk_end_sample;
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
    let chunk_start_ms = capture.meeting_start_offset_ms + samples_to_ms(chunk_start_sample);
    let chunk_end_ms = capture.meeting_start_offset_ms + samples_to_ms(chunk_end_sample);
    if let Some(transcript_pass) = transcript_pass {
        // Compressed and sealed on the writer's thread; this call only queues a copy.
        transcript_pass.writer.append_chunk(
            &capture.meeting_id,
            ChunkSpan {
                start_ms: chunk_start_ms,
                end_ms: chunk_end_ms,
            },
            language,
            samples.clone(),
        );
    }
    chunk_audio_store.insert(&recording_id, samples);
    let vad_chunk = VadAudioChunk::new(
        AudioRecording::new(recording_id, capture.meeting_id.clone(), device_id.clone()),
        chunk_start_ms,
        chunk_end_ms,
        audio_bytes,
    )
    .map_err(|_| AudioError::InvalidConfiguration {
        reason: "chunk bounds are inverted".to_owned(),
    })?
    .with_language(language)
    .with_track(track);
    recording_manager.submit_chunk(vad_chunk)?;
    Ok(true)
}

/// Builds a chunker for one track.
///
/// Inputs: The chunking mode. Outputs: The chunker, or why the VAD model could not load.
/// Side effects: May load the VAD model.
fn new_chunker(chunking_mode: &ChunkingMode) -> Result<VadChunker, AudioError> {
    Ok(match chunking_mode {
        ChunkingMode::Vad(vad_config) => VadChunker::new(vad_config.clone())?,
        ChunkingMode::TimeBased => VadChunker::time_based(VadConfig::english()),
    })
}

/// Takes every chunk the computer's sound still holds (#108).
///
/// Inputs: The capture. Outputs: The ready chunks and the buffered tail, in order; none without
/// computer sound. Side effects: Empties that track's chunker.
fn flush_computer_sound(capture: &mut ActiveCapture) -> Vec<FinalizedAudioChunk> {
    let Some(track) = capture.computer_sound.as_mut() else {
        return Vec::new();
    };
    let mut chunks = track.chunker.drain_ready_chunks();
    chunks.extend(track.chunker.finish());
    chunks
}
