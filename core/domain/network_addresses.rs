//! Addresses that let a device reach its peers on other networks: the relay and static peers set in
//! Settings (Issue #64), and the pairing invites one device shows another (Issue #65).
//!
//! An invite is plain text, `note-taker-invite:1;<peer id>;<expires at ms>;<address>,<address>`, so
//! it fits a QR code and contains nothing secret. It only tells a device where to look: pairing
//! still needs the numeric code on both screens. Its contents are untrusted input, so parsing bounds
//! every part; the network adapter then checks each address before dialing it.

use serde::{Deserialize, Serialize};

use crate::domain::models::DeviceId;

/// What every invite starts with; the `1` is the format version.
const INVITE_PREFIX: &str = "note-taker-invite:1;";
/// Longest invite accepted, well above what `MAX_INVITE_ADDRESSES` real addresses need.
const MAX_INVITE_CHARACTERS: usize = 2_048;
/// Most addresses one invite may carry.
pub const MAX_INVITE_ADDRESSES: usize = 8;
/// Longest accepted peer ID; libp2p Ed25519 peer IDs are 52 characters.
const MAX_INVITE_PEER_ID_CHARACTERS: usize = 128;

/// The relay and static peers the owner chose.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct NetworkAddresses {
    /// Address of the relay, ending in `/p2p/<relay id>`.
    pub relay: Option<String>,
    /// Addresses of devices mDNS cannot find, each ending in `/p2p/<peer id>`.
    pub peers: Vec<String>,
}

/// What a device keeps about its routes between restarts.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct StoredNetworkAddresses {
    /// The owner's choice in Settings; `None` until saved, so build-time and environment values
    /// still apply.
    pub configured: Option<NetworkAddresses>,
    /// Addresses of devices paired through an invite, kept so they reconnect after a restart.
    pub invited_peers: Vec<String>,
}

/// Why invite text was refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PairingInviteError {
    /// The text is not an invite this version understands.
    #[error("the pairing invite is malformed")]
    Malformed,
}

/// Where to find a device that wants to pair, until a deadline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingInvite {
    peer_id: DeviceId,
    expires_at_ms: u64,
    addresses: Vec<String>,
}

impl PairingInvite {
    /// Creates an invite.
    ///
    /// Inputs: The inviting device, when the invite ends, and its transport addresses without the
    /// trailing `/p2p/<peer id>`; at most `MAX_INVITE_ADDRESSES` are kept.
    /// Outputs: The invite. Side effects: None.
    pub fn new(peer_id: DeviceId, expires_at_ms: u64, mut addresses: Vec<String>) -> Self {
        addresses.truncate(MAX_INVITE_ADDRESSES);
        Self {
            peer_id,
            expires_at_ms,
            addresses,
        }
    }

    /// Parses untrusted invite text.
    ///
    /// Inputs: Text as pasted, surrounding whitespace allowed.
    /// Outputs: The invite, or `Malformed` when any part is out of shape or bounds. An expired
    /// invite parses; `is_expired` tells it apart so the owner gets the right message.
    /// Side effects: None.
    pub fn parse(text: &str) -> Result<Self, PairingInviteError> {
        let text = text.trim();
        if text.len() > MAX_INVITE_CHARACTERS {
            return Err(PairingInviteError::Malformed);
        }
        let body = text
            .strip_prefix(INVITE_PREFIX)
            .ok_or(PairingInviteError::Malformed)?;
        let mut fields = body.split(';');
        let (Some(peer_id), Some(expires_at_ms), Some(addresses), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            return Err(PairingInviteError::Malformed);
        };
        let peer_id_is_valid = !peer_id.is_empty()
            && peer_id.len() <= MAX_INVITE_PEER_ID_CHARACTERS
            && peer_id.chars().all(|letter| letter.is_ascii_alphanumeric());
        let addresses: Vec<String> = addresses.split(',').map(str::to_owned).collect();
        let addresses_are_valid = addresses.len() <= MAX_INVITE_ADDRESSES
            && addresses.iter().all(|address| {
                address.starts_with('/')
                    && address.chars().all(|letter| {
                        letter.is_ascii_alphanumeric() || matches!(letter, '/' | '.' | ':' | '-')
                    })
            });
        if !peer_id_is_valid || !addresses_are_valid {
            return Err(PairingInviteError::Malformed);
        }
        Ok(Self {
            peer_id: DeviceId::new(peer_id),
            expires_at_ms: expires_at_ms
                .parse()
                .map_err(|_| PairingInviteError::Malformed)?,
            addresses,
        })
    }

    /// Writes the invite as text that `parse` accepts.
    ///
    /// Side effects: None.
    pub fn to_text(&self) -> String {
        format!(
            "{INVITE_PREFIX}{};{};{}",
            self.peer_id.as_str(),
            self.expires_at_ms,
            self.addresses.join(",")
        )
    }

    /// Borrows the inviting device's ID.
    pub fn peer_id(&self) -> &DeviceId {
        &self.peer_id
    }

    /// Returns when the invite ends, in milliseconds since the Unix epoch.
    pub fn expires_at_ms(&self) -> u64 {
        self.expires_at_ms
    }

    /// Reports whether the invite has ended at `now_ms`.
    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms >= self.expires_at_ms
    }

    /// Returns each address with `/p2p/<peer id>` appended, ready to dial.
    ///
    /// Side effects: None.
    pub fn dial_addresses(&self) -> Vec<String> {
        self.addresses
            .iter()
            .map(|address| format!("{address}/p2p/{}", self.peer_id.as_str()))
            .collect()
    }
}

/// How far a device has got with its relay.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RelayState {
    /// No relay is set.
    #[default]
    NotConfigured,
    /// A relay is set but not paired with this device, so it will not serve it yet.
    NotPaired,
    /// Paired, but not connected right now.
    Connecting,
    /// Connected, with no reservation yet.
    Connected,
    /// Other devices can reach this one through the relay.
    Reserved,
}

impl RelayState {
    /// Returns the name used in the status JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotConfigured => "not_configured",
            Self::NotPaired => "not_paired",
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Reserved => "reserved",
        }
    }
}
