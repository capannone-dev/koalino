// Defines how Flutter attaches pictures and documents to a meeting (Issue #54): a multi-file
// picker, and the Rust call that copies the files into the app's storage and links them.

import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:ffi/ffi.dart';
import 'package:file_picker/file_picker.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';

/// File types Rust accepts as attachments (`AttachmentFileStore`).
const List<String> attachableExtensions = <String>[
  'jpg',
  'jpeg',
  'png',
  'webp',
  'heic',
  'pdf',
  'txt',
  'md',
  'docx',
  'xlsx',
  'pptx',
  'odt',
];

/// What one attach request achieved.
final class AttachFilesResult {
  /// Creates an immutable result.
  const AttachFilesResult({required this.attached, required this.failedPaths});

  /// Attachments Rust stored, in request order.
  final List<MeetingAttachment> attached;

  /// Paths Rust could not attach (unsupported type, unreadable, too large).
  final List<String> failedPaths;
}

/// Decodes the `note_taker_attach_files` response.
///
/// Returns the result for an `{"ok":true,...}` object; throws `StateError` carrying the Rust
/// error for `{"ok":false,...}` and `FormatException` for any other shape.
AttachFilesResult decodeAttachFilesResponse(String responseJson) {
  final Object? decodedResponse = jsonDecode(responseJson);
  if (decodedResponse is! Map) {
    throw const FormatException('Attach response must be a JSON object.');
  }
  final Map<String, Object?> json = decodedResponse.cast<String, Object?>();
  if (json['ok'] != true) {
    throw StateError('Rust rejected the attachments '
        '(${json['error_code'] ?? 'unknown_error'}): ${json['error'] ?? ''}');
  }
  return AttachFilesResult(
    attached: decodeMeetingAttachments(json['attached']),
    failedPaths: ((json['failed'] as List<Object?>?) ?? const <Object?>[])
        .cast<Map<Object?, Object?>>()
        .map((Map<Object?, Object?> failure) => failure['path']! as String)
        .toList(growable: false),
  );
}

/// Attaches files to meetings.
abstract interface class MeetingAttachmentBridge {
  /// Copies `filePaths` into the app's storage and links them to `meetingId`.
  ///
  /// Inputs: An existing meeting and a non-empty list of absolute paths.
  /// Outputs: What was attached and what failed; throws `StateError` when Rust rejects the
  /// whole request (for example, an unknown meeting).
  /// Side effects: Rust copies the files; while the meeting is being recorded they carry its
  /// current meeting time, otherwise they go at the end of the meeting. Sources are left.
  Future<AttachFilesResult> attachFiles(
      String meetingId, List<String> filePaths);
}

/// Calls `note_taker_attach_files` on a background isolate, since it copies files.
final class RustMeetingAttachmentBridge implements MeetingAttachmentBridge {
  /// Creates the bridge; the native library is opened inside each isolate.
  const RustMeetingAttachmentBridge();

  @override
  Future<AttachFilesResult> attachFiles(
          String meetingId, List<String> filePaths) async =>
      decodeAttachFilesResponse(
          await _attachFilesInIsolate(meetingId, jsonEncode(filePaths)));
}

/// Runs `note_taker_attach_files` and copies its response out, freeing every native buffer.
///
/// Declared at top level so the isolate closure captures only the two strings.
Future<String> _attachFilesInIsolate(String meetingId, String pathsJson) =>
    Isolate.run(() {
      final DynamicLibrary coreLibrary = _openCoreLibrary();
      final _DartAttachFiles attachFiles =
          coreLibrary.lookupFunction<_NativeAttachFiles, _DartAttachFiles>(
              'note_taker_attach_files');
      final _DartFreeString freeString =
          coreLibrary.lookupFunction<_NativeFreeString, _DartFreeString>(
              'note_taker_free_string');
      final Pointer<Utf8> nativeMeetingId = meetingId.toNativeUtf8();
      final Pointer<Utf8> nativePathsJson = pathsJson.toNativeUtf8();
      try {
        final Pointer<Utf8> responsePointer =
            attachFiles(nativeMeetingId, nativePathsJson);
        if (responsePointer == nullptr) {
          throw StateError('Rust returned no attach response.');
        }
        try {
          return responsePointer.toDartString();
        } finally {
          freeString(responsePointer);
        }
      } finally {
        calloc
          ..free(nativeMeetingId)
          ..free(nativePathsJson);
      }
    });

/// Opens the platform-specific core library.
DynamicLibrary _openCoreLibrary() {
  if (Platform.isAndroid || Platform.isLinux) {
    return DynamicLibrary.open('libnote_taker_core.so');
  }
  if (Platform.isWindows) {
    return DynamicLibrary.open('note_taker_core.dll');
  }
  throw UnsupportedError('The Rust attachment bridge is unavailable here.');
}

/// Picks several files to attach.
abstract interface class AttachmentFilesPicker {
  /// Opens the platform picker for the attachable types.
  ///
  /// Inputs: None.
  /// Outputs: The selected absolute paths; empty when the owner cancels.
  /// Side effects: Opens a platform-owned picker.
  Future<List<String>> pickFiles();
}

/// Uses the standard cross-platform picker with multiple selection.
final class PlatformAttachmentFilesPicker implements AttachmentFilesPicker {
  /// Creates the picker.
  const PlatformAttachmentFilesPicker();

  @override
  Future<List<String>> pickFiles() async {
    final FilePickerResult? selection = await FilePicker.platform.pickFiles(
      allowMultiple: true,
      type: FileType.custom,
      allowedExtensions: attachableExtensions,
    );
    return <String>[
      for (final PlatformFile file
          in selection?.files ?? const <PlatformFile>[])
        if (file.path case final String path) path,
    ];
  }
}

/// Represents `char *note_taker_attach_files(const char *meeting_id, const char *paths_json)`.
typedef _NativeAttachFiles = Pointer<Utf8> Function(
    Pointer<Utf8> meetingId, Pointer<Utf8> pathsJson);

/// Represents the Dart callable form of `note_taker_attach_files`.
typedef _DartAttachFiles = Pointer<Utf8> Function(
    Pointer<Utf8> meetingId, Pointer<Utf8> pathsJson);

/// Represents `void note_taker_free_string(char *value)`.
typedef _NativeFreeString = Void Function(Pointer<Utf8> value);

/// Represents the Dart callable form of `note_taker_free_string`.
typedef _DartFreeString = void Function(Pointer<Utf8> value);
