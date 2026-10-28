// Exports a meeting through the Rust core (`note_taker_export_meeting`, Issue #48) and saves the
// result where the owner chooses. On desktops the save dialog only picks a path and the core
// writes there; on Android and iOS the dialog (the system document picker) takes the bytes.

import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';
import 'package:file_picker/file_picker.dart';
import 'package:path_provider/path_provider.dart';

/// The formats the core exports.
enum MeetingExportFormat {
  /// A Markdown document (attachments copied into a folder beside it).
  markdown('markdown', 'md', 'Markdown'),

  /// A PDF document with embedded images.
  pdf('pdf', 'pdf', 'PDF'),

  /// Machine-readable JSON.
  json('json', 'json', 'JSON');

  const MeetingExportFormat(this.coreName, this.fileExtension, this.label);

  /// The `format` argument of `note_taker_export_meeting`.
  final String coreName;

  /// The file extension, without the dot.
  final String fileExtension;

  /// The name shown in the Export menu.
  final String label;
}

/// What an export produced.
sealed class MeetingExportResult {
  const MeetingExportResult();
}

/// The core wrote `filePath` (`sizeBytes` long).
final class MeetingExportSuccess extends MeetingExportResult {
  const MeetingExportSuccess({required this.filePath, required this.sizeBytes});

  final String filePath;
  final int sizeBytes;

  @override
  bool operator ==(Object other) =>
      other is MeetingExportSuccess &&
      other.filePath == filePath &&
      other.sizeBytes == sizeBytes;

  @override
  int get hashCode => Object.hash(filePath, sizeBytes);
}

/// The export failed; `errorCode` is the core's (`meeting_not_found`, `export_failed`, …).
final class MeetingExportFailure extends MeetingExportResult {
  const MeetingExportFailure({required this.errorCode});

  final String errorCode;

  @override
  bool operator ==(Object other) =>
      other is MeetingExportFailure && other.errorCode == errorCode;

  @override
  int get hashCode => errorCode.hashCode;
}

/// Decodes the core's export answer.
///
/// Inputs: The JSON returned by `note_taker_export_meeting`, or `null` when there was none.
/// Outputs: A success with the file, or a failure with the core's error code
/// (`runtime_unavailable` without an answer, `invalid_response` for malformed JSON).
/// Side effects: None.
MeetingExportResult meetingExportResultFromJson(String? responseJson) {
  if (responseJson == null) {
    return const MeetingExportFailure(errorCode: 'runtime_unavailable');
  }
  final Object? decoded;
  try {
    decoded = jsonDecode(responseJson);
  } on FormatException {
    return const MeetingExportFailure(errorCode: 'invalid_response');
  }
  if (decoded is! Map<String, Object?>) {
    return const MeetingExportFailure(errorCode: 'invalid_response');
  }
  final Object? filePath = decoded['file_path'];
  final Object? sizeBytes = decoded['size_bytes'];
  if (decoded['ok'] == true && filePath is String && sizeBytes is int) {
    return MeetingExportSuccess(filePath: filePath, sizeBytes: sizeBytes);
  }
  final Object? errorCode = decoded['error_code'];
  return MeetingExportFailure(
      errorCode: errorCode is String ? errorCode : 'invalid_response');
}

/// Exports meetings through the core.
abstract interface class ExportBridge {
  /// Exports `meetingId` as `format`.
  ///
  /// Inputs: The meeting, the format, where to write (`null`: the core's default export folder),
  /// and whether to include attachments.
  /// Outputs: The written file, or the failure.
  /// Side effects: Writes the export file (and, for Markdown, an attachments folder).
  Future<MeetingExportResult> exportMeeting(
    String meetingId,
    MeetingExportFormat format, {
    String? outputPath,
    bool includeAttachments = true,
  });
}

/// Calls `note_taker_export_meeting` on a background isolate, since it writes synchronously.
final class RustExportBridge implements ExportBridge {
  const RustExportBridge();

  @override
  Future<MeetingExportResult> exportMeeting(
    String meetingId,
    MeetingExportFormat format, {
    String? outputPath,
    bool includeAttachments = true,
  }) async {
    final String optionsJson = jsonEncode(<String, Object>{
      'include_attachments': includeAttachments,
      if (outputPath != null) 'output_path': outputPath,
    });
    final String coreFormat = format.coreName;
    final String? responseJson = await Isolate.run(
        () => _exportMeetingJson(meetingId, coreFormat, optionsJson));
    return meetingExportResultFromJson(responseJson);
  }
}

/// Saves an export where the owner chooses.
abstract interface class ExportSaver {
  /// Whether the dialog takes the file's bytes (Android, iOS) instead of returning a path.
  bool get savesBytes;

  /// Asks where to save on a desktop.
  ///
  /// Inputs: The suggested file name. Outputs: The chosen path, or `null` when cancelled.
  /// Throws when no dialog can be shown (for example no zenity or kdialog on Linux).
  Future<String?> chooseSavePath(String suggestedFileName);

  /// Saves `bytes` through the system document picker on a phone.
  ///
  /// Outputs: true when saved, false when cancelled. Side effects: Writes the chosen document.
  Future<bool> saveBytes(String suggestedFileName, Uint8List bytes);
}

/// The save dialogs of `file_picker`.
final class FilePickerExportSaver implements ExportSaver {
  const FilePickerExportSaver();

  @override
  bool get savesBytes => Platform.isAndroid || Platform.isIOS;

  /// Opens the desktop save dialog in the owner's Downloads folder.
  ///
  /// Without a start folder, Windows opens the process's working directory (System32 when
  /// launched by some shells). Side effects: Shows a modal dialog over the app window.
  @override
  Future<String?> chooseSavePath(String suggestedFileName) async =>
      FilePicker.platform.saveFile(
          dialogTitle: 'Export meeting',
          fileName: suggestedFileName,
          initialDirectory: await _downloadsDirectoryPath(),
          lockParentWindow: true);

  @override
  Future<bool> saveBytes(String suggestedFileName, Uint8List bytes) async =>
      await FilePicker.platform.saveFile(
          dialogTitle: 'Export meeting',
          fileName: suggestedFileName,
          bytes: bytes) !=
      null;
}

/// The Downloads folder's path, or `null` (the dialog's own default) when the platform has none.
///
/// Side effects: None; a failed lookup is treated as no folder.
Future<String?> _downloadsDirectoryPath() async {
  try {
    return (await getDownloadsDirectory())?.path;
  } on Exception {
    return null;
  }
}

/// Calls the core and copies its answer out; runs inside the export isolate.
///
/// Outputs: The answer JSON, or `null` when the core returned none.
/// Side effects: The core writes the export; native strings are freed here.
String? _exportMeetingJson(
    String meetingId, String format, String optionsJson) {
  final DynamicLibrary coreLibrary = _openCoreLibrary();
  final _DartExportMeeting exportMeeting =
      coreLibrary.lookupFunction<_NativeExportMeeting, _DartExportMeeting>(
          'note_taker_export_meeting');
  final _DartFreeString freeString =
      coreLibrary.lookupFunction<_NativeFreeString, _DartFreeString>(
          'note_taker_free_string');
  final Pointer<Utf8> nativeMeetingId = meetingId.toNativeUtf8();
  final Pointer<Utf8> nativeFormat = format.toNativeUtf8();
  final Pointer<Utf8> nativeOptions = optionsJson.toNativeUtf8();
  try {
    final Pointer<Utf8> response =
        exportMeeting(nativeMeetingId, nativeFormat, nativeOptions);
    if (response == nullptr) {
      return null;
    }
    try {
      return response.toDartString();
    } finally {
      freeString(response);
    }
  } finally {
    calloc
      ..free(nativeMeetingId)
      ..free(nativeFormat)
      ..free(nativeOptions);
  }
}

/// Opens the platform's core library.
DynamicLibrary _openCoreLibrary() {
  if (Platform.isAndroid || Platform.isLinux) {
    return DynamicLibrary.open('libnote_taker_core.so');
  }
  if (Platform.isWindows) {
    return DynamicLibrary.open('note_taker_core.dll');
  }
  throw UnsupportedError('The Rust export bridge is unavailable here.');
}

/// Represents `char *note_taker_export_meeting(const char *, const char *, const char *)`.
typedef _NativeExportMeeting = Pointer<Utf8> Function(
    Pointer<Utf8> meetingId, Pointer<Utf8> format, Pointer<Utf8> options);

/// Represents the Dart callable form of `note_taker_export_meeting`.
typedef _DartExportMeeting = Pointer<Utf8> Function(
    Pointer<Utf8> meetingId, Pointer<Utf8> format, Pointer<Utf8> options);

/// Represents `void note_taker_free_string(char *)`.
typedef _NativeFreeString = Void Function(Pointer<Utf8> value);

/// Represents the Dart callable form of `note_taker_free_string`.
typedef _DartFreeString = void Function(Pointer<Utf8> value);
