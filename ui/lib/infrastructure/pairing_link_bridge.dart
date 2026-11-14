// Pairing invite links opened from outside the app (Issue #122). A phone's camera reads the QR
// code another device shows, a `note-taker-invite:` link, and Android opens Koalino with it.
// `PairingLinks.kt` keeps the link as pending and announces it on `note_taker/pairing_links`;
// this bridge takes it. Desktops are not opened by links, so they get a bridge that never has one.

import 'dart:async';
import 'dart:io' show Platform;

import 'package:flutter/services.dart'
    show MethodCall, MethodChannel, MissingPluginException, PlatformException;

/// The platform channel `MainActivity` registers for pairing links.
const String pairingLinksChannelName = 'note_taker/pairing_links';

/// The scheme of the links the invite QR code holds.
const String pairingLinkScheme = 'note-taker-invite:';

/// Hands over invite links that opened the app.
abstract interface class PairingLinkBridge {
  /// Emits each time a link arrives while the app runs.
  Stream<void> get arrivals;

  /// Takes the pending link's text, or `null`; it is then no longer pending.
  ///
  /// Outputs: The link as opened, untrusted (Rust checks it when it is used).
  /// Side effects: Clears the platform's pending link.
  Future<String?> takePending();
}

/// The bridge for this platform: Android's links, or none.
PairingLinkBridge platformPairingLinkBridge() => Platform.isAndroid
    ? AndroidPairingLinkBridge()
    : const NoPairingLinkBridge();

/// A bridge without links, for desktops and tests.
final class NoPairingLinkBridge implements PairingLinkBridge {
  /// Creates the bridge.
  const NoPairingLinkBridge();

  /// Never emits.
  @override
  Stream<void> get arrivals => const Stream<void>.empty();

  /// Always `null`.
  @override
  Future<String?> takePending() async => null;
}

/// Android's links over `note_taker/pairing_links`.
final class AndroidPairingLinkBridge implements PairingLinkBridge {
  /// Creates the bridge and listens for arrivals.
  ///
  /// Side effects: Installs the channel's method-call handler.
  AndroidPairingLinkBridge() {
    _channel.setMethodCallHandler(_handleCall);
  }

  static const MethodChannel _channel = MethodChannel(pairingLinksChannelName);

  final StreamController<void> _arrivals = StreamController<void>.broadcast();

  @override
  Stream<void> get arrivals => _arrivals.stream;

  /// Side effects: Clears Kotlin's pending link. A channel failure, or text that is not an
  /// invite link, reads as no link.
  @override
  Future<String?> takePending() async {
    try {
      final String? link = await _channel.invokeMethod<String>('takePending');
      return link != null && link.startsWith(pairingLinkScheme) ? link : null;
    } on PlatformException {
      return null;
    } on MissingPluginException {
      return null;
    }
  }

  /// Announces `linkArrived` from Kotlin.
  Future<void> _handleCall(MethodCall call) async {
    if (call.method == 'linkArrived') {
      _arrivals.add(null);
    }
  }
}
