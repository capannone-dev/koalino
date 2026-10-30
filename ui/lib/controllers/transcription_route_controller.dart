// Holds the Settings screen's choice of where recordings are transcribed (Issue #59). The core
// keeps and applies the choice; this only reads it, offers this device, the best device and each
// paired device, and reports a refusal in plain words.

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/p2p_bridge.dart';

/// One option in the selector.
typedef TranscriptionRouteChoice = ({String route, String label});

/// Owns the transcription choice for Settings.
final class TranscriptionRouteController extends ChangeNotifier {
  /// Creates the controller.
  ///
  /// Inputs: The bridge to the core. Side effects: None until `load`.
  TranscriptionRouteController({required P2pBridge p2pBridge})
      : _p2pBridge = p2pBridge;

  final P2pBridge _p2pBridge;

  P2pStatus? _status;
  String _route = 'local';
  String? _message;
  bool _isSaving = false;
  bool _disposed = false;

  /// Whether the choice can be offered: device sync is on and its status was read.
  bool get available => _status?.enabled ?? false;

  /// The route in effect, always one of `choices`.
  String get route => _route;

  /// Why the last change failed, or `null`.
  String? get message => _message;

  /// Whether a change is being saved; the selector is disabled meanwhile.
  bool get isSaving => _isSaving;

  /// This device, the best device, each paired device, and a stored choice no longer paired.
  List<TranscriptionRouteChoice> get choices {
    final List<PairedDevice> devices =
        _status?.pairedDevices ?? const <PairedDevice>[];
    final bool routeIsPeer = _route != 'local' && _route != 'auto';
    return <TranscriptionRouteChoice>[
      (route: 'local', label: 'This device'),
      (route: 'auto', label: 'Best available'),
      for (final PairedDevice device in devices)
        (route: device.peerId, label: device.deviceName),
      if (routeIsPeer &&
          !devices.any((PairedDevice device) => device.peerId == _route))
        (route: _route, label: 'A removed device'),
    ];
  }

  /// Reads the choice and the paired devices.
  ///
  /// Side effects: Calls the core; a failure leaves the selector hidden.
  Future<void> load() async {
    try {
      final P2pStatus status = await _p2pBridge.getStatus();
      _status = status;
      _route = status.transcriptionRoute;
    } on P2pException {
      _status = null;
    }
    _notify();
  }

  /// Applies a choice, keeping the old one when the core refuses it.
  ///
  /// Inputs: One of `choices`' routes. Side effects: Calls the core; the core stores the choice.
  Future<void> choose(String chosenRoute) async {
    _isSaving = true;
    _message = null;
    _notify();
    try {
      await _p2pBridge.setTranscriptionRoute(chosenRoute);
      _route = chosenRoute;
    } on P2pException {
      _message = 'The transcription device could not be changed. '
          'Check that it is still paired.';
    }
    _isSaving = false;
    _notify();
  }

  /// Notifies listeners unless disposed while a call was running.
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
