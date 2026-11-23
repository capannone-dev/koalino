// Defines the Dart-native boundary for reading meetings, their titles, transcripts and
// attachments, for renaming and deleting meetings, and for splitting recordings the app combined.

import 'dart:convert';
import 'dart:ffi';
import 'dart:isolate';

import 'package:ffi/ffi.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

import 'core_library.dart';

/// A device that recorded a meeting (#88), as Rust names it.
final class RecordingDevice {
  /// Creates an immutable device entry.
  const RecordingDevice({
    required this.deviceId,
    required this.name,
    required this.thisDevice,
    this.connected,
  });

  /// The ID written on the device's meetings.
  final String deviceId;

  /// This device's name or a paired device's; `null` when Rust does not know it.
  final String? name;

  /// Whether it is this device.
  final bool thisDevice;

  /// Whether it is connected now; `null` for this device and without P2P.
  final bool? connected;

  /// The name to show: its own, else "this device" or "another device".
  String get label =>
      name ?? (thisDevice ? l10n.thisDevice2 : l10n.anotherDevice);

  /// The label, with "(not connected)" for a paired device that is away.
  String get labelWithConnection =>
      connected == false ? l10n.notConnected(label) : label;

  /// Decodes one element of `recorded_on`.
  factory RecordingDevice.fromJson(Map<String, Object?> json) {
    return RecordingDevice(
      deviceId: json['device_id']! as String,
      name: json['name'] as String?,
      thisDevice: json['this_device'] as bool? ?? false,
      connected: json['connected'] as bool?,
    );
  }
}

/// How far combining a meeting's recordings got, as this device sees it (#88).
enum CombiningState {
  /// Only one device recorded the meeting so far.
  oneDevice,

  /// Audio from `waitingFor` still has to reach the combining device.
  waiting,

  /// Every recording's audio is here; the combined pass runs when the device is free.
  readyToCombine,

  /// This device still has to send its audio to the combining device.
  sending,

  /// The transcript is built from every recording's audio.
  combined,

  /// Audio from `waitingFor` did not arrive in time; the transcript is built without it.
  notCombined;

  /// Decodes the `state` text Rust writes; `null` for one this app does not know.
  static CombiningState? parse(String? state) => switch (state) {
        'one_device' => CombiningState.oneDevice,
        'waiting' => CombiningState.waiting,
        'ready_to_combine' => CombiningState.readyToCombine,
        'sending' => CombiningState.sending,
        'combined' => CombiningState.combined,
        'not_combined' => CombiningState.notCombined,
        _ => null,
      };
}

/// A meeting's combining status from `combining` in the meeting JSON (#88).
final class MeetingCombining {
  /// Creates an immutable status.
  const MeetingCombining({
    required this.state,
    this.combineOn,
    this.waitingFor = const <RecordingDevice>[],
    this.keptUntil,
  });

  /// How far combining got.
  final CombiningState state;

  /// The device that combines the recordings; `null` for one device.
  final RecordingDevice? combineOn;

  /// The devices whose audio is awaited, or that could not be combined.
  final List<RecordingDevice> waitingFor;

  /// When the audio held here is deleted if not combined by then; `null` when none is held.
  final DateTime? keptUntil;

  /// Whether nothing more will happen: combined, not combined, or one device with no audio kept.
  bool get isSettled => switch (state) {
        CombiningState.combined || CombiningState.notCombined => true,
        CombiningState.oneDevice => keptUntil == null,
        _ => false,
      };

  /// One or two plain sentences for the meeting's details.
  String get description {
    final String names = waitingFor
        .map((RecordingDevice device) => device.labelWithConnection)
        .join(', ');
    final String plainNames =
        waitingFor.map((RecordingDevice device) => device.label).join(', ');
    final String combiner =
        combineOn?.labelWithConnection ?? l10n.anotherDevice;
    final DateTime? until = keptUntil;
    final String kept = until == null
        ? ''
        : l10n.itIsKeptEncryptedUntil(formatCalendarDay(until));
    return switch (state) {
      CombiningState.oneDevice => until == null
          ? l10n.recordedOnThisDeviceOnly
          : l10n.recordedOnThisDeviceOnlyA(formatCalendarDay(until)),
      CombiningState.waiting when waitingFor.isEmpty =>
        l10n.combinesTheRecordingsNothingIsLeft(combiner),
      CombiningState.waiting when combineOn?.thisDevice ?? false =>
        l10n.waitingForAudioFrom(names, kept),
      CombiningState.waiting =>
        l10n.combinesTheRecordingsWaitingForAudio(combiner, names),
      CombiningState.readyToCombine => l10n.audioFromEveryDeviceIsHere,
      CombiningState.sending => l10n.thisDeviceSAudioStillHas(combiner, kept),
      CombiningState.combined => l10n.transcriptCombinedFromEveryDevice,
      CombiningState.notCombined => l10n.notCombinedAudioFromDidNot(plainNames),
    };
  }

  /// Decodes `combining`; `null` for a missing value or an unknown state.
  static MeetingCombining? fromJson(Object? json) {
    if (json is! Map<String, Object?>) {
      return null;
    }
    final CombiningState? state =
        CombiningState.parse(json['state'] as String?);
    if (state == null) {
      return null;
    }
    final Object? combineOn = json['combine_on'];
    final String? keptUntil = json['kept_until'] as String?;
    return MeetingCombining(
      state: state,
      combineOn: combineOn is Map<String, Object?>
          ? RecordingDevice.fromJson(combineOn)
          : null,
      waitingFor: <RecordingDevice>[
        for (final Object? device
            in (json['waiting_for'] as List<Object?>?) ?? <Object?>[])
          if (device is Map<String, Object?>) RecordingDevice.fromJson(device),
      ],
      keptUntil:
          keptUntil == null ? null : DateTime.tryParse(keptUntil)?.toUtc(),
    );
  }
}

/// Describes one meeting row returned by `note_taker_get_meetings`.
final class MeetingSummary {
  /// Creates immutable meeting metadata.
  const MeetingSummary({
    required this.meetingId,
    required this.title,
    required this.createdAt,
    required this.durationMs,
    this.transcriptionLanguages = const <String>[],
    this.recordedOn = const <RecordingDevice>[],
    this.combinedRecordings = const <String>[],
    this.combining,
  });

  /// Stable meeting identifier used for every follow-up request.
  final String meetingId;

  /// The person-chosen title, or the dated fallback for an untitled meeting; never blank.
  final String title;

  /// UTC instant at which the meeting was created.
  final DateTime createdAt;

  /// End of the latest transcript segment in milliseconds (0 before any audio is transcribed).
  final int durationMs;

  /// Speech-to-text languages chosen while recording, in first-use order: `auto` or Whisper
  /// codes. Empty for meetings recorded before languages were kept, or imported ones.
  final List<String> transcriptionLanguages;

  /// The devices that recorded the meeting, its own first (#88); empty from older cores.
  final List<RecordingDevice> recordedOn;

  /// Other devices' recordings the app combined into this meeting; Split lists them again.
  final List<String> combinedRecordings;

  /// How far combining the recordings got, as this device sees it; `null` from older cores.
  final MeetingCombining? combining;

  /// "Laptop + Pixel" for a meeting combined from several devices' recordings, else `null`.
  String? get recordedOnLabel => combinedRecordings.isEmpty
      ? null
      : recordedOn.map((RecordingDevice device) => device.label).join(' + ');

  /// Decodes one meeting object, filling a missing or blank title with `fallbackMeetingTitle`.
  ///
  /// Inputs: A JSON object with `meeting_id`, `created_at` (RFC 3339), and optional `title`,
  /// `duration_ms` and `transcription_languages` (non-string entries are skipped).
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
      transcriptionLanguages: <String>[
        ...((json['transcription_languages'] as List<Object?>?) ?? <Object?>[])
            .whereType<String>(),
      ],
      recordedOn: <RecordingDevice>[
        for (final Object? device
            in (json['recorded_on'] as List<Object?>?) ?? <Object?>[])
          if (device is Map<String, Object?>) RecordingDevice.fromJson(device),
      ],
      combinedRecordings: <String>[
        ...((json['combined_recordings'] as List<Object?>?) ?? <Object?>[])
            .whereType<String>(),
      ],
      combining: MeetingCombining.fromJson(json['combining']),
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
    this.overlappingSpeech = false,
    this.uncertainSpeaker = false,
    this.uncertainWords = const <int>[],
  });

  /// Offset of the segment start from the meeting start.
  final int startTimeMs;

  /// Offset of the segment end from the meeting start.
  final int endTimeMs;

  /// Speaker name resolved by Rust, `Speaker N` for an unnamed voice (#118), or `Unattributed`.
  final String speaker;

  /// Transcribed text of the segment.
  final String text;

  /// Whether the segment lies largely in overlapping speech, where words are often missed (#90).
  final bool overlappingSpeech;

  /// Whether the speaker is a guess (#91).
  final bool uncertainSpeaker;

  /// Positions of the unclear words among the text's whitespace-separated words (#91).
  final List<int> uncertainWords;

  /// Decodes one element of the `segments` array; a core without the marks reads as plain.
  factory MeetingTranscriptSegment.fromJson(Map<String, Object?> json) {
    final Object? positions = json['uncertain_words'];
    return MeetingTranscriptSegment(
      startTimeMs: (json['start_time_ms']! as num).toInt(),
      endTimeMs: (json['end_time_ms']! as num).toInt(),
      speaker: json['speaker']! as String,
      text: json['text']! as String,
      overlappingSpeech: json['overlapping_speech'] == true,
      uncertainSpeaker: json['uncertain_speaker'] == true,
      uncertainWords: positions is List<Object?>
          ? positions
              .whereType<num>()
              .map((num position) => position.toInt())
              .toList(growable: false)
          : const <int>[],
    );
  }
}

/// Describes one picture or document attached to a meeting (Issue #54).
final class MeetingAttachment {
  /// Creates immutable attachment metadata.
  const MeetingAttachment({
    required this.attachmentId,
    required this.fileName,
    required this.filePath,
    required this.mimeType,
    required this.meetingTimeMs,
  });

  /// Stable attachment identifier.
  final String attachmentId;

  /// Name to show, without the prefix Rust adds to the stored copy.
  final String fileName;

  /// Absolute path of the app's private copy.
  final String filePath;

  /// MIME type Rust derived from the file type, e.g. `image/jpeg`.
  final String mimeType;

  /// Meeting time at which the attachment belongs in the transcript; `null` for the end.
  final int? meetingTimeMs;

  /// Whether the attachment can be shown as a picture.
  bool get isImage => mimeType.startsWith('image/');

  /// Decodes one element of an `attachments` (or `attached`) array.
  ///
  /// Inputs: A JSON object with `attachment_id`, `file_name`, `file_path`, `mime_type`, and a
  /// nullable `meeting_time_ms`.
  /// Outputs: The attachment; throws `TypeError` for a malformed object.
  factory MeetingAttachment.fromJson(Map<String, Object?> json) {
    return MeetingAttachment(
      attachmentId: json['attachment_id']! as String,
      fileName: json['file_name']! as String,
      filePath: json['file_path']! as String,
      mimeType: json['mime_type']! as String,
      meetingTimeMs: (json['meeting_time_ms'] as num?)?.toInt(),
    );
  }
}

/// Decodes a JSON array of attachment objects; a missing array means none.
List<MeetingAttachment> decodeMeetingAttachments(Object? attachmentsJson) =>
    ((attachmentsJson as List<Object?>?) ?? const <Object?>[])
        .cast<Map<Object?, Object?>>()
        .map((Map<Object?, Object?> attachmentJson) =>
            MeetingAttachment.fromJson(attachmentJson.cast<String, Object?>()))
        .toList(growable: false);

/// Describes one meeting returned by `note_taker_get_meeting`.
final class MeetingDetail {
  /// Creates immutable meeting content.
  const MeetingDetail({
    required this.summary,
    required this.transcript,
    required this.segments,
    this.attachments = const <MeetingAttachment>[],
  });

  /// Title, creation time, and duration shared with the list row.
  final MeetingSummary summary;

  /// Flat transcript text, present only for meetings imported without segments.
  final String? transcript;

  /// Speaker-attributed transcript in timeline order (empty when none was stored).
  final List<MeetingTranscriptSegment> segments;

  /// Pictures and documents attached to the meeting, in the order they were added.
  final List<MeetingAttachment> attachments;
}

/// Builds the title Rust shows for an untitled meeting, such as `Meeting 2026-01-15 14:30 UTC`.
///
/// Inputs: The meeting's creation instant (converted to UTC).
/// Outputs: The same fallback `MeetingMetadata::display_title` produces in Rust.
/// Side effects: None.
String fallbackMeetingTitle(DateTime createdAt) =>
    l10n.meeting(formatMeetingCreatedAt(createdAt));

/// Formats a creation instant as `YYYY-MM-DD HH:MM UTC`.
String formatMeetingCreatedAt(DateTime createdAt) {
  final DateTime utcTime = createdAt.toUtc();
  String twoDigits(int value) => value.toString().padLeft(2, '0');
  return '${utcTime.year.toString().padLeft(4, '0')}-${twoDigits(utcTime.month)}-'
      '${twoDigits(utcTime.day)} ${twoDigits(utcTime.hour)}:'
      '${twoDigits(utcTime.minute)} UTC';
}

/// Formats an instant's UTC calendar day as `YYYY-MM-DD`.
String formatCalendarDay(DateTime instant) {
  final DateTime utcTime = instant.toUtc();
  String twoDigits(int value) => value.toString().padLeft(2, '0');
  return '${utcTime.year.toString().padLeft(4, '0')}-${twoDigits(utcTime.month)}-'
      '${twoDigits(utcTime.day)}';
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
    attachments: decodeMeetingAttachments(json['attachments']),
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

  /// Deletes a meeting for good, here and on paired devices.
  ///
  /// Inputs: The meeting. Outputs: Completes when it is gone, also when it already was.
  /// Throws `StateError` when Rust refuses, such as while it is being recorded.
  Future<void> deleteMeeting(String meetingId);

  /// Lists the recordings combined into a meeting on their own again, for good (#88).
  ///
  /// Inputs: The meeting. Outputs: Completes once they are split, also when there was nothing to
  /// split. Throws `StateError` when Rust refuses.
  /// Side effects: Writes the recordings' documents, which sync to paired devices.
  Future<void> splitMeeting(String meetingId);
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

  @override
  Future<void> splitMeeting(String meetingId) async {
    final int statusCode = await _splitMeeting(meetingId);
    if (statusCode != 0) {
      throw StateError('Rust refused the split (status $statusCode).');
    }
  }

  @override
  Future<void> deleteMeeting(String meetingId) async {
    final int statusCode = await _deleteMeeting(meetingId);
    if (statusCode != 0 && statusCode != 5) {
      throw StateError('Rust refused the deletion (status $statusCode).');
    }
  }
}

/// Calls `note_taker_delete_meeting` on a background isolate, because it writes to SQLite.
///
/// Declared at top level so the isolate closure captures only the string.
Future<int> _deleteMeeting(String meetingId) => Isolate.run(() {
      final Pointer<Utf8> nativeMeetingId = meetingId.toNativeUtf8();
      try {
        return _openCoreLibrary()
            .lookupFunction<_NativeDeleteMeeting, _DartDeleteMeeting>(
                'note_taker_delete_meeting')(nativeMeetingId);
      } finally {
        calloc.free(nativeMeetingId);
      }
    });

/// Calls `note_taker_split_meeting` on a background isolate, because it writes to SQLite.
///
/// Declared at top level so the isolate closure captures only the string.
Future<int> _splitMeeting(String meetingId) => Isolate.run(() {
      final Pointer<Utf8> nativeMeetingId = meetingId.toNativeUtf8();
      try {
        return _openCoreLibrary()
            .lookupFunction<_NativeDeleteMeeting, _DartDeleteMeeting>(
                'note_taker_split_meeting')(nativeMeetingId);
      } finally {
        calloc.free(nativeMeetingId);
      }
    });

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
  return openCoreLibrary();
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

/// Represents `int note_taker_delete_meeting(const char *meeting_id)`, and
/// `note_taker_split_meeting`, which has the same signature.
typedef _NativeDeleteMeeting = Int32 Function(Pointer<Utf8> meetingId);

/// Represents the Dart callable form of `note_taker_delete_meeting`.
typedef _DartDeleteMeeting = int Function(Pointer<Utf8> meetingId);

/// Represents `void note_taker_free_string(char *value)`.
typedef _NativeFreeString = Void Function(Pointer<Utf8> value);

/// Represents the Dart callable form of `note_taker_free_string`.
typedef _DartFreeString = void Function(Pointer<Utf8> value);
