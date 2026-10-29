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
//!
//! `seal_for_peer` encrypts a message for the other device with AES-256-GCM. Its key is derived
//! from the pair key with HKDF-SHA256 (salt `note-taker/pair-message/v1`, info: the message
//! purpose, such as `NTC1`), so each purpose has its own key. Layout: random nonce (12 bytes) ‖
//! ciphertext ‖ tag (16). The purpose, the sender's and the receiver's peer IDs are authenticated
//! as associated data, so a message cannot be reflected back or replayed as another kind.
//! Nothing here logs.

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng, Payload, rand_core::RngCore},
};
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::domain::{errors::CryptoError, models::DeviceId, peer_trust::PairKey};

/// HKDF salt, versioned so a future derivation cannot collide with this one.
const PAIR_KEY_SALT: &[u8] = b"note-taker/pair-key/v1";
/// HKDF salt of the per-purpose message keys derived from a pair key.
const PAIR_MESSAGE_SALT: &[u8] = b"note-taker/pair-message/v1";
/// Length of the random AES-GCM nonce.
const NONCE_LENGTH: usize = 12;
/// Length of the AES-GCM authentication tag.
const TAG_LENGTH: usize = 16;

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

/// Encrypts `plaintext` for the other device of a pair.
///
/// Inputs: The pair key, the message purpose (its four-byte header), this device's peer ID, the
/// receiving device's peer ID, and the plaintext.
/// Outputs: Nonce ‖ ciphertext ‖ tag, or `EncryptionFailed`.
/// Side effects: Draws a random nonce.
pub fn seal_for_peer(
    pair_key: &PairKey,
    purpose: &[u8],
    sender: &DeviceId,
    receiver: &DeviceId,
    plaintext: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    let cipher = message_cipher(pair_key, purpose)?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext,
                aad: &associated_data(purpose, sender, receiver),
            },
        )
        .map_err(|_| CryptoError::EncryptionFailed)?;
    let mut sealed = Vec::with_capacity(NONCE_LENGTH + ciphertext.len());
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(&ciphertext);
    Ok(sealed)
}

/// Decrypts a value `sender` sealed for `receiver` with `seal_for_peer`.
///
/// Inputs: The pair key, the expected purpose, the sending and receiving peer IDs, and the
/// sealed bytes.
/// Outputs: The plaintext, wiped on drop; `MalformedCiphertext` for a value too short to hold a
/// nonce and tag; `AuthenticationFailed` for a wrong key, direction, purpose, or any change.
/// Side effects: None.
pub fn open_from_peer(
    pair_key: &PairKey,
    purpose: &[u8],
    sender: &DeviceId,
    receiver: &DeviceId,
    sealed: &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    if sealed.len() < NONCE_LENGTH + TAG_LENGTH {
        return Err(CryptoError::MalformedCiphertext);
    }
    let (nonce, ciphertext) = sealed.split_at(NONCE_LENGTH);
    message_cipher(pair_key, purpose)?
        .decrypt(
            Nonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: &associated_data(purpose, sender, receiver),
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::AuthenticationFailed)
}

/// Derives the AES-256-GCM cipher for one message purpose.
///
/// Outputs: The cipher, or `KeyAgreement` if HKDF refuses the length (it never does for 32 bytes).
/// Side effects: None.
fn message_cipher(pair_key: &PairKey, purpose: &[u8]) -> Result<Aes256Gcm, CryptoError> {
    let mut key_bytes = Zeroizing::new([0_u8; 32]);
    Hkdf::<Sha256>::new(Some(PAIR_MESSAGE_SALT), pair_key.as_bytes())
        .expand(purpose, key_bytes.as_mut())
        .map_err(|_| CryptoError::KeyAgreement)?;
    Aes256Gcm::new_from_slice(key_bytes.as_ref()).map_err(|_| CryptoError::EncryptionFailed)
}

/// Builds the associated data: purpose ‖ 0 ‖ sender ‖ 0 ‖ receiver.
fn associated_data(purpose: &[u8], sender: &DeviceId, receiver: &DeviceId) -> Vec<u8> {
    let mut associated_data =
        Vec::with_capacity(purpose.len() + sender.as_str().len() + receiver.as_str().len() + 2);
    associated_data.extend_from_slice(purpose);
    associated_data.push(0);
    associated_data.extend_from_slice(sender.as_str().as_bytes());
    associated_data.push(0);
    associated_data.extend_from_slice(receiver.as_str().as_bytes());
    associated_data
}
