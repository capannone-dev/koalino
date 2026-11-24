//! Orchestrates pairing between the owner's devices and keeps the allowlist in step (Issue #43).
//!
//! Pairing uses numeric comparison:
//!
//! 1. **Pairable device:** the owner makes one device pairable (`make_pairable`).
//! 2. **Initiating device:** on another device the owner picks it from `nearby_devices` and calls
//!    `start_pairing`. Both devices then show the same `PairingCode`.
//! 3. **Responding device:** gets an `IncomingRequest`. If the codes match, the owner calls
//!    `accept_incoming`, which trusts the initiator at once and answers.
//! 4. **Initiating device again:** trusts the responder only when **both** of these have happened,
//!    in either order:
//!    - the owner confirmed the matching code there (`confirm_code`);
//!    - the responder accepted.
//!
//! An attacker advertising itself as the intended device has a different peer ID, so the two
//! screens show different codes and the owner declines. Every trust decision is persisted in the
//! `TrustedPeerStore` before the transport admits the peer.

use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use crate::{
    domain::{
        errors::{NetworkError, StorageError},
        models::DeviceId,
        peer_trust::{DeviceName, PairingCode, TrustedPeer},
    },
    ports::peer_trust::{PairingEvent, PairingTransport, TrustedPeerStore},
};

/// Why a pairing operation failed.
#[derive(Debug, thiserror::Error)]
pub enum PairingError {
    /// No pairing with that peer is in progress (it finished, failed, or never started).
    #[error("no pairing with this device is in progress")]
    UnknownPairing,
    /// The transport refused the operation.
    #[error(transparent)]
    Network(#[from] NetworkError),
    /// The allowlist could not be read or written.
    #[error(transparent)]
    Storage(#[from] StorageError),
}

/// What the owner should see after a pairing step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PairingUpdate {
    /// Another device asks to pair; show its name and the code, and ask to accept or reject.
    IncomingRequest {
        /// The requesting device.
        peer_id: DeviceId,
        /// Its announced name.
        device_name: DeviceName,
        /// The code this device shows; it must equal the one on the requesting device.
        code: PairingCode,
    },
    /// The local confirmation is recorded; the other device has not accepted yet.
    AwaitingRemote {
        /// The device being paired.
        peer_id: DeviceId,
    },
    /// The device is now trusted and will sync.
    Paired {
        /// The newly trusted device.
        peer_id: DeviceId,
        /// Its name.
        device_name: DeviceName,
    },
    /// The other device's owner declined.
    Rejected {
        /// The device that declined.
        peer_id: DeviceId,
    },
    /// The request could not be delivered, or nobody answered in time.
    Failed {
        /// The unreachable device.
        peer_id: DeviceId,
    },
}

/// A request from another device that awaits the owner's answer, as shown on screen.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncomingPairingRequest {
    /// The requesting device.
    pub peer_id: DeviceId,
    /// Its announced name.
    pub device_name: DeviceName,
    /// The code this device shows; the owner compares it with the requester's.
    pub code: PairingCode,
}

/// A pairing this device started, as shown on screen.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutgoingPairingStatus {
    /// The device being paired.
    pub peer_id: DeviceId,
    /// The code this device shows.
    pub code: PairingCode,
    /// Whether the owner already confirmed the matching code here.
    pub locally_confirmed: bool,
}

/// A pairing this device started.
struct OutgoingPairing {
    /// Whether the owner confirmed the matching code here.
    locally_confirmed: bool,
    /// The remote name, once the other device accepted.
    accepted_as: Option<DeviceName>,
}

/// Coordinates pairing between a transport and the persisted allowlist.
pub struct PairingManager<Transport, Store> {
    transport: Transport,
    store: Store,
    /// This device's name, announced to peers.
    local_name: DeviceName,
    /// Returns the current time in milliseconds since the Unix epoch.
    clock: Box<dyn Fn() -> u64 + Send + Sync>,
    /// Pairings this device started, by peer.
    outgoing: HashMap<DeviceId, OutgoingPairing>,
    /// Requests awaiting the owner's answer, by peer.
    incoming: HashMap<DeviceId, DeviceName>,
    /// Peers this manager admitted into the transport's gate, compared with the store by
    /// `reconcile_trusted_peers`.
    admitted_peers: HashSet<DeviceId>,
}

impl<Transport, Store> PairingManager<Transport, Store>
where
    Transport: PairingTransport,
    Store: TrustedPeerStore,
{
    /// Creates a manager with no pairing in progress.
    ///
    /// Inputs: The transport, the allowlist, this device's name, and a millisecond clock.
    /// Outputs: The manager. Side effects: Tells the transport the name it answers nearby
    /// devices with (#121).
    pub fn new(
        mut transport: Transport,
        store: Store,
        local_name: DeviceName,
        clock: Box<dyn Fn() -> u64 + Send + Sync>,
    ) -> Self {
        transport.set_local_name(&local_name);
        Self {
            transport,
            store,
            local_name,
            clock,
            outgoing: HashMap::new(),
            incoming: HashMap::new(),
            admitted_peers: HashSet::new(),
        }
    }

    /// Admits every stored trusted peer into the transport; call once at startup.
    ///
    /// Outputs: Success, or a storage or network error.
    /// Side effects: Reads the allowlist and updates the transport's gate.
    pub fn load_trusted_peers(&mut self) -> Result<(), PairingError> {
        for peer in self.store.trusted_peers()? {
            self.admit(peer.peer_id())?;
        }
        Ok(())
    }

    /// Brings the transport's gate in line with the stored allowlist.
    ///
    /// Other processes sharing the database (the CLI's `devices revoke`) change the allowlist
    /// without going through this manager; the P2P task calls this about once a second so such a
    /// change takes effect without a restart.
    /// Outputs: Whether any peer was admitted or dropped, or a storage or network error (peers
    /// handled before the error keep their new state; the next call retries the rest).
    /// Side effects: Reads the allowlist; revokes peers no longer stored (closing their
    /// connections) and admits stored peers not yet admitted.
    pub fn reconcile_trusted_peers(&mut self) -> Result<bool, PairingError> {
        let stored_peers: HashSet<DeviceId> = self
            .store
            .trusted_peers()?
            .into_iter()
            .map(|peer| peer.peer_id().clone())
            .collect();
        let dropped_peers: Vec<DeviceId> = self
            .admitted_peers
            .difference(&stored_peers)
            .cloned()
            .collect();
        let added_peers: Vec<DeviceId> = stored_peers
            .difference(&self.admitted_peers)
            .cloned()
            .collect();
        for peer_id in &dropped_peers {
            self.transport.revoke_peer(peer_id)?;
            self.admitted_peers.remove(peer_id);
        }
        for peer_id in &added_peers {
            self.admit(peer_id)?;
        }
        Ok(!dropped_peers.is_empty() || !added_peers.is_empty())
    }

    /// Admits one peer into the transport's gate and remembers it.
    ///
    /// Side effects: Updates the transport's gate.
    fn admit(&mut self, peer_id: &DeviceId) -> Result<(), PairingError> {
        self.transport.trust_peer(peer_id)?;
        self.admitted_peers.insert(peer_id.clone());
        Ok(())
    }

    /// Lets unpaired devices reach this one for `duration`, so they can request pairing.
    ///
    /// Side effects: Opens the transport's pairing window.
    pub fn make_pairable(&mut self, duration: Duration) {
        self.transport.open_pairing_window(duration);
    }

    /// Lists unpaired devices visible on the local network that can be paired now.
    ///
    /// Outputs: The devices, without those in `incoming_requests` or `outgoing_pairings`, which
    /// come back if that pairing ends without trust.
    /// Side effects: None.
    pub fn nearby_devices(&self) -> Vec<DeviceId> {
        self.transport
            .nearby_unpaired_peers()
            .into_iter()
            .filter(|peer_id| {
                !self.outgoing.contains_key(peer_id) && !self.incoming.contains_key(peer_id)
            })
            .collect()
    }

    /// Lists the paired devices, oldest first.
    ///
    /// Outputs: The allowlist, or a storage error. Side effects: None.
    pub fn trusted_devices(&self) -> Result<Vec<TrustedPeer>, PairingError> {
        Ok(self.store.trusted_peers()?)
    }

    /// This device's announced name.
    pub fn local_name(&self) -> &DeviceName {
        &self.local_name
    }

    /// Renames this device (#121): later pairing requests and answers, and nearby devices that
    /// ask, get the new name; connected paired devices are told it, and the others when they
    /// next connect.
    ///
    /// Side effects: Updates the transport's answer and sends the name to connected paired peers.
    pub fn rename(&mut self, local_name: DeviceName) {
        self.transport.set_local_name(&local_name);
        self.local_name = local_name;
    }

    /// Stores the names paired devices gave since the last call (#121), so a device renamed on
    /// its own screen shows its new name here.
    ///
    /// Outputs: Whether any stored name changed, or a storage error. Side effects: Updates the
    /// allowlist's names; a name from a device that is not paired is dropped.
    pub fn apply_paired_peer_names(&mut self) -> Result<bool, PairingError> {
        let mut renamed = false;
        for (peer_id, device_name) in self.transport.take_paired_peer_names() {
            renamed |= self.store.rename_peer(&peer_id, &device_name)?;
        }
        Ok(renamed)
    }

    /// The name a nearby unpaired device gave (#121), or `None` while it has not.
    pub fn nearby_device_name(&self, peer_id: &DeviceId) -> Option<DeviceName> {
        self.transport.nearby_peer_name(peer_id)
    }

    /// Lists requests awaiting the owner's answer, ordered by peer ID.
    ///
    /// Side effects: None.
    pub fn incoming_requests(&self) -> Vec<IncomingPairingRequest> {
        let mut requests: Vec<IncomingPairingRequest> = self
            .incoming
            .iter()
            .map(|(peer_id, device_name)| IncomingPairingRequest {
                peer_id: peer_id.clone(),
                device_name: device_name.clone(),
                code: self.code_with(peer_id),
            })
            .collect();
        requests.sort_by(|left, right| left.peer_id.as_str().cmp(right.peer_id.as_str()));
        requests
    }

    /// Lists pairings this device started that have not finished, ordered by peer ID.
    ///
    /// Side effects: None.
    pub fn outgoing_pairings(&self) -> Vec<OutgoingPairingStatus> {
        let mut pairings: Vec<OutgoingPairingStatus> = self
            .outgoing
            .iter()
            .map(|(peer_id, pairing)| OutgoingPairingStatus {
                peer_id: peer_id.clone(),
                code: self.code_with(peer_id),
                locally_confirmed: pairing.locally_confirmed,
            })
            .collect();
        pairings.sort_by(|left, right| left.peer_id.as_str().cmp(right.peer_id.as_str()));
        pairings
    }

    /// Asks `peer_id` to pair and returns the code both devices will show.
    ///
    /// Inputs: A device from `nearby_devices`.
    /// Outputs: The code to display, or a network error.
    /// Side effects: Sends a pairing request; restarts a pairing already in progress with it.
    pub fn start_pairing(&mut self, peer_id: &DeviceId) -> Result<PairingCode, PairingError> {
        self.transport.request_pairing(peer_id, &self.local_name)?;
        self.outgoing.insert(
            peer_id.clone(),
            OutgoingPairing {
                locally_confirmed: false,
                accepted_as: None,
            },
        );
        Ok(self.code_with(peer_id))
    }

    /// Records the owner's confirmation that the codes match for a pairing this device started.
    ///
    /// Outputs: `Paired` when the other device already accepted, otherwise `AwaitingRemote`; or
    /// `UnknownPairing`.
    /// Side effects: May persist and admit the peer.
    pub fn confirm_code(&mut self, peer_id: &DeviceId) -> Result<PairingUpdate, PairingError> {
        let pairing = self
            .outgoing
            .get_mut(peer_id)
            .ok_or(PairingError::UnknownPairing)?;
        pairing.locally_confirmed = true;
        match pairing.accepted_as.clone() {
            Some(device_name) => {
                self.outgoing.remove(peer_id);
                let update = self.complete(peer_id, device_name)?;
                self.end_pairing_when_idle();
                Ok(update)
            }
            None => Ok(PairingUpdate::AwaitingRemote {
                peer_id: peer_id.clone(),
            }),
        }
    }

    /// Abandons a pairing this device started, or declines an incoming one.
    ///
    /// Side effects: Answers a pending incoming request with a rejection; ends pairing when
    /// nothing else is in progress.
    pub fn cancel(&mut self, peer_id: &DeviceId) -> Result<(), PairingError> {
        self.outgoing.remove(peer_id);
        if self.incoming.remove(peer_id).is_some() {
            self.transport.answer_pairing(peer_id, None)?;
        }
        self.end_pairing_when_idle();
        Ok(())
    }

    /// Accepts an incoming request after the owner saw matching codes.
    ///
    /// Outputs: `Paired`, or `UnknownPairing` when no request from that peer is pending.
    /// Side effects: Persists and admits the peer, then answers with this device's name.
    pub fn accept_incoming(&mut self, peer_id: &DeviceId) -> Result<PairingUpdate, PairingError> {
        let device_name = self
            .incoming
            .remove(peer_id)
            .ok_or(PairingError::UnknownPairing)?;
        let update = self.complete(peer_id, device_name)?;
        self.transport
            .answer_pairing(peer_id, Some(&self.local_name))?;
        // Only after answering: ending pairing drops the transport's held answer channels.
        self.end_pairing_when_idle();
        Ok(update)
    }

    /// Declines an incoming request.
    ///
    /// Outputs: Success, or `UnknownPairing`. Side effects: Answers with a rejection.
    pub fn reject_incoming(&mut self, peer_id: &DeviceId) -> Result<(), PairingError> {
        self.incoming
            .remove(peer_id)
            .ok_or(PairingError::UnknownPairing)?;
        self.transport.answer_pairing(peer_id, None)?;
        self.end_pairing_when_idle();
        Ok(())
    }

    /// Unpairs `peer_id`: removes it from the allowlist and disconnects it.
    ///
    /// Outputs: Whether it was paired, or an error.
    /// Side effects: Deletes the allowlist row and updates the transport.
    pub fn revoke(&mut self, peer_id: &DeviceId) -> Result<bool, PairingError> {
        let was_trusted = self.store.revoke_peer(peer_id)?;
        self.transport.revoke_peer(peer_id)?;
        self.admitted_peers.remove(peer_id);
        Ok(was_trusted)
    }

    /// Drains the transport's pairing events and returns what the owner should see.
    ///
    /// Outputs: One update per meaningful event; answers to requests this device never sent are
    /// ignored. Side effects: May persist and admit a peer whose pairing completes.
    pub fn process_events(&mut self) -> Result<Vec<PairingUpdate>, PairingError> {
        let mut updates = Vec::new();
        while let Some(event) = self.transport.poll_pairing_event() {
            if let Some(update) = self.handle_event(event)? {
                updates.push(update);
            }
        }
        Ok(updates)
    }

    /// Applies one pairing event.
    fn handle_event(&mut self, event: PairingEvent) -> Result<Option<PairingUpdate>, PairingError> {
        match event {
            PairingEvent::Requested {
                peer_id,
                device_name,
            } => {
                let code = self.code_with(&peer_id);
                self.incoming.insert(peer_id.clone(), device_name.clone());
                Ok(Some(PairingUpdate::IncomingRequest {
                    peer_id,
                    device_name,
                    code,
                }))
            }
            PairingEvent::Answered {
                peer_id,
                accepted_as: Some(device_name),
            } => {
                let Some(pairing) = self.outgoing.get_mut(&peer_id) else {
                    tracing::warn!("ignored a pairing acceptance this device did not request");
                    return Ok(None);
                };
                if pairing.locally_confirmed {
                    self.outgoing.remove(&peer_id);
                    let update = self.complete(&peer_id, device_name)?;
                    self.end_pairing_when_idle();
                    Ok(Some(update))
                } else {
                    pairing.accepted_as = Some(device_name);
                    Ok(None)
                }
            }
            PairingEvent::Answered {
                peer_id,
                accepted_as: None,
            } => Ok(self
                .finish_unsuccessfully(&peer_id)
                .then_some(PairingUpdate::Rejected { peer_id })),
            PairingEvent::Failed { peer_id } => Ok(self
                .finish_unsuccessfully(&peer_id)
                .then_some(PairingUpdate::Failed { peer_id })),
        }
    }

    /// Forgets an outgoing pairing that ended without success; returns whether it existed.
    fn finish_unsuccessfully(&mut self, peer_id: &DeviceId) -> bool {
        let existed = self.outgoing.remove(peer_id).is_some();
        self.end_pairing_when_idle();
        existed
    }

    /// Persists and admits `peer_id`. Callers end pairing afterwards, once any answer is sent.
    fn complete(
        &mut self,
        peer_id: &DeviceId,
        device_name: DeviceName,
    ) -> Result<PairingUpdate, PairingError> {
        self.store.trust_peer(&TrustedPeer::new(
            peer_id.clone(),
            device_name.clone(),
            (self.clock)(),
        ))?;
        self.admit(peer_id)?;
        Ok(PairingUpdate::Paired {
            peer_id: peer_id.clone(),
            device_name,
        })
    }

    /// Closes the pairing window once no pairing is in progress.
    fn end_pairing_when_idle(&mut self) {
        if self.outgoing.is_empty() && self.incoming.is_empty() {
            self.transport.end_pairing();
        }
    }

    /// The code shown for a pairing between this device and `peer_id`.
    fn code_with(&self, peer_id: &DeviceId) -> PairingCode {
        PairingCode::for_peers(&self.transport.local_peer_id(), peer_id)
    }

    /// Borrows the transport (for composition and tests).
    pub fn transport(&self) -> &Transport {
        &self.transport
    }

    /// Mutably borrows the transport, e.g. to drive its event loop.
    pub fn transport_mut(&mut self) -> &mut Transport {
        &mut self.transport
    }

    /// Borrows the allowlist store.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Reads this manager's clock.
    ///
    /// Outputs: The current time in milliseconds since the Unix epoch. Side effects: None.
    pub fn now_ms(&self) -> u64 {
        (self.clock)()
    }
}
