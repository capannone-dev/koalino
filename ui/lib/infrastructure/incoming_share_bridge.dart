// Files and e-mails other apps share to Koalino (Issue #85). On Android `IncomingShares.kt`
// copies each share into its own cache folder and announces it on `note_taker/incoming_shares`;
// this bridge takes the pending share and releases its folder once Rust has copied the files
// into a project. Desktops have no share sheet, so they get a bridge that never has a share.

import 'dart:async';
import 'dart:io' show Platform;

import 'package:flutter/services.dart'
    show MethodCall, MethodChannel, MissingPluginException, PlatformException;

/// The platform channel `MainActivity` registers for incoming shares.
const String incomingSharesChannelName = 'note_taker/incoming_shares';

/// One share, copied into a folder of its own.
final class IncomingShare {
  /// Creates an immutable share.
  const IncomingShare({
    required this.directory,
    required this.filePaths,
    required this.failedCount,
  });

  /// The folder holding the copies; released after use.
  final String directory;

  /// The copied files, in the order the sending app gave them.
  final List<String> filePaths;

  /// Files that could not be read or were larger than 100 MiB.
  final int failedCount;

  /// Decodes the map Kotlin sends, or returns `null` for a malformed one.
  static IncomingShare? fromChannel(Object? raw) {
    if (raw is! Map<Object?, Object?>) {
      return null;
    }
    final Object? directory = raw['directory'];
    final Object? paths = raw['paths'];
    final Object? failedCount = raw['failedCount'];
    if (directory is! String || paths is! List<Object?>) {
      return null;
    }
    return IncomingShare(
      directory: directory,
      filePaths: paths.whereType<String>().toList(growable: false),
      failedCount: failedCount is int ? failedCount : 0,
    );
  }
}

/// Hands over what other apps share.
abstract interface class IncomingShareBridge {
  /// Emits each time a share arrives while the app runs.
  Stream<void> get arrivals;

  /// Takes the pending share, or `null`; it is then no longer pending.
  ///
  /// Side effects: Clears the platform's pending share.
  Future<IncomingShare?> takePending();

  /// Deletes the copies of `share`.
  ///
  /// Side effects: Removes its cache folder. Failures are ignored; stale folders are removed
  /// after a day anyway.
  Future<void> release(IncomingShare share);
}

/// The bridge for this platform: Android's share sheet, or none.
IncomingShareBridge platformIncomingShareBridge() => Platform.isAndroid
    ? AndroidIncomingShareBridge()
    : const NoIncomingShareBridge();

/// A bridge without shares, for desktops and tests.
final class NoIncomingShareBridge implements IncomingShareBridge {
  /// Creates the bridge.
  const NoIncomingShareBridge();

  /// Never emits.
  @override
  Stream<void> get arrivals => const Stream<void>.empty();

  /// Always `null`.
  @override
  Future<IncomingShare?> takePending() async => null;

  /// Does nothing.
  @override
  Future<void> release(IncomingShare share) async {}
}

/// Android's shares over `note_taker/incoming_shares`.
final class AndroidIncomingShareBridge implements IncomingShareBridge {
  /// Creates the bridge and listens for arrivals.
  ///
  /// Side effects: Installs the channel's method-call handler.
  AndroidIncomingShareBridge() {
    _channel.setMethodCallHandler(_handleCall);
  }

  static const MethodChannel _channel =
      MethodChannel(incomingSharesChannelName);

  final StreamController<void> _arrivals = StreamController<void>.broadcast();

  @override
  Stream<void> get arrivals => _arrivals.stream;

  /// Side effects: Clears Kotlin's pending share. A channel failure reads as no share.
  @override
  Future<IncomingShare?> takePending() async {
    try {
      return IncomingShare.fromChannel(
          await _channel.invokeMethod<Object?>('takePending'));
    } on PlatformException {
      return null;
    } on MissingPluginException {
      return null;
    }
  }

  /// Side effects: Kotlin deletes the folder in the background.
  @override
  Future<void> release(IncomingShare share) async {
    try {
      await _channel.invokeMethod<void>(
          'release', <String, String>{'directory': share.directory});
    } on PlatformException {
      // Left for the daily clean-up.
    }
  }

  /// Turns Kotlin's `sharesArrived` into an event.
  Future<void> _handleCall(MethodCall call) async {
    if (call.method == 'sharesArrived') {
      _arrivals.add(null);
    }
  }
}
