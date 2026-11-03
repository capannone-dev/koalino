//! Specifies relaying through the owner's daemon over real loopback TCP, with mDNS off so the only
//! route between the two devices is the relay: paired devices sync through it, the destination's
//! gate still refuses strangers, a stranger admitted for pairing cannot keep using it, and losing
//! the relay only costs the reservation until it returns.

use std::time::Duration;

use super::{Libp2pNetworkAdapter, NodeIdentity};
use crate::{
    domain::{models::DeviceId, network_addresses::RelayState},
    ports::network::{NetworkEvent, P2pNetwork},
};

/// Generous bound for loopback operations.
const DEADLINE: Duration = Duration::from_secs(20);

/// Builds a device with mDNS off.
fn device() -> Libp2pNetworkAdapter {
    Libp2pNetworkAdapter::without_mdns_for_tests(NodeIdentity::ephemeral(), false)
        .unwrap_or_else(|error| panic!("device must initialize: {error}"))
}

/// Builds a relay on `identity` listening on `listen_address`, and returns its dialable address.
async fn start_relay(
    identity: NodeIdentity,
    listen_address: &str,
) -> (Libp2pNetworkAdapter, String) {
    let peer_id = identity.peer_device_id();
    let mut relay = Libp2pNetworkAdapter::without_mdns_for_tests(identity, true)
        .unwrap_or_else(|error| panic!("relay must initialize: {error}"));
    relay
        .listen_on(listen_address)
        .unwrap_or_else(|error| panic!("relay must listen: {error}"));
    tokio::time::timeout(DEADLINE, async {
        while relay.listen_addresses().is_empty() {
            relay
                .drive_next_swarm_event()
                .await
                .unwrap_or_else(|error| panic!("relay must run: {error}"));
        }
    })
    .await
    .unwrap_or_else(|_| panic!("relay has no listen address"));
    let listen_address = relay.listen_addresses()[0].clone();
    relay
        .advertise_relay_address(&listen_address)
        .unwrap_or_else(|error| panic!("address must be valid: {error}"));
    (relay, format!("{listen_address}/p2p/{}", peer_id.as_str()))
}

/// Makes `first` and `second` trust each other.
fn pair(first: &mut Libp2pNetworkAdapter, second: &mut Libp2pNetworkAdapter) {
    let (first_id, second_id) = (first.local_device_id(), second.local_device_id());
    first
        .trust_peer(&second_id)
        .unwrap_or_else(|error| panic!("peer must be trusted: {error}"));
    second
        .trust_peer(&first_id)
        .unwrap_or_else(|error| panic!("peer must be trusted: {error}"));
}

/// Drives the nodes, redialing as the P2P task does, until `condition` holds or time runs out.
async fn drive_until(
    nodes: &mut [&mut Libp2pNetworkAdapter],
    condition: impl FnMut(&mut [&mut Libp2pNetworkAdapter]) -> bool,
) -> bool {
    drive_until_with(nodes, true, condition).await
}

/// Drives the nodes until `condition` holds or `DEADLINE` passes, redialing paired peers every
/// 200 ms only when `redial_paired` is set. Returns whether the condition held. The checks wait for
/// something to happen instead of sampling after a fixed time, so a busy machine only slows them
/// (#81).
async fn drive_until_with(
    nodes: &mut [&mut Libp2pNetworkAdapter],
    redial_paired: bool,
    mut condition: impl FnMut(&mut [&mut Libp2pNetworkAdapter]) -> bool,
) -> bool {
    tokio::time::timeout(DEADLINE, async {
        let mut redial = tokio::time::interval(Duration::from_millis(200));
        while !condition(nodes) {
            let redial_due = {
                let drives = nodes
                    .iter_mut()
                    .map(|node| Box::pin(node.drive_next_swarm_event()))
                    .collect::<Vec<_>>();
                tokio::select! {
                    (result, _, _) = futures::future::select_all(drives) => {
                        result.unwrap_or_else(|error| panic!("swarm must run: {error}"));
                        false
                    }
                    _ = redial.tick() => true,
                }
            };
            if redial_paired && redial_due {
                for node in nodes.iter_mut() {
                    node.redial_paired_peers();
                }
            }
        }
    })
    .await
    .is_ok()
}

/// Whether `node` queued the payload from `sender`.
fn received(node: &mut Libp2pNetworkAdapter, sender: &DeviceId, payload: &[u8]) -> bool {
    while let Ok(Some(event)) = node.poll_event() {
        if let NetworkEvent::MessageReceived {
            peer_device_id,
            payload: received_payload,
        } = event
            && &peer_device_id == sender
            && received_payload == payload
        {
            return true;
        }
    }
    false
}

#[tokio::test]
async fn paired_devices_sync_through_the_relay() {
    let (mut relay, relay_address) =
        start_relay(NodeIdentity::ephemeral(), "/ip4/127.0.0.1/tcp/0").await;
    let (mut laptop, mut phone) = (device(), device());
    pair(&mut relay, &mut laptop);
    pair(&mut relay, &mut phone);
    pair(&mut laptop, &mut phone);
    for device in [&mut laptop, &mut phone] {
        device
            .use_relay(&relay_address)
            .unwrap_or_else(|error| panic!("relay address must be valid: {error}"));
    }
    let (laptop_id, phone_id) = (laptop.local_device_id(), phone.local_device_id());

    let met = drive_until(&mut [&mut relay, &mut laptop, &mut phone], |nodes| {
        nodes[1].has_relay_reservation()
            && nodes[2].has_relay_reservation()
            && nodes[1].is_connected_to(&phone_id)
    })
    .await;
    assert!(met, "the devices must connect through the relay");
    assert!(relay.relayed_circuits() > 0, "the route must be the relay");

    laptop
        .send_encrypted(&phone_id, b"relayed")
        .unwrap_or_else(|error| panic!("a paired device must accept a payload: {error}"));
    let delivered = drive_until(&mut [&mut relay, &mut laptop, &mut phone], |nodes| {
        received(nodes[2], &laptop_id, b"relayed")
    })
    .await;
    assert!(delivered, "the payload must arrive");
}

/// The relay's clients are all paired with it, but a device still only reaches its own peers.
#[tokio::test]
async fn the_destination_refuses_a_device_it_is_not_paired_with() {
    let (mut relay, relay_address) =
        start_relay(NodeIdentity::ephemeral(), "/ip4/127.0.0.1/tcp/0").await;
    let (mut laptop, mut other) = (device(), device());
    pair(&mut relay, &mut laptop);
    pair(&mut relay, &mut other);
    // Only `other` trusts the laptop, so its circuit must stop at the laptop's gate.
    let laptop_id = laptop.local_device_id();
    other
        .trust_peer(&laptop_id)
        .unwrap_or_else(|error| panic!("peer must be trusted: {error}"));
    for device in [&mut laptop, &mut other] {
        device
            .use_relay(&relay_address)
            .unwrap_or_else(|error| panic!("relay address must be valid: {error}"));
    }

    // The gate refuses at the handshake, so the laptop never counts `other` as connected, while
    // `other` may see each attempt open briefly before the laptop drops it. Check the laptop's
    // side on every step until `other` has tried the circuit several times.
    let other_id = other.local_device_id();
    let mut laptop_admitted_other = false;
    let attempted = drive_until(&mut [&mut relay, &mut laptop, &mut other], |nodes| {
        laptop_admitted_other |= nodes[1].is_connected_to(&other_id);
        laptop_admitted_other
            || nodes[1].has_relay_reservation()
                && nodes[2].has_relay_reservation()
                && nodes[0].relayed_circuits() >= 3
    })
    .await;

    assert!(
        !laptop_admitted_other,
        "the laptop must refuse a device it is not paired with"
    );
    assert!(
        attempted,
        "other must try to reach the laptop through the relay"
    );
}

/// A pairing window admits strangers to the relay's device, not to its relay service.
#[tokio::test]
async fn a_stranger_admitted_for_pairing_cannot_use_the_relay() {
    let (mut relay, relay_address) =
        start_relay(NodeIdentity::ephemeral(), "/ip4/127.0.0.1/tcp/0").await;
    relay.open_pairing_window(DEADLINE * 2);
    let relay_id = relay.local_device_id();
    let mut stranger = device();
    stranger
        .trust_peer(&relay_id)
        .unwrap_or_else(|error| panic!("peer must be trusted: {error}"));
    stranger
        .use_relay(&relay_address)
        .unwrap_or_else(|error| panic!("relay address must be valid: {error}"));

    let stranger_id = stranger.local_device_id();
    let mut was_refused = false;
    drive_until(&mut [&mut relay, &mut stranger], |nodes| {
        was_refused = nodes[0].refused_relay_clients() > 0;
        was_refused
    })
    .await;
    assert!(was_refused, "the reservation request must be noticed");
    // The open pairing window lets the stranger dial again, so stop redialing and wait for the
    // relay to drop the connection it refused, however long a busy machine takes.
    let dropped = drive_until_with(&mut [&mut relay, &mut stranger], false, |nodes| {
        !nodes[0].is_connected_to(&stranger_id)
    })
    .await;

    assert!(dropped, "the relay must disconnect the stranger it refused");
}

/// Losing the relay costs only the reservation, and it comes back with the relay.
#[tokio::test]
async fn the_reservation_returns_with_the_relay() {
    let probe = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap_or_else(|error| panic!("a port must be free: {error}"));
    let port = probe
        .local_addr()
        .unwrap_or_else(|error| panic!("a bound port has an address: {error}"))
        .port();
    drop(probe);
    let listen_address = format!("/ip4/127.0.0.1/tcp/{port}");
    let relay_identity = NodeIdentity::ephemeral();
    let (mut relay, relay_address) =
        start_relay(NodeIdentity(relay_identity.0.clone()), &listen_address).await;
    let mut laptop = device();
    pair(&mut relay, &mut laptop);
    laptop
        .use_relay(&relay_address)
        .unwrap_or_else(|error| panic!("relay address must be valid: {error}"));
    assert!(
        drive_until(&mut [&mut relay, &mut laptop], |nodes| nodes[1]
            .has_relay_reservation())
        .await
    );

    drop(relay);
    assert!(
        drive_until(&mut [&mut laptop], |nodes| !nodes[0]
            .has_relay_reservation())
        .await
    );

    let (mut relay, _) = start_relay(relay_identity, &listen_address).await;
    let laptop_id = laptop.local_device_id();
    relay
        .trust_peer(&laptop_id)
        .unwrap_or_else(|error| panic!("peer must be trusted: {error}"));
    assert!(
        drive_until(&mut [&mut relay, &mut laptop], |nodes| nodes[1]
            .has_relay_reservation())
        .await
    );
}

#[tokio::test]
async fn relay_addresses_must_name_the_relay() {
    let mut laptop = device();
    assert!(laptop.use_relay("/ip4/127.0.0.1/tcp/4001").is_err());
    assert!(laptop.use_relay("not an address").is_err());
    let own_address = format!(
        "/ip4/127.0.0.1/tcp/4001/p2p/{}",
        laptop.local_device_id().as_str()
    );
    assert!(laptop.use_relay(&own_address).is_err());
}

/// Verifies Settings can replace and remove the relay while running, forgetting its candidate.
#[tokio::test]
async fn the_relay_can_be_replaced_and_removed_while_running() {
    let mut adapter = device();
    let relay_id = device().local_device_id();
    let relay_address = format!("/ip4/192.0.2.1/tcp/4001/p2p/{}", relay_id.as_str());

    adapter
        .replace_network_addresses(Some(&relay_address), &[])
        .unwrap_or_else(|error| panic!("address must apply: {error}"));
    let state_with_relay = adapter.relay_state();
    let nearby_with_relay = adapter.nearby_unpaired_peers();
    adapter
        .replace_network_addresses(None, &[])
        .unwrap_or_else(|error| panic!("removal must apply: {error}"));

    assert_eq!(state_with_relay, RelayState::NotPaired);
    assert_eq!(nearby_with_relay, vec![relay_id]);
    assert_eq!(adapter.relay_state(), RelayState::NotConfigured);
    assert!(adapter.nearby_unpaired_peers().is_empty());
}
