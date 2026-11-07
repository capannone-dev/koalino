//! Carries a meeting's retained speech between paired devices for the P2P service (#88).
//!
//! The after-meeting worker of the device that does not combine a meeting's recordings hands each
//! part of its speech to the P2P task as a `SpeechTransferJob` and waits on the job's reply. The
//! task seals the part with the pair key (`NTR1`), and the receiving device stores it through its
//! `SpeechInbox` and answers with a receipt (`NTR2`). Meetings name devices by the ID written on
//! them, not by peer ID; each device's manifest carries both, so the task translates between them
//! and speech from a device whose manifest has no storage ID is refused. A job fails at once when the device is not
//! connected or has no agreed key, and when the device leaves, drops the message, or does not
//! answer within `SPEECH_TRANSFER_TIMEOUT`. Audio is never logged; logs carry peer IDs only.

use std::time::{Duration, Instant};

use uuid::Uuid;

use super::P2pService;
use crate::{
    application::speech_transfer_exchange::{
        SpeechReceipt, decode_speech_part, decode_speech_receipt, encode_speech_part,
        encode_speech_receipt,
    },
    domain::models::DeviceId,
    ports::{
        file_transfer::FileTransferNetwork,
        network::P2pNetwork,
        peer_trust::{PairingTransport, TrustedPeerStore},
        speech_transfer::{SpeechInbox, SpeechPart, SpeechTransferError},
        storage::NoteRepository,
    },
};

/// How long a paired device may take to store one part before the sender gives up on it.
pub const SPEECH_TRANSFER_TIMEOUT: Duration = Duration::from_secs(120);

/// One part of a meeting's speech to send to a paired device.
pub struct SpeechTransferJob {
    /// The device to send it to, by the ID written on its meetings.
    pub device: DeviceId,
    /// The part; its audio is wiped once sealed.
    pub part: SpeechPart,
    /// Receives the outcome.
    pub reply: std::sync::mpsc::Sender<Result<(), SpeechTransferError>>,
}

/// A part waiting for its receipt.
pub(super) struct PendingSpeech {
    peer: DeviceId,
    reply: std::sync::mpsc::Sender<Result<(), SpeechTransferError>>,
    deadline: Instant,
}

impl<Transport, Store, Repository> P2pService<Transport, Store, Repository>
where
    Transport: PairingTransport + P2pNetwork + FileTransferNetwork,
    Store: TrustedPeerStore,
    Repository: NoteRepository,
{
    /// Stores speech that paired devices send for combining (#88); without it they are refused.
    ///
    /// Outputs: The service. Side effects: None.
    pub fn with_speech_inbox(mut self, inbox: Box<dyn SpeechInbox>) -> Self {
        self.speech_inbox = Some(inbox);
        self
    }

    /// Seals and sends one part of a meeting's speech.
    ///
    /// Inputs: The job, whose outcome goes to its `reply`.
    /// Side effects: Sends the sealed part and drops this copy of the audio. A device that is not
    /// connected, has not sent its manifest, or has no agreed pair key gets nothing, and the reply
    /// is `Unreachable`.
    pub fn request_speech_transfer(&mut self, job: SpeechTransferJob) {
        let SpeechTransferJob {
            device,
            part,
            reply,
        } = job;
        let Some(peer) = self.speech_peer(&device) else {
            let _ = reply.send(Err(SpeechTransferError::Unreachable));
            return;
        };
        let Some(pair_key) = self.usable_pair_key(&peer) else {
            let _ = reply.send(Err(SpeechTransferError::Unreachable));
            return;
        };
        let job_id = Uuid::new_v4();
        let local_peer_id = self.pairing_manager.transport().local_peer_id();
        let message = match encode_speech_part(job_id, &part, &pair_key, &local_peer_id, &peer) {
            Ok(message) => message,
            Err(error) => {
                tracing::warn!(peer = %peer.as_str(), error = %error, "a speech part could not be sealed");
                let _ = reply.send(Err(SpeechTransferError::TooLarge));
                return;
            }
        };
        drop(part);
        if let Err(error) = self
            .pairing_manager
            .transport_mut()
            .send_encrypted(&peer, &message)
        {
            tracing::warn!(peer = %peer.as_str(), error = %error, "a speech part could not be sent");
            let _ = reply.send(Err(SpeechTransferError::Unreachable));
            return;
        }
        self.pending_speech.insert(
            job_id,
            PendingSpeech {
                peer,
                reply,
                deadline: Instant::now() + SPEECH_TRANSFER_TIMEOUT,
            },
        );
    }

    /// Returns the connected peer whose manifest names `device` as its storage ID.
    fn speech_peer(&self, device: &DeviceId) -> Option<DeviceId> {
        self.peer_manifests
            .iter()
            .find(|(peer, manifest)| {
                manifest.storage_device_id.as_ref() == Some(device)
                    && self.connected_peers.contains(*peer)
            })
            .map(|(peer, _)| peer.clone())
    }

    /// Opens a part a paired device sent, stores it, and answers with a receipt.
    ///
    /// Side effects: Writes the speech through the inbox and sends a sealed receipt. A message
    /// that does not open is logged (peer ID only) and dropped; the sender then times out.
    pub(super) fn accept_speech_part(&mut self, peer: &DeviceId, payload: &[u8]) {
        let Some(pair_key) = self.usable_pair_key(peer) else {
            tracing::warn!(peer = %peer.as_str(), "speech arrived without an agreed pair key and was dropped");
            return;
        };
        let local_peer_id = self.pairing_manager.transport().local_peer_id();
        let (job_id, part) = match decode_speech_part(payload, &pair_key, peer, &local_peer_id) {
            Ok(decoded) => decoded,
            Err(error) => {
                tracing::warn!(peer = %peer.as_str(), error = %error, "a speech part was rejected");
                return;
            }
        };
        let sender_device = self
            .peer_manifests
            .get(peer)
            .and_then(|manifest| manifest.storage_device_id.clone());
        let stored = match (self.speech_inbox.as_mut(), sender_device) {
            (Some(inbox), Some(sender_device)) => match inbox.receive_part(&sender_device, part) {
                Ok(()) => true,
                Err(error) => {
                    tracing::warn!(peer = %peer.as_str(), error = %error, "a speech part was not stored");
                    false
                }
            },
            _ => false,
        };
        let receipt = SpeechReceipt { job_id, stored };
        let sent = encode_speech_receipt(&receipt, &pair_key, &local_peer_id, peer)
            .map_err(|error| error.to_string())
            .and_then(|message| {
                self.pairing_manager
                    .transport_mut()
                    .send_encrypted(peer, &message)
                    .map_err(|error| error.to_string())
            });
        if let Err(error) = sent {
            tracing::warn!(peer = %peer.as_str(), error = %error, "a speech receipt could not be sent");
        }
    }

    /// Opens a receipt and answers the waiting job. A receipt from another device, or for an
    /// unknown job, is ignored.
    pub(super) fn accept_speech_receipt(&mut self, peer: &DeviceId, payload: &[u8]) {
        let Some(pair_key) = self.usable_pair_key(peer) else {
            return;
        };
        let local_peer_id = self.pairing_manager.transport().local_peer_id();
        let receipt = match decode_speech_receipt(payload, &pair_key, peer, &local_peer_id) {
            Ok(receipt) => receipt,
            Err(error) => {
                tracing::warn!(peer = %peer.as_str(), error = %error, "a speech receipt was rejected");
                return;
            }
        };
        if self
            .pending_speech
            .get(&receipt.job_id)
            .is_none_or(|pending| pending.peer != *peer)
        {
            return;
        }
        if let Some(pending) = self.pending_speech.remove(&receipt.job_id) {
            let _ = pending.reply.send(if receipt.stored {
                Ok(())
            } else {
                Err(SpeechTransferError::Refused)
            });
        }
    }

    /// Fails every part waiting on `peer`. Side effects: Answers them with `Unreachable`.
    pub(super) fn fail_pending_speech(&mut self, peer: &DeviceId) {
        self.pending_speech.retain(|_, pending| {
            let keep = pending.peer != *peer;
            if !keep {
                let _ = pending.reply.send(Err(SpeechTransferError::Unreachable));
            }
            keep
        });
    }

    /// Fails parts past their deadline. Side effects: Answers them with `Unreachable`.
    pub(super) fn expire_pending_speech(&mut self) {
        let now = Instant::now();
        self.pending_speech.retain(|_, pending| {
            let keep = pending.deadline > now;
            if !keep {
                let _ = pending.reply.send(Err(SpeechTransferError::Unreachable));
            }
            keep
        });
    }
}
