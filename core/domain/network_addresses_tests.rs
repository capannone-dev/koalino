//! Specifies pairing invite text: it round-trips, and untrusted text is bounded and refused.

use super::network_addresses::{MAX_INVITE_ADDRESSES, PairingInvite, PairingInviteError};
use crate::domain::models::DeviceId;

/// An invite with one direct and one relayed address.
fn invite() -> PairingInvite {
    PairingInvite::new(
        DeviceId::new("12D3KooWPeer"),
        5_000,
        vec![
            "/ip4/203.0.113.5/tcp/4001".to_owned(),
            "/ip4/203.0.113.9/tcp/4001/p2p/12D3KooWRelay/p2p-circuit".to_owned(),
        ],
    )
}

#[test]
fn an_invite_survives_its_text_form() {
    let parsed = PairingInvite::parse(&format!("  {}\n", invite().to_text()));

    assert_eq!(parsed, Ok(invite()));
}

#[test]
fn dial_addresses_end_with_the_inviting_device() {
    assert_eq!(
        invite().dial_addresses(),
        vec![
            "/ip4/203.0.113.5/tcp/4001/p2p/12D3KooWPeer",
            "/ip4/203.0.113.9/tcp/4001/p2p/12D3KooWRelay/p2p-circuit/p2p/12D3KooWPeer",
        ]
    );
}

#[test]
fn expiry_is_reported_from_the_deadline_on() {
    assert!(!invite().is_expired(4_999));
    assert!(invite().is_expired(5_000));
}

#[test]
fn malformed_text_is_refused() {
    let too_many = ["/ip4/1.2.3.4/tcp/1"; MAX_INVITE_ADDRESSES + 1].join(",");
    for text in [
        String::new(),
        "https://example.com".to_owned(),
        "note-taker-invite:2;peer;5;/ip4/1.2.3.4/tcp/1".to_owned(),
        "note-taker-invite:1;peer;5".to_owned(),
        "note-taker-invite:1;peer;5;/ip4/1.2.3.4/tcp/1;extra".to_owned(),
        "note-taker-invite:1;;5;/ip4/1.2.3.4/tcp/1".to_owned(),
        "note-taker-invite:1;pe er;5;/ip4/1.2.3.4/tcp/1".to_owned(),
        "note-taker-invite:1;peer;soon;/ip4/1.2.3.4/tcp/1".to_owned(),
        "note-taker-invite:1;peer;5;not-an-address".to_owned(),
        "note-taker-invite:1;peer;5;/ip4/1.2.3.4/tcp/1<script>".to_owned(),
        format!("note-taker-invite:1;peer;5;{too_many}"),
        format!(
            "note-taker-invite:1;{};5;/ip4/1.2.3.4/tcp/1",
            "a".repeat(129)
        ),
        format!("note-taker-invite:1;peer;5;/{}", "a".repeat(3_000)),
    ] {
        assert_eq!(
            PairingInvite::parse(&text),
            Err(PairingInviteError::Malformed),
            "{text}"
        );
    }
}
