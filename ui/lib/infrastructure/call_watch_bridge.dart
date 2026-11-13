// Calls the core's call watch (#111): one look at which apps use the microphone, and the two
// Settings (offer to record a call; stop when it ends). Requests go through
// `note_taker_call_watch` on a background isolate, since a look runs a system tool.

import 'dart:convert';

import 'package:note_taker_ui/infrastructure/llm_bridge.dart';

/// One look at the microphone.
final class CallPoll {
  /// Creates a poll answer.
  const CallPoll(
      {required this.available, required this.inCall, this.app, this.event});

  /// Reads the core's JSON.
  factory CallPoll.fromJson(Map<String, Object?> json) => CallPoll(
        available: json['available'] == true,
        inCall: json['in_call'] == true,
        app: json['app'] as String?,
        event: json['event'] as String?,
      );

  /// Whether this system can tell which apps use the microphone.
  final bool available;

  /// Whether a call is in progress.
  final bool inCall;

  /// The call app's name, such as `Teams`.
  final String? app;

  /// `started` or `ended` on the look that noticed it, else null.
  final String? event;
}

/// The owner's choices.
final class CallWatchSettings {
  /// Creates the settings.
  const CallWatchSettings({required this.offer, required this.autoStop});

  /// Reads the core's JSON.
  factory CallWatchSettings.fromJson(Map<String, Object?> json) =>
      CallWatchSettings(
        offer: json['offer'] != false,
        autoStop: json['auto_stop'] != false,
      );

  /// Offer to record when a call starts.
  final bool offer;

  /// Stop a recording started from the offer when the call ends.
  final bool autoStop;
}

/// Talks to the core's call watch.
final class CallWatchBridge {
  /// Creates the bridge; `call` is replaced in tests.
  const CallWatchBridge({LlmNativeCall call = callRustLlm}) : _call = call;

  final LlmNativeCall _call;

  /// Takes one look. Throws `LlmException`. Side effects: Runs a system tool in the core.
  Future<CallPoll> poll() async =>
      CallPoll.fromJson(await _command(<String, Object?>{'command': 'poll'}));

  /// Reads the settings. Throws `LlmException`. Side effects: None.
  Future<CallWatchSettings> settings() async => CallWatchSettings.fromJson(
      await _command(<String, Object?>{'command': 'get'}));

  /// Saves one or both settings. Throws `LlmException`. Side effects: Writes the settings file.
  Future<CallWatchSettings> set({bool? offer, bool? autoStop}) async =>
      CallWatchSettings.fromJson(await _command(<String, Object?>{
        'command': 'set',
        if (offer != null) 'offer': offer,
        if (autoStop != null) 'auto_stop': autoStop,
      }));

  Future<Map<String, Object?>> _command(Map<String, Object?> request) async =>
      decodeLlmAnswer(
          await _call('note_taker_call_watch', <String?>[jsonEncode(request)]));
}
