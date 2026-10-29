//! Agrees the per-pair key that protects review clips sent between paired devices (Issue #50).
//!
//! Every device keeps one static X25519 secret (sealed at rest by the peer store) and sends its
//! public key to each paired device over the Noise-authenticated stream. A device combines its
//! secret with the other's public key, so both arrive at the same Diffie-Hellman output without
//! it ever crossing the network. HKDF-SHA256 turns that output into the `PairKey`:
//!
//! - salt: `note-taker/pair-key/v1`;
//! - info: the two peer IDs and their public keys, ordered by peer ID, so the key is bound to the
//!   pair and both devices build the same info.
//!
//! The secret is static, so the key stays the same across reconnections and re-pairing and
//! needs no coordination. Forward secrecy for traffic comes from the Noise transport underneath.
//! Nothing here logs.

use aes_gcm::aead::{OsRng, rand_core::RngCore};
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::domain::{errors::CryptoError, models::DeviceId, peer_trust::PairKey};

/// HKDF salt, versioned so a future derivation cannot collide with this one.
const PAIR_KEY_SALT: &[u8] = b"note-taker/pair-key/v1";

/// This device's static X25519 secret; wiped on drop and never printed.
pub struct PairAgreementSecret(StaticSecret);

impl PairAgreementSecret {
    /// Generates a secret from the operating system's secure random number generator.
    ///
    /// Outputs: A fresh secret. Side effects: Reads operating-system randomness.
    pub fn generate() -> Self {
        let mut secret_bytes = Zeroizing::new([0_u8; 32]);
        OsRng.fill_bytes(secret_bytes.as_mut());
        Self(StaticSecret::from(*secret_bytes))
    }

    /// Restores a secret from its stored bytes.
    ///
    /// Inputs: The bytes `to_bytes` produced. Outputs: The secret, or `MalformedCiphertext` when
    /// they are not 32 bytes. Side effects: None.
    pub fn from_bytes(secret_bytes: &[u8]) -> Result<Self, CryptoError> {
        let secret_bytes: Zeroizing<[u8; 32]> = Zeroizing::new(
            secret_bytes
                .try_into()
                .map_err(|_| CryptoError::MalformedCiphertext)?,
        );
        Ok(Self(StaticSecret::from(*secret_bytes)))
    }

    /// The secret's bytes, for sealed storage only.
    pub fn to_bytes(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(self.0.to_bytes())
    }

    /// The public key sent to paired devices.
    pub fn public_key(&self) -> [u8; 32] {
        PublicKey::from(&self.0).to_bytes()
    }

    /// Derives the key this device shares with `remote_peer`.
    ///
    /// Inputs: This device's peer ID, the paired device's authenticated peer ID, and the public
    /// key it sent.
    /// Outputs: The pair key, or `KeyAgreement` for a low-order public key.
    /// Side effects: None.
    pub fn derive_pair_key(
        &self,
        local_peer: &DeviceId,
        remote_peer: &DeviceId,
        remote_public_key: &[u8; 32],
    ) -> Result<PairKey, CryptoError> {
        let shared_secret = self.0.diffie_hellman(&PublicKey::from(*remote_public_key));
        if !shared_secret.was_contributory() {
            return Err(CryptoError::KeyAgreement);
        }
        let local_public_key = self.public_key();
        let (first, second) = if local_peer.as_str() <= remote_peer.as_str() {
            (
                (local_peer, &local_public_key),
                (remote_peer, remote_public_key),
            )
        } else {
            (
                (remote_peer, remote_public_key),
                (local_peer, &local_public_key),
            )
        };
        let mut info = Vec::with_capacity(first.0.as_str().len() + second.0.as_str().len() + 66);
        info.extend_from_slice(first.0.as_str().as_bytes());
        info.push(0);
        info.extend_from_slice(second.0.as_str().as_bytes());
        info.push(0);
        info.extend_from_slice(first.1);
        info.extend_from_slice(second.1);
        let mut key_bytes = Zeroizing::new([0_u8; 32]);
        Hkdf::<Sha256>::new(Some(PAIR_KEY_SALT), shared_secret.as_bytes())
            .expand(&info, key_bytes.as_mut())
            .map_err(|_| CryptoError::KeyAgreement)?;
        Ok(PairKey::from_bytes(*key_bytes))
    }
}

impl std::fmt::Debug for PairAgreementSecret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PairAgreementSecret(redacted)")
    }
}
