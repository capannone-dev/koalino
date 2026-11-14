//! Sends a chat question to the paired computers in turn until one answers (#107).
//!
//! A phone has no language model, so its chat asks a paired computer. It does not know which
//! computers offer it (the owner turns it on per computer), so it asks the connected ones one at
//! a time. A computer that does not offer it declines at once (`not_offered`). One that stays
//! silent for `ACCEPT_TIMEOUT` (an older version, for example) is skipped. Once a computer
//! accepts, the relay waits up to `ANSWER_TIMEOUT` for the answer, because a model can take
//! minutes. This module is pure bookkeeping: the P2P service seals and sends what it returns.
//! Nothing here logs questions or answers.

use std::{
    collections::{HashMap, VecDeque},
    sync::mpsc,
    time::{Duration, Instant},
};

use uuid::Uuid;

use crate::{
    application::paired_chat_exchange::{
        PairedChatAnswer, PairedChatOutcome, PairedChatRequest, PairedChatResponse, PairedChatTurn,
    },
    domain::{chat::ChatScope, models::DeviceId},
};

/// How long a computer has to say it is working on a question.
pub const ACCEPT_TIMEOUT: Duration = Duration::from_secs(20);
/// How long an accepted question may take to answer.
pub const ANSWER_TIMEOUT: Duration = Duration::from_secs(6 * 60);

/// The answer, or an error code: `no_paired_computer`, `paired_computer_timeout`,
/// `paired_computer_left`, or the code a computer declined with (such as `llm_unreachable`).
pub type PairedChatResult = Result<PairedChatAnswer, String>;

/// A question to send, and where its answer goes.
pub struct PairedChatJob {
    /// What the chat is about.
    pub scope: ChatScope,
    /// The question.
    pub question: String,
    /// The chat so far, oldest first.
    pub history: Vec<PairedChatTurn>,
    /// Receives the answer or an error code, once.
    pub reply: mpsc::Sender<PairedChatResult>,
}

/// What to send next: the computer, and the question to seal for it.
pub type NextSend = (DeviceId, PairedChatRequest);

/// One question in flight.
struct Pending {
    request: PairedChatRequest,
    /// The computer asked now.
    asking: DeviceId,
    /// Computers not asked yet, in order.
    untried: VecDeque<DeviceId>,
    /// Whether `asking` said it is working on it.
    accepted: bool,
    deadline: Instant,
    /// The most telling refusal so far.
    refusal: Option<String>,
    reply: mpsc::Sender<PairedChatResult>,
}

/// The questions in flight.
#[derive(Default)]
pub struct PairedChatRelay {
    pending: HashMap<Uuid, Pending>,
}

impl PairedChatRelay {
    /// Starts a question.
    ///
    /// Inputs: The job, the connected paired computers to try in order, and the time.
    /// Outputs: The first computer and the request to send it, or `None` when there is none (the
    /// job is then answered with `no_paired_computer`). Side effects: Keeps the job.
    pub fn begin(
        &mut self,
        job: PairedChatJob,
        candidates: Vec<DeviceId>,
        now: Instant,
    ) -> Option<NextSend> {
        let request = PairedChatRequest {
            job_id: Uuid::new_v4(),
            scope: job.scope,
            question: job.question,
            history: job.history,
        };
        let mut untried: VecDeque<DeviceId> = candidates.into();
        let Some(first) = untried.pop_front() else {
            let _ = job.reply.send(Err("no_paired_computer".to_owned()));
            return None;
        };
        let job_id = request.job_id;
        self.pending.insert(
            job_id,
            Pending {
                request: request.clone(),
                asking: first.clone(),
                untried,
                accepted: false,
                deadline: now + ACCEPT_TIMEOUT,
                refusal: None,
                reply: job.reply,
            },
        );
        Some((first, request))
    }

    /// Applies a computer's response.
    ///
    /// Outputs: The next computer to ask, when this one declined and another is left.
    /// Side effects: Answers the job when it is answered or no computer is left; a response
    /// from a device that was not asked, or to an unknown question, is ignored.
    pub fn on_response(
        &mut self,
        peer: &DeviceId,
        response: PairedChatResponse,
        now: Instant,
    ) -> Option<NextSend> {
        let pending = self
            .pending
            .get_mut(&response.job_id)
            .filter(|pending| pending.asking == *peer)?;
        match response.outcome {
            PairedChatOutcome::Accepted => {
                pending.accepted = true;
                pending.deadline = now + ANSWER_TIMEOUT;
                None
            }
            PairedChatOutcome::Answered(answer) => {
                if let Some(pending) = self.pending.remove(&response.job_id) {
                    let _ = pending.reply.send(Ok(answer));
                }
                None
            }
            PairedChatOutcome::Declined { error_code } => {
                // `not_offered` says nothing about the question; any other code does.
                if error_code != "not_offered" {
                    pending.refusal = Some(error_code);
                }
                self.advance(response.job_id, now, "no_paired_computer")
            }
        }
    }

    /// Moves on from computers that left.
    ///
    /// Outputs: The next sends for the questions `peer` was asked. Side effects: Answers those
    /// with `paired_computer_left` (or an earlier refusal) when no computer is left.
    pub fn on_disconnected(&mut self, peer: &DeviceId, now: Instant) -> Vec<NextSend> {
        let asked: Vec<Uuid> = self
            .pending
            .iter()
            .filter(|(_, pending)| pending.asking == *peer)
            .map(|(job_id, _)| *job_id)
            .collect();
        asked
            .into_iter()
            .filter_map(|job_id| self.advance(job_id, now, "paired_computer_left"))
            .collect()
    }

    /// Moves on from computers past their deadline.
    ///
    /// Outputs: The next sends. Side effects: A computer that never accepted is skipped; a
    /// question accepted but not answered in time is answered with `paired_computer_timeout`.
    pub fn expire(&mut self, now: Instant) -> Vec<NextSend> {
        let late: Vec<(Uuid, bool)> = self
            .pending
            .iter()
            .filter(|(_, pending)| pending.deadline <= now)
            .map(|(job_id, pending)| (*job_id, pending.accepted))
            .collect();
        let mut sends = Vec::new();
        for (job_id, accepted) in late {
            if accepted {
                if let Some(pending) = self.pending.remove(&job_id) {
                    let _ = pending
                        .reply
                        .send(Err("paired_computer_timeout".to_owned()));
                }
            } else if let Some(next) = self.advance(job_id, now, "no_paired_computer") {
                sends.push(next);
            }
        }
        sends
    }

    /// Moves on after the question could not be sent to the computer asked.
    ///
    /// Outputs: The next send, if a computer is left. Side effects: As `advance`.
    pub fn send_failed(&mut self, job_id: Uuid, now: Instant) -> Option<NextSend> {
        self.advance(job_id, now, "no_paired_computer")
    }

    /// Reports whether no question is in flight. Side effects: None.
    pub fn is_idle(&self) -> bool {
        self.pending.is_empty()
    }

    /// Asks the next computer, or answers the job when none is left.
    ///
    /// Inputs: The job, the time, and the error code when nothing better was heard.
    /// Side effects: Updates or removes the job.
    fn advance(&mut self, job_id: Uuid, now: Instant, otherwise: &str) -> Option<NextSend> {
        let pending = self.pending.get_mut(&job_id)?;
        match pending.untried.pop_front() {
            Some(next) => {
                pending.asking = next.clone();
                pending.accepted = false;
                pending.deadline = now + ACCEPT_TIMEOUT;
                Some((next, pending.request.clone()))
            }
            None => {
                if let Some(pending) = self.pending.remove(&job_id) {
                    let code = pending.refusal.unwrap_or_else(|| otherwise.to_owned());
                    let _ = pending.reply.send(Err(code));
                }
                None
            }
        }
    }
}
