// Calls the core's meeting edit commands (#96): the owner's notes, the summary's text and single
// transcript segments. Every command goes through `note_taker_meeting_edit` with one JSON request
// and answers `{"ok":true,...}` or `{"ok":false,"error_code","error"}`; calls run on background
// isolates so the UI thread never waits on the database. Note text is never logged.

import 'dart:convert';

import 'package:note_taker_ui/infrastructure/llm_bridge.dart';

/// A refused or failed edit, with the core's stable error code.
final class MeetingEditException implements Exception {
  /// Creates the exception.
  const MeetingEditException(this.code);

  /// Such as `empty`, `too_long` or `segment_not_found`.
  final String code;

  /// A sentence for people; the core's own text is not shown.
  String get message => switch (code) {
        'empty' => 'Write something first.',
        'too_long' => 'The text is too long.',
        'meeting_not_found' => 'That meeting no longer exists.',
        'segment_not_found' =>
          'That part of the transcript changed meanwhile. Reopen the meeting.',
        'storage_failed' => 'The meeting could not be saved.',
        'runtime_unavailable' => 'The app is still starting. Try again.',
        _ => 'Something went wrong. Try again.',
      };

  @override
  String toString() => 'MeetingEditException($code)';
}

/// Edits meetings through the core.
final class MeetingEditBridge {
  /// Creates the bridge; `call` is replaced in tests.
  const MeetingEditBridge({LlmNativeCall call = callRustLlm}) : _call = call;

  final LlmNativeCall _call;

  /// Sends one command and decodes its answer.
  ///
  /// Throws `MeetingEditException` for a refusal or an unreadable answer.
  Future<Map<String, Object?>> _command(Map<String, Object?> request) async {
    final String answer =
        await _call('note_taker_meeting_edit', <String?>[jsonEncode(request)]);
    try {
      return decodeLlmAnswer(answer);
    } on LlmException catch (error) {
      throw MeetingEditException(error.code);
    }
  }

  /// Reads the meeting's notes (Markdown), or null when there are none.
  ///
  /// Side effects: Reads the meeting document.
  Future<String?> notes(String meetingId) async =>
      (await _command(<String, Object?>{
        'command': 'notes',
        'meeting_id': meetingId,
      }))['notes'] as String?;

  /// Replaces the meeting's notes.
  ///
  /// Outputs: The notes as stored. Side effects: Writes the meeting document, which syncs to
  /// paired devices.
  Future<String> setNotes(String meetingId, String text) async =>
      (await _command(<String, Object?>{
        'command': 'set_notes',
        'meeting_id': meetingId,
        'text': text,
      }))['notes'] as String? ??
      text;

  /// Replaces the summary's text, or writes a summary by hand when there is none.
  ///
  /// Outputs: The summary as stored, marked edited. Side effects: Writes the meeting document.
  Future<StoredSummary> editSummary(String meetingId, String text) async =>
      StoredSummary.fromJson((await _command(<String, Object?>{
        'command': 'edit_summary',
        'meeting_id': meetingId,
        'text': text,
      }))['summary']! as Map<String, Object?>);

  /// Replaces one segment's text, found by its start and end; blank text removes it.
  ///
  /// Outputs: The text as stored, or null when the segment was removed. Side effects: Flags the
  /// transcript as edited, so later passes leave it alone, and writes the meeting's segments.
  Future<String?> editSegment(
      String meetingId, int startTimeMs, int endTimeMs, String text) async {
    final Object? segment = (await _command(<String, Object?>{
      'command': 'edit_segment',
      'meeting_id': meetingId,
      'start_time_ms': startTimeMs,
      'end_time_ms': endTimeMs,
      'text': text,
    }))['segment'];
    return segment is Map<String, Object?> ? segment['text'] as String? : null;
  }
}
