// A fake Rust core for the AI connections and summaries bridge (#95): queued JSON answers per
// function or command, and a record of every call.

import 'dart:async';
import 'dart:convert';

/// Answers core calls from a queue and records them.
final class FakeLlmCore {
  final List<(String, List<String?>)> calls = <(String, List<String?>)>[];
  final Map<String, List<Object?>> answers = <String, List<Object?>>{};

  /// Answers that arrive only when the test completes them, keyed like `answers`; used once.
  final Map<String, Completer<String>> pending = <String, Completer<String>>{};

  /// Queues an answer (a JSON-able map) for `key`: the function, or `command:<name>`.
  void answer(String key, Object? json) =>
      answers.putIfAbsent(key, () => <Object?>[]).add(json);

  Future<String> call(String function, List<String?> arguments) async {
    calls.add((function, arguments));
    String key = function;
    if (function == 'note_taker_llm_command') {
      key =
          'command:${(jsonDecode(arguments.single!) as Map<String, Object?>)['command']}';
    }
    if (pending.remove(key) case final Completer<String> later) {
      return later.future;
    }
    final List<Object?> queue = answers[key] ?? <Object?>[];
    if (queue.isEmpty) {
      return '{"ok":false,"error_code":"runtime_unavailable"}';
    }
    return jsonEncode(queue.removeAt(0));
  }

  /// The request JSON of the last command named `name`.
  Map<String, Object?> lastCommand(String name) => calls
      .where((call) => call.$1 == 'note_taker_llm_command')
      .map((call) => jsonDecode(call.$2.single!) as Map<String, Object?>)
      .lastWhere((request) => request['command'] == name);
}

/// A connection as the core describes it.
Map<String, Object?> connection(String id, String label,
        {bool isDefault = false}) =>
    <String, Object?>{
      'id': id,
      'label': label,
      'kind': 'open_ai_compatible',
      'base_url': 'http://192.168.1.5:11434/v1',
      'model': 'llama3.1:8b',
      'context_tokens': 8192,
      'plain_http': true,
      'has_key': false,
      'is_default': isDefault,
    };

/// The settings as the core describes them.
Map<String, Object?> settings({List<Object?>? connections, bool auto = true}) =>
    <String, Object?>{
      'ok': true,
      'connections': connections ??
          <Object?>[connection('c-1', 'Office', isDefault: true)],
      'default_connection': 'c-1',
      'template': 'Summarize {{title}}.',
      'default_template': 'Built in',
      'custom_template': true,
      'auto_summarize': auto,
      'placeholders': <String>['title', 'date'],
    };

/// A stored summary as the core describes it.
const Map<String, Object?> summary = <String, Object?>{
  'text': '## Overview\nBudget approved.',
  'connection_label': 'Office',
  'model': 'llama3.1:8b',
  'created_at': '2026-11-09T10:00:00Z',
  'prompt': 'p',
  'edited': false,
  'truncated': false,
};

/// The summary as JSON text.
String summaryJson() => jsonEncode(summary);
