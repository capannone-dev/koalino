// Calls the core's chat with notes (#97): one meeting, one project, or every meeting, summary
// and project. One JSON command goes through `note_taker_chat` on a background isolate, since
// asking waits for the model. Questions and answers are never logged.

import 'dart:convert';

import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

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
          l10n.theAiServerRejectedTheRequest(detail),
        ('llm_tool_failed', final String detail) =>
          l10n.claudeCodeOrCodexCouldNot(detail),
        _ => _sentence,
      };

  String get _sentence => switch (code) {
        'empty' => l10n.typeAQuestion,
        'too_long' => l10n.theQuestionIsTooLong,
        'no_connection' => l10n.addAnAiConnectionInSettings,
        'connection_not_found' => l10n.thatAiConnectionNoLongerExists,
        'nothing_to_chat' => l10n.thereIsNothingToTalkAbout,
        'context_too_small' => l10n.theAiConnectionSContextIs,
        'meeting_not_found' => l10n.thatMeetingNoLongerExists,
        'project_not_found' => l10n.thatProjectNoLongerExists,
        'llm_unreachable' => l10n.theAiServerCouldNotBe,
        'llm_unauthorized' => l10n.theAiServerRefusedTheApi,
        'llm_rate_limited' => l10n.theAiServerIsBusyTry,
        'llm_refused' => l10n.theModelDeclinedToAnswer,
        'llm_tool_missing' => l10n.claudeCodeOrCodexIsNot,
        'storage_failed' => l10n.theNotesCouldNotBeRead,
        'no_paired_computer' => l10n.noPairedComputerAnsweredOnA,
        'paired_computer_timeout' => l10n.theComputerTookTooLongTo,
        'paired_computer_left' =>
          l10n.theComputerDisconnectedBeforeAnsweringTry,
        'rate_limited' => l10n.theComputerAnswersAtMost30,
        'busy' => l10n.theComputerIsAnsweringOtherQuestions,
        'p2p_unavailable' => l10n.pairedDevicesAreOffOnThis,
        'runtime_unavailable' => l10n.theAppIsStillStartingTry,
        _ when code.startsWith('llm_') => l10n.theAiServerCouldNotAnswer,
        _ => l10n.somethingWentWrongTryAgain,
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
