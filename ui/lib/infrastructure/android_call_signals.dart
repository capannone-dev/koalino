// Notices calls on Android (#111). Android hides which app captures the microphone, so Kotlin
// (`CallSignals.kt`, on `note_taker/call_signals`) reports a voice call in progress: another app
// holds the audio in communication mode, or captures the voice-call source. The core's call watch
// then decides when a call starts and ends, as on desktops. The offer shows as a notification
// (`CallOffer.kt`, on `note_taker/call_offer`): Record opens the app and answers `record`, since
// a recording can only start while the app is in front; Not now or swiping it away answers
// `dismissed`. While recording, `silenced` says whether Android is feeding the recording silence
// because a call holds the microphone, which it does for the whole call.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:note_taker_ui/infrastructure/call_notifier.dart';
import 'package:note_taker_ui/infrastructure/call_watch_bridge.dart';

/// Asks Kotlin whether a call holds the microphone.
final class AndroidCallSignals {
  /// Creates the reader; `channel` is replaced in tests.
  const AndroidCallSignals(
      {MethodChannel channel = const MethodChannel('note_taker/call_signals')})
      : _channel = channel;

  final MethodChannel _channel;

  /// The users Kotlin saw: one unnamed call app during a call, else none.
  ///
  /// Outputs: The users. Throws `PlatformException` or `MissingPluginException` when Kotlin
  /// cannot answer. Side effects: None.
  Future<List<HostMicrophoneUser>> users() async {
    final List<Object?> answer =
        await _channel.invokeListMethod<Object?>('users') ?? <Object?>[];
    return <HostMicrophoneUser>[
      for (final Object? user in answer)
        if (user case {'process': final String process})
          HostMicrophoneUser(
              process: process,
              name: switch (user) {
                {'name': final String name} => name,
                _ => '',
              }),
    ];
  }

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

/// The call offer as an Android notification.
final class AndroidCallNotifier implements CallNotifier {
  /// Creates the notifier and listens for its answers; `channel` is replaced in tests.
  AndroidCallNotifier(
      {MethodChannel channel = const MethodChannel('note_taker/call_offer')})
      : _channel = channel {
    _channel.setMethodCallHandler(handleCall);
  }

  final MethodChannel _channel;
  Completer<CallNotificationAnswer>? _pending;

  /// Side effects: Posts the notification; answers `unavailable` when notifications are off or
  /// Kotlin cannot show one.
  @override
  Future<CallNotificationAnswer> offer(String app) async {
    _finish(CallNotificationAnswer.dismissed);
    final Completer<CallNotificationAnswer> pending =
        Completer<CallNotificationAnswer>();
    _pending = pending;
    final bool shown;
    try {
      shown = await _channel
              .invokeMethod<bool>('show', <String, Object?>{'app': app}) ??
          false;
    } on PlatformException {
      _finish(CallNotificationAnswer.unavailable);
      return pending.future;
    } on MissingPluginException {
      _finish(CallNotificationAnswer.unavailable);
      return pending.future;
    }
    if (!shown) {
      _finish(CallNotificationAnswer.unavailable);
    }
    return pending.future;
  }

  /// Side effects: Removes the notification; the offer's answer is `dismissed`.
  @override
  void withdraw() {
    _finish(CallNotificationAnswer.dismissed);
    unawaited(
        _channel.invokeMethod<void>('withdraw').catchError((Object _) {}));
  }

  /// Takes Kotlin's `answer` (`record` or `dismissed`). Public for tests.
  ///
  /// Side effects: Completes the waiting offer.
  Future<void> handleCall(MethodCall call) async {
    if (call.method == 'answer') {
      _finish(call.arguments == 'record'
          ? CallNotificationAnswer.record
          : CallNotificationAnswer.dismissed);
    }
  }

  void _finish(CallNotificationAnswer answer) {
    final Completer<CallNotificationAnswer>? pending = _pending;
    _pending = null;
    if (pending != null && !pending.isCompleted) {
      pending.complete(answer);
    }
  }
}
