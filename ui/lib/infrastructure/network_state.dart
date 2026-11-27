// Tells whether the device is on an unmetered network (#189), so Automatic's model is downloaded
// by itself without spending mobile data. Android asks the system through `note_taker/network`;
// computers count as unmetered, as their downloads always did.
import 'dart:io';

import 'package:flutter/services.dart';

/// Reports the kind of network.
abstract interface class NetworkState {
  /// Outputs: `true` on Wi-Fi or another unmetered network; `false` on mobile data, a metered
  /// hotspot, no network, or when Android cannot answer. Side effects: None.
  Future<bool> isUnmetered();
}

/// The app's network state: the Android system's answer, or always unmetered on computers.
final class PlatformNetworkState implements NetworkState {
  /// Creates it.
  const PlatformNetworkState();

  static const MethodChannel _channel = MethodChannel('note_taker/network');

  @override
  Future<bool> isUnmetered() async {
    if (!Platform.isAndroid) {
      return true;
    }
    try {
      return await _channel.invokeMethod<bool>('isUnmetered') ?? false;
    } on Object {
      return false;
    }
  }
}
