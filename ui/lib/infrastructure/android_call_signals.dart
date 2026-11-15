// Tells a recording on Android that a call holds the microphone (#111). Kotlin
// (`CallSignals.kt`, on `note_taker/call_signals`) answers `silenced`: whether Android is feeding
// the recording silence because a call has the microphone, which it does for the whole call.
// The phone does not offer to record calls (`offersToRecordCalls`), since it cannot hear them.

import 'package:flutter/services.dart';

/// Asks Kotlin whether a call silences this app's recording.
final class AndroidCallSignals {
  /// Creates the reader; `channel` is replaced in tests.
  const AndroidCallSignals(
      {MethodChannel channel = const MethodChannel('note_taker/call_signals')})
      : _channel = channel;

  final MethodChannel _channel;

  /// Whether Android is giving this app's recording silence because a call holds the
  /// microphone (#111): its concurrent-capture policy lets the call's capture through and
  /// silences the others.
  ///
  /// Outputs: `true` while silenced; `false` when not, or when Kotlin cannot tell (before
  /// Android 10). Throws `PlatformException` or `MissingPluginException` when Kotlin cannot
  /// answer. Side effects: None.
  Future<bool> silenced() async =>
      await _channel.invokeMethod<bool>('silenced') ?? false;
}
