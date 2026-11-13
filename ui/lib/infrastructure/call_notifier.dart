// Shows the offer to record a call as a system notification (#111), so it is seen while the
// app's window is hidden. On Linux it runs `notify-send` with Record and Not now buttons
// (libnotify 0.7.9 or later prints the chosen button); elsewhere, or without `notify-send`, only
// the in-app banner shows. No plugin is needed.

import 'dart:async';
import 'dart:convert';
import 'dart:io';

/// What the owner did with the notification.
enum CallNotificationAnswer {
  /// Tapped Record.
  record,

  /// Tapped Not now, closed it, or it expired or was withdrawn.
  dismissed,
}

/// Shows and withdraws the call offer outside the app.
abstract interface class CallNotifier {
  /// Shows the offer for `app` and completes with the owner's answer.
  Future<CallNotificationAnswer> offer(String app);

  /// Withdraws the offer shown, if any (answered in the app, or the call ended).
  void withdraw();
}

/// Shows nothing; the banner alone offers.
final class NoCallNotifier implements CallNotifier {
  /// Creates the notifier.
  const NoCallNotifier();

  @override
  Future<CallNotificationAnswer> offer(String app) async =>
      CallNotificationAnswer.dismissed;

  @override
  void withdraw() {}
}

/// Starts a process; replaced in tests.
typedef ProcessStarter = Future<Process> Function(
    String executable, List<String> arguments);

/// The desktop notification through `notify-send` (Linux).
final class NotifySendCallNotifier implements CallNotifier {
  /// Creates the notifier; `start` is replaced in tests.
  NotifySendCallNotifier({ProcessStarter start = Process.start})
      : _start = start;

  final ProcessStarter _start;
  Process? _shown;

  /// The arguments for `app`'s offer, which expires after two minutes.
  static List<String> argumentsFor(String app) => <String>[
        '--app-name=Koalino',
        '--icon=audio-input-microphone',
        '--expire-time=120000',
        '--action=record=Record',
        '--action=dismiss=Not now',
        '$app is using the microphone',
        'Record this call?',
      ];

  /// Side effects: Starts `notify-send` and waits for its answer; a missing tool counts as
  /// dismissed.
  @override
  Future<CallNotificationAnswer> offer(String app) async {
    withdraw();
    final Process process;
    try {
      process = await _start('notify-send', argumentsFor(app));
    } on ProcessException {
      return CallNotificationAnswer.dismissed;
    }
    _shown = process;
    final String chosen =
        (await process.stdout.transform(utf8.decoder).join()).trim();
    unawaited(process.stderr.drain<void>());
    await process.exitCode;
    if (identical(_shown, process)) {
      _shown = null;
    }
    return chosen == 'record'
        ? CallNotificationAnswer.record
        : CallNotificationAnswer.dismissed;
  }

  /// Side effects: Ends the waiting `notify-send`, so its answer no longer counts.
  @override
  void withdraw() {
    _shown?.kill();
    _shown = null;
  }
}

/// The notifier for this platform.
CallNotifier platformCallNotifier() =>
    Platform.isLinux ? NotifySendCallNotifier() : const NoCallNotifier();
