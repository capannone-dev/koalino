//! Runs paired devices' transcriptions on a thread of their own (Issue #59).
//!
//! The P2P thread must never wait for AI work, so jobs go through a small queue to this worker.
//! The engine is built on the first job: a device nobody asks never loads a second model. Each
//! job's audio sits in a private `ChunkAudioStore` that the engine reads and deletes; whatever is
//! left is discarded when the job ends, so raw audio from another device never outlives its job
//! and never touches the disk here. Nothing here logs audio or text.

use std::{
    sync::{
        Arc,
        mpsc::{Receiver, SyncSender, TrySendError, channel, sync_channel},
    },
    thread,
};

use anyhow::{Context, Result};
use uuid::Uuid;
use zeroize::Zeroize;

use crate::{
    adapters::audio::chunk_audio_store::ChunkAudioStore,
    domain::models::{AudioRecording, DeviceId, MeetingId, RecordingId, TranscriptionContext},
    ports::{
        ai::TranscriptionEngine,
        remote_transcription::{FinishedTranscription, HostedTranscription, TranscriptionHost},
    },
};

/// How many jobs may wait; a device asking beyond this is told to transcribe elsewhere.
const JOB_QUEUE_CAPACITY: usize = 4;

/// The P2P thread's handle to the worker. Dropping it ends the worker after its current job.
pub struct ThreadedTranscriptionHost {
    jobs: SyncSender<HostedTranscription>,
    finished: Receiver<FinishedTranscription>,
}

impl ThreadedTranscriptionHost {
    /// Starts the worker.
    ///
    /// Inputs: Builds the engine over the store it should read audio from; a failure fails that
    /// job and the next job tries again.
    /// Outputs: The handle, or an error when the thread cannot start.
    /// Side effects: Spawns the `note-taker-transcription-host` thread.
    pub fn spawn<Engine, BuildEngine>(build_engine: BuildEngine) -> Result<Self>
    where
        Engine: TranscriptionEngine,
        BuildEngine: Fn(&Arc<ChunkAudioStore>) -> Result<Engine> + Send + 'static,
    {
        let (job_sender, job_receiver) = sync_channel::<HostedTranscription>(JOB_QUEUE_CAPACITY);
        let (finished_sender, finished_receiver) = channel();
        thread::Builder::new()
            .name("note-taker-transcription-host".to_owned())
            .spawn(move || {
                let store = Arc::new(ChunkAudioStore::new());
                let mut engine: Option<Engine> = None;
                while let Ok(job) = job_receiver.recv() {
                    let (peer, job_id) = (job.peer.clone(), job.job_id);
                    if engine.is_none() {
                        match build_engine(&store) {
                            Ok(built) => engine = Some(built),
                            Err(error) => tracing::warn!(
                                error = %format!("{error:#}"),
                                "no engine for a paired device's transcription"
                            ),
                        }
                    }
                    let text = run_job(engine.as_ref(), &store, job);
                    let finished = FinishedTranscription { peer, job_id, text };
                    if finished_sender.send(finished).is_err() {
                        return;
                    }
                }
            })
            .context("unable to start the transcription host")?;
        Ok(Self {
            jobs: job_sender,
            finished: finished_receiver,
        })
    }
}

/// Transcribes one job and deletes its audio.
///
/// Inputs: The engine, if built; the store it reads; the job.
/// Outputs: The text, or `None` on any failure.
/// Side effects: Runs AI inference; the audio is gone from memory on return.
fn run_job<Engine: TranscriptionEngine>(
    engine: Option<&Engine>,
    store: &ChunkAudioStore,
    mut job: HostedTranscription,
) -> Option<String> {
    let Some(engine) = engine else {
        job.samples.zeroize();
        return None;
    };
    let recording_id = RecordingId::new(format!("hosted-{}", Uuid::new_v4()));
    store.insert(&recording_id, std::mem::take(&mut job.samples));
    // Only the engine's audio lookup uses these IDs; the requester keeps the real meeting.
    let recording = AudioRecording::new(
        recording_id.clone(),
        MeetingId::new("hosted"),
        DeviceId::new(job.peer.as_str()),
    );
    let context = TranscriptionContext::new(job.previous_text.take()).with_language(job.language);
    let result = engine.transcribe(&recording, &context);
    store.discard(&recording_id);
    match result {
        Ok(transcript) => Some(transcript.content().to_owned()),
        Err(error) => {
            tracing::warn!(error = %error, "a paired device's transcription failed");
            None
        }
    }
}

impl TranscriptionHost for ThreadedTranscriptionHost {
    /// Queues a job, or refuses it when the queue is full.
    fn submit(&mut self, job: HostedTranscription) -> bool {
        match self.jobs.try_send(job) {
            Ok(()) => true,
            Err(TrySendError::Full(mut job) | TrySendError::Disconnected(mut job)) => {
                job.samples.zeroize();
                false
            }
        }
    }

    /// Takes one finished job without waiting.
    fn poll_finished(&mut self) -> Option<FinishedTranscription> {
        self.finished.try_recv().ok()
    }
}

#[cfg(test)]
#[path = "transcription_host_tests.rs"]
mod tests;
