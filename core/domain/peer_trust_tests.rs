//! Specifies the pairing domain: device-name validation and the numeric-comparison code that
//! both devices show during pairing.

use super::{
    errors::DeviceNameError,
    models::DeviceId,
    peer_trust::{DeviceName, PairingCode, TrustedPeer},
};

/// Names are trimmed and must be 1–64 characters with no control characters.
#[test]
fn device_names_are_trimmed_and_validated() {
    assert_eq!(
        DeviceName::parse("  Ettore's laptop ")
            .map(|name| name.as_str().to_owned())
            .ok(),
        Some(String::from("Ettore's laptop"))
    );
    assert_eq!(DeviceName::parse("   ").err(), Some(DeviceNameError::Blank));
    assert_eq!(
        DeviceName::parse(&"x".repeat(65)).err(),
        Some(DeviceNameError::TooLong { max_characters: 64 })
    );
    assert!(DeviceName::parse(&"é".repeat(64)).is_ok());
    assert_eq!(
        DeviceName::parse("phone\u{1b}[31m").err(),
        Some(DeviceNameError::ControlCharacter)
    );
}

/// Both devices compute the same six-digit code, whichever side computes it.
#[test]
fn the_pairing_code_is_the_same_on_both_devices() {
    let laptop = DeviceId::new("12D3KooWLaptop");
    let phone = DeviceId::new("12D3KooWPhone");

    let laptop_code = PairingCode::for_peers(&laptop, &phone);
    let phone_code = PairingCode::for_peers(&phone, &laptop);

    assert_eq!(laptop_code, phone_code);
    assert_eq!(laptop_code.as_str().len(), 6);
    assert!(
        laptop_code
            .as_str()
            .bytes()
            .all(|byte| byte.is_ascii_digit())
    );
}

/// An impostor peer yields a different code, which is what the person's comparison catches.
#[test]
fn a_different_peer_yields_a_different_code() {
    let laptop = DeviceId::new("12D3KooWLaptop");

    let real_code = PairingCode::for_peers(&laptop, &DeviceId::new("12D3KooWPhone"));
    let impostor_code = PairingCode::for_peers(&laptop, &DeviceId::new("12D3KooWImpostor"));

    assert_ne!(real_code, impostor_code);
}

/// Codes are fixed for given IDs (golden value), so every platform's implementation agrees.
#[test]
fn the_pairing_code_has_a_stable_golden_value() {
    let code = PairingCode::for_peers(&DeviceId::new("peer-a"), &DeviceId::new("peer-b"));

    assert_eq!(code.as_str(), GOLDEN_CODE_FOR_PEER_A_AND_B);
}

/// Computed independently (Python `hashlib`) from the documented derivation; changing the
/// derivation would break pairing between versions.
const GOLDEN_CODE_FOR_PEER_A_AND_B: &str = "636944";

/// A trusted peer keeps its ID, name, and pairing time.
#[test]
fn a_trusted_peer_exposes_its_fields() {
    let name = DeviceName::parse("Phone").unwrap_or_else(|error| panic!("valid name: {error}"));
    let peer = TrustedPeer::new(DeviceId::new("12D3KooWPhone"), name, 1_700_000_000_000);

    assert_eq!(peer.peer_id().as_str(), "12D3KooWPhone");
    assert_eq!(peer.device_name().as_str(), "Phone");
    assert_eq!(peer.paired_at_ms(), 1_700_000_000_000);
}
