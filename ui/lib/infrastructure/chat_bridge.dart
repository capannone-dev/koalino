// Calls the core's chat with notes (#97): one meeting, one project, or every meeting, summary
// and project. One JSON command goes through `note_taker_chat` on a background isolate, since
// asking waits for the model. Questions and answers are never logged.

import 'dart:convert';

import 'package:note_taker_ui/infrastructure/llm_bridge.dart';

/// What a chat is about.
final class ChatTopic {
  /// One meeting.
  const ChatTopic.meeting(String meetingId)
      : scope = 'meeting',
        id = meetingId;

  /// One project.
  const ChatTopic.project(String projectId)
      : scope = 'project',
        id = projectId;

  /// Every meeting, summary and project.
  const ChatTopic.everything()
      : scope = 'all',
        id = null;

  /// `meeting`, `project` or `all`.
  final String scope;
  final String? id;

  Map<String, Object?> get _json =>
      <String, Object?>{'scope': scope, if (id != null) 'id': id};
}

/// A place an answer cites (#110): a meeting, at a moment when the answer gave one, or a
/// project file.
final class ChatSource {
  /// Creates the source.
  const ChatSource({
    required this.number,
    required this.label,
    this.atMs,
    this.meetingId,
    this.projectId,
    this.resourceId,
  });

  /// Reads the core's JSON.
  factory ChatSource.fromJson(Map<String, Object?> json) => ChatSource(
        number: (json['number'] as num?)?.toInt() ?? 0,
        label: json['label'] as String? ?? '',
        atMs: (json['at_ms'] as num?)?.toInt(),
        meetingId: json['meeting_id'] as String?,
        projectId: json['project_id'] as String?,
        resourceId: json['resource_id'] as String?,
      );

  /// The number the answer cites it by, as in `[1]`.
  final int number;

  /// Such as `Meeting "Review"` or `File plan.md`.
  final String label;

  /// Where in the meeting, in milliseconds from its start; null for the whole meeting or a file.
  final int? atMs;

  /// The meeting, for a meeting source.
  final String? meetingId;

  /// The project and file, for a file source; a file outside any project has no project.
  final String? projectId;
  final String? resourceId;
}

/// One message of a chat.
final class ChatEntry {
  /// Creates the message.
  const ChatEntry({
    required this.fromOwner,
    required this.text,
    required this.createdAt,
    this.included = const <String>[],
    this.omitted = const <String>[],
    this.connection,
    this.sources = const <ChatSource>[],
  });

  /// Reads the core's JSON.
  factory ChatEntry.fromJson(Map<String, Object?> json) {
    List<String> strings(Object? value) => <String>[
          for (final Object? item in value as List<Object?>? ?? <Object?>[])
            if (item is String) item,
        ];
    return ChatEntry(
      fromOwner: json['role'] == 'user',
      text: json['text'] as String? ?? '',
      createdAt: DateTime.tryParse(json['created_at'] as String? ?? '') ??
          DateTime.now(),
      included: strings(json['included']),
      omitted: strings(json['omitted']),
      connection: json['connection'] as String?,
      sources: <ChatSource>[
        for (final Object? source
            in json['sources'] as List<Object?>? ?? <Object?>[])
          if (source case final Map<String, Object?> source)
            ChatSource.fromJson(source),
      ],
    );
  }

  /// Whether the owner wrote it (otherwise it is the model's answer).
  final bool fromOwner;
  final String text;
  final DateTime createdAt;

  /// For an answer: what the model was given, and what was left out for length.
  final List<String> included;
  final List<String> omitted;

  /// For an answer: the connection's label.
  final String? connection;

  /// For an answer: the places it cites, in order of first mention (#110).
  final List<ChatSource> sources;
}

/// A refused or failed chat command, with the core's stable code.
final class ChatException implements Exception {
  /// Creates the exception.
  const ChatException(this.code, {this.detail});

  final String code;

  /// The AI server's own reason for a rejection (`error_detail`). Shown, never logged.
  final String? detail;

  /// A sentence for people; the core's own text is not shown. A rejection with the server's
  /// reason shows that reason.
  String get message => switch ((code, detail)) {
        ('llm_rejected', final String detail) =>
          'The AI server rejected the request.\nThe server said: $detail',
        ('llm_tool_failed', final String detail) =>
          'Claude Code or Codex could not answer.\nIt said: $detail',
        _ => _sentence,
      };

  String get _sentence => switch (code) {
        'empty' => 'Type a question.',
        'too_long' => 'The question is too long.',
        'no_connection' =>
          'Add an AI connection in Settings, or choose a default one.',
        'connection_not_found' => 'That AI connection no longer exists.',
        'nothing_to_chat' =>
          'There is nothing to talk about yet: no transcript, summary, notes or files.',
        'context_too_small' =>
          'The AI connection\'s context is too small. Choose one with a larger context.',
        'meeting_not_found' => 'That meeting no longer exists.',
        'project_not_found' => 'That project no longer exists.',
        'llm_unreachable' =>
          'The AI server could not be reached. Check the connection.',
        'llm_unauthorized' => 'The AI server refused the API key.',
        'llm_rate_limited' => 'The AI server is busy. Try again later.',
        'llm_refused' => 'The model declined to answer.',
        'llm_tool_missing' =>
          'Claude Code or Codex is not installed on this computer, or the app cannot find it.',
        'storage_failed' => 'The notes could not be read.',
        'no_paired_computer' =>
          'No paired computer answered. On a computer, open Settings → AI and choose a '
              'connection under "Summaries for my other devices", then keep it on and connected.',
        'paired_computer_timeout' => 'The computer took too long to answer.',
        'paired_computer_left' =>
          'The computer disconnected before answering. Try again.',
        'rate_limited' =>
          'The computer answers at most 30 questions an hour. Try again later.',
        'busy' =>
          'The computer is answering other questions. Try again shortly.',
        'p2p_unavailable' => 'Paired devices are off on this phone.',
        'runtime_unavailable' => 'The app is still starting. Try again.',
        _ when code.startsWith('llm_') =>
          'The AI server could not answer. Try again.',
        _ => 'Something went wrong. Try again.',
      };

  @override
  String toString() => 'ChatException($code)';
}

/// Chats through the core.
final class ChatBridge {
  /// Creates the bridge; `call` is replaced in tests.
  const ChatBridge(
      {LlmNativeCall call = callRustLlm, this.canAskPairedComputer = false})
      : _call = call;

  final LlmNativeCall _call;

  /// Whether this device can ask a paired computer to answer (the phone, #107).
  final bool canAskPairedComputer;

  /// Reads a chat's messages, oldest first. Throws `ChatException`.
  Future<List<ChatEntry>> history(ChatTopic topic) async {
    final Map<String, Object?> answer =
        await _command(<String, Object?>{'command': 'history', ...topic._json});
    return <ChatEntry>[
      for (final Object? message
          in answer['messages'] as List<Object?>? ?? <Object?>[])
        if (message case final Map<String, Object?> message)
          ChatEntry.fromJson(message),
    ];
  }

  /// Asks a question with the default AI connection, or a paired computer.
  ///
  /// Outputs: The question and the answer as kept. Throws `ChatException`. Side effects: The
  /// question, the recent messages and the notes that fit go to the connection's server. With
  /// `onPairedComputer` (#107), the question and the recent messages go sealed to a paired
  /// computer, which answers from its own copy of the notes; this can take minutes.
  Future<(ChatEntry, ChatEntry)> ask(ChatTopic topic, String question,
      {bool onPairedComputer = false}) async {
    final Map<String, Object?> answer = await _command(<String, Object?>{
      'command': 'ask',
      ...topic._json,
      'question': question,
      if (onPairedComputer) 'on_paired_computer': true,
    });
    return (
      ChatEntry.fromJson(
          answer['question'] as Map<String, Object?>? ?? <String, Object?>{}),
      ChatEntry.fromJson(
          answer['answer'] as Map<String, Object?>? ?? <String, Object?>{}),
    );
  }

  /// Deletes a chat. Throws `ChatException`.
  Future<void> clear(ChatTopic topic) =>
      _command(<String, Object?>{'command': 'clear', ...topic._json});

  Future<Map<String, Object?>> _command(Map<String, Object?> request) async {
    try {
      return decodeLlmAnswer(
          await _call('note_taker_chat', <String?>[jsonEncode(request)]));
    } on LlmException catch (error) {
      throw ChatException(error.code, detail: error.detail);
    }
  }
}
