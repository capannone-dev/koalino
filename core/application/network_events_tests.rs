//! Defines application-layer handling of portable network events before libp2p integration.

use std::collections::VecDeque;

use crate::{
    application::network_events::{NetworkEventHandler, NetworkEventHandlingError},
    domain::{errors::NetworkError, models::DeviceId},
    ports::network::{NetworkEvent, P2pNetwork},
};

/// Supplies deterministic portable events without creating TCP or UDP sockets in unit tests.
struct MockP2pNetwork {
    queued_results: VecDeque<Result<Option<NetworkEvent>, NetworkError>>,
}

impl MockP2pNetwork {
    /// Creates a mock port that returns the supplied results in FIFO order.
    fn new(
        queued_results: impl IntoIterator<Item = Result<Option<NetworkEvent>, NetworkError>>,
    ) -> Self {
        Self {
            queued_results: queued_results.into_iter().collect(),
        }
    }
}

impl P2pNetwork for MockP2pNetwork {
    fn send_encrypted(
        &mut self,
        _peer_device_id: &DeviceId,
        _payload: &[u8],
    ) -> Result<(), NetworkError> {
        Ok(())
    }

    fn broadcast_encrypted(&mut self, _payload: &[u8]) -> Result<(), NetworkError> {
        Ok(())
    }

    fn poll_event(&mut self) -> Result<Option<NetworkEvent>, NetworkError> {
        self.queued_results.pop_front().unwrap_or(Ok(None))
    }
}

/// Verifies that a discovered authenticated peer becomes reachable through the application state.
#[test]
fn discovered_peer_is_recorded_as_reachable() {
    let discovered_device_id = DeviceId::new("peer-milan");
    let mut mock_network = MockP2pNetwork::new([Ok(Some(NetworkEvent::PeerDiscovered(
        discovered_device_id.clone(),
    )))]);
    let mut network_event_handler = NetworkEventHandler::new();

    network_event_handler
        .process_next_event(&mut mock_network)
        .unwrap_or_else(|error| panic!("discovery event should be accepted: {error}"));

    assert_eq!(
        network_event_handler.reachable_peers(),
        &[discovered_device_id]
    );
}

/// Verifies that expiration removes a previously discovered peer from application-visible reachability.
#[test]
fn expired_peer_is_removed_from_reachable_peers() {
    let expired_device_id = DeviceId::new("peer-florence");
    let mut mock_network = MockP2pNetwork::new([
        Ok(Some(NetworkEvent::PeerDiscovered(
            expired_device_id.clone(),
        ))),
        Ok(Some(NetworkEvent::PeerExpired(expired_device_id))),
    ]);
    let mut network_event_handler = NetworkEventHandler::new();

    network_event_handler
        .process_next_event(&mut mock_network)
        .unwrap_or_else(|error| panic!("discovery event should be accepted: {error}"));
    network_event_handler
        .process_next_event(&mut mock_network)
        .unwrap_or_else(|error| panic!("expiration event should be accepted: {error}"));

    assert!(network_event_handler.reachable_peers().is_empty());
}

/// Verifies that one application poll consumes at most one event and preserves event ordering.
#[test]
fn events_are_processed_in_port_order() {
    let first_device_id = DeviceId::new("peer-rome");
    let second_device_id = DeviceId::new("peer-turin");
    let mut mock_network = MockP2pNetwork::new([
        Ok(Some(NetworkEvent::PeerDiscovered(first_device_id.clone()))),
        Ok(Some(NetworkEvent::PeerDiscovered(second_device_id.clone()))),
    ]);
    let mut network_event_handler = NetworkEventHandler::new();

    network_event_handler
        .process_next_event(&mut mock_network)
        .unwrap_or_else(|error| panic!("first event should be accepted: {error}"));
    assert_eq!(
        network_event_handler.reachable_peers(),
        std::slice::from_ref(&first_device_id)
    );

    network_event_handler
        .process_next_event(&mut mock_network)
        .unwrap_or_else(|error| panic!("second event should be accepted: {error}"));
    assert_eq!(
        network_event_handler.reachable_peers(),
        &[first_device_id, second_device_id]
    );
}

/// Verifies that a network-port failure remains portable at the application boundary.
#[test]
fn network_port_failure_is_returned_without_libp2p_details() {
    let mut mock_network = MockP2pNetwork::new([Err(NetworkError::Disconnected)]);
    let mut network_event_handler = NetworkEventHandler::new();

    let received_error = network_event_handler
        .process_next_event(&mut mock_network)
        .expect_err("a port failure must be returned to the caller");

    assert!(matches!(
        received_error,
        NetworkEventHandlingError::Network(NetworkError::Disconnected)
    ));
}
