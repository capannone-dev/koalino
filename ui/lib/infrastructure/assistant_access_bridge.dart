// Calls the core's assistant access setting (#112): whether the owner's AI assistant (Claude
// Code, Codex, Claude Desktop...) may read the notes through the read-only `note_taker_mcp`
// server, and how to connect it. One JSON command goes through `note_taker_assistant_access` on
// a background isolate.

import 'dart:convert';

import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// The setting and how to connect an assistant.
final class AssistantAccessStatus {
  /// Creates the status.
  const AssistantAccessStatus({
    required this.enabled,
    this.serverPath,
    this.config,
    this.claudeCode,
    this.passphraseNeeded = false,
  });

  /// Reads the core's JSON.
  factory AssistantAccessStatus.fromJson(Map<String, Object?> json) =>
      AssistantAccessStatus(
        enabled: json['enabled'] == true,
        serverPath: json['server_path'] as String?,
        config: switch (json['config']) {
          final Map<String, Object?> config =>
            const JsonEncoder.withIndent('  ').convert(config),
          _ => null,
        },
        claudeCode: json['claude_code'] as String?,
        passphraseNeeded: json['passphrase_needed'] == true,
      );

  /// Whether the assistant may read the notes.
  final bool enabled;

  /// The server binary, or null where it is not installed.
  final String? serverPath;

  /// The `mcpServers` entry for Claude Desktop and similar clients, as indented JSON.
  final String? config;

  /// The `claude mcp add` command line.
  final String? claudeCode;

  /// Whether the notes are unlocked with a passphrase, which the server would need as well.
  final bool passphraseNeeded;
}

/// A refused or failed command, with the core's stable code.
final class AssistantAccessException implements Exception {
  /// Creates the exception.
  const AssistantAccessException(this.code);

  final String code;

  /// A sentence for people.
  String get message => switch (code) {
        'storage_failed' => l10n.theSettingCouldNotBeSaved,
        'runtime_unavailable' => l10n.theAppIsStillStartingTry,
        _ => l10n.somethingWentWrongTryAgain,
      };

  @override
  String toString() => 'AssistantAccessException($code)';
}

/// Reads and changes assistant access through the core.
final class AssistantAccessBridge {
  /// Creates the bridge; `call` is replaced in tests.
  const AssistantAccessBridge({LlmNativeCall call = callRustLlm})
      : _call = call;

  final LlmNativeCall _call;

  /// Reads the setting. Throws `AssistantAccessException`. Side effects: None.
  Future<AssistantAccessStatus> status() =>
      _command(<String, Object?>{'command': 'get'});

  /// Turns access on or off. Throws `AssistantAccessException`. Side effects: Saves the
  /// setting; a running server reads it at its next call.
  Future<AssistantAccessStatus> set({required bool enabled}) =>
      _command(<String, Object?>{'command': 'set', 'enabled': enabled});

  Future<AssistantAccessStatus> _command(Map<String, Object?> request) async {
    try {
      return AssistantAccessStatus.fromJson(decodeLlmAnswer(await _call(
          'note_taker_assistant_access', <String?>[jsonEncode(request)])));
    } on LlmException catch (error) {
      throw AssistantAccessException(error.code);
    }
  }
}
