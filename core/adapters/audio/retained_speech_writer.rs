//! Compresses and keeps a recording's speech on a background thread (#88).
//!
//! Each speech chunk of a recording is kept sealed for the after-meeting pass
//! (`RetainedAudioStore`). Compressing it with Opus takes a noticeable share of a second per chunk
//! on a phone, too long for the call that delivers captured audio, which holds the engine. The
//! recorder therefore only queues the chunk here; one `retained-speech` thread compresses and
//! appends the chunks in the order they were queued. `flush` waits until every queued chunk is
//! written, and the recorder calls it before it lets the pass run, so the pass never reads a
//! meeting whose last chunks are still queued. Queued samples are wiped once written; a chunk
//! that cannot be kept is logged without its audio, and the live transcript is unaffected.

use std::{
    sync::{
        Arc, Mutex, PoisonError,
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
};

use zeroize::Zeroizing;

use super::retained_audio_store::RetainedAudioStore;
use crate::{
    application::transcript_refinement::ChunkSpan,
    domain::{models::MeetingId, transcription_language::TranscriptionLanguage},
};

/// Work for the writer thread.
enum Job {
    /// Compress and append one chunk.
    Append {
        meeting_id: MeetingId,
        span: ChunkSpan,
        language: Option<TranscriptionLanguage>,
        samples: Zeroizing<Vec<i16>>,
    },
    /// Answer once every chunk queued before this one is written.
    Flush(Sender<()>),
}

/// Queues speech chunks for a thread that compresses and keeps them, in order.
pub struct RetainedSpeechWriter {
    /// The queue to the thread; `None` once the writer is being dropped.
    jobs: Mutex<Option<Sender<Job>>>,
    /// The writer thread, joined when the writer is dropped.
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl RetainedSpeechWriter {
    /// Starts the writer thread over `store`.
    ///
    /// Inputs: The store the chunks are kept in.
    /// Outputs: The writer, or the error that kept the thread from starting.
    /// Side effects: Spawns the `retained-speech` thread.
    pub fn spawn(store: Arc<RetainedAudioStore>) -> std::io::Result<Self> {
        let (jobs, queue) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("retained-speech".to_owned())
            .spawn(move || write_queued(&store, &queue))?;
        Ok(Self {
            jobs: Mutex::new(Some(jobs)),
            thread: Mutex::new(Some(thread)),
        })
    }

    /// Queues one chunk to be compressed and kept; returns without waiting for it.
    ///
    /// Inputs: The meeting, the chunk's place in it, its language, and its 16 kHz mono samples.
    /// Outputs: None; a chunk that cannot be queued or kept is logged (without audio) and skipped,
    /// as the pass then reads the meeting without it.
    /// Side effects: Hands the samples to the writer thread, which wipes them once written.
    pub fn append_chunk(
        &self,
        meeting_id: &MeetingId,
        span: ChunkSpan,
        language: Option<TranscriptionLanguage>,
        samples: Vec<i16>,
    ) {
        let job = Job::Append {
            meeting_id: meeting_id.clone(),
            span,
            language,
            samples: Zeroizing::new(samples),
        };
        if !self.send(job) {
            tracing::warn!(
                "a chunk could not be kept for the after-meeting pass: the writer stopped"
            );
        }
    }

    /// Waits until every chunk queued so far is written (or failed).
    ///
    /// Side effects: Blocks the calling thread while the writer catches up; returns at once when
    /// the writer thread is gone.
    pub fn flush(&self) {
        let (done, finished) = mpsc::channel();
        if self.send(Job::Flush(done)) {
            let _ = finished.recv();
        }
    }

    /// Sends a job to the thread. Outputs: Whether the thread took it.
    fn send(&self, job: Job) -> bool {
        self.jobs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|jobs| jobs.send(job).is_ok())
    }
}

impl Drop for RetainedSpeechWriter {
    /// Closes the queue and waits for the thread to write what was queued.
    fn drop(&mut self) {
        drop(
            self.jobs
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take(),
        );
        if let Some(thread) = self
            .thread
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            && thread.join().is_err()
        {
            tracing::warn!("the retained-speech writer stopped unexpectedly");
        }
    }
}

/// The writer thread: carries out jobs in order until the queue closes.
fn write_queued(store: &RetainedAudioStore, queue: &Receiver<Job>) {
    for job in queue {
        match job {
            Job::Append {
                meeting_id,
                span,
                language,
                samples,
            } => {
                if let Err(error) = store.append_chunk(&meeting_id, span, language, &samples) {
                    // The live transcript is unaffected; the pass will read the meeting without it.
                    tracing::warn!(meeting_id = %meeting_id.as_str(), error = %error, "a chunk could not be kept for the after-meeting pass");
                }
            }
            Job::Flush(done) => {
                let _ = done.send(());
            }
        }
    }
}
