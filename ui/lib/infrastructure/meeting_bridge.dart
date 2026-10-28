// Defines the Dart-native boundary for reading meetings, their titles, and their transcripts,
// and for renaming meetings.

import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:ffi/ffi.dart';

/// Describes one meeting row returned by `note_taker_get_meetings`.
final class MeetingSummary {
  /// Creates immutable meeting metadata.
  const MeetingSummary({
    required this.meetingId,
    required this.title,
    required this.createdAt,
    required this.durationMs,
  });

  /// Stable meeting identifier used for every follow-up request.
  final String meetingId;

  /// The person-chosen title, or the dated fallback for an untitled meeting; never blank.
  final String title;

  /// UTC instant at which the meeting was created.
  final DateTime createdAt;

  /// End of the latest transcript segment in milliseconds (0 before any audio is transcribed).
  final int durationMs;

  /// Decodes one meeting object, filling a missing or blank title with `fallbackMeetingTitle`.
  ///
  /// Inputs: A JSON object with `meeting_id`, `created_at` (RFC 3339), and optional `title`
  /// and `duration_ms`.
  /// Outputs: The summary; throws `TypeError` or `FormatException` for a malformed object.
  factory MeetingSummary.fromJson(Map<String, Object?> json) {
    final DateTime createdAt =
        DateTime.parse(json['created_at']! as String).toUtc();
    final String? title = (json['title'] as String?)?.trim();
    return MeetingSummary(
      meetingId: json['meeting_id']! as String,
      title: title == null || title.isEmpty
          ? fallbackMeetingTitle(createdAt)
          : title,
      createdAt: createdAt,
      durationMs: (json['duration_ms'] as num?)?.toInt() ?? 0,
    );
  }
}

/// Describes one speaker-attributed transcript segment of a meeting.
final class MeetingTranscriptSegment {
  /// Creates an immutable segment.
  const MeetingTranscriptSegment({
    required this.startTimeMs,
    required this.endTimeMs,
    required this.speaker,
    required this.text,
  });

  /// Offset of the segment start from the meeting start.
  final int startTimeMs;

  /// Offset of the segment end from the meeting start.
  final int endTimeMs;

  /// Speaker name resolved by Rust, or `Unknown speaker` / `Unattributed`.
  final String speaker;

  /// Transcribed text of the segment.
  final String text;

  /// Decodes one element of the `segments` array.
  factory MeetingTranscriptSegment.fromJson(Map<String, Object?> json) {
    return MeetingTranscriptSegment(
      startTimeMs: (json['start_time_ms']! as num).toInt(),
      endTimeMs: (json['end_time_ms']! as num).toInt(),
      speaker: json['speaker']! as String,
      text: json['text']! as String,
    );
  }
}

/// Describes one meeting returned by `note_taker_get_meeting`.
final class MeetingDetail {
  /// Creates immutable meeting content.
  const MeetingDetail({
    required this.summary,
    required this.transcript,
    required this.segments,
  });

  /// Title, creation time, and duration shared with the list row.
  final MeetingSummary summary;

  /// Flat transcript text, present only for meetings imported without segments.
  final String? transcript;

  /// Speaker-attributed transcript in timeline order (empty when none was stored).
  final List<MeetingTranscriptSegment> segments;
}

/// Builds the title Rust shows for an untitled meeting, such as `Meeting 2026-01-15 14:30 UTC`.
///
/// Inputs: The meeting's creation instant (converted to UTC).
/// Outputs: The same fallback `MeetingMetadata::display_title` produces in Rust.
/// Side effects: None.
String fallbackMeetingTitle(DateTime createdAt) =>
    'Meeting ${formatMeetingCreatedAt(createdAt)}';

/// Formats a creation instant as `YYYY-MM-DD HH:MM UTC`.
String formatMeetingCreatedAt(DateTime createdAt) {
  final DateTime utcTime = createdAt.toUtc();
  String twoDigits(int value) => value.toString().padLeft(2, '0');
  return '${utcTime.year.toString().padLeft(4, '0')}-${twoDigits(utcTime.month)}-'
      '${twoDigits(utcTime.day)} ${twoDigits(utcTime.hour)}:'
      '${twoDigits(utcTime.minute)} UTC';
}

/// Formats milliseconds as `M:SS`, or `H:MM:SS` from one hour, truncating partial seconds.
String formatMeetingDuration(int durationMs) {
  final int totalSeconds = durationMs ~/ 1000;
  final int hours = totalSeconds ~/ 3600;
  final int minutes = totalSeconds ~/ 60 % 60;
  final String seconds = (totalSeconds % 60).toString().padLeft(2, '0');
  return hours > 0
      ? '$hours:${minutes.toString().padLeft(2, '0')}:$seconds'
      : '$minutes:$seconds';
}

/// Decodes the `note_taker_get_meetings` response.
///
/// Returns the meetings for a JSON array; throws `StateError` carrying the Rust error for an
/// `{"ok":false,...}` object and `FormatException` for any other shape.
List<MeetingSummary> decodeMeetings(String responseJson) {
  final Object? decodedResponse = jsonDecode(responseJson);
  if (decodedResponse is List) {
    return decodedResponse
        .cast<Map<Object?, Object?>>()
        .map((Map<Object?, Object?> json) =>
            MeetingSummary.fromJson(json.cast<String, Object?>()))
        .toList(growable: false);
  }
  if (decodedResponse is Map) {
    throw _nativeError(decodedResponse.cast<String, Object?>());
  }
  throw const FormatException('Meeting list response must be a JSON array.');
}

/// Decodes the `note_taker_get_meeting` response.
///
/// Returns the meeting for a meeting object; throws `StateError` carrying the Rust error for an
/// `{"ok":false,...}` object and `FormatException` for any other shape.
MeetingDetail decodeMeetingDetail(String responseJson) {
  final Object? decodedResponse = jsonDecode(responseJson);
  if (decodedResponse is! Map) {
    throw const FormatException('Meeting response must be a JSON object.');
  }
  final Map<String, Object?> json = decodedResponse.cast<String, Object?>();
  if (json['ok'] == false) {
    throw _nativeError(json);
  }
  final List<Object?> segmentsJson =
      (json['segments'] as List<Object?>?) ?? const <Object?>[];
  return MeetingDetail(
    summary: MeetingSummary.fromJson(json),
    transcript: json['transcript'] as String?,
    segments: segmentsJson
        .cast<Map<Object?, Object?>>()
        .map((Map<Object?, Object?> segmentJson) =>
            MeetingTranscriptSegment.fromJson(
                segmentJson.cast<String, Object?>()))
        .toList(growable: false),
  );
}

/// Builds the exception for a Rust `{"ok":false,"error_code","error"}` response.
StateError _nativeError(Map<String, Object?> json) =>
    StateError('Rust rejected the meeting request '
        '(${json['error_code'] ?? 'unknown_error'}): ${json['error'] ?? ''}');

/// Why Rust rejected a rename, decoded from the `note_taker_rename_meeting` status code.
enum MeetingRenameFailure {
  /// Status 1: the title is empty, longer than 200 characters, or contains control characters
  /// (or an argument was not a valid string).
  invalidTitle,

  /// Status 2: the title could not be written to storage.
  storageFailure,

  /// Status 3: the native runtime could not start.
  runtimeUnavailable,

  /// Status 5: no meeting has this ID (for example, it was deleted meanwhile).
  meetingNotFound,

  /// Any other status, which this version of the app does not know.
  unknown,
}

/// Thrown when Rust rejects a rename.
final class MeetingRenameException implements Exception {
  /// Creates an exception for `failure`, keeping the raw `statusCode` for diagnostics.
  const MeetingRenameException(this.failure, this.statusCode);

  /// The decoded reason.
  final MeetingRenameFailure failure;

  /// The status code `note_taker_rename_meeting` returned.
  final int statusCode;

  @override
  String toString() =>
      'MeetingRenameException(${failure.name}, status $statusCode)';
}

/// Maps a non-zero `note_taker_rename_meeting` status code to the exception to throw.
///
/// Inputs: The status code (0 means success and must not be passed).
/// Outputs: The exception; unknown codes map to `MeetingRenameFailure.unknown`.
/// Side effects: None.
MeetingRenameException meetingRenameExceptionFor(int statusCode) =>
    MeetingRenameException(
      switch (statusCode) {
        1 => MeetingRenameFailure.invalidTitle,
        2 => MeetingRenameFailure.storageFailure,
        3 => MeetingRenameFailure.runtimeUnavailable,
        5 => MeetingRenameFailure.meetingNotFound,
        _ => MeetingRenameFailure.unknown,
      },
      statusCode,
    );

/// Declares the meeting operations available to controllers.
abstract interface class MeetingBridge {
  /// Loads every local meeting in repository (creation) order.
  Future<List<MeetingSummary>> getMeetings();

  /// Loads one meeting's title, metadata, and transcript.
  Future<MeetingDetail> getMeeting(String meetingId);

  /// Replaces a meeting's title.
  ///
  /// Inputs: The meeting ID and the new title. Rust trims the title and requires 1–200
  /// characters without control characters.
  /// Outputs: Completes on success; throws `MeetingRenameException` when Rust rejects the rename.
  /// Side effects: Writes the title to the meeting's CRDT document and SQLite, so it replicates
  /// to peers on the next sync.
  Future<void> renameMeeting(String meetingId, String newTitle);
}

/// Implements meeting reads with the Rust C ABI.
///
/// Both calls read SQLite synchronously, so each runs on a short-lived background isolate that
/// opens the platform core library itself.
final class RustMeetingBridge implements MeetingBridge {
  /// Creates a bridge; the native library is opened lazily inside each isolate.
  const RustMeetingBridge();

  @override
  Future<List<MeetingSummary>> getMeetings() async =>
      decodeMeetings(await _fetchMeetingsJson());

  @override
  Future<MeetingDetail> getMeeting(String meetingId) async =>
      decodeMeetingDetail(await _fetchMeetingJson(meetingId));

  @override
  Future<void> renameMeeting(String meetingId, String newTitle) async {
    final int statusCode = await _renameMeeting(meetingId, newTitle);
    if (statusCode != 0) {
      throw meetingRenameExceptionFor(statusCode);
    }
  }
}

/// Calls `note_taker_rename_meeting` on a background isolate, because it writes to SQLite.
///
/// Declared at top level so the isolate closure captures only the two strings.
Future<int> _renameMeeting(String meetingId, String newTitle) =>
    Isolate.run(() {
      final DynamicLibrary coreLibrary = _openCoreLibrary();
      final _DartRenameMeeting renameMeeting =
          coreLibrary.lookupFunction<_NativeRenameMeeting, _DartRenameMeeting>(
              'note_taker_rename_meeting');
      final Pointer<Utf8> nativeMeetingId = meetingId.toNativeUtf8();
      final Pointer<Utf8> nativeTitle = newTitle.toNativeUtf8();
      try {
        return renameMeeting(nativeMeetingId, nativeTitle);
      } finally {
        calloc
          ..free(nativeMeetingId)
          ..free(nativeTitle);
      }
    });

/// Calls `note_taker_get_meetings` on a background isolate.
Future<String> _fetchMeetingsJson() => Isolate.run(() => _callNativeJson(
      (DynamicLibrary coreLibrary) => coreLibrary.lookupFunction<
          _NativeNoArgumentJson,
          _DartNoArgumentJson>('note_taker_get_meetings')(),
    ));

/// Calls `note_taker_get_meeting` on a background isolate.
///
/// Declared at top level so the isolate closure captures only the meeting ID string.
Future<String> _fetchMeetingJson(String meetingId) =>
    Isolate.run(() => _callNativeJson((DynamicLibrary coreLibrary) {
          final Pointer<Utf8> nativeMeetingId = meetingId.toNativeUtf8();
          try {
            return coreLibrary.lookupFunction<_NativeOneStringJson,
                _DartOneStringJson>('note_taker_get_meeting')(nativeMeetingId);
          } finally {
            calloc.free(nativeMeetingId);
          }
        }));

/// Runs a caller-freed JSON command against the core library and copies its response out.
///
/// Throws `StateError` when Rust returns null (the response could not be allocated).
String _callNativeJson(
    Pointer<Utf8> Function(DynamicLibrary coreLibrary) invokeCommand) {
  final DynamicLibrary coreLibrary = _openCoreLibrary();
  final _DartFreeString freeString =
      coreLibrary.lookupFunction<_NativeFreeString, _DartFreeString>(
          'note_taker_free_string');
  final Pointer<Utf8> responsePointer = invokeCommand(coreLibrary);
  if (responsePointer == nullptr) {
    throw StateError('Rust returned no meeting response.');
  }
  try {
    return responsePointer.toDartString();
  } finally {
    freeString(responsePointer);
  }
}

/// Opens the platform-specific core library without exposing FFI to widgets.
DynamicLibrary _openCoreLibrary() {
  if (Platform.isAndroid || Platform.isLinux) {
    return DynamicLibrary.open('libnote_taker_core.so');
  }
  if (Platform.isWindows) {
    return DynamicLibrary.open('note_taker_core.dll');
  }
  throw UnsupportedError('The Rust meeting bridge is unavailable here.');
}

/// Represents `char *note_taker_get_meetings(void)`.
typedef _NativeNoArgumentJson = Pointer<Utf8> Function();

/// Represents the Dart callable form of `note_taker_get_meetings`.
typedef _DartNoArgumentJson = Pointer<Utf8> Function();

/// Represents `char *note_taker_get_meeting(const char *meeting_id)`.
typedef _NativeOneStringJson = Pointer<Utf8> Function(Pointer<Utf8> argument);

/// Represents the Dart callable form of `note_taker_get_meeting`.
typedef _DartOneStringJson = Pointer<Utf8> Function(Pointer<Utf8> argument);

/// Represents `int note_taker_rename_meeting(const char *meeting_id, const char *new_title)`.
typedef _NativeRenameMeeting = Int32 Function(
    Pointer<Utf8> meetingId, Pointer<Utf8> newTitle);

/// Represents the Dart callable form of `note_taker_rename_meeting`.
typedef _DartRenameMeeting = int Function(
    Pointer<Utf8> meetingId, Pointer<Utf8> newTitle);

/// Represents `void note_taker_free_string(char *value)`.
typedef _NativeFreeString = Void Function(Pointer<Utf8> value);

/// Represents the Dart callable form of `note_taker_free_string`.
typedef _DartFreeString = void Function(Pointer<Utf8> value);
