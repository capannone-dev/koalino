//! Specifies pairing orchestration with a scripted transport and an in-memory allowlist.
//!
//! The rule under test: a device becomes trusted only after its owner confirmed matching codes on
//! this device. The initiating side additionally waits for the other device's acceptance.

use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::Duration,
};

use super::pairing_manager::{
    IncomingPairingRequest, OutgoingPairingStatus, PairingError, PairingManager, PairingUpdate,
};
use crate::{
    domain::{
        errors::{NetworkError, StorageError},
        models::DeviceId,
        peer_trust::{DeviceName, PairKey, PairingCode, TrustedPeer},
    },
    ports::peer_trust::{PairKeyPinning, PairingEvent, PairingTransport, TrustedPeerStore},
};

/// The peer ID of the device under test.
const LOCAL_PEER: &str = "peer-local";

/// Records every transport call and replays scripted pairing events.
#[derive(Default)]
struct ScriptedTransport {
    calls: Vec<String>,
    events: VecDeque<PairingEvent>,
    trusted: Vec<DeviceId>,
}

impl PairingTransport for ScriptedTransport {
    fn local_peer_id(&self) -> DeviceId {
        DeviceId::new(LOCAL_PEER)
    }
    fn open_pairing_window(&mut self, duration: Duration) {
        self.calls
            .push(format!("open_window {}", duration.as_secs()));
    }
    fn end_pairing(&mut self) {
        self.calls.push(String::from("end_pairing"));
    }
    fn nearby_unpaired_peers(&self) -> Vec<DeviceId> {
        vec![DeviceId::new("peer-nearby")]
    }
    fn request_pairing(
        &mut self,
        peer_id: &DeviceId,
        local_name: &DeviceName,
    ) -> Result<(), NetworkError> {
        self.calls.push(format!(
            "request {} as {}",
            peer_id.as_str(),
            local_name.as_str()
        ));
        Ok(())
    }
    fn answer_pairing(
        &mut self,
        peer_id: &DeviceId,
        accept_as: Option<&DeviceName>,
    ) -> Result<(), NetworkError> {
        self.calls.push(format!(
            "answer {} {}",
            peer_id.as_str(),
            accept_as.map_or("reject", DeviceName::as_str)
        ));
        Ok(())
    }
    fn trust_peer(&mut self, peer_id: &DeviceId) -> Result<(), NetworkError> {
        self.trusted.push(peer_id.clone());
        Ok(())
    }
    fn revoke_peer(&mut self, peer_id: &DeviceId) -> Result<(), NetworkError> {
        self.trusted.retain(|trusted| trusted != peer_id);
        self.calls.push(format!("revoke {}", peer_id.as_str()));
        Ok(())
    }
    fn poll_pairing_event(&mut self) -> Option<PairingEvent> {
        self.events.pop_front()
    }
}

/// An allowlist kept in memory, with the pair keys pinned per peer.
#[derive(Default)]
struct MemoryTrustStore(Mutex<Vec<TrustedPeer>>, Mutex<HashMap<DeviceId, [u8; 32]>>);

impl MemoryTrustStore {
    /// Returns the stored peers.
    fn peers(&self) -> Vec<TrustedPeer> {
        self.0.lock().map(|peers| peers.clone()).unwrap_or_default()
    }
}

impl TrustedPeerStore for MemoryTrustStore {
    fn trust_peer(&self, peer: &TrustedPeer) -> Result<(), StorageError> {
        let mut peers = self.0.lock().map_err(|_| StorageError::Unavailable)?;
        peers.retain(|existing| existing.peer_id() != peer.peer_id());
        peers.push(peer.clone());
        Ok(())
    }
    fn revoke_peer(&self, peer_id: &DeviceId) -> Result<bool, StorageError> {
        let mut peers = self.0.lock().map_err(|_| StorageError::Unavailable)?;
        let before = peers.len();
        peers.retain(|existing| existing.peer_id() != peer_id);
        self.1
            .lock()
            .map_err(|_| StorageError::Unavailable)?
            .remove(peer_id);
        Ok(peers.len() != before)
    }
    fn trusted_peers(&self) -> Result<Vec<TrustedPeer>, StorageError> {
        Ok(self.peers())
    }
    fn is_trusted(&self, peer_id: &DeviceId) -> Result<bool, StorageError> {
        Ok(self.peers().iter().any(|peer| peer.peer_id() == peer_id))
    }
    fn pin_pair_key(
        &self,
        peer_id: &DeviceId,
        key: &PairKey,
    ) -> Result<PairKeyPinning, StorageError> {
        if !self.is_trusted(peer_id)? {
            return Ok(PairKeyPinning::UnknownPeer);
        }
        let mut keys = self.1.lock().map_err(|_| StorageError::Unavailable)?;
        Ok(match keys.get(peer_id) {
            None => {
                keys.insert(peer_id.clone(), *key.as_bytes());
                PairKeyPinning::Stored
            }
            Some(pinned) if pinned == key.as_bytes() => PairKeyPinning::AlreadyPinned,
            Some(_) => PairKeyPinning::Mismatch,
        })
    }
    fn pair_key(&self, peer_id: &DeviceId) -> Result<Option<PairKey>, StorageError> {
        Ok(self
            .1
            .lock()
            .map_err(|_| StorageError::Unavailable)?
            .get(peer_id)
            .copied()
            .map(PairKey::from_bytes))
    }
}

/// Returns a validated name.
fn name(text: &str) -> DeviceName {
    DeviceName::parse(text).unwrap_or_else(|error| panic!("valid name: {error}"))
}

/// Builds a manager named "Laptop" whose clock always reads 42 ms.
fn manager() -> PairingManager<ScriptedTransport, MemoryTrustStore> {
    PairingManager::new(
        ScriptedTransport::default(),
        MemoryTrustStore::default(),
        name("Laptop"),
        Box::new(|| 42),
    )
}

/// Starting pairing sends a request and shows the code derived from both peer IDs.
#[test]
fn starting_pairing_requests_it_and_returns_the_shared_code() {
    let mut pairing = manager();
    let phone = DeviceId::new("peer-phone");

    let code = pairing
        .start_pairing(&phone)
        .unwrap_or_else(|error| panic!("pairing must start: {error}"));

    assert_eq!(
        code,
        PairingCode::for_peers(&DeviceId::new(LOCAL_PEER), &phone)
    );
    assert_eq!(
        pairing.transport().calls,
        vec!["request peer-phone as Laptop"]
    );
}

/// The initiator trusts the other device only after both the remote acceptance and the local
/// confirmation, in either order.
#[test]
fn the_initiator_trusts_only_after_acceptance_and_local_confirmation() {
    for confirm_first in [true, false] {
        let mut pairing = manager();
        let phone = DeviceId::new("peer-phone");
        pairing
            .start_pairing(&phone)
            .unwrap_or_else(|error| panic!("pairing must start: {error}"));
        let accepted = PairingEvent::Answered {
            peer_id: phone.clone(),
            accepted_as: Some(name("Phone")),
        };

        let mut updates = Vec::new();
        if confirm_first {
            updates.push(
                pairing
                    .confirm_code(&phone)
                    .unwrap_or_else(|error| panic!("confirm must succeed: {error}")),
            );
            assert!(pairing.store().peers().is_empty());
            pairing.transport_mut().events.push_back(accepted);
            updates.extend(
                pairing
                    .process_events()
                    .unwrap_or_else(|error| panic!("events must process: {error}")),
            );
        } else {
            pairing.transport_mut().events.push_back(accepted);
            updates.extend(
                pairing
                    .process_events()
                    .unwrap_or_else(|error| panic!("events must process: {error}")),
            );
            assert!(pairing.store().peers().is_empty());
            updates.push(
                pairing
                    .confirm_code(&phone)
                    .unwrap_or_else(|error| panic!("confirm must succeed: {error}")),
            );
        }

        assert_eq!(
            updates.last(),
            Some(&PairingUpdate::Paired {
                peer_id: phone.clone(),
                device_name: name("Phone"),
            })
        );
        assert_eq!(
            pairing.store().peers(),
            vec![TrustedPeer::new(phone.clone(), name("Phone"), 42)]
        );
        assert_eq!(pairing.transport().trusted, vec![phone]);
        assert!(
            pairing
                .transport()
                .calls
                .contains(&String::from("end_pairing"))
        );
    }
}

/// A rejection or a delivery failure ends the pairing without trusting anyone.
#[test]
fn a_rejection_or_failure_trusts_nobody() {
    for (event, expected) in [
        (
            PairingEvent::Answered {
                peer_id: DeviceId::new("peer-phone"),
                accepted_as: None,
            },
            PairingUpdate::Rejected {
                peer_id: DeviceId::new("peer-phone"),
            },
        ),
        (
            PairingEvent::Failed {
                peer_id: DeviceId::new("peer-phone"),
            },
            PairingUpdate::Failed {
                peer_id: DeviceId::new("peer-phone"),
            },
        ),
    ] {
        let mut pairing = manager();
        let phone = DeviceId::new("peer-phone");
        pairing
            .start_pairing(&phone)
            .unwrap_or_else(|error| panic!("pairing must start: {error}"));
        pairing.transport_mut().events.push_back(event);

        let updates = pairing
            .process_events()
            .unwrap_or_else(|error| panic!("events must process: {error}"));

        assert_eq!(updates, vec![expected]);
        assert!(pairing.store().peers().is_empty());
        assert!(matches!(
            pairing.confirm_code(&phone),
            Err(PairingError::UnknownPairing)
        ));
    }
}

/// An answer from a device this node never asked is ignored.
#[test]
fn an_unsolicited_acceptance_is_ignored() {
    let mut pairing = manager();
    pairing
        .transport_mut()
        .events
        .push_back(PairingEvent::Answered {
            peer_id: DeviceId::new("peer-intruder"),
            accepted_as: Some(name("Intruder")),
        });

    let updates = pairing
        .process_events()
        .unwrap_or_else(|error| panic!("events must process: {error}"));

    assert!(updates.is_empty());
    assert!(pairing.store().peers().is_empty());
    assert!(pairing.transport().trusted.is_empty());
}

/// An incoming request is shown with its code; accepting trusts and answers with this name.
#[test]
fn accepting_an_incoming_request_trusts_the_requester() {
    let mut pairing = manager();
    let phone = DeviceId::new("peer-phone");
    pairing
        .transport_mut()
        .events
        .push_back(PairingEvent::Requested {
            peer_id: phone.clone(),
            device_name: name("Phone"),
        });

    let updates = pairing
        .process_events()
        .unwrap_or_else(|error| panic!("events must process: {error}"));
    assert_eq!(
        updates,
        vec![PairingUpdate::IncomingRequest {
            peer_id: phone.clone(),
            device_name: name("Phone"),
            code: PairingCode::for_peers(&DeviceId::new(LOCAL_PEER), &phone),
        }]
    );
    let accepted = pairing
        .accept_incoming(&phone)
        .unwrap_or_else(|error| panic!("accept must succeed: {error}"));

    assert_eq!(
        accepted,
        PairingUpdate::Paired {
            peer_id: phone.clone(),
            device_name: name("Phone")
        }
    );
    assert_eq!(
        pairing.store().peers(),
        vec![TrustedPeer::new(phone.clone(), name("Phone"), 42)]
    );
    assert_eq!(pairing.transport().trusted, vec![phone]);
    // The answer must go out before pairing ends: ending pairing drops held answer channels.
    let calls = &pairing.transport().calls;
    let answer_index = calls
        .iter()
        .position(|call| call == "answer peer-phone Laptop");
    let end_index = calls.iter().position(|call| call == "end_pairing");
    assert!(
        answer_index.is_some() && end_index.is_some() && answer_index < end_index,
        "{calls:?}"
    );
}

/// Rejecting an incoming request answers with a rejection and trusts nobody.
#[test]
fn rejecting_an_incoming_request_trusts_nobody() {
    let mut pairing = manager();
    let phone = DeviceId::new("peer-phone");
    pairing
        .transport_mut()
        .events
        .push_back(PairingEvent::Requested {
            peer_id: phone.clone(),
            device_name: name("Phone"),
        });
    pairing
        .process_events()
        .unwrap_or_else(|error| panic!("events must process: {error}"));

    pairing
        .reject_incoming(&phone)
        .unwrap_or_else(|error| panic!("reject must succeed: {error}"));

    assert!(pairing.store().peers().is_empty());
    assert!(
        pairing
            .transport()
            .calls
            .contains(&String::from("answer peer-phone reject"))
    );
    assert!(matches!(
        pairing.accept_incoming(&phone),
        Err(PairingError::UnknownPairing)
    ));
}

/// Startup loads stored peers into the transport; revoking removes them from both.
#[test]
fn stored_peers_are_loaded_and_revocation_removes_them_everywhere() {
    let mut pairing = manager();
    let phone = DeviceId::new("peer-phone");
    pairing
        .store()
        .trust_peer(&TrustedPeer::new(phone.clone(), name("Phone"), 1))
        .unwrap_or_else(|error| panic!("peer must be stored: {error}"));

    pairing
        .load_trusted_peers()
        .unwrap_or_else(|error| panic!("peers must load: {error}"));
    assert_eq!(pairing.transport().trusted, vec![phone.clone()]);
    let revoked = pairing
        .revoke(&phone)
        .unwrap_or_else(|error| panic!("revoke must succeed: {error}"));

    assert!(revoked);
    assert!(pairing.store().peers().is_empty());
    assert!(pairing.transport().trusted.is_empty());
}

/// Another process sharing the database (the CLI's `devices revoke`) edits the allowlist directly;
/// reconciling brings the transport's gate back in line with the store and reports what changed.
#[test]
fn reconciling_applies_allowlist_changes_made_by_other_processes() {
    let mut pairing = manager();
    let phone = DeviceId::new("peer-phone");
    let tablet = DeviceId::new("peer-tablet");
    pairing
        .store()
        .trust_peer(&TrustedPeer::new(phone.clone(), name("Phone"), 1))
        .unwrap_or_else(|error| panic!("peer must be stored: {error}"));
    pairing
        .load_trusted_peers()
        .unwrap_or_else(|error| panic!("peers must load: {error}"));
    let unchanged = pairing
        .reconcile_trusted_peers()
        .unwrap_or_else(|error| panic!("reconcile must succeed: {error}"));
    assert!(!unchanged, "nothing changed yet");

    pairing
        .store()
        .revoke_peer(&phone)
        .unwrap_or_else(|error| panic!("peer must be revoked: {error}"));
    pairing
        .store()
        .trust_peer(&TrustedPeer::new(tablet.clone(), name("Tablet"), 2))
        .unwrap_or_else(|error| panic!("peer must be stored: {error}"));
    let changed = pairing
        .reconcile_trusted_peers()
        .unwrap_or_else(|error| panic!("reconcile must succeed: {error}"));

    assert!(changed);
    assert_eq!(pairing.transport().trusted, vec![tablet]);
    let again = pairing
        .reconcile_trusted_peers()
        .unwrap_or_else(|error| panic!("reconcile must succeed: {error}"));
    assert!(!again, "a second pass finds nothing to do");
}

/// Making this device pairable opens the transport's window for the requested duration.
#[test]
fn making_the_device_pairable_opens_the_window() {
    let mut pairing = manager();

    pairing.make_pairable(Duration::from_secs(120));

    assert_eq!(pairing.transport().calls, vec!["open_window 120"]);
    assert_eq!(pairing.nearby_devices(), vec![DeviceId::new("peer-nearby")]);
}

/// The pairings in progress are listed for the owner's screens: incoming requests with the
/// requester's name and code, outgoing ones with their code and whether the owner confirmed it.
#[test]
fn pairings_in_progress_are_listed_with_their_codes() {
    let mut pairing = manager();
    let phone = DeviceId::new("peer-phone");
    let tablet = DeviceId::new("peer-tablet");
    pairing
        .transport_mut()
        .events
        .push_back(PairingEvent::Requested {
            peer_id: tablet.clone(),
            device_name: name("Tablet"),
        });
    pairing
        .process_events()
        .unwrap_or_else(|error| panic!("events must process: {error}"));
    pairing
        .start_pairing(&phone)
        .and_then(|_| pairing.confirm_code(&phone))
        .unwrap_or_else(|error| panic!("pairing must start and confirm: {error}"));

    let local = DeviceId::new(LOCAL_PEER);
    assert_eq!(pairing.local_name(), &name("Laptop"));
    assert_eq!(
        pairing.incoming_requests(),
        vec![IncomingPairingRequest {
            peer_id: tablet.clone(),
            device_name: name("Tablet"),
            code: PairingCode::for_peers(&local, &tablet),
        }]
    );
    assert_eq!(
        pairing.outgoing_pairings(),
        vec![OutgoingPairingStatus {
            peer_id: phone.clone(),
            code: PairingCode::for_peers(&local, &phone),
            locally_confirmed: true,
        }]
    );
}

/// A device leaves Nearby while a pairing with it is in progress, in either direction, so it
/// cannot be paired twice, and comes back when the pairing is cancelled.
#[test]
fn a_device_being_paired_is_not_listed_as_nearby() {
    let nearby = DeviceId::new("peer-nearby");
    let mut pairing = manager();
    assert_eq!(pairing.nearby_devices(), vec![nearby.clone()]);

    pairing
        .start_pairing(&nearby)
        .unwrap_or_else(|error| panic!("pairing must start: {error}"));
    assert!(pairing.nearby_devices().is_empty());
    pairing
        .cancel(&nearby)
        .unwrap_or_else(|error| panic!("pairing must cancel: {error}"));
    assert_eq!(pairing.nearby_devices(), vec![nearby.clone()]);

    pairing
        .transport_mut()
        .events
        .push_back(PairingEvent::Requested {
            peer_id: nearby.clone(),
            device_name: name("Phone"),
        });
    pairing
        .process_events()
        .unwrap_or_else(|error| panic!("events must process: {error}"));
    assert!(pairing.nearby_devices().is_empty());
    pairing
        .cancel(&nearby)
        .unwrap_or_else(|error| panic!("request must be declined: {error}"));
    assert_eq!(pairing.nearby_devices(), vec![nearby]);
}
