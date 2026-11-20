// The app's side of kept meeting recordings (#140): the per-device switch, what is kept, playing a
// meeting's recording and wiping them all, through `note_taker_recordings_command`.
//
// The core keeps recordings sealed; playing one decrypts it to a single temporary WAV file, which
// `release` deletes. Calls run on a background isolate (`callRustLlm`).

import 'dart:convert';

import 'llm_bridge.dart';

/// What Settings shows: the switch, how many recordings are kept and their size.
final class RecordingsStatus {
  /// Creates a status.
  const RecordingsStatus(
      {required this.keep, required this.count, required this.bytes});

  /// Reads the core's `{"keep","count","bytes"}`.
  ///
  /// Throws `TypeError` for a malformed answer.
  factory RecordingsStatus.fromJson(Map<String, Object?> json) =>
      RecordingsStatus(
        keep: json['keep'] as bool? ?? false,
        count: (json['count'] as num?)?.toInt() ?? 0,
        bytes: (json['bytes'] as num?)?.toInt() ?? 0,
      );

  /// Whether new meetings' recordings are kept.
  final bool keep;

  /// How many meetings have a kept recording.
  final int count;

  /// Their size on disk, in bytes.
  final int bytes;
}

/// A recording ready to play: the decrypted file and its length.
final class PreparedRecording {
  /// Creates it.
  const PreparedRecording({required this.path, required this.durationMs});

  /// The temporary WAV file, deleted by `release`.
  final String path;

  /// The recording's length in milliseconds.
  final int durationMs;
}

/// Calls the core's recordings command.
final class RecordingsBridge {
  /// Creates the bridge over `call`, the core's functions by default.
  const RecordingsBridge({LlmNativeCall call = callRustLlm}) : _call = call;

  final LlmNativeCall _call;

  /// Runs one command. Throws `LlmException` with the core's `error_code`.
  Future<Map<String, Object?>> _command(Map<String, Object?> request) async =>
      decodeLlmAnswer(await _call(
          'note_taker_recordings_command', <String?>[jsonEncode(request)]));

  /// Reads the switch and what is kept. Side effects: Reads the settings and the folder.
  Future<RecordingsStatus> status() async => RecordingsStatus.fromJson(
      await _command(<String, Object?>{'command': 'status'}));

  /// Turns keeping recordings on or off. Side effects: Writes the settings file.
  Future<RecordingsStatus> setKeep(bool keep) async =>
      RecordingsStatus.fromJson(await _command(
          <String, Object?>{'command': 'set_keep', 'keep': keep}));

  /// Deletes every kept recording. Outputs: How many were deleted.
  /// Side effects: Deletes files; cannot be undone.
  Future<int> wipe() async =>
      ((await _command(<String, Object?>{'command': 'wipe'}))['deleted']
              as num?)
          ?.toInt() ??
      0;

  /// Whether the meeting has a kept recording, without decrypting it.
  Future<bool> has(String meetingId) async =>
      (await _command(<String, Object?>{
        'command': 'has',
        'meeting_id': meetingId,
      }))['available'] ==
      true;

  /// Decrypts the meeting's recording for playback.
  ///
  /// Outputs: The file and length. Throws `LlmException('not_found')` when none is kept.
  /// Side effects: Replaces the temporary playback file.
  Future<PreparedRecording> prepare(String meetingId) async {
    final Map<String, Object?> answer = await _command(<String, Object?>{
      'command': 'prepare',
      'meeting_id': meetingId,
    });
    return PreparedRecording(
      path: answer['path']! as String,
      durationMs: (answer['duration_ms'] as num?)?.toInt() ?? 0,
    );
  }

  /// Deletes the temporary playback file. Side effects: Removes it.
  Future<void> release() async {
    await _command(<String, Object?>{'command': 'release'});
  }
}
