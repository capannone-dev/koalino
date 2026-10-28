//! Defines the paired-device vocabulary for P2P sync between one owner's devices (Issue #43).
//!
//! A `TrustedPeer` is a device the owner paired with this one; only trusted peers may sync.
//! Pairing uses numeric comparison: both devices derive the same `PairingCode` from the two
//! authenticated peer IDs, and the owner confirms on each device that the codes match. A device
//! impersonating the intended peer has a different peer ID, so its code differs.

use sha2::{Digest, Sha256};

use super::{errors::DeviceNameError, models::DeviceId};

/// Domain-separation prefix of the pairing-code hash; changing it breaks cross-version pairing.
const PAIRING_CODE_DOMAIN: &[u8] = b"note-taker/pairing-code/v1\0";

/// Number of distinct pairing codes (six decimal digits).
const PAIRING_CODE_SPACE: u64 = 1_000_000;

/// A validated, owner-chosen device name such as "Ettore's laptop".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceName(String);

impl DeviceName {
    /// Longest accepted name, counted in Unicode scalar values after trimming.
    pub const MAX_CHARACTERS: usize = 64;

    /// Validates `raw_name`.
    ///
    /// Inputs: The name as typed or as received from a peer.
    /// Outputs: The trimmed name, or why it is blank, too long, or contains a control character.
    /// Side effects: None.
    pub fn parse(raw_name: &str) -> Result<Self, DeviceNameError> {
        let trimmed_name = raw_name.trim();
        if trimmed_name.is_empty() {
            return Err(DeviceNameError::Blank);
        }
        if trimmed_name.chars().count() > Self::MAX_CHARACTERS {
            return Err(DeviceNameError::TooLong {
                max_characters: Self::MAX_CHARACTERS,
            });
        }
        if trimmed_name.chars().any(char::is_control) {
            return Err(DeviceNameError::ControlCharacter);
        }
        Ok(Self(trimmed_name.to_owned()))
    }

    /// Borrows the name text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A device the owner paired with this one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrustedPeer {
    peer_id: DeviceId,
    device_name: DeviceName,
    paired_at_ms: u64,
}

impl TrustedPeer {
    /// Creates a trusted-peer record.
    ///
    /// Inputs: The peer's authenticated libp2p peer ID (in portable form), its name, and the
    /// pairing time in milliseconds since the Unix epoch.
    /// Outputs: The record. Side effects: None.
    pub fn new(peer_id: DeviceId, device_name: DeviceName, paired_at_ms: u64) -> Self {
        Self {
            peer_id,
            device_name,
            paired_at_ms,
        }
    }

    /// The peer's authenticated peer ID.
    pub fn peer_id(&self) -> &DeviceId {
        &self.peer_id
    }

    /// The peer's display name.
    pub fn device_name(&self) -> &DeviceName {
        &self.device_name
    }

    /// When the pairing was confirmed, in milliseconds since the Unix epoch.
    pub fn paired_at_ms(&self) -> u64 {
        self.paired_at_ms
    }
}

/// The six-digit code both devices display during pairing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingCode(String);

impl PairingCode {
    /// Derives the code for a pairing between two peers.
    ///
    /// Inputs: The two authenticated peer IDs, in either order.
    /// Outputs: Six decimal digits: SHA-256 over the domain prefix, the lexicographically smaller
    /// ID, a NUL, and the larger ID; the first 8 bytes read big-endian, modulo 1,000,000,
    /// zero-padded. The order of arguments does not matter.
    /// Side effects: None.
    pub fn for_peers(first_peer: &DeviceId, second_peer: &DeviceId) -> Self {
        let (smaller_peer, larger_peer) = if first_peer.as_str() <= second_peer.as_str() {
            (first_peer, second_peer)
        } else {
            (second_peer, first_peer)
        };
        let digest = Sha256::new()
            .chain_update(PAIRING_CODE_DOMAIN)
            .chain_update(smaller_peer.as_str().as_bytes())
            .chain_update([0])
            .chain_update(larger_peer.as_str().as_bytes())
            .finalize();
        let mut leading_bytes = [0_u8; 8];
        leading_bytes.copy_from_slice(&digest[..8]);
        let code_number = u64::from_be_bytes(leading_bytes) % PAIRING_CODE_SPACE;
        Self(format!("{code_number:06}"))
    }

    /// Borrows the six digits.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
