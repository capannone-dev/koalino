//! Defines the paired-device vocabulary for P2P sync between one owner's devices (Issue #43).
//!
//! A `TrustedPeer` is a device the owner paired with this one; only trusted peers may sync.
//! Pairing uses numeric comparison: both devices derive the same `PairingCode` from the two
//! authenticated peer IDs, and the owner confirms on each device that the codes match. A device
//! impersonating the intended peer has a different peer ID, so its code differs.
//!
//! Each pair also agrees a `PairKey` (Issue #50) that protects review clips sent between the two
//! devices on top of the transport's encryption.

use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

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
    /// The ID the device writes on the meetings it records (#88), once its manifest said.
    storage_device_id: Option<DeviceId>,
    /// When this device last synced with it (#153), in milliseconds since the Unix epoch; `None`
    /// until it first does.
    last_synced_at_ms: Option<u64>,
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
            storage_device_id: None,
            last_synced_at_ms: None,
        }
    }

    /// Returns the record with the time this device last synced with it (#153).
    ///
    /// Inputs: Milliseconds since the Unix epoch, or `None` for never. Outputs: The record.
    /// Side effects: None.
    pub fn with_last_synced_at_ms(mut self, last_synced_at_ms: Option<u64>) -> Self {
        self.last_synced_at_ms = last_synced_at_ms;
        self
    }

    /// When this device last synced with it (#153): the last message it sent here, or the end
    /// of its last connection. `None` until it first syncs.
    pub fn last_synced_at_ms(&self) -> Option<u64> {
        self.last_synced_at_ms
    }

    /// Returns the record with the ID the device writes on its meetings.
    ///
    /// Inputs: That ID, from the device's manifest. Outputs: The record. Side effects: None.
    pub fn with_storage_device_id(mut self, storage_device_id: Option<DeviceId>) -> Self {
        self.storage_device_id = storage_device_id;
        self
    }

    /// The ID the device writes on the meetings it records (#88); `None` until its manifest
    /// said, or for devices older than #88.
    pub fn storage_device_id(&self) -> Option<&DeviceId> {
        self.storage_device_id.as_ref()
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

/// The 256-bit key two paired devices agreed (Issue #50); wiped on drop and never printed.
pub struct PairKey(Zeroizing<[u8; 32]>);

impl PairKey {
    /// Wraps key bytes, as derived from the agreement or opened from storage.
    ///
    /// Inputs: 32 secret bytes. Outputs: The key. Side effects: None.
    pub fn from_bytes(key_bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(key_bytes))
    }

    /// Borrows the key bytes; never log or persist them unsealed.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Compares two keys in constant time.
    ///
    /// Outputs: `true` when every byte is equal. Side effects: None.
    pub fn matches(&self, other: &Self) -> bool {
        self.0
            .iter()
            .zip(other.0.iter())
            .fold(0_u8, |difference, (left, right)| {
                difference | (left ^ right)
            })
            == 0
    }
}

impl std::fmt::Debug for PairKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PairKey(redacted)")
    }
}
