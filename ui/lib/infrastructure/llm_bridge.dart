// Reaches the Rust core's AI connections and meeting summaries (#95). Every call answers
// `{"ok":true,...}` or `{"ok":false,"error_code","error"}`; calls run on background isolates
// because they read files, and a summary waits on the model for minutes. The app keeps no API
// keys (#117): connections are servers the owner runs, or Claude Code and Codex signed in here.
// A phone can also ask a paired computer to write a summary with that computer's connection
// (#107); the request travels in the meeting's synced document.

import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:ffi/ffi.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// The protocols a connection speaks, as the core names them.
enum LlmKind {
  /// OpenAI's Chat Completions, spoken by Ollama, llama.cpp, LM Studio and vLLM servers.
  openAiCompatible('open_ai_compatible'),

  /// Claude Code installed and signed in on this computer, so a Claude plan answers (#105).
  claudeCode('claude_code'),

  /// Codex installed and signed in on this computer, so a ChatGPT plan answers (#105).
  codex('codex');

  const LlmKind(this.wire);

  /// The protocol's name in the connection form, in the app's language (#124).
  String get label => switch (this) {
        LlmKind.openAiCompatible => l10n.openaiCompatibleServer,
        LlmKind.claudeCode => 'Claude Code',
        LlmKind.codex => 'Codex',
      };

  /// Whether it runs a tool on this computer: no address, desktop only.
  bool get isLocalCli => this == claudeCode || this == codex;

  /// The name in the core's JSON.
  final String wire;

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
    this.hasKey = false,
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
        hasKey: json['has_key'] as bool? ?? false,
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

  /// Whether an API key is saved for it (#138); the key itself never reaches the app's UI.
  final bool hasKey;
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
    this.apiKey,
    this.clearKey = false,
  });

  final String label;
  final LlmKind kind;
  final String baseUrl;
  final String model;
  final int contextTokens;

  /// A new API key to save (#138); null or blank keeps the saved one.
  final String? apiKey;

  /// Whether to delete the saved key.
  final bool clearKey;
}

/// A ready-made starting point for a connection.
final class LlmPreset {
  /// Creates a preset; `description` says in a line what it is and what it costs (#138).
  const LlmPreset(
      this.name, this.kind, this.baseUrl, this.model, this.contextTokens,
      [this.description = '', this.needsKey = false]);

  final String name;
  final LlmKind kind;
  final String baseUrl;
  final String model;
  final int contextTokens;

  /// One line on the service: who runs it, its cost, where to get a key.
  final String description;

  /// Whether the service needs an API key.
  final bool needsKey;
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
List<LlmPreset> get llmPresets => <LlmPreset>[
      LlmPreset(
          l10n.localServerOllama,
          LlmKind.openAiCompatible,
          'http://192.168.1.10:11434/v1',
          'llama3.1:8b',
          8192,
          l10n.presetOllama),
      LlmPreset(
          'LM Studio',
          LlmKind.openAiCompatible,
          'http://192.168.1.10:1234/v1',
          'qwen3-8b',
          32768,
          l10n.presetLmStudio),
      LlmPreset(l10n.claudeCodeClaudePlan, LlmKind.claudeCode, '', '', 200000),
      LlmPreset(l10n.codexChatgptPlan, LlmKind.codex, '', '', 200000),
      // Online services with an API key (#138). Model names change often: Test checks them.
      LlmPreset(
          'Gemini',
          LlmKind.openAiCompatible,
          'https://generativelanguage.googleapis.com/v1beta/openai',
          'gemini-2.5-flash',
          1000000,
          l10n.presetGemini,
          true),
      LlmPreset(
          'DeepSeek',
          LlmKind.openAiCompatible,
          'https://api.deepseek.com',
          'deepseek-chat',
          128000,
          l10n.presetDeepseek,
          true),
      LlmPreset(
          'Qwen',
          LlmKind.openAiCompatible,
          'https://dashscope-intl.aliyuncs.com/compatible-mode/v1',
          'qwen-plus',
          131072,
          l10n.presetQwen,
          true),
      LlmPreset('Kimi', LlmKind.openAiCompatible, 'https://api.moonshot.ai/v1',
          'kimi-latest', 131072, l10n.presetKimi, true),
      LlmPreset(
          'Mistral',
          LlmKind.openAiCompatible,
          'https://api.mistral.ai/v1',
          'mistral-medium-latest',
          128000,
          l10n.presetMistral,
          true),
      LlmPreset(
          'OpenRouter',
          LlmKind.openAiCompatible,
          'https://openrouter.ai/api/v1',
          'google/gemini-2.5-flash',
          1000000,
          l10n.presetOpenrouter,
          true),
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
    this.pairedSummaryConnection,
    this.chatPrompt = '',
    this.defaultChatPrompt = '',
    this.customChatPrompt = false,
    this.chatPlaceholders = const <String>['scope'],
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
        pairedSummaryConnection: json['paired_summary_connection'] as String?,
        placeholders:
            (json['placeholders'] as List<Object?>? ?? const <Object?>[])
                .whereType<String>()
                .toList(growable: false),
        chatPrompt: json['chat_prompt'] as String? ?? '',
        defaultChatPrompt: json['default_chat_prompt'] as String? ?? '',
        customChatPrompt: json['custom_chat_prompt'] as bool? ?? false,
        chatPlaceholders: (json['chat_placeholders'] as List<Object?>? ??
                const <Object?>['scope'])
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

  /// The connection that writes the summaries paired devices ask for (#107), or null when this
  /// device does not answer them.
  final String? pairedSummaryConnection;

  /// The chat prompt in use (#145): the owner's, or the built-in one. Shared by paired devices.
  final String chatPrompt;

  /// The built-in chat prompt.
  final String defaultChatPrompt;

  /// Whether the chat prompt was changed from the built-in one.
  final bool customChatPrompt;

  /// The `{{placeholder}}` names the chat prompt may use.
  final List<String> chatPlaceholders;
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

/// How far a paired computer has got with a summary request (#107).
enum PairedRequestState {
  /// No computer has taken it yet: none is on, paired and set to answer.
  waiting,

  /// A computer is writing the summary.
  working,

  /// A computer tried and failed; asking again replaces the request.
  failed;

  /// Reads the core's name; unknown names count as waiting.
  static PairedRequestState fromWire(Object? wire) => switch (wire) {
        'working' => working,
        'failed' => failed,
        _ => waiting,
      };
}

/// This device's request that a paired computer write a meeting's summary (#107).
final class PairedSummaryRequest {
  /// Creates the request.
  const PairedSummaryRequest(
      {required this.state, required this.requestedAt, this.errorCode});

  /// Reads the request from the core's JSON.
  factory PairedSummaryRequest.fromJson(Map<String, Object?> json) =>
      PairedSummaryRequest(
        state: PairedRequestState.fromWire(json['state']),
        requestedAt:
            DateTime.tryParse(json['requested_at'] as String? ?? '')?.toUtc() ??
                DateTime.fromMillisecondsSinceEpoch(0, isUtc: true),
        errorCode: json['error_code'] as String?,
      );

  final PairedRequestState state;
  final DateTime requestedAt;

  /// The computer's error code when it failed, such as `llm_tool_failed`.
  final String? errorCode;
}

/// A meeting's summary, its own prompt and a paired computer's request, each null when absent.
final class MeetingSummaryState {
  /// Creates the state.
  const MeetingSummaryState({this.summary, this.meetingPrompt, this.request});

  final StoredSummary? summary;

  /// The meeting's own template, or null when it uses the Settings one.
  final String? meetingPrompt;

  /// The request that a paired computer write the summary (#107), or null.
  final PairedSummaryRequest? request;
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
          l10n.theAiServerRejectedTheRequest(detail),
        ('llm_tool_failed', final String detail) =>
          l10n.claudeCodeOrCodexCouldNot(detail),
        _ => _sentence,
      };

  String get _sentence => switch (code) {
        'invalid_connection' => l10n.checkTheAddressAndModelPlain,
        'invalid_template' => l10n.promptUnknownPlaceholder('{{placeholder}}'),
        'connection_not_found' => l10n.thatConnectionNoLongerExists,
        'meeting_not_found' => l10n.thatMeetingNoLongerExists,
        'no_connection' => l10n.addAnAiConnectionInSettings2,
        'nothing_to_summarize' => l10n.thisMeetingHasNoTranscriptOr,
        'too_long' => l10n.thisMeetingIsTooLongFor,
        'llm_unreachable' => l10n.theAiServerCouldNotBe2,
        'llm_unauthorized' => l10n.theAiServerRefusedAccessUse,
        'llm_rate_limited' => l10n.theAiServerIsBusyOr,
        'llm_rejected' => l10n.theAiServerRejectedTheRequest2,
        'llm_failed' => l10n.theAiServerFailedTryAgain,
        'llm_refused' => l10n.theModelDeclinedToAnswer,
        'llm_invalid_response' => l10n.theAiServerSAnswerCould,
        'llm_tool_missing' => l10n.claudeCodeOrCodexIsNot,
        'llm_tool_failed' => l10n.claudeCodeOrCodexCouldNot2,
        'save_failed' => l10n.theSettingsCouldNotBeSaved,
        'storage_failed' => l10n.theMeetingCouldNotBeRead,
        'runtime_unavailable' => l10n.theAppIsStillStartingTry,
        _ => l10n.somethingWentWrongTryAgain,
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
      if (input.apiKey case final String key when key.trim().isNotEmpty)
        'api_key': key.trim(),
      if (input.clearKey) 'clear_key': true,
    });
    return LlmConnection.fromJson(
        answer['connection']! as Map<String, Object?>);
  }

  /// Removes a connection. Side effects: Writes the settings file.
  Future<void> deleteConnection(String id) =>
      _command(<String, Object?>{'command': 'delete_connection', 'id': id});

  /// Changes only the fields given; `resetTemplate` and `resetChatPrompt` restore the built-in
  /// prompts (both are shared with paired devices, #145), and
  /// `stopAnsweringPairedDevices` stops this device writing summaries for paired ones (#107).
  ///
  /// Outputs: The settings as saved. Side effects: Writes the settings file.
  Future<LlmSettings> update({
    String? defaultConnection,
    String? template,
    bool resetTemplate = false,
    String? chatPrompt,
    bool resetChatPrompt = false,
    bool? autoSummarize,
    String? pairedSummaryConnection,
    bool stopAnsweringPairedDevices = false,
  }) async =>
      LlmSettings.fromJson(await _command(<String, Object?>{
        'command': 'update',
        if (defaultConnection != null) 'default_connection': defaultConnection,
        if (resetTemplate)
          'template': null
        else if (template != null)
          'template': template,
        if (resetChatPrompt)
          'chat_prompt': null
        else if (chatPrompt != null)
          'chat_prompt': chatPrompt,
        if (autoSummarize != null) 'auto_summarize': autoSummarize,
        if (stopAnsweringPairedDevices)
          'paired_summary_connection': null
        else if (pairedSummaryConnection != null)
          'paired_summary_connection': pairedSummaryConnection,
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

  /// Reads a meeting's summary, its own prompt and any request to a paired computer.
  /// Side effects: Reads the meeting document.
  Future<MeetingSummaryState> meetingSummary(String meetingId) async {
    final Map<String, Object?> answer = decodeLlmAnswer(
        await _call('note_taker_get_meeting_summary', <String?>[meetingId]));
    return MeetingSummaryState(
      summary: switch (answer['summary']) {
        final Map<String, Object?> summary => StoredSummary.fromJson(summary),
        _ => null,
      },
      meetingPrompt: answer['meeting_prompt'] as String?,
      request: switch (answer['request']) {
        final Map<String, Object?> request =>
          PairedSummaryRequest.fromJson(request),
        _ => null,
      },
    );
  }

  /// Asks a paired computer to write the meeting's summary (#107), or cancels the request.
  ///
  /// Outputs: The new request, or null once cancelled. Side effects: Writes the request into the
  /// meeting document, which syncs to paired devices; nothing is sent to an AI server from here.
  Future<PairedSummaryRequest?> requestPairedSummary(String meetingId,
      {bool cancel = false}) async {
    final Map<String, Object?> answer = decodeLlmAnswer(
        await _call('note_taker_request_paired_summary', <String?>[
      jsonEncode(<String, Object?>{
        'meeting_id': meetingId,
        if (cancel) 'cancel': true,
      })
    ]));
    return switch (answer['request']) {
      final Map<String, Object?> request =>
        PairedSummaryRequest.fromJson(request),
      _ => null,
    };
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
