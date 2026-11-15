// Shows the offer to record a call as a system notification (#111), so it is seen while the
// app's window is hidden. On Linux it runs `notify-send` with Record and Not now buttons
// (libnotify 0.7.9 or later prints the chosen button). On Windows the core shows a toast
// (`note_taker_call_toast`, through WinRT; Windows PowerShell cannot receive a toast's button
// events). Android uses `android_call_signals.dart`. Elsewhere, or without the tool, only the
// in-app banner shows. No plugin is needed.

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// What the owner did with the notification.
enum CallNotificationAnswer {
  /// Tapped Record.
  record,

  /// Tapped Not now, closed it, or it expired or was withdrawn.
  dismissed,

  /// No notification could be shown (no notifier on this platform, or `notify-send` missing or
  /// failing); the in-app banner alone offers, so it must stay.
  unavailable,
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
      CallNotificationAnswer.unavailable;

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
        '--action=record=${l10n.record}',
        '--action=dismiss=${l10n.notNow}',
        l10n.isUsingTheMicrophone(app),
        l10n.recordThisCall,
      ];

  /// Side effects: Starts `notify-send` and waits for its answer; a missing tool, or one that
  /// fails, answers `unavailable`.
  @override
  Future<CallNotificationAnswer> offer(String app) async {
    withdraw();
    final Process process;
    try {
      process = await _start('notify-send', argumentsFor(app));
    } on ProcessException {
      return CallNotificationAnswer.unavailable;
    }
    _shown = process;
    final String chosen =
        (await process.stdout.transform(utf8.decoder).join()).trim();
    unawaited(process.stderr.drain<void>());
    final int exitCode = await process.exitCode;
    final bool withdrawn = !identical(_shown, process);
    if (!withdrawn) {
      _shown = null;
    }
    if (exitCode != 0 && !withdrawn) {
      return CallNotificationAnswer.unavailable;
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

/// The toast the core shows (Windows).
final class CoreToastCallNotifier implements CallNotifier {
  /// Creates the notifier; `call` is replaced in tests.
  const CoreToastCallNotifier({LlmNativeCall call = callRustLlm})
      : _call = call;

  final LlmNativeCall _call;

  /// Side effects: Shows the toast; the core's call waits on a background isolate until it is
  /// answered, withdrawn or two minutes pass. A click on the toast itself, or a core that cannot
  /// show one, answers `unavailable`, which leaves the banner up.
  @override
  Future<CallNotificationAnswer> offer(String app) async {
    try {
      final Map<String, Object?> answer =
          decodeLlmAnswer(await _call('note_taker_call_toast', <String?>[
        jsonEncode(<String, Object?>{'command': 'offer', 'app': app})
      ]));
      return switch (answer['answer']) {
        'record' => CallNotificationAnswer.record,
        'dismissed' => CallNotificationAnswer.dismissed,
        _ => CallNotificationAnswer.unavailable,
      };
    } on Object {
      return CallNotificationAnswer.unavailable;
    }
  }

  /// Side effects: Hides the toast; its waiting offer answers `dismissed`.
  @override
  void withdraw() {
    unawaited(_call('note_taker_call_toast', <String?>[
      jsonEncode(<String, Object?>{'command': 'withdraw'})
    ]).then((String _) {}, onError: (Object _) {}));
  }
}

/// The notifier for this platform.
CallNotifier platformCallNotifier() => Platform.isLinux
    ? NotifySendCallNotifier()
    : Platform.isWindows
        ? const CoreToastCallNotifier()
        : const NoCallNotifier();
