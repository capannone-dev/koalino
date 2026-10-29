//! Specifies the `NTK1` message that carries a device's X25519 public key to a paired device
//! (Issue #50): exact framing, and rejection of anything but a 32-byte key.

use super::pair_key_exchange::{
    PairKeyMessageError, decode_pair_key_message, encode_pair_key_message,
};

/// A public key is framed as `NTK1` followed by its 32 bytes, and decodes back unchanged.
#[test]
fn a_public_key_round_trips() {
    let public_key = [5_u8; 32];

    let message = encode_pair_key_message(&public_key);

    assert_eq!(&message[..4], b"NTK1");
    assert_eq!(&message[4..], &public_key);
    assert_eq!(decode_pair_key_message(&message), Some(Ok(public_key)));
}

/// Other envelopes (sync, manifests) are not pair-key messages.
#[test]
fn other_envelopes_are_not_pair_key_messages() {
    assert_eq!(decode_pair_key_message(b"NTS1{}"), None);
    assert_eq!(decode_pair_key_message(b"NTM1{}"), None);
    assert_eq!(decode_pair_key_message(b""), None);
}

/// A key that is shorter or longer than 32 bytes is rejected.
#[test]
fn a_wrongly_sized_key_is_rejected() {
    let mut short = b"NTK1".to_vec();
    short.extend_from_slice(&[5; 31]);
    let mut long = b"NTK1".to_vec();
    long.extend_from_slice(&[5; 33]);

    assert_eq!(
        decode_pair_key_message(&short),
        Some(Err(PairKeyMessageError::Malformed))
    );
    assert_eq!(
        decode_pair_key_message(&long),
        Some(Err(PairKeyMessageError::Malformed))
    );
}
