//! Runs the after-meeting transcript pass (#87) on its own thread, when the device allows it.
//!
//! A recording keeps its speech sealed in a `RetainedAudioStore`. After Stop, and at start for
//! meetings left over, this worker re-transcribes each one in 30 s windows with the chosen model
//! (`application::transcript_refinement`), replaces the text, and deletes the audio. It waits while a
//! recording is running and, on a phone, until it is charging; it stops between windows when that
//! changes and tries again later. The model is loaded only for a round of work and dropped after,
//! so it holds no memory between meetings. Retained audio never lingers: it is deleted after a
//! pass, when unreadable, when its meeting is gone, or after three failed attempts. Logs carry
//! meeting ids and counts only.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

use crate::{
    adapters::audio::{
        chunk_audio_store::ChunkAudioStore,
        retained_audio_store::{RetainedAudioError, RetainedAudioStore},
    },
    application::transcript_refinement::{RefinementChunk, RefinementError, refine_meeting},
    domain::models::{DeviceId, MeetingId},
    ports::{ai::TranscriptionEngine, storage::NoteRepository},
};

/// Failed attempts after which a meeting's audio is deleted and its live transcript stands.
const MAX_ATTEMPTS: u32 = 3;
/// How often the worker looks again without being woken, for example after the phone is plugged in.
const IDLE_CHECK: Duration = Duration::from_secs(300);

/// Returns where a database's retained meeting audio lives: `<data folder>/retained-audio`.
pub fn retained_audio_directory(database_path: &Path) -> PathBuf {
    database_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("retained-audio")
}

/// Builds the engine for one round of work, reading audio from the given store.
pub type EngineFactory =
    Box<dyn Fn(Arc<ChunkAudioStore>) -> anyhow::Result<Box<dyn TranscriptionEngine>> + Send>;
/// Opens the note repository for one round of work.
pub type RepositoryFactory = Box<dyn Fn() -> anyhow::Result<Box<dyn NoteRepository>> + Send>;

/// What decides whether the pass may run now, shared with the recorder and the platform.
pub struct RefinementControl {
    /// True on a phone: the pass runs only while charging.
    needs_charging: bool,
    charging: AtomicBool,
    recording: AtomicBool,
    shutting_down: AtomicBool,
    wake: (Mutex<bool>, Condvar),
    /// Rounds asked for through `request_round`.
    requested_rounds: AtomicU64,
    /// The latest requested round the worker has finished, and its signal.
    finished_round: (Mutex<u64>, Condvar),
}

impl RefinementControl {
    /// Creates the control.
    ///
    /// Inputs: Whether the device must be charging (phones). Outputs: The control, not charging
    /// and not recording. Side effects: None.
    pub fn new(needs_charging: bool) -> Self {
        Self {
            needs_charging,
            charging: AtomicBool::new(false),
            recording: AtomicBool::new(false),
            shutting_down: AtomicBool::new(false),
            wake: (Mutex::new(false), Condvar::new()),
            requested_rounds: AtomicU64::new(0),
            finished_round: (Mutex::new(0), Condvar::new()),
        }
    }

    /// Asks for a round of work that starts after this call.
    ///
    /// Outputs: A ticket for `wait_for_round`. Side effects: Wakes the worker.
    pub fn request_round(&self) -> u64 {
        let ticket = self.requested_rounds.fetch_add(1, Ordering::SeqCst) + 1;
        self.wake();
        ticket
    }

    /// Waits until the worker has finished the round `ticket` asked for, or `timeout` passes.
    ///
    /// Outputs: Whether that round finished. Side effects: Blocks the calling thread.
    pub fn wait_for_round(&self, ticket: u64, timeout: Duration) -> bool {
        let (finished, signal) = &self.finished_round;
        let guard = finished.lock().unwrap_or_else(PoisonError::into_inner);
        let (guard, _) = signal
            .wait_timeout_while(guard, timeout, |round| *round < ticket)
            .unwrap_or_else(PoisonError::into_inner);
        *guard >= ticket
    }

    /// Records that a round asked for up to `round` is done.
    fn finish_round(&self, round: u64) {
        let (finished, signal) = &self.finished_round;
        let mut guard = finished.lock().unwrap_or_else(PoisonError::into_inner);
        *guard = (*guard).max(round);
        signal.notify_all();
    }

    /// Reports whether the pass may run now: no recording, not shutting down, and charging when
    /// that is required.
    pub fn may_run(&self) -> bool {
        !self.recording.load(Ordering::SeqCst)
            && !self.shutting_down.load(Ordering::SeqCst)
            && (!self.needs_charging || self.charging.load(Ordering::SeqCst))
    }

    /// Records whether a recording is running. Side effects: Wakes the worker when one ends.
    pub fn set_recording(&self, recording: bool) {
        self.recording.store(recording, Ordering::SeqCst);
        if !recording {
            self.wake();
        }
    }

    /// Records whether the device is charging. Side effects: Wakes the worker when it starts.
    pub fn set_charging(&self, charging: bool) {
        let was_charging = self.charging.swap(charging, Ordering::SeqCst);
        if charging && !was_charging {
            self.wake();
        }
    }

    /// Asks the worker to look for work now.
    pub fn wake(&self) {
        let (pending, signal) = &self.wake;
        *pending.lock().unwrap_or_else(PoisonError::into_inner) = true;
        signal.notify_all();
    }

    /// Stops the worker after the window in progress. Side effects: Wakes it so it can exit.
    pub fn shut_down(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
        self.wake();
    }

    /// Waits for a wake-up or `timeout`, whichever is first.
    fn wait(&self, timeout: Duration) {
        let (pending, signal) = &self.wake;
        let guard = pending.lock().unwrap_or_else(PoisonError::into_inner);
        let (mut guard, _) = signal
            .wait_timeout_while(guard, timeout, |woken| !*woken)
            .unwrap_or_else(PoisonError::into_inner);
        *guard = false;
    }
}

/// Re-transcribes meetings whose speech was retained.
pub struct RefinementWorker {
    store: Arc<RetainedAudioStore>,
    control: Arc<RefinementControl>,
    make_engine: EngineFactory,
    open_repository: RepositoryFactory,
    device_id: DeviceId,
    attempts: HashMap<String, u32>,
}

impl RefinementWorker {
    /// Creates the worker; nothing runs until `run_pending` or `spawn`.
    pub fn new(
        store: Arc<RetainedAudioStore>,
        control: Arc<RefinementControl>,
        make_engine: EngineFactory,
        open_repository: RepositoryFactory,
        device_id: DeviceId,
    ) -> Self {
        Self {
            store,
            control,
            make_engine,
            open_repository,
            device_id,
            attempts: HashMap::new(),
        }
    }

    /// Starts the worker thread.
    ///
    /// Outputs: The thread's handle, or the error starting it. Side effects: The thread runs
    /// `run_pending` at once, after each wake-up, and every five minutes, until shut down.
    pub fn spawn(mut self) -> std::io::Result<thread::JoinHandle<()>> {
        thread::Builder::new()
            .name("transcript-pass".to_owned())
            .spawn(move || {
                while !self.control.shutting_down.load(Ordering::SeqCst) {
                    let round = self.control.requested_rounds.load(Ordering::SeqCst);
                    self.run_pending();
                    self.control.finish_round(round);
                    self.control.wait(IDLE_CHECK);
                }
            })
    }

    /// Processes every meeting with retained audio while the control allows it.
    ///
    /// Outputs: How many meetings were re-transcribed. Side effects: Loads the model and opens the
    /// repository once if there is work, runs inference, rewrites transcripts, and deletes
    /// retained audio as described in the module header.
    pub fn run_pending(&mut self) -> usize {
        let pending = self.store.pending_meetings();
        if pending.is_empty() || !self.control.may_run() {
            return 0;
        }
        let audio_source = Arc::new(ChunkAudioStore::new());
        let engine = match (self.make_engine)(Arc::clone(&audio_source)) {
            Ok(engine) => engine,
            Err(error) => {
                tracing::warn!(error = %format!("{error:#}"), "the transcript pass cannot load its model");
                return 0;
            }
        };
        let repository = match (self.open_repository)() {
            Ok(repository) => repository,
            Err(error) => {
                tracing::warn!(error = %format!("{error:#}"), "the transcript pass cannot open the notes");
                return 0;
            }
        };
        let mut refined = 0;
        for meeting_id in pending {
            if !self.control.may_run() {
                break;
            }
            refined += usize::from(self.refine_one(
                &meeting_id,
                engine.as_ref(),
                &audio_source,
                repository.as_ref(),
            ));
            audio_source.clear();
        }
        refined
    }

    /// Runs the pass for one meeting and decides what happens to its audio.
    ///
    /// Outputs: Whether the transcript was replaced.
    fn refine_one(
        &mut self,
        meeting_id: &MeetingId,
        engine: &dyn TranscriptionEngine,
        audio_source: &ChunkAudioStore,
        repository: &dyn NoteRepository,
    ) -> bool {
        let chunks = match self.store.load(meeting_id) {
            Ok(chunks) => chunks,
            Err(RetainedAudioError::Unreadable) => {
                tracing::warn!(meeting_id = %meeting_id.as_str(), "retained audio is unreadable; deleting it");
                self.delete(meeting_id);
                return false;
            }
            Err(error) => {
                tracing::warn!(meeting_id = %meeting_id.as_str(), error = %error, "retained audio could not be read");
                return false;
            }
        };
        if chunks.is_empty() || !meeting_exists(repository, meeting_id) {
            self.delete(meeting_id);
            return false;
        }
        let borrowed: Vec<RefinementChunk<'_>> = chunks
            .iter()
            .map(|chunk| RefinementChunk {
                span: chunk.span(),
                language: chunk.language(),
                samples: chunk.samples(),
            })
            .collect();
        let control = Arc::clone(&self.control);
        let provide = |recording_id: &crate::domain::models::RecordingId, samples: Vec<i16>| {
            audio_source.insert(recording_id, samples);
        };
        let cancelled = || !control.may_run();
        match refine_meeting(
            meeting_id,
            &borrowed,
            engine,
            &provide,
            repository,
            &self.device_id,
            &cancelled,
        ) {
            Ok(outcome) => {
                tracing::info!(
                    meeting_id = %meeting_id.as_str(),
                    window_count = outcome.window_count,
                    segment_count = outcome.segment_count,
                    "Meeting transcript re-transcribed after the meeting"
                );
                self.delete(meeting_id);
                true
            }
            Err(RefinementError::Cancelled) => false,
            Err(error) => {
                let attempts = self
                    .attempts
                    .entry(meeting_id.as_str().to_owned())
                    .or_insert(0);
                *attempts += 1;
                tracing::warn!(meeting_id = %meeting_id.as_str(), error = %error, attempts = *attempts, "the transcript pass failed; the live transcript stands");
                if *attempts >= MAX_ATTEMPTS {
                    self.delete(meeting_id);
                }
                false
            }
        }
    }

    /// Deletes a meeting's retained audio and forgets its attempts.
    fn delete(&mut self, meeting_id: &MeetingId) {
        self.attempts.remove(meeting_id.as_str());
        if let Err(error) = self.store.delete(meeting_id) {
            tracing::warn!(meeting_id = %meeting_id.as_str(), error = %error, "retained audio could not be deleted");
        }
    }
}

/// Reports whether the meeting still stands: false only when it was deleted meanwhile, so a failed
/// lookup never costs a meeting its pass.
fn meeting_exists(repository: &dyn NoteRepository, meeting_id: &MeetingId) -> bool {
    !repository.is_meeting_deleted(meeting_id).unwrap_or(false)
}
