// Reaches the Rust core's AI connections and meeting summaries (#95). Every call answers
// `{"ok":true,...}` or `{"ok":false,"error_code","error"}`; calls run on background isolates
// because they read files, and a summary waits on the model for minutes. The app keeps no API
// keys (#117): connections are servers the owner runs, or Claude Code and Codex signed in here.

import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:ffi/ffi.dart';

/// The protocols a connection speaks, as the core names them.
enum LlmKind {
  /// OpenAI's Chat Completions, spoken by Ollama, llama.cpp, LM Studio and vLLM servers.
  openAiCompatible('open_ai_compatible', 'OpenAI-compatible server'),

  /// Claude Code installed and signed in on this computer, so a Claude plan answers (#105).
  claudeCode('claude_code', 'Claude Code'),

  /// Codex installed and signed in on this computer, so a ChatGPT plan answers (#105).
  codex('codex', 'Codex');

  const LlmKind(this.wire, this.label);

  /// Whether it runs a tool on this computer: no address, desktop only.
  bool get isLocalCli => this == claudeCode || this == codex;

  /// The name in the core's JSON.
  final String wire;

  /// The name shown.
  final String label;

  /// Reads the core's name; unknown names are OpenAI-compatible.
  static LlmKind fromWire(Object? wire) => LlmKind.values.firstWhere(
        (LlmKind kind) => kind.wire == wire,
        orElse: () => LlmKind.openAiCompatible,
      );
}

/// A saved connection.
final class LlmConnection {
  /// Creates an immutable connection.
  const LlmConnection({
    required this.id,
    required this.label,
    required this.kind,
    required this.baseUrl,
    required this.model,
    required this.contextTokens,
    required this.plainHttp,
    required this.isDefault,
    this.toolInstalled = false,
  });

  /// Reads one connection from the core's JSON.
  ///
  /// Throws `TypeError` for a malformed object.
  factory LlmConnection.fromJson(Map<String, Object?> json) => LlmConnection(
        id: json['id']! as String,
        label: json['label']! as String,
        kind: LlmKind.fromWire(json['kind']),
        baseUrl: json['base_url']! as String,
        model: json['model']! as String,
        contextTokens: (json['context_tokens'] as num?)?.toInt() ?? 0,
        plainHttp: json['plain_http'] as bool? ?? false,
        isDefault: json['is_default'] as bool? ?? false,
        toolInstalled: json['tool_installed'] as bool? ?? false,
      );

  /// Stable identifier chosen by the core.
  final String id;

  /// The name shown.
  final String label;

  /// The protocol.
  final LlmKind kind;

  /// The server address, such as `http://192.168.1.10:11434/v1`.
  final String baseUrl;

  /// The model name the server knows.
  final String model;

  /// How many tokens the model reads at once; long meetings are condensed in parts to fit.
  final int contextTokens;

  /// Whether it uses unencrypted `http://` (allowed only on this device or the local network).
  final bool plainHttp;

  /// Whether summaries use it unless another is chosen.
  final bool isDefault;

  /// For Claude Code and Codex: whether the tool was found on this computer.
  final bool toolInstalled;
}

/// What saving a connection sends.
final class LlmConnectionInput {
  /// Creates the input.
  const LlmConnectionInput({
    required this.label,
    required this.kind,
    required this.baseUrl,
    required this.model,
    required this.contextTokens,
  });

  final String label;
  final LlmKind kind;
  final String baseUrl;
  final String model;
  final int contextTokens;
}

/// A ready-made starting point for a connection.
final class LlmPreset {
  /// Creates a preset.
  const LlmPreset(
      this.name, this.kind, this.baseUrl, this.model, this.contextTokens);

  final String name;
  final LlmKind kind;
  final String baseUrl;
  final String model;
  final int contextTokens;
}

/// Whether this device can run Claude Code or Codex: desktops only.
bool get canRunLocalCli =>
    Platform.isLinux || Platform.isMacOS || Platform.isWindows;

/// The presets this device offers; Claude Code and Codex only on desktops.
List<LlmPreset> get availableLlmPresets => <LlmPreset>[
      for (final LlmPreset preset in llmPresets)
        if (!preset.kind.isLocalCli || canRunLocalCli) preset,
    ];

/// The presets offered when adding a connection, matching the web's.
const List<LlmPreset> llmPresets = <LlmPreset>[
  LlmPreset('Local server (Ollama)', LlmKind.openAiCompatible,
      'http://192.168.1.10:11434/v1', 'llama3.1:8b', 8192),
  LlmPreset('LM Studio', LlmKind.openAiCompatible,
      'http://192.168.1.10:1234/v1', 'qwen3-8b', 32768),
  LlmPreset('Claude Code (Claude plan)', LlmKind.claudeCode, '', '', 200000),
  LlmPreset('Codex (ChatGPT plan)', LlmKind.codex, '', '', 200000),
];

/// The AI settings.
final class LlmSettings {
  /// Creates the settings.
  const LlmSettings({
    required this.connections,
    required this.defaultConnection,
    required this.template,
    required this.defaultTemplate,
    required this.customTemplate,
    required this.autoSummarize,
    required this.placeholders,
  });

  /// Reads the settings from the core's JSON.
  ///
  /// Throws `TypeError` for a malformed object.
  factory LlmSettings.fromJson(Map<String, Object?> json) => LlmSettings(
        connections:
            (json['connections'] as List<Object?>? ?? const <Object?>[])
                .map((Object? row) =>
                    LlmConnection.fromJson(row! as Map<String, Object?>))
                .toList(growable: false),
        defaultConnection: json['default_connection'] as String?,
        template: json['template'] as String? ?? '',
        defaultTemplate: json['default_template'] as String? ?? '',
        customTemplate: json['custom_template'] as bool? ?? false,
        autoSummarize: json['auto_summarize'] as bool? ?? true,
        placeholders:
            (json['placeholders'] as List<Object?>? ?? const <Object?>[])
                .whereType<String>()
                .toList(growable: false),
      );

  final List<LlmConnection> connections;

  /// The default connection's ID, or null without connections.
  final String? defaultConnection;

  /// The summary prompt template in use.
  final String template;

  /// The built-in template.
  final String defaultTemplate;

  /// Whether the template was changed from the built-in one.
  final bool customTemplate;

  /// Whether finished meetings are summarized with the default connection (#101).
  final bool autoSummarize;

  /// The `{{placeholder}}` names a template may use.
  final List<String> placeholders;
}

/// A meeting's stored summary.
final class StoredSummary {
  /// Creates the summary.
  const StoredSummary({
    required this.text,
    required this.connectionLabel,
    required this.model,
    required this.createdAt,
    required this.edited,
    required this.truncated,
  });

  /// Reads the summary from the core's JSON.
  factory StoredSummary.fromJson(Map<String, Object?> json) => StoredSummary(
        text: json['text']! as String,
        connectionLabel: json['connection_label'] as String? ?? '',
        model: json['model'] as String? ?? '',
        createdAt:
            DateTime.tryParse(json['created_at'] as String? ?? '')?.toUtc() ??
                DateTime.fromMillisecondsSinceEpoch(0, isUtc: true),
        edited: json['edited'] as bool? ?? false,
        truncated: json['truncated'] as bool? ?? false,
      );

  /// The summary, in Markdown.
  final String text;
  final String connectionLabel;
  final String model;
  final DateTime createdAt;

  /// Whether the owner edited it after the model wrote it.
  final bool edited;

  /// Whether the model stopped before the end.
  final bool truncated;
}

/// A meeting's summary and its own prompt, each null when absent.
final class MeetingSummaryState {
  /// Creates the state.
  const MeetingSummaryState({this.summary, this.meetingPrompt});

  final StoredSummary? summary;

  /// The meeting's own template, or null when it uses the Settings one.
  final String? meetingPrompt;
}

/// What writing a summary reports.
final class SummaryResult {
  /// Creates the result.
  const SummaryResult({
    required this.summary,
    required this.parts,
    required this.imagesSent,
    required this.imagesSkipped,
  });

  final StoredSummary summary;

  /// How many parts the transcript was read in (1 when it fit at once).
  final int parts;

  /// Pictures the model saw.
  final int imagesSent;

  /// Pictures left out (too large, an unsupported format, or refused by the model).
  final int imagesSkipped;
}

/// A refused or failed AI call, with the core's stable error code.
final class LlmException implements Exception {
  /// Creates the exception.
  const LlmException(this.code, {this.detail});

  /// Such as `too_long`, `llm_unreachable` or `runtime_unavailable`.
  final String code;

  /// The AI server's own reason for a rejection (`error_detail`), such as a low credit
  /// balance. Shown to the owner, never logged.
  final String? detail;

  /// A sentence for people; the core's own text is not shown, since it can hold details. A
  /// rejection with the server's reason shows that reason instead of a guess.
  String get message => switch ((code, detail)) {
        ('llm_rejected', final String detail) =>
          'The AI server rejected the request.\nThe server said: $detail',
        ('llm_tool_failed', final String detail) =>
          'Claude Code or Codex could not answer.\nIt said: $detail',
        _ => _sentence,
      };

  String get _sentence => switch (code) {
        'invalid_connection' =>
          'Check the address and model. Plain http:// works only on this device or the local network.',
        'invalid_template' =>
          'The prompt has an unknown or unclosed {{placeholder}}.',
        'connection_not_found' => 'That connection no longer exists.',
        'meeting_not_found' => 'That meeting no longer exists.',
        'no_connection' => 'Add an AI connection in Settings first.',
        'nothing_to_summarize' =>
          'This meeting has no transcript or notes yet.',
        'too_long' =>
          'This meeting is too long for this connection. Choose one with a larger context.',
        'llm_unreachable' => 'The AI server could not be reached.',
        'llm_unauthorized' =>
          'The AI server refused access. Use a server that needs no API key.',
        'llm_rate_limited' =>
          'The AI server is busy or over its limit. Try again later.',
        'llm_rejected' =>
          'The AI server rejected the request. Check the model name.',
        'llm_failed' => 'The AI server failed. Try again.',
        'llm_refused' => 'The model declined to answer.',
        'llm_invalid_response' => "The AI server's answer could not be read.",
        'llm_tool_missing' =>
          'Claude Code or Codex is not installed on this computer, or the app cannot find it.',
        'llm_tool_failed' =>
          'Claude Code or Codex could not answer. Check that it is signed in.',
        'save_failed' => 'The settings could not be saved.',
        'storage_failed' => 'The meeting could not be read or saved.',
        'runtime_unavailable' => 'The app is still starting. Try again.',
        _ => 'Something went wrong. Try again.',
      };

  @override
  String toString() => 'LlmException($code)';
}

/// Calls one core function by name with string (or null) arguments and returns its JSON answer.
typedef LlmNativeCall = Future<String> Function(
    String functionName, List<String?> arguments);

/// Decodes an answer, throwing `LlmException` for `{"ok":false}` or malformed JSON.
Map<String, Object?> decodeLlmAnswer(String answer) {
  final Object? decoded;
  try {
    decoded = jsonDecode(answer);
  } on FormatException {
    throw const LlmException('invalid_answer');
  }
  if (decoded is! Map<String, Object?>) {
    throw const LlmException('invalid_answer');
  }
  if (decoded['ok'] != true) {
    final Object? detail = decoded['error_detail'];
    throw LlmException(decoded['error_code'] as String? ?? 'unknown',
        detail: detail is String && detail.isNotEmpty ? detail : null);
  }
  return decoded;
}

/// The core's AI settings and meeting summaries.
final class LlmBridge {
  /// Creates the bridge over `call`, the core's functions by default.
  const LlmBridge({LlmNativeCall call = callRustLlm}) : _call = call;

  final LlmNativeCall _call;

  /// Runs one `note_taker_llm_command`.
  Future<Map<String, Object?>> _command(Map<String, Object?> request) async =>
      decodeLlmAnswer(await _call(
          'note_taker_llm_command', <String?>[jsonEncode(request)]));

  /// Reads the settings. Side effects: Reads the settings file and checks the key store.
  Future<LlmSettings> settings() async => LlmSettings.fromJson(
      await _command(<String, Object?>{'command': 'settings'}));

  /// Creates a connection, or edits the one with `id`.
  ///
  /// Outputs: The connection as saved. Throws `LlmException` (`invalid_connection`, ...).
  /// Side effects: Writes the settings file.
  Future<LlmConnection> saveConnection(LlmConnectionInput input,
      {String? id}) async {
    final Map<String, Object?> answer = await _command(<String, Object?>{
      'command': 'save_connection',
      if (id != null) 'id': id,
      'label': input.label,
      'kind': input.kind.wire,
      'base_url': input.baseUrl,
      'model': input.model,
      'context_tokens': input.contextTokens,
    });
    return LlmConnection.fromJson(
        answer['connection']! as Map<String, Object?>);
  }

  /// Removes a connection. Side effects: Writes the settings file.
  Future<void> deleteConnection(String id) =>
      _command(<String, Object?>{'command': 'delete_connection', 'id': id});

  /// Changes only the fields given; `resetTemplate` restores the built-in prompt.
  ///
  /// Outputs: The settings as saved. Side effects: Writes the settings file.
  Future<LlmSettings> update({
    String? defaultConnection,
    String? template,
    bool resetTemplate = false,
    bool? autoSummarize,
  }) async =>
      LlmSettings.fromJson(await _command(<String, Object?>{
        'command': 'update',
        if (defaultConnection != null) 'default_connection': defaultConnection,
        if (resetTemplate)
          'template': null
        else if (template != null)
          'template': template,
        if (autoSummarize != null) 'auto_summarize': autoSummarize,
      }));

  /// Sends a fixed one-line request, holding no meeting content, and returns the reply.
  ///
  /// Side effects: One request to the connection's server.
  Future<String> testConnection(String id) async =>
      (await _command(<String, Object?>{
        'command': 'test_connection',
        'id': id
      }))['reply'] as String? ??
      '';

  /// Reads a meeting's summary and its own prompt. Side effects: Reads the meeting document.
  Future<MeetingSummaryState> meetingSummary(String meetingId) async {
    final Map<String, Object?> answer = decodeLlmAnswer(
        await _call('note_taker_get_meeting_summary', <String?>[meetingId]));
    return MeetingSummaryState(
      summary: switch (answer['summary']) {
        final Map<String, Object?> summary => StoredSummary.fromJson(summary),
        _ => null,
      },
      meetingPrompt: answer['meeting_prompt'] as String?,
    );
  }

  /// Writes a meeting's summary with `connectionId`, or the default connection when null.
  ///
  /// Side effects: Sends the meeting's transcript, notes and photos to the connection's server
  /// and stores the summary, which syncs to paired devices. Takes as long as the model does.
  Future<SummaryResult> summarize(String meetingId,
      {String? connectionId}) async {
    final Map<String, Object?> answer = decodeLlmAnswer(await _call(
        'note_taker_summarize_meeting', <String?>[meetingId, connectionId]));
    return SummaryResult(
      summary:
          StoredSummary.fromJson(answer['summary']! as Map<String, Object?>),
      parts: (answer['parts'] as num?)?.toInt() ?? 1,
      imagesSent: (answer['images_sent'] as num?)?.toInt() ?? 0,
      imagesSkipped: (answer['images_skipped'] as num?)?.toInt() ?? 0,
    );
  }

  /// Sets the meeting's own prompt, or clears it when `prompt` is null.
  ///
  /// Throws `LlmException('invalid_template')`. Side effects: Writes the meeting document.
  Future<void> setMeetingPrompt(String meetingId, String? prompt) async {
    decodeLlmAnswer(await _call(
        'note_taker_set_meeting_summary_prompt', <String?>[meetingId, prompt]));
  }
}

/// Calls the core on a background isolate.
///
/// Inputs: A function taking one or two C strings and returning caller-freed JSON.
/// Outputs: The JSON, or a `runtime_unavailable` answer when the core returns null.
Future<String> callRustLlm(String functionName, List<String?> arguments) =>
    Isolate.run(() => _callNative(functionName, arguments));

/// Calls the function inside the isolate and frees every native string.
String _callNative(String functionName, List<String?> arguments) {
  final DynamicLibrary core = Platform.isWindows
      ? DynamicLibrary.open('note_taker_core.dll')
      : DynamicLibrary.open('libnote_taker_core.so');
  final List<Pointer<Utf8>> native = arguments
      .map((String? text) =>
          text == null ? nullptr.cast<Utf8>() : text.toNativeUtf8())
      .toList(growable: false);
  try {
    final Pointer<Utf8> answer = switch (native.length) {
      1 => core.lookupFunction<Pointer<Utf8> Function(Pointer<Utf8>),
          Pointer<Utf8> Function(Pointer<Utf8>)>(functionName)(native[0]),
      2 => core.lookupFunction<
          Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>),
          Pointer<Utf8> Function(Pointer<Utf8>,
              Pointer<Utf8>)>(functionName)(native[0], native[1]),
      _ => throw ArgumentError.value(arguments, 'arguments', 'one or two'),
    };
    if (answer == nullptr) {
      return '{"ok":false,"error_code":"runtime_unavailable"}';
    }
    try {
      return answer.toDartString();
    } finally {
      core.lookupFunction<Void Function(Pointer<Utf8>),
          void Function(Pointer<Utf8>)>('note_taker_free_string')(answer);
    }
  } finally {
    for (final Pointer<Utf8> pointer in native) {
      if (pointer != nullptr) {
        calloc.free(pointer);
      }
    }
  }
}
