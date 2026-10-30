//! Owns the P2P task's routes to other networks (Issues #64 and #65): the relay and static peers
//! chosen in Settings, and the devices found through invites.
//!
//! Settings apply live, and are stored in the database so they survive a restart. Until the owner
//! saves them, build-time and environment values apply. A device found through an invite stays a
//! pairing candidate until the invite expires; once paired, its addresses are stored, so it can
//! reconnect after a restart. Addresses of a removed device are forgotten with it.
//!
//! Nothing here grants trust: a candidate is admitted only by the code comparison in
//! `PairingManager`.

use anyhow::{Context, Result};

use crate::{
    adapters::network::libp2p_adapter::Libp2pNetworkAdapter,
    application::p2p_service::{
        NetworkCommand, NetworkCommandError, NetworkCommandOutcome, NetworkRouteStatus,
    },
    domain::{
        models::DeviceId,
        network_addresses::{NetworkAddresses, PairingInvite, StoredNetworkAddresses},
    },
    ports::peer_trust::{NetworkAddressStore, PairingTransport},
};

/// A device found through an invite that has not been paired yet.
struct InvitedPeer {
    peer_id: DeviceId,
    addresses: Vec<String>,
    expires_at_ms: u64,
}

/// The routes in effect and where they are stored.
pub struct NetworkRoutes {
    store: Box<dyn NetworkAddressStore>,
    stored: StoredNetworkAddresses,
    /// What applies now: the stored choice, or build-time and environment values without one.
    effective: NetworkAddresses,
    /// Whether this node offers a relay service itself.
    offers_relay: bool,
    invited: Vec<InvitedPeer>,
}

impl NetworkRoutes {
    /// Loads the stored routes and applies them to `adapter`.
    ///
    /// Inputs: The store; `configured`, from the environment or build-time defines; and whether
    /// this node is a relay.
    /// Outputs: The routes, or an error for invalid environment values. A stored address that no
    /// longer applies is skipped with a warning, so it cannot stop P2P from starting.
    /// Side effects: Reads the store and records addresses in the adapter.
    pub fn start(
        store: Box<dyn NetworkAddressStore>,
        configured: NetworkAddresses,
        offers_relay: bool,
        adapter: &mut Libp2pNetworkAdapter,
    ) -> Result<Self> {
        let stored = store
            .load_network_addresses()
            .context("unable to read the network addresses")?;
        let effective = match &stored.configured {
            Some(saved) => {
                let saved = keep_usable_addresses(adapter, saved);
                adapter
                    .replace_network_addresses(saved.relay.as_deref(), &[])
                    .context("the stored relay address is invalid")?;
                for peer_address in &saved.peers {
                    adapter
                        .add_static_peer(peer_address)
                        .context("a stored peer address is invalid")?;
                }
                saved
            }
            None => {
                for peer_address in &configured.peers {
                    adapter
                        .add_static_peer(peer_address)
                        .context("NOTE_TAKER_P2P_PEERS contains an invalid peer address")?;
                }
                if let Some(relay_address) = &configured.relay {
                    adapter
                        .use_relay(relay_address)
                        .context("NOTE_TAKER_P2P_RELAY is invalid")?;
                }
                configured
            }
        };
        for peer_address in &stored.invited_peers {
            if adapter.add_static_peer(peer_address).is_err() {
                tracing::warn!("skipped an invited device's stored address");
            }
        }
        Ok(Self {
            store,
            stored,
            effective,
            offers_relay,
            invited: Vec::new(),
        })
    }

    /// Applies new Settings values, storing them first.
    ///
    /// Inputs: Untrusted addresses from the owner.
    /// Outputs: `InvalidRelayAddress` or `InvalidPeerAddress` with nothing changed, or `Storage`.
    /// A circuit address is not a valid relay, since a relay must be dialed directly.
    /// Side effects: Writes the store and updates the adapter. Connections stay open.
    pub fn set_addresses(
        &mut self,
        addresses: NetworkAddresses,
        adapter: &mut Libp2pNetworkAdapter,
    ) -> Result<(), NetworkCommandError> {
        if let Some(relay_address) = &addresses.relay
            && (adapter.check_peer_address(relay_address).is_err()
                || relay_address.contains("/p2p-circuit"))
        {
            return Err(NetworkCommandError::InvalidRelayAddress);
        }
        if addresses
            .peers
            .iter()
            .any(|peer_address| adapter.check_peer_address(peer_address).is_err())
        {
            return Err(NetworkCommandError::InvalidPeerAddress);
        }
        self.stored.configured = Some(addresses.clone());
        self.save()?;
        let mut all_peers = addresses.peers.clone();
        all_peers.extend(self.stored.invited_peers.iter().cloned());
        adapter
            .replace_network_addresses(addresses.relay.as_deref(), &all_peers)
            .map_err(|_| NetworkCommandError::InvalidPeerAddress)?;
        self.effective = addresses;
        Ok(())
    }

    /// Reads an invite and lists its device as a pairing candidate.
    ///
    /// Inputs: Untrusted text, the current time, and whether its device is already paired.
    /// Outputs: The device's ID, or `InviteMalformed`, `InviteExpired`, `InviteFromThisDevice`, or
    /// `AlreadyPaired`, with nothing changed.
    /// Side effects: Records the addresses in the adapter until the invite expires.
    pub fn join_invite(
        &mut self,
        invite_text: &str,
        now_ms: u64,
        is_paired: impl Fn(&DeviceId) -> bool,
        adapter: &mut Libp2pNetworkAdapter,
    ) -> Result<DeviceId, NetworkCommandError> {
        let invite =
            PairingInvite::parse(invite_text).map_err(|_| NetworkCommandError::InviteMalformed)?;
        if invite.is_expired(now_ms) {
            return Err(NetworkCommandError::InviteExpired);
        }
        if *invite.peer_id() == adapter.local_peer_id() {
            return Err(NetworkCommandError::InviteFromThisDevice);
        }
        if is_paired(invite.peer_id()) {
            return Err(NetworkCommandError::AlreadyPaired);
        }
        let addresses = invite.dial_addresses();
        if addresses.is_empty() {
            return Err(NetworkCommandError::InviteMalformed);
        }
        for address in &addresses {
            match adapter.check_peer_address(address) {
                Ok(named_peer) if named_peer == *invite.peer_id() => {}
                _ => return Err(NetworkCommandError::InviteMalformed),
            }
        }
        adapter
            .add_invited_peer(&addresses)
            .map_err(|_| NetworkCommandError::InviteMalformed)?;
        self.invited
            .retain(|invited| invited.peer_id != *invite.peer_id());
        self.invited.push(InvitedPeer {
            peer_id: invite.peer_id().clone(),
            addresses,
            expires_at_ms: invite.expires_at_ms(),
        });
        Ok(invite.peer_id().clone())
    }

    /// Stores the addresses of invited devices that are now paired, and drops expired invites.
    ///
    /// Inputs: The current time, whether a device is paired, and whether a pairing with it is still
    /// in progress, which keeps it past the deadline.
    /// Side effects: May write the store and update the adapter's nearby list.
    pub fn settle_invites(
        &mut self,
        now_ms: u64,
        is_paired: impl Fn(&DeviceId) -> bool,
        is_pairing: impl Fn(&DeviceId) -> bool,
        adapter: &mut Libp2pNetworkAdapter,
    ) {
        let mut newly_paired = Vec::new();
        self.invited.retain_mut(|invited| {
            if is_paired(&invited.peer_id) {
                newly_paired.append(&mut invited.addresses);
                return false;
            }
            if now_ms >= invited.expires_at_ms && !is_pairing(&invited.peer_id) {
                adapter.forget_nearby_peer(&invited.peer_id);
                return false;
            }
            true
        });
        if newly_paired.is_empty() {
            return;
        }
        for address in &newly_paired {
            if adapter.add_static_peer(address).is_err() {
                tracing::warn!("skipped an invited device's address");
            }
        }
        self.stored.invited_peers.extend(newly_paired);
        if self.save().is_err() {
            tracing::warn!("could not store an invited device's addresses");
        }
    }

    /// Forgets a removed device's invited addresses, so it does not return as a candidate.
    ///
    /// Side effects: May write the store and rebuild the adapter's static peers.
    pub fn forget_peer(&mut self, peer_id: &DeviceId, adapter: &mut Libp2pNetworkAdapter) {
        let suffix = format!("/p2p/{}", peer_id.as_str());
        let before = self.stored.invited_peers.len();
        self.stored
            .invited_peers
            .retain(|address| !address.ends_with(&suffix));
        if self.stored.invited_peers.len() == before {
            return;
        }
        if self.save().is_err() {
            tracing::warn!("could not forget a removed device's addresses");
        }
        let mut all_peers = self.effective.peers.clone();
        all_peers.extend(self.stored.invited_peers.iter().cloned());
        if adapter
            .replace_network_addresses(self.effective.relay.as_deref(), &all_peers)
            .is_err()
        {
            tracing::warn!("could not update the peer addresses");
        }
    }

    /// Describes the routes for the status JSON.
    ///
    /// Side effects: None.
    pub fn status(&self, adapter: &Libp2pNetworkAdapter) -> NetworkRouteStatus {
        NetworkRouteStatus {
            relay_address: self.effective.relay.clone(),
            relay_state: adapter.relay_state(),
            peers: self.effective.peers.clone(),
            offers_relay: self.offers_relay,
        }
    }

    /// Writes the stored routes.
    fn save(&self) -> Result<(), NetworkCommandError> {
        self.store
            .save_network_addresses(&self.stored)
            .map_err(|_| NetworkCommandError::Storage)
    }
}

/// Drops stored addresses that cannot be used, such as one naming this device.
fn keep_usable_addresses(
    adapter: &Libp2pNetworkAdapter,
    saved: &NetworkAddresses,
) -> NetworkAddresses {
    let is_usable = |address: &String| {
        let usable = adapter.check_peer_address(address).is_ok();
        if !usable {
            tracing::warn!("skipped a stored address that cannot be used");
        }
        usable
    };
    NetworkAddresses {
        relay: saved.relay.iter().find(|relay| is_usable(relay)).cloned(),
        peers: saved
            .peers
            .iter()
            .filter(|peer| is_usable(peer))
            .cloned()
            .collect(),
    }
}

/// Answers `command`, given the network's state.
///
/// Inputs: The routes, the adapter, the current time, the pairing deadline for a new invite, and
/// whether a device is paired.
/// Outputs: What the command produced, or why it was refused. `CreateInvite` needs the caller to
/// open the pairing window as well, since only the service can.
/// Side effects: Those of the routes' methods.
pub fn run_network_command(
    command: NetworkCommand,
    routes: &mut NetworkRoutes,
    adapter: &mut Libp2pNetworkAdapter,
    now_ms: u64,
    invite_lifetime_ms: u64,
    is_paired: impl Fn(&DeviceId) -> bool,
) -> Result<NetworkCommandOutcome, NetworkCommandError> {
    match command {
        NetworkCommand::SetAddresses(addresses) => {
            routes.set_addresses(addresses, adapter)?;
            Ok(NetworkCommandOutcome::Done)
        }
        NetworkCommand::CreateInvite => {
            let addresses = adapter.shareable_addresses();
            if addresses.is_empty() {
                return Err(NetworkCommandError::NoAddresses);
            }
            let invite = PairingInvite::new(
                adapter.local_peer_id(),
                now_ms.saturating_add(invite_lifetime_ms),
                addresses,
            );
            Ok(NetworkCommandOutcome::Invite {
                text: invite.to_text(),
                expires_at_ms: invite.expires_at_ms(),
            })
        }
        NetworkCommand::JoinInvite(invite_text) => routes
            .join_invite(&invite_text, now_ms, is_paired, adapter)
            .map(NetworkCommandOutcome::Joined),
    }
}
