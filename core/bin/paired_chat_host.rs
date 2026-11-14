//! Answers paired devices' chat questions on a thread of their own (#107).
//!
//! The P2P thread must never wait for a model, so questions go through a small queue to this
//! worker, one at a time. A computer whose owner did not choose a connection for paired devices
//! declines at once (`not_offered`), as does a full queue (`busy`) and a device asking more than
//! `PAIRED_CHATS_PER_HOUR` questions in an hour (`rate_limited`), which bounds what a paired
//! device can spend on the owner's model plan. Nothing here logs questions or answers.

use std::{
    collections::VecDeque,
    sync::mpsc::{Receiver, SyncSender, TrySendError, channel, sync_channel},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};

use crate::{
    application::{
        p2p_service::{FinishedPairedChat, PairedChatHost},
        paired_chat_exchange::{PairedChatOutcome, PairedChatRequest},
    },
    domain::models::DeviceId,
};

/// Questions answered for paired devices per hour, at most.
pub const PAIRED_CHATS_PER_HOUR: usize = 30;
/// How many questions may wait.
const QUEUE_CAPACITY: usize = 4;
/// The window of `PAIRED_CHATS_PER_HOUR`.
const HOUR: Duration = Duration::from_secs(60 * 60);

/// One queued question.
type Question = (DeviceId, PairedChatRequest);

/// The P2P thread's handle to the worker. Dropping it ends the worker after its current answer.
pub struct ThreadedPairedChatHost {
    offered: Box<dyn Fn() -> bool + Send>,
    questions: SyncSender<Question>,
    finished: Receiver<FinishedPairedChat>,
    /// When recent questions were taken, oldest first.
    taken_at: VecDeque<Instant>,
}

impl ThreadedPairedChatHost {
    /// Starts the worker.
    ///
    /// Inputs: `offered` (whether this computer answers paired devices now; read on every
    /// question, so the owner's setting applies at once) and `answer` (answers one question;
    /// it may block for minutes). Outputs: The handle, or an error when the thread cannot start.
    /// Side effects: Spawns the `note-taker-paired-chat` thread.
    pub fn spawn<Offered, Answer>(offered: Offered, mut answer: Answer) -> Result<Self>
    where
        Offered: Fn() -> bool + Send + 'static,
        Answer: FnMut(&DeviceId, PairedChatRequest) -> PairedChatOutcome + Send + 'static,
    {
        let (question_sender, question_receiver) = sync_channel::<Question>(QUEUE_CAPACITY);
        let (finished_sender, finished_receiver) = channel();
        thread::Builder::new()
            .name("note-taker-paired-chat".to_owned())
            .spawn(move || {
                while let Ok((peer, request)) = question_receiver.recv() {
                    let job_id = request.job_id;
                    let outcome = answer(&peer, request);
                    let finished = FinishedPairedChat {
                        peer,
                        job_id,
                        outcome,
                    };
                    if finished_sender.send(finished).is_err() {
                        return;
                    }
                }
            })
            .context("unable to start answering paired devices' chats")?;
        Ok(Self {
            offered: Box::new(offered),
            questions: question_sender,
            finished: finished_receiver,
            taken_at: VecDeque::new(),
        })
    }
}

impl PairedChatHost for ThreadedPairedChatHost {
    fn submit(&mut self, peer: DeviceId, request: PairedChatRequest) -> Result<(), String> {
        if !(self.offered)() {
            return Err("not_offered".to_owned());
        }
        let now = Instant::now();
        while self
            .taken_at
            .front()
            .is_some_and(|taken| now.duration_since(*taken) >= HOUR)
        {
            self.taken_at.pop_front();
        }
        if self.taken_at.len() >= PAIRED_CHATS_PER_HOUR {
            return Err("rate_limited".to_owned());
        }
        match self.questions.try_send((peer, request)) {
            Ok(()) => {
                self.taken_at.push_back(now);
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err("busy".to_owned()),
            Err(TrySendError::Disconnected(_)) => Err("not_offered".to_owned()),
        }
    }

    fn poll_finished(&mut self) -> Option<FinishedPairedChat> {
        self.finished.try_recv().ok()
    }
}
