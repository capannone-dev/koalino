// Calls the core's notes folder (#98): whether this computer keeps plain files mirroring
// projects and meetings, where, and how the last round went. One JSON command goes through
// `note_taker_notes_folder` on a background isolate, since turning it on and syncing write the
// whole folder.

import 'dart:convert';

import 'package:note_taker_ui/infrastructure/llm_bridge.dart';

/// The folder's settings and last round.
final class NotesFolderStatus {
  /// Creates the status.
  const NotesFolderStatus({
    required this.enabled,
    this.path,
    this.suggestedPath,
    this.lastRoundAt,
    this.lastError,
    this.written = 0,
    this.imported = 0,
  });

  /// Reads the core's JSON.
  factory NotesFolderStatus.fromJson(Map<String, Object?> json) =>
      NotesFolderStatus(
        enabled: json['enabled'] == true,
        path: json['path'] as String?,
        suggestedPath: json['suggested_path'] as String?,
        lastRoundAt: DateTime.tryParse(json['last_round_at'] as String? ?? ''),
        lastError: json['last_error'] as String?,
        written: (json['written'] as num?)?.toInt() ?? 0,
        imported: (json['imported'] as num?)?.toInt() ?? 0,
      );

  final bool enabled;

  /// The chosen folder, or null before one is chosen.
  final String? path;

  /// `~/NoteTaker`, offered when no folder is chosen.
  final String? suggestedPath;
  final DateTime? lastRoundAt;

  /// The last round's error code, or null when it went well.
  final String? lastError;

  /// Files the last round wrote, and edits or files it took in.
  final int written;
  final int imported;
}

/// A refused or failed command, with the core's stable code.
final class NotesFolderException implements Exception {
  /// Creates the exception.
  const NotesFolderException(this.code);

  final String code;

  /// A sentence for people.
  String get message => notesFolderErrorMessage(code);

  @override
  String toString() => 'NotesFolderException($code)';
}

/// Words an error code, such as the last round's.
String notesFolderErrorMessage(String code) => switch (code) {
      'invalid_folder' =>
        'Choose a full folder path, inside your home folder but not the home folder itself.',
      'folder_failed' =>
        'The folder could not be written. Check that it exists and you can write there.',
      'storage_failed' => 'The notes could not be read or saved.',
      'runtime_unavailable' => 'The app is still starting. Try again.',
      _ => 'Something went wrong. Try again.',
    };

/// Keeps the notes folder through the core.
final class NotesFolderBridge {
  /// Creates the bridge; `call` is replaced in tests.
  const NotesFolderBridge({LlmNativeCall call = callRustLlm}) : _call = call;

  final LlmNativeCall _call;

  /// Reads the status. Throws `NotesFolderException`. Side effects: None.
  Future<NotesFolderStatus> status() =>
      _command(<String, Object?>{'command': 'get'});

  /// Turns the folder on (in `path`, or the saved or suggested one) or off.
  ///
  /// Throws `NotesFolderException`. Side effects: Saves the setting; turning it on writes the
  /// folder at once.
  Future<NotesFolderStatus> set({required bool enabled, String? path}) =>
      _command(<String, Object?>{
        'command': 'set',
        'enabled': enabled,
        if (path != null) 'path': path,
      });

  /// Reads edits back and writes the folder now. Throws `NotesFolderException`.
  Future<NotesFolderStatus> syncNow() =>
      _command(<String, Object?>{'command': 'sync_now'});

  Future<NotesFolderStatus> _command(Map<String, Object?> request) async {
    try {
      return NotesFolderStatus.fromJson(decodeLlmAnswer(await _call(
          'note_taker_notes_folder', <String?>[jsonEncode(request)])));
    } on LlmException catch (error) {
      throw NotesFolderException(error.code);
    }
  }
}
