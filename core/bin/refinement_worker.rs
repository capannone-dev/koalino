//! Runs the after-meeting work (#87, #88) on its own thread, when the device allows it.
//!
//! A recording keeps its speech sealed in a `RetainedAudioStore`. After Stop, at start, and every
//! five minutes, a round of this worker:
//!
//! 1. folds recordings of one meeting made here and on a paired device
//!    (`application::combined_meetings::find_combinations`);
//! 2. plans each retained meeting (`plan_retained_audio`) and carries the plan out: it sends the
//!    speech to the device that combines the meeting, combines every recording's speech in one pass
//!    (`application::combined_refinement`), re-transcribes a meeting alone in 30 s windows
//!    (`application::transcript_refinement`), or deletes speech nobody will use.
//!
//! Nothing runs while a recording is running. Passes run on a phone only while it is charging,
//! and stop between windows when that changes; a device without a pass model only folds, sends
//! and deletes. The model is loaded only for a round of work and dropped after. Retained audio
//! never lingers: it is deleted after a pass or a confirmed transfer, when unreadable, when its
//! meeting is gone, after three failed attempts, or when it has waited a day. `RetainedSpeechInbox`
//! stores speech paired devices send for combining. Logs carry meeting ids and counts only.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, SystemTime},
};

use zeroize::Zeroizing;

use crate::{
    adapters::audio::{
        chunk_audio_store::ChunkAudioStore,
        retained_audio_store::{RetainedAudioError, RetainedAudioStore, RetainedChunk},
    },
    application::{
        combined_meetings::{AudioPlan, find_combinations, plan_retained_audio},
        combined_refinement::{CombinedRecording, refine_combined_meeting},
        speech_transfer_exchange::{PART_AUDIO_BUDGET_BYTES, split_into_parts},
        transcript_refinement::{
            ChunkSpan, RefinementChunk, RefinementError, RefinementOutcome, refine_meeting,
        },
    },
    domain::models::{DeviceId, MeetingId},
    ports::{
        ai::TranscriptionEngine,
        speech_transfer::{
            SpeechChunk, SpeechInbox, SpeechPart, SpeechSender, SpeechTransferError,
        },
        storage::NoteRepository,
    },
};

/// Failed attempts after which a meeting's audio is deleted and its live transcript stands.
const MAX_ATTEMPTS: u32 = 3;
/// How often the worker looks again without being woken, for example after the phone is plugged in.
const IDLE_CHECK: Duration = Duration::from_secs(300);
/// How far back recordings are compared to find one meeting recorded on two devices.
const FOLD_WINDOW: chrono::TimeDelta = chrono::TimeDelta::days(2);

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
    /// Sends speech to the device that combines a meeting (#88); set once P2P runs.
    speech_sender: Mutex<Option<Arc<dyn SpeechSender>>>,
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
            speech_sender: Mutex::new(None),
        }
    }

    /// Lets the worker send speech to paired devices (#88).
    ///
    /// Side effects: Wakes the worker, which may have speech waiting to be sent.
    pub fn set_speech_sender(&self, sender: Arc<dyn SpeechSender>) {
        *self
            .speech_sender
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(sender);
        self.wake();
    }

    /// Returns the speech sender, if P2P runs.
    fn speech_sender(&self) -> Option<Arc<dyn SpeechSender>> {
        self.speech_sender
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Reports whether this device is a phone, which leaves combining to a desktop.
    pub fn is_phone(&self) -> bool {
        self.needs_charging
    }

    /// Reports whether work that needs no charging may run: no recording and not shutting down.
    pub fn is_idle(&self) -> bool {
        !self.recording.load(Ordering::SeqCst) && !self.shutting_down.load(Ordering::SeqCst)
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

/// Folds, sends, combines and re-transcribes meetings whose speech was retained.
pub struct RefinementWorker {
    store: Arc<RetainedAudioStore>,
    control: Arc<RefinementControl>,
    /// Loads the pass's model; `None` on a device without one, which never runs a pass.
    make_engine: Option<EngineFactory>,
    open_repository: RepositoryFactory,
    device_id: DeviceId,
    attempts: HashMap<String, u32>,
    /// The current time; replaced in tests.
    now: Box<dyn Fn() -> SystemTime + Send>,
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
            make_engine: Some(make_engine),
            open_repository,
            device_id,
            attempts: HashMap::new(),
            now: Box::new(SystemTime::now),
        }
    }

    /// Creates a worker for a device without a pass model (#88): it keeps its speech only to send
    /// it to a paired device that combines the meeting.
    pub fn without_pass(
        store: Arc<RetainedAudioStore>,
        control: Arc<RefinementControl>,
        open_repository: RepositoryFactory,
        device_id: DeviceId,
    ) -> Self {
        Self {
            store,
            control,
            make_engine: None,
            open_repository,
            device_id,
            attempts: HashMap::new(),
            now: Box::new(SystemTime::now),
        }
    }

    /// Replaces the clock, so tests can let time pass.
    pub fn with_clock(mut self, now: Box<dyn Fn() -> SystemTime + Send>) -> Self {
        self.now = now;
        self
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

    /// Runs one round: folds recordings, then carries out each retained meeting's plan.
    ///
    /// Outputs: How many meetings were re-transcribed (a combination counts once). Side effects:
    /// Writes folds; sends speech; loads the model and runs inference if a pass is due; rewrites
    /// transcripts; deletes retained audio as described in the module header.
    pub fn run_pending(&mut self) -> usize {
        if !self.control.is_idle() {
            return 0;
        }
        let repository = match (self.open_repository)() {
            Ok(repository) => repository,
            Err(error) => {
                tracing::warn!(error = %format!("{error:#}"), "the transcript pass cannot open the notes");
                return 0;
            }
        };
        self.fold_recordings(repository.as_ref());
        let pending = self.store.pending_meetings();
        if pending.is_empty() {
            return 0;
        }
        let recordings = match repository.meeting_recordings() {
            Ok(recordings) => recordings,
            Err(error) => {
                tracing::warn!(error = %error, "meetings could not be listed for the transcript pass");
                return 0;
            }
        };
        let held: HashSet<MeetingId> = pending.iter().cloned().collect();
        let mut engine: Option<(Box<dyn TranscriptionEngine>, Arc<ChunkAudioStore>)> = None;
        let mut handled: HashSet<MeetingId> = HashSet::new();
        let mut refined = 0;
        for meeting_id in pending {
            if handled.contains(&meeting_id) || !self.control.is_idle() {
                continue;
            }
            if !meeting_exists(repository.as_ref(), &meeting_id) {
                self.delete(&meeting_id);
                continue;
            }
            let age = self
                .store
                .written_at(&meeting_id)
                .and_then(|written| (self.now)().duration_since(written).ok())
                .unwrap_or_default();
            let plan = plan_retained_audio(
                &meeting_id,
                &recordings,
                &|meeting| held.contains(meeting),
                &self.device_id,
                age,
                self.make_engine.is_some(),
            );
            match plan {
                AudioPlan::Wait => {}
                AudioPlan::Discard => {
                    tracing::info!(meeting_id = %meeting_id.as_str(), "retained audio is no longer needed; deleting it");
                    self.delete(&meeting_id);
                }
                AudioPlan::Send { to } => self.send(&meeting_id, &to),
                AudioPlan::PassAlone | AudioPlan::Combine { .. } => {
                    if !self.control.may_run() {
                        continue;
                    }
                    let Some((engine, audio_source)) = self.loaded_engine(&mut engine) else {
                        return refined;
                    };
                    let succeeded = match plan {
                        AudioPlan::Combine { root, members } => {
                            handled.insert(root.clone());
                            handled.extend(members.iter().map(|(member, _)| member.clone()));
                            self.combine(&root, &members, engine, audio_source, repository.as_ref())
                        }
                        _ => {
                            self.refine_one(&meeting_id, engine, audio_source, repository.as_ref())
                        }
                    };
                    refined += usize::from(succeeded);
                    audio_source.clear();
                }
            }
        }
        refined
    }

    /// Loads the pass's model once per round.
    ///
    /// Outputs: The engine and its audio source, or `None` when there is no model or it cannot load.
    fn loaded_engine<'engine>(
        &self,
        engine: &'engine mut Option<(Box<dyn TranscriptionEngine>, Arc<ChunkAudioStore>)>,
    ) -> Option<(&'engine dyn TranscriptionEngine, &'engine ChunkAudioStore)> {
        if engine.is_none() {
            let make_engine = self.make_engine.as_ref()?;
            let audio_source = Arc::new(ChunkAudioStore::new());
            match make_engine(Arc::clone(&audio_source)) {
                Ok(loaded) => *engine = Some((loaded, audio_source)),
                Err(error) => {
                    tracing::warn!(error = %format!("{error:#}"), "the transcript pass cannot load its model");
                    return None;
                }
            }
        }
        engine
            .as_ref()
            .map(|(engine, audio_source)| (engine.as_ref(), audio_source.as_ref()))
    }

    /// Folds recordings of one meeting made here and on a paired device (#88).
    ///
    /// Side effects: Reads recent transcripts and writes folds, which sync to paired devices.
    /// Failures are logged and retried next round.
    fn fold_recordings(&self, repository: &dyn NoteRepository) {
        let found = repository.meeting_recordings().and_then(|recordings| {
            let since = chrono::DateTime::<chrono::Utc>::from((self.now)()) - FOLD_WINDOW;
            find_combinations(
                &recordings,
                &|meeting| repository.load_transcript_segments(meeting),
                &self.device_id,
                self.control.is_phone(),
                since,
            )
        });
        let found = match found {
            Ok(found) => found,
            Err(error) => {
                tracing::warn!(error = %error, "recordings could not be compared");
                return;
            }
        };
        for (meeting_id, combination) in found {
            match repository.combine_meeting(&meeting_id, &combination) {
                Ok(()) => tracing::info!(
                    meeting_id = %meeting_id.as_str(),
                    into = %combination.into.as_str(),
                    combine_on = %combination.combine_on.as_str(),
                    "One meeting was recorded on two devices; folded into the first recording"
                ),
                Err(error) => {
                    tracing::warn!(meeting_id = %meeting_id.as_str(), error = %error, "a recording could not be folded");
                }
            }
        }
    }

    /// Sends a meeting's speech to the device that combines it, then deletes it here.
    ///
    /// Side effects: Blocks while each part is delivered; deletes the audio after the last
    /// receipt. Any failure keeps the audio for the next round.
    fn send(&mut self, meeting_id: &MeetingId, to: &DeviceId) {
        let Some(sender) = self.control.speech_sender() else {
            return;
        };
        let chunks = match self.store.load(meeting_id) {
            Ok(chunks) => chunks,
            Err(RetainedAudioError::Unreadable) => {
                self.delete(meeting_id);
                return;
            }
            Err(error) => {
                tracing::warn!(meeting_id = %meeting_id.as_str(), error = %error, "retained audio could not be read for sending");
                return;
            }
        };
        let speech: Vec<SpeechChunk> = chunks
            .iter()
            .map(|chunk| SpeechChunk {
                start_ms: chunk.span().start_ms,
                end_ms: chunk.span().end_ms,
                language: chunk.language(),
                samples: Zeroizing::new(chunk.samples().to_vec()),
            })
            .collect();
        drop(chunks);
        let parts = split_into_parts(meeting_id, speech, PART_AUDIO_BUDGET_BYTES);
        let part_count = parts.len();
        for part in parts {
            if !self.control.is_idle() {
                return;
            }
            if let Err(error) = sender.send_part(to, part) {
                tracing::info!(meeting_id = %meeting_id.as_str(), peer = %to.as_str(), error = %error, "speech could not be sent yet; trying again later");
                return;
            }
        }
        tracing::info!(meeting_id = %meeting_id.as_str(), peer = %to.as_str(), part_count, "Speech sent to the device that combines the meeting");
        self.delete(meeting_id);
    }

    /// Re-transcribes a meeting from every recording of it held here, then deletes their audio.
    ///
    /// Outputs: Whether the transcript was replaced.
    fn combine(
        &mut self,
        root: &MeetingId,
        members: &[(MeetingId, i64)],
        engine: &dyn TranscriptionEngine,
        audio_source: &ChunkAudioStore,
        repository: &dyn NoteRepository,
    ) -> bool {
        let mut loaded: Vec<(MeetingId, i64, Vec<RetainedChunk>)> = Vec::new();
        for (meeting_id, offset_ms) in
            std::iter::once((root.clone(), 0)).chain(members.iter().cloned())
        {
            match self.store.load(&meeting_id) {
                Ok(chunks) => loaded.push((meeting_id, offset_ms, chunks)),
                Err(RetainedAudioError::Unreadable) => {
                    tracing::warn!(meeting_id = %meeting_id.as_str(), "retained audio is unreadable; deleting it");
                    self.delete(&meeting_id);
                    return false;
                }
                Err(error) => {
                    tracing::warn!(meeting_id = %meeting_id.as_str(), error = %error, "retained audio could not be read");
                    return false;
                }
            }
        }
        let mut recordings =
            loaded
                .iter()
                .map(|(meeting_id, offset_ms, chunks)| CombinedRecording {
                    meeting_id: meeting_id.clone(),
                    chunks: chunks.iter().map(borrow_chunk).collect(),
                    offset_ms: *offset_ms,
                });
        let Some(root_recording) = recordings.next() else {
            return false;
        };
        let member_recordings: Vec<CombinedRecording<'_>> = recordings.collect();
        let control = Arc::clone(&self.control);
        let outcome = refine_combined_meeting(
            &root_recording,
            &member_recordings,
            engine,
            &|recording_id, samples| audio_source.insert(recording_id, samples),
            repository,
            &self.device_id,
            &|| !control.may_run(),
        );
        let all: Vec<MeetingId> = loaded
            .iter()
            .map(|(meeting_id, _, _)| meeting_id.clone())
            .collect();
        drop(member_recordings);
        drop(root_recording);
        drop(loaded);
        self.settle(
            root,
            &all,
            outcome,
            "Meeting re-transcribed from every device that recorded it",
        )
    }

    /// Decides what happens to the audio after a pass over `meetings` (the first is the transcript
    /// rewritten).
    ///
    /// Outputs: Whether the transcript was replaced. Side effects: Deletes the audio after success
    /// or after three failures; logs counts only.
    fn settle(
        &mut self,
        root: &MeetingId,
        meetings: &[MeetingId],
        outcome: Result<RefinementOutcome, RefinementError>,
        done: &'static str,
    ) -> bool {
        match outcome {
            Ok(outcome) => {
                tracing::info!(
                    meeting_id = %root.as_str(),
                    recording_count = meetings.len(),
                    window_count = outcome.window_count,
                    segment_count = outcome.segment_count,
                    "{done}"
                );
                for meeting_id in meetings {
                    self.delete(meeting_id);
                }
                true
            }
            Err(RefinementError::Cancelled) => false,
            Err(error) => {
                let attempts = self.attempts.entry(root.as_str().to_owned()).or_insert(0);
                *attempts += 1;
                let attempts = *attempts;
                tracing::warn!(meeting_id = %root.as_str(), error = %error, attempts, "the transcript pass failed; the live transcript stands");
                if attempts >= MAX_ATTEMPTS {
                    for meeting_id in meetings {
                        self.delete(meeting_id);
                    }
                }
                false
            }
        }
    }

    /// Runs the pass for one meeting alone and decides what happens to its audio.
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
        if chunks.is_empty() {
            self.delete(meeting_id);
            return false;
        }
        let borrowed: Vec<RefinementChunk<'_>> = chunks.iter().map(borrow_chunk).collect();
        let control = Arc::clone(&self.control);
        let provide = |recording_id: &crate::domain::models::RecordingId, samples: Vec<i16>| {
            audio_source.insert(recording_id, samples);
        };
        let cancelled = || !control.may_run();
        let outcome = refine_meeting(
            meeting_id,
            &borrowed,
            engine,
            &provide,
            repository,
            &self.device_id,
            &cancelled,
        );
        drop(borrowed);
        drop(chunks);
        self.settle(
            meeting_id,
            std::slice::from_ref(meeting_id),
            outcome,
            "Meeting transcript re-transcribed after the meeting",
        )
    }

    /// Deletes a meeting's retained audio and forgets its attempts.
    fn delete(&mut self, meeting_id: &MeetingId) {
        self.attempts.remove(meeting_id.as_str());
        if let Err(error) = self.store.delete(meeting_id) {
            tracing::warn!(meeting_id = %meeting_id.as_str(), error = %error, "retained audio could not be deleted");
        }
    }
}

/// Borrows a retained chunk for a pass.
fn borrow_chunk(chunk: &RetainedChunk) -> RefinementChunk<'_> {
    RefinementChunk {
        span: chunk.span(),
        language: chunk.language(),
        samples: chunk.samples(),
    }
}

/// Stores speech that paired devices send for combining (#88), sealed under this device's key.
///
/// Only speech of a meeting this device combines is taken: one folded into another meeting with
/// this device named to combine it, or the first recording of such a fold. Parts must come in
/// order; a part already stored is confirmed again, and part 0 starts over.
pub struct RetainedSpeechInbox {
    store: Arc<RetainedAudioStore>,
    open_repository: RepositoryFactory,
    device_id: DeviceId,
    control: Arc<RefinementControl>,
    /// The next part expected for each meeting being received.
    next_parts: HashMap<MeetingId, u32>,
}

impl RetainedSpeechInbox {
    /// Creates the inbox.
    ///
    /// Inputs: The store, the repository opener, this device, and the worker's control (woken
    /// when a meeting's speech is complete). Outputs: The inbox. Side effects: None.
    pub fn new(
        store: Arc<RetainedAudioStore>,
        open_repository: RepositoryFactory,
        device_id: DeviceId,
        control: Arc<RefinementControl>,
    ) -> Self {
        Self {
            store,
            open_repository,
            device_id,
            control,
            next_parts: HashMap::new(),
        }
    }

    /// Reports whether this device combines `meeting_id`'s recordings.
    fn combines_here(&self, meeting_id: &MeetingId) -> bool {
        let Ok(recordings) = (self.open_repository)()
            .and_then(|repository| repository.meeting_recordings().map_err(anyhow::Error::from))
        else {
            return false;
        };
        recordings.iter().any(|recording| {
            recording.combined_into.as_ref().is_some_and(|combination| {
                combination.combine_on == self.device_id
                    && (recording.meeting_id == *meeting_id || combination.into == *meeting_id)
            })
        })
    }
}

impl SpeechInbox for RetainedSpeechInbox {
    fn receive_part(
        &mut self,
        from: &DeviceId,
        part: SpeechPart,
    ) -> Result<(), SpeechTransferError> {
        let meeting_id = part.meeting_id.clone();
        if part.part_index == 0 {
            if !self.combines_here(&meeting_id) {
                tracing::info!(meeting_id = %meeting_id.as_str(), peer = %from.as_str(), "speech for a meeting not combined here was refused");
                return Err(SpeechTransferError::Refused);
            }
            if self.store.begin_incoming(&meeting_id).is_err() {
                return Err(SpeechTransferError::Refused);
            }
            self.next_parts.insert(meeting_id.clone(), 0);
        }
        let Some(expected) = self.next_parts.get(&meeting_id).copied() else {
            return Err(SpeechTransferError::Refused);
        };
        if part.part_index < expected {
            return Ok(());
        }
        if part.part_index > expected {
            return Err(SpeechTransferError::Refused);
        }
        for chunk in &part.chunks {
            let span = ChunkSpan {
                start_ms: chunk.start_ms,
                end_ms: chunk.end_ms,
            };
            if self
                .store
                .append_incoming(&meeting_id, span, chunk.language, &chunk.samples)
                .is_err()
            {
                self.next_parts.remove(&meeting_id);
                return Err(SpeechTransferError::Refused);
            }
        }
        let next = expected + 1;
        if next < part.part_count {
            self.next_parts.insert(meeting_id, next);
            return Ok(());
        }
        self.next_parts.remove(&meeting_id);
        if self.store.finish_incoming(&meeting_id).is_err() {
            return Err(SpeechTransferError::Refused);
        }
        tracing::info!(meeting_id = %meeting_id.as_str(), peer = %from.as_str(), part_count = part.part_count, "Received a paired device's speech of a meeting combined here");
        self.control.wake();
        Ok(())
    }
}

/// Reports whether the meeting still stands: false only when it was deleted meanwhile, so a failed
/// lookup never costs a meeting its pass.
fn meeting_exists(repository: &dyn NoteRepository, meeting_id: &MeetingId) -> bool {
    !repository.is_meeting_deleted(meeting_id).unwrap_or(false)
}
