//! Specifies the per-pair key agreement (Issue #50): both devices of a pair derive the same key,
//! every pair gets its own key, low-order public keys are refused, and secrets stay out of logs.

use super::pair_key::PairAgreementSecret;
use crate::domain::{errors::CryptoError, models::DeviceId};

/// Restores a secret from fixed bytes, so the tests are deterministic.
fn secret(fill_byte: u8) -> PairAgreementSecret {
    PairAgreementSecret::from_bytes(&[fill_byte; 32])
        .unwrap_or_else(|error| panic!("32 bytes must form a secret: {error}"))
}

/// The laptop and the phone each combine their own secret with the other's public key and get
/// the same pair key.
#[test]
fn both_devices_of_a_pair_derive_the_same_key() {
    let laptop_id = DeviceId::new("peer-laptop");
    let phone_id = DeviceId::new("peer-phone");
    let laptop_secret = secret(1);
    let phone_secret = secret(2);

    let on_laptop = laptop_secret
        .derive_pair_key(&laptop_id, &phone_id, &phone_secret.public_key())
        .unwrap_or_else(|error| panic!("laptop must derive: {error}"));
    let on_phone = phone_secret
        .derive_pair_key(&phone_id, &laptop_id, &laptop_secret.public_key())
        .unwrap_or_else(|error| panic!("phone must derive: {error}"));

    assert!(on_laptop.matches(&on_phone));
}

/// A third device paired with the laptop gets a different key, and so does the same agreement
/// claimed under other peer IDs.
#[test]
fn each_pair_has_its_own_key() {
    let laptop_id = DeviceId::new("peer-laptop");
    let phone_id = DeviceId::new("peer-phone");
    let laptop_secret = secret(1);
    let phone_key = laptop_secret
        .derive_pair_key(&laptop_id, &phone_id, &secret(2).public_key())
        .unwrap_or_else(|error| panic!("phone pair must derive: {error}"));
    let tablet_key = laptop_secret
        .derive_pair_key(
            &laptop_id,
            &DeviceId::new("peer-tablet"),
            &secret(3).public_key(),
        )
        .unwrap_or_else(|error| panic!("tablet pair must derive: {error}"));
    let relabelled_key = laptop_secret
        .derive_pair_key(
            &laptop_id,
            &DeviceId::new("peer-other"),
            &secret(2).public_key(),
        )
        .unwrap_or_else(|error| panic!("relabelled pair must derive: {error}"));

    assert!(!phone_key.matches(&tablet_key));
    assert!(!phone_key.matches(&relabelled_key));
}

/// A public key of low order (here all zeros) would make the agreement output predictable, so it
/// is refused instead of producing a key an attacker knows.
#[test]
fn a_low_order_public_key_is_refused() {
    let derived = secret(1).derive_pair_key(
        &DeviceId::new("peer-laptop"),
        &DeviceId::new("peer-phone"),
        &[0; 32],
    );

    assert!(matches!(derived, Err(CryptoError::KeyAgreement)));
}

/// Generated secrets differ, survive a round trip through their bytes, and only accept 32 bytes.
#[test]
fn secrets_are_random_and_round_trip() {
    let first = PairAgreementSecret::generate();
    let second = PairAgreementSecret::generate();
    assert_ne!(first.public_key(), second.public_key());

    let restored = PairAgreementSecret::from_bytes(first.to_bytes().as_slice())
        .unwrap_or_else(|error| panic!("stored bytes must restore: {error}"));
    assert_eq!(restored.public_key(), first.public_key());

    assert!(matches!(
        PairAgreementSecret::from_bytes(&[7; 31]),
        Err(CryptoError::MalformedCiphertext)
    ));
}

/// Debug output names the types but never prints secret or key bytes.
#[test]
fn secrets_and_keys_are_redacted_in_debug_output() {
    let laptop_secret = secret(1);
    let pair_key = laptop_secret
        .derive_pair_key(
            &DeviceId::new("peer-laptop"),
            &DeviceId::new("peer-phone"),
            &secret(2).public_key(),
        )
        .unwrap_or_else(|error| panic!("pair must derive: {error}"));

    let printed = format!("{laptop_secret:?} {pair_key:?}");

    assert!(printed.contains("redacted"));
    assert!(!printed.contains("1, 1"));
    assert!(!printed.contains(&format!("{:?}", pair_key.as_bytes())));
}
