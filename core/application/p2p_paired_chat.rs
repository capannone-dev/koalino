//! Lets a paired computer answer a phone's chat with notes, for the P2P service (#107).
//!
//! **Asking** (the phone): the chat hands the task a `PairedChatJob`, and `PairedChatRelay` picks
//! the connected paired computers in turn. The task seals each question with the pair key (`NTQ1`)
//! and moves on when a computer declines, leaves, or does not accept within the relay's timeout.
//!
//! **Answering** (the computer): an `NTQ1` from a paired device goes to the `PairedChatHost`. A
//! computer without a host, or whose host does not offer chat, declines at once (`not_offered`).
//! Otherwise it says `Accepted` and later sends the answer or the reason there is none (`NTQ2`).
//!
//! Questions, history and answers are never logged; logs carry peer IDs only.

use std::time::Instant;

use uuid::Uuid;

use super::P2pService;
use crate::{
    application::{
        paired_chat_exchange::{
            PairedChatOutcome, PairedChatRequest, PairedChatResponse, decode_paired_chat_request,
            decode_paired_chat_response, encode_paired_chat_request, encode_paired_chat_response,
        },
        paired_chat_relay::{NextSend, PairedChatJob},
    },
    domain::models::DeviceId,
    ports::{
        file_transfer::FileTransferNetwork,
        network::P2pNetwork,
        peer_trust::{PairingTransport, TrustedPeerStore},
        storage::NoteRepository,
    },
};

/// An answer the host finished.
pub struct FinishedPairedChat {
    /// The device that asked.
    pub peer: DeviceId,
    /// The question's ID on that device.
    pub job_id: Uuid,
    /// The answer, or why there is none.
    pub outcome: PairedChatOutcome,
}

/// Answers paired devices' chat questions away from the P2P thread.
pub trait PairedChatHost: Send {
    /// Queues a question without waiting.
    ///
    /// Outputs: `Ok` when it will be answered, or the error code to decline with at once
    /// (`not_offered` when this computer does not answer for its devices, `busy` when the queue
    /// is full). Side effects: Holds the question until it is answered.
    fn submit(&mut self, peer: DeviceId, request: PairedChatRequest) -> Result<(), String>;

    /// Takes one finished answer, if any. Side effects: Removes it from the queue.
    fn poll_finished(&mut self) -> Option<FinishedPairedChat>;
}

impl<Transport, Store, Repository> P2pService<Transport, Store, Repository>
where
    Transport: PairingTransport + P2pNetwork + FileTransferNetwork,
    Store: TrustedPeerStore,
    Repository: NoteRepository,
{
    /// Answers paired devices' chat questions with `host`; without one they are declined.
    ///
    /// Outputs: The service. Side effects: None.
    pub fn with_paired_chat_host(mut self, host: Box<dyn PairedChatHost>) -> Self {
        self.paired_chat_host = Some(host);
        self
    }

    /// Asks the connected paired computers a chat question, one at a time.
    ///
    /// Inputs: The job, whose answer or error code goes to its `reply`.
    /// Side effects: Seals and sends the question to the first computer with an agreed pair key;
    /// with none, the job is answered with `no_paired_computer`.
    pub fn request_paired_chat(&mut self, job: PairedChatJob) {
        let mut candidates: Vec<DeviceId> = self
            .connected_peers
            .iter()
            .filter(|peer| self.usable_pair_key(peer).is_some())
            .cloned()
            .collect();
        candidates.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        let first = self.paired_chats.begin(job, candidates, Instant::now());
        self.send_paired_chat_questions(first.into_iter().collect());
    }

    /// Seals and sends questions; a question that cannot be sent moves to the next computer.
    ///
    /// Side effects: Reads pair keys and sends; failures are logged with peer IDs only.
    fn send_paired_chat_questions(&mut self, mut sends: Vec<NextSend>) {
        while let Some((peer, request)) = sends.pop() {
            if self.send_paired_chat_question(&peer, &request) {
                continue;
            }
            if let Some(next) = self
                .paired_chats
                .send_failed(request.job_id, Instant::now())
            {
                sends.push(next);
            }
        }
    }

    /// Seals and sends one question. Outputs: Whether it was queued.
    fn send_paired_chat_question(&mut self, peer: &DeviceId, request: &PairedChatRequest) -> bool {
        if !self.connected_peers.contains(peer) {
            return false;
        }
        let Some(pair_key) = self.usable_pair_key(peer) else {
            return false;
        };
        let local_peer_id = self.pairing_manager.transport().local_peer_id();
        let sent = encode_paired_chat_request(request, &pair_key, &local_peer_id, peer)
            .map_err(|error| error.to_string())
            .and_then(|message| {
                self.pairing_manager
                    .transport_mut()
                    .send_encrypted(peer, &message)
                    .map_err(|error| error.to_string())
            });
        if let Err(error) = &sent {
            tracing::warn!(peer = %peer.as_str(), error = %error, "a chat question could not be sent");
        }
        sent.is_ok()
    }

    /// Opens a paired device's question and hands it to the host.
    ///
    /// Side effects: Answers `Accepted`, or declines at once when there is no host or it does not
    /// take the question. A message that does not open is logged and dropped.
    pub(super) fn accept_paired_chat_request(&mut self, peer: &DeviceId, payload: &[u8]) {
        let Some(pair_key) = self.usable_pair_key(peer) else {
            tracing::warn!(peer = %peer.as_str(), "a chat question arrived without an agreed pair key and was dropped");
            return;
        };
        let local_peer_id = self.pairing_manager.transport().local_peer_id();
        let request = match decode_paired_chat_request(payload, &pair_key, peer, &local_peer_id) {
            Ok(request) => request,
            Err(error) => {
                tracing::warn!(peer = %peer.as_str(), error = %error, "a chat question was rejected");
                return;
            }
        };
        let job_id = request.job_id;
        let submitted = match self.paired_chat_host.as_mut() {
            Some(host) => host.submit(peer.clone(), request),
            None => Err("not_offered".to_owned()),
        };
        let outcome = match submitted {
            Ok(()) => PairedChatOutcome::Accepted,
            Err(error_code) => PairedChatOutcome::Declined { error_code },
        };
        self.send_paired_chat_response(FinishedPairedChat {
            peer: peer.clone(),
            job_id,
            outcome,
        });
    }

    /// Sends the host's finished answers.
    ///
    /// Side effects: Seals and sends each; a device that left simply misses it.
    pub(super) fn send_finished_paired_chats(&mut self) {
        for _ in 0..super::MAX_EVENTS_PER_PASS {
            let Some(finished) = self
                .paired_chat_host
                .as_mut()
                .and_then(|host| host.poll_finished())
            else {
                return;
            };
            self.send_paired_chat_response(finished);
        }
    }

    /// Seals and sends one response.
    ///
    /// Side effects: Reads the pair key and sends; failures are logged without text.
    fn send_paired_chat_response(&mut self, finished: FinishedPairedChat) {
        let FinishedPairedChat {
            peer,
            job_id,
            outcome,
        } = finished;
        let Some(pair_key) = self.usable_pair_key(&peer) else {
            return;
        };
        let local_peer_id = self.pairing_manager.transport().local_peer_id();
        let sent = encode_paired_chat_response(
            &PairedChatResponse { job_id, outcome },
            &pair_key,
            &local_peer_id,
            &peer,
        )
        .map_err(|error| error.to_string())
        .and_then(|message| {
            self.pairing_manager
                .transport_mut()
                .send_encrypted(&peer, &message)
                .map_err(|error| error.to_string())
        });
        if let Err(error) = sent {
            tracing::warn!(peer = %peer.as_str(), error = %error, "a chat answer could not be sent back");
        }
    }

    /// Hands a computer's response to the relay.
    ///
    /// Side effects: May answer the job, or ask the next computer.
    pub(super) fn accept_paired_chat_response(&mut self, peer: &DeviceId, payload: &[u8]) {
        let Some(pair_key) = self.usable_pair_key(peer) else {
            return;
        };
        let local_peer_id = self.pairing_manager.transport().local_peer_id();
        let response = match decode_paired_chat_response(payload, &pair_key, peer, &local_peer_id) {
            Ok(response) => response,
            Err(error) => {
                tracing::warn!(peer = %peer.as_str(), error = %error, "a chat answer was rejected");
                return;
            }
        };
        let next = self
            .paired_chats
            .on_response(peer, response, Instant::now());
        self.send_paired_chat_questions(next.into_iter().collect());
    }

    /// Moves questions asked of `peer` to the next computer. Side effects: May send.
    pub(super) fn paired_chat_peer_lost(&mut self, peer: &DeviceId) {
        let sends = self.paired_chats.on_disconnected(peer, Instant::now());
        self.send_paired_chat_questions(sends);
    }

    /// Moves on from computers past their deadline. Side effects: May send or answer jobs.
    pub(super) fn expire_paired_chats(&mut self) {
        let sends = self.paired_chats.expire(Instant::now());
        self.send_paired_chat_questions(sends);
    }
}
