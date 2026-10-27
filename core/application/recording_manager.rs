//! Runs meeting pipeline sessions for live recordings on one dedicated worker thread.
//!
//! The port traits are not `Send` (several engines hold non-thread-safe model handles), so the
//! worker builds its `PipelineFactory` itself from a `Send` builder closure and keeps it for its
//! whole life. Callers on any thread talk to the worker through a FIFO command channel:
//! `begin_recording` opens a session, `submit_chunk` queues VAD chunks without blocking capture,
//! and `end_recording` returns only after every chunk queued before it has been processed. Heavy
//! transcription therefore never runs on a UI thread or on the async runtime.
//!
//! A chunk that fails is logged and counted; the recording continues. Dropping the manager ends
//! an active recording after draining its queue, then stops the worker. Nothing here logs
//! transcript text, audio, or embeddings.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
};

use crate::{
    domain::models::MeetingId,
    ports::{
        ai::{DiarizationEngine, TranscriptionEngine},
        storage::NoteRepository,
        voice::VoiceIdentityMatcher,
    },
};

use super::pipeline::{
    meeting_session::{MeetingSessionError, VadAudioChunk},
    pipeline_factory::{BorrowedMeetingSession, PipelineAvailability, PipelineFactory},
};

/// Reports why a recording command could not be carried out.
#[derive(Debug, thiserror::Error)]
pub enum RecordingManagerError {
    /// The pipeline builder failed, e.g. no transcription engine is configured.
    #[error("meeting pipeline is unavailable: {reason}")]
    PipelineUnavailable {
        /// The builder's error message.
        reason: String,
    },
    /// A recording is already active.
    #[error("a recording is already in progress")]
    AlreadyRecording,
    /// No recording is active.
    #[error("no recording is in progress")]
    NotRecording,
    /// The meeting session could not be opened.
    #[error(transparent)]
    Session(#[from] MeetingSessionError),
    /// The worker thread has stopped (it panicked or was shut down).
    #[error("the recording worker has stopped")]
    WorkerStopped,
}

/// Summarizes one finished recording.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordingSummary {
    meeting_id: MeetingId,
    processed_chunk_count: usize,
    failed_chunk_count: usize,
    segment_count: usize,
}

impl RecordingSummary {
    /// Returns the recorded meeting.
    pub fn meeting_id(&self) -> &MeetingId {
        &self.meeting_id
    }

    /// Returns how many chunks were transcribed and persisted.
    pub fn processed_chunk_count(&self) -> usize {
        self.processed_chunk_count
    }

    /// Returns how many chunks failed and were skipped.
    pub fn failed_chunk_count(&self) -> usize {
        self.failed_chunk_count
    }

    /// Returns how many transcript segments the recording persisted.
    pub fn segment_count(&self) -> usize {
        self.segment_count
    }
}

/// A repository query run on the worker thread, which owns the repository.
type RepositoryTask = Box<dyn FnOnce(&dyn NoteRepository) + Send>;

/// One request to the worker; processed strictly in send order.
enum WorkerCommand {
    /// Opens a session for the meeting.
    Begin {
        meeting_id: MeetingId,
        reply: Sender<Result<(), RecordingManagerError>>,
    },
    /// Processes one chunk in the active session.
    Chunk(VadAudioChunk),
    /// Closes the active session after all earlier chunks.
    End {
        reply: Sender<Result<RecordingSummary, RecordingManagerError>>,
    },
    /// Runs a query against the worker's repository.
    WithRepository(RepositoryTask),
}

/// Handle to the recording worker; safe to share between threads.
pub struct RecordingManager {
    command_sender: Option<Sender<WorkerCommand>>,
    worker_handle: Option<JoinHandle<()>>,
    availability: PipelineAvailability,
    /// Chunks submitted but not yet processed.
    queued_chunk_count: Arc<AtomicUsize>,
    /// Mirrors whether the worker has an open session, for cheap status queries.
    recording_active: Arc<AtomicBool>,
}

impl RecordingManager {
    /// Starts the worker thread and builds the pipeline on it.
    ///
    /// Inputs: `build_pipeline` runs once on the worker and returns the composed factory, or an
    /// error explaining which required dependency is missing.
    /// Outputs: The manager once the pipeline is ready, or `PipelineUnavailable` (the worker has
    /// then exited), or `WorkerStopped` if the thread could not start.
    /// Side effects: Spawns a thread; blocks the caller while the pipeline (and any model) loads,
    /// so call it from a background thread or `spawn_blocking`, never a UI thread.
    pub fn spawn<
        Builder,
        BuildError,
        TranscriptionPort,
        DiarizationPort,
        MatcherPort,
        RepositoryPort,
    >(
        build_pipeline: Builder,
    ) -> Result<Self, RecordingManagerError>
    where
        Builder: FnOnce() -> Result<
                PipelineFactory<TranscriptionPort, DiarizationPort, MatcherPort, RepositoryPort>,
                BuildError,
            > + Send
            + 'static,
        BuildError: std::fmt::Display,
        TranscriptionPort: TranscriptionEngine,
        DiarizationPort: DiarizationEngine + Sync,
        MatcherPort: VoiceIdentityMatcher,
        RepositoryPort: NoteRepository,
    {
        let (command_sender, command_receiver) = mpsc::channel();
        let (ready_sender, ready_receiver) = mpsc::channel();
        let queued_chunk_count = Arc::new(AtomicUsize::new(0));
        let recording_active = Arc::new(AtomicBool::new(false));
        let worker_state = WorkerState {
            queued_chunk_count: Arc::clone(&queued_chunk_count),
            recording_active: Arc::clone(&recording_active),
        };
        let worker_handle = thread::Builder::new()
            .name("meeting-pipeline".to_owned())
            .spawn(move || {
                let factory = match build_pipeline() {
                    Ok(factory) => factory,
                    Err(error) => {
                        let _ = ready_sender.send(Err(error.to_string()));
                        return;
                    }
                };
                if ready_sender.send(Ok(factory.availability())).is_err() {
                    return;
                }
                run_worker(&factory, &command_receiver, &worker_state);
            })
            .map_err(|_| RecordingManagerError::WorkerStopped)?;

        let availability = match ready_receiver.recv() {
            Ok(Ok(availability)) => availability,
            Ok(Err(reason)) => {
                let _ = worker_handle.join();
                return Err(RecordingManagerError::PipelineUnavailable { reason });
            }
            Err(_) => {
                let _ = worker_handle.join();
                return Err(RecordingManagerError::WorkerStopped);
            }
        };
        tracing::info!(
            diarization_available = availability.diarization,
            voice_matching_available = availability.voice_matching,
            "Meeting pipeline ready"
        );
        Ok(Self {
            command_sender: Some(command_sender),
            worker_handle: Some(worker_handle),
            availability,
            queued_chunk_count,
            recording_active,
        })
    }

    /// Reports which optional pipeline dependencies the worker was built with.
    pub fn availability(&self) -> PipelineAvailability {
        self.availability
    }

    /// Reports whether a recording session is open.
    pub fn is_recording(&self) -> bool {
        self.recording_active.load(Ordering::SeqCst)
    }

    /// Returns how many submitted chunks are still waiting or being processed.
    pub fn queued_chunk_count(&self) -> usize {
        self.queued_chunk_count.load(Ordering::SeqCst)
    }

    /// Opens a pipeline session for `meeting_id`, creating the meeting if needed.
    ///
    /// Inputs: The meeting to record into.
    /// Outputs: `Ok(())`, `AlreadyRecording`, a session error, or `WorkerStopped`.
    /// Side effects: Blocks until the worker has opened the session; may create the meeting.
    pub fn begin_recording(&self, meeting_id: MeetingId) -> Result<(), RecordingManagerError> {
        let (reply, reply_receiver) = mpsc::channel();
        self.send(WorkerCommand::Begin { meeting_id, reply })?;
        reply_receiver
            .recv()
            .map_err(|_| RecordingManagerError::WorkerStopped)?
    }

    /// Queues one VAD chunk for the active recording without waiting for it.
    ///
    /// Inputs: A chunk of the meeting passed to `begin_recording`; its audio is never logged.
    /// Outputs: `Ok(())` once queued, `NotRecording`, or `WorkerStopped`.
    /// Side effects: The worker later transcribes, diarizes, and persists the chunk.
    pub fn submit_chunk(&self, vad_chunk: VadAudioChunk) -> Result<(), RecordingManagerError> {
        if !self.is_recording() {
            return Err(RecordingManagerError::NotRecording);
        }
        self.queued_chunk_count.fetch_add(1, Ordering::SeqCst);
        self.send(WorkerCommand::Chunk(vad_chunk)).inspect_err(|_| {
            self.queued_chunk_count.fetch_sub(1, Ordering::SeqCst);
        })
    }

    /// Ends the active recording once every chunk queued before this call is processed.
    ///
    /// Inputs: None.
    /// Outputs: The recording summary, `NotRecording`, or `WorkerStopped`.
    /// Side effects: Blocks until the queue drains (graceful shutdown), then closes the session.
    pub fn end_recording(&self) -> Result<RecordingSummary, RecordingManagerError> {
        let (reply, reply_receiver) = mpsc::channel();
        self.send(WorkerCommand::End { reply })?;
        reply_receiver
            .recv()
            .map_err(|_| RecordingManagerError::WorkerStopped)?
    }

    /// Runs `query` against the worker's repository after all earlier commands.
    ///
    /// Inputs: A `Send` closure that reads or writes through the repository port.
    /// Outputs: The closure's result, or `WorkerStopped`.
    /// Side effects: Whatever `query` does; blocks until it has run.
    pub fn with_note_repository<Output, Query>(
        &self,
        query: Query,
    ) -> Result<Output, RecordingManagerError>
    where
        Output: Send + 'static,
        Query: FnOnce(&dyn NoteRepository) -> Output + Send + 'static,
    {
        let (reply, reply_receiver) = mpsc::channel();
        self.send(WorkerCommand::WithRepository(Box::new(
            move |note_repository| {
                let _ = reply.send(query(note_repository));
            },
        )))?;
        reply_receiver
            .recv()
            .map_err(|_| RecordingManagerError::WorkerStopped)
    }

    /// Sends one command to the worker.
    ///
    /// Inputs: The command.
    /// Outputs: `Ok(())`, or `WorkerStopped` when the worker has exited.
    /// Side effects: Enqueues the command.
    fn send(&self, command: WorkerCommand) -> Result<(), RecordingManagerError> {
        self.command_sender
            .as_ref()
            .ok_or(RecordingManagerError::WorkerStopped)?
            .send(command)
            .map_err(|_| RecordingManagerError::WorkerStopped)
    }
}

impl Drop for RecordingManager {
    /// Closes the command channel and waits for the worker to drain and exit.
    fn drop(&mut self) {
        self.command_sender = None;
        if let Some(worker_handle) = self.worker_handle.take()
            && worker_handle.join().is_err()
        {
            tracing::error!("Meeting pipeline worker panicked");
        }
    }
}

/// Counters shared between the handle and the worker.
struct WorkerState {
    queued_chunk_count: Arc<AtomicUsize>,
    recording_active: Arc<AtomicBool>,
}

/// One open recording on the worker.
struct ActiveRecording<'factory, TranscriptionPort, DiarizationPort, MatcherPort, RepositoryPort> {
    session: BorrowedMeetingSession<
        'factory,
        TranscriptionPort,
        DiarizationPort,
        MatcherPort,
        RepositoryPort,
    >,
    meeting_id: MeetingId,
    processed_chunk_count: usize,
    failed_chunk_count: usize,
    segment_count: usize,
}

/// Processes commands until the channel closes, then ends any active recording.
///
/// Inputs: The worker-owned factory, the command channel, and the shared counters.
/// Outputs: None.
/// Side effects: Runs pipeline sessions (transcription, diarization, SQLite writes).
fn run_worker<TranscriptionPort, DiarizationPort, MatcherPort, RepositoryPort>(
    factory: &PipelineFactory<TranscriptionPort, DiarizationPort, MatcherPort, RepositoryPort>,
    command_receiver: &Receiver<WorkerCommand>,
    worker_state: &WorkerState,
) where
    TranscriptionPort: TranscriptionEngine,
    DiarizationPort: DiarizationEngine + Sync,
    MatcherPort: VoiceIdentityMatcher,
    RepositoryPort: NoteRepository,
{
    let mut active_recording = None;
    for command in command_receiver {
        match command {
            WorkerCommand::Begin { meeting_id, reply } => {
                let outcome = if active_recording.is_some() {
                    Err(RecordingManagerError::AlreadyRecording)
                } else {
                    factory
                        .create_session(&meeting_id)
                        .map(|session| {
                            worker_state.recording_active.store(true, Ordering::SeqCst);
                            tracing::info!(meeting_id = %meeting_id.as_str(), "Recording started");
                            active_recording = Some(ActiveRecording {
                                session,
                                meeting_id,
                                processed_chunk_count: 0,
                                failed_chunk_count: 0,
                                segment_count: 0,
                            });
                        })
                        .map_err(RecordingManagerError::from)
                };
                let _ = reply.send(outcome);
            }
            WorkerCommand::Chunk(vad_chunk) => {
                match active_recording.as_mut() {
                    Some(recording) => process_chunk(recording, &vad_chunk),
                    None => tracing::warn!("Audio chunk dropped: no recording is in progress"),
                }
                worker_state
                    .queued_chunk_count
                    .fetch_sub(1, Ordering::SeqCst);
            }
            WorkerCommand::End { reply } => {
                let outcome = active_recording
                    .take()
                    .map(|recording| finish_recording(recording, worker_state))
                    .ok_or(RecordingManagerError::NotRecording);
                let _ = reply.send(outcome);
            }
            WorkerCommand::WithRepository(query) => query(factory.note_repository()),
        }
    }
    if let Some(recording) = active_recording {
        let summary = finish_recording(recording, worker_state);
        tracing::warn!(
            meeting_id = %summary.meeting_id().as_str(),
            "Recording ended because the pipeline shut down"
        );
    }
}

/// Runs one chunk through the session and updates the recording's counters.
///
/// Inputs: The active recording and its chunk.
/// Outputs: None; failures are logged and counted, never propagated.
/// Side effects: Transcribes, diarizes, and persists through the session's ports.
fn process_chunk<TranscriptionPort, DiarizationPort, MatcherPort, RepositoryPort>(
    recording: &mut ActiveRecording<
        '_,
        TranscriptionPort,
        DiarizationPort,
        MatcherPort,
        RepositoryPort,
    >,
    vad_chunk: &VadAudioChunk,
) where
    TranscriptionPort: TranscriptionEngine,
    DiarizationPort: DiarizationEngine + Sync,
    MatcherPort: VoiceIdentityMatcher,
    RepositoryPort: NoteRepository,
{
    match futures::executor::block_on(recording.session.process_vad_chunk(vad_chunk)) {
        Ok(produced_segments) => {
            recording.processed_chunk_count += 1;
            recording.segment_count += produced_segments.len();
        }
        Err(error) => {
            recording.failed_chunk_count += 1;
            tracing::warn!(
                meeting_id = %recording.meeting_id.as_str(),
                error = %error,
                "Audio chunk skipped after a pipeline failure"
            );
        }
    }
}

/// Closes a recording and reports its summary.
///
/// Inputs: The recording to close and the shared counters.
/// Outputs: The summary.
/// Side effects: Drops the session and clears the recording flag.
fn finish_recording<TranscriptionPort, DiarizationPort, MatcherPort, RepositoryPort>(
    recording: ActiveRecording<'_, TranscriptionPort, DiarizationPort, MatcherPort, RepositoryPort>,
    worker_state: &WorkerState,
) -> RecordingSummary {
    worker_state.recording_active.store(false, Ordering::SeqCst);
    let summary = RecordingSummary {
        meeting_id: recording.meeting_id,
        processed_chunk_count: recording.processed_chunk_count,
        failed_chunk_count: recording.failed_chunk_count,
        segment_count: recording.segment_count,
    };
    tracing::info!(
        meeting_id = %summary.meeting_id.as_str(),
        processed_chunk_count = summary.processed_chunk_count,
        failed_chunk_count = summary.failed_chunk_count,
        segment_count = summary.segment_count,
        "Recording stopped after draining queued chunks"
    );
    summary
}
