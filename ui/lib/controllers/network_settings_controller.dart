// Holds the Settings screen's relay and static peer addresses (Issue #64). The core validates,
// stores, and applies them at once; this only reads them and reports a refusal in plain words.

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/p2p_bridge.dart';

/// Owns the network addresses for Settings.
final class NetworkSettingsController extends ChangeNotifier {
  /// Creates the controller. Inputs: The bridge to the core. Side effects: None until `load`.
  NetworkSettingsController({required P2pBridge p2pBridge})
      : _p2pBridge = p2pBridge;

  final P2pBridge _p2pBridge;

  NetworkRoutes? _routes;
  bool _available = false;
  String? _message;
  bool _isSaving = false;
  bool _disposed = false;

  /// Whether the fields can be offered: device sync is on and its status was read.
  bool get available => _available;

  /// The routes in effect, or `null` before they load.
  NetworkRoutes? get routes => _routes;

  /// The relay address in effect, or an empty string.
  String get relayAddress => _routes?.relayAddress ?? '';

  /// The static peers in effect, one per line.
  String get peerAddresses => _routes?.peers.join('\n') ?? '';

  /// A refusal or confirmation to show, or `null`.
  String? get message => _message;

  /// Whether a save is in progress.
  bool get isSaving => _isSaving;

  /// How far this device has got with its relay, in the owner's words, or `null` without one.
  String? get relayStatus => switch (_routes?.relayState) {
        'not_paired' =>
          'Not paired yet. Make the relay discoverable, then pair with it under Nearby devices in Devices.',
        'connecting' => 'Connecting to the relay…',
        'connected' =>
          'Connected to the relay. Asking it to accept connections…',
        'reserved' =>
          'Connected. Your other devices can reach this one through the relay.',
        _ => null,
      };

  /// Reads the routes.
  ///
  /// Side effects: Calls the bridge and notifies listeners. A failure leaves the fields hidden.
  Future<void> load() async {
    try {
      final P2pStatus status = await _p2pBridge.getStatus();
      _available = status.enabled;
      _routes = status.network;
    } on Object {
      _available = false;
    }
    _notify();
  }

  /// Saves the fields; they apply at once, with no restart.
  ///
  /// Inputs: The relay field and the peers field (one address per line); blank means none.
  /// Side effects: Calls the bridge, sets `message`, reloads, and notifies listeners. A refused
  /// value changes nothing.
  Future<void> save({required String relay, required String peers}) async {
    final String trimmedRelay = relay.trim();
    final List<String> peerList = peers
        .split('\n')
        .map((String peer) => peer.trim())
        .where((String peer) => peer.isNotEmpty)
        .toList(growable: false);
    _isSaving = true;
    _message = null;
    _notify();
    try {
      await _p2pBridge.setNetworkAddresses(
        relay: trimmedRelay.isEmpty ? null : trimmedRelay,
        peers: peerList,
      );
      _message = 'Saved. This applies now, with no restart.';
      await load();
    } on P2pException catch (error) {
      _message = switch (error.errorCode) {
        'invalid_relay_address' =>
          'That relay address is not valid. It looks like /ip4/203.0.113.5/tcp/4001/p2p/12D3Koo…',
        'invalid_peer_address' =>
          'A device address is not valid. Each ends in /p2p/ and the device\'s ID.',
        'unavailable' => 'Device sync is not running.',
        _ => 'Unable to save the addresses.',
      };
    } on Object {
      _message = 'Unable to save the addresses.';
    }
    _isSaving = false;
    _notify();
  }

  /// Notifies listeners unless disposed.
  void _notify() {
    if (!_disposed) {
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
