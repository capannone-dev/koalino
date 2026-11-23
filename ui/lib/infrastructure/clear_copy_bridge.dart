// Gives the app's viewers clear copies of sealed attachments (#99).
//
// Attachment files (meeting photos and files, project resources) are sealed at rest; only the
// core can read them. Flutter's image, text and PDF views read files, so before showing one the
// app asks `note_taker_clear_copy` for a clear copy in the core's private `clear-copies` folder.
// The core removes copies 30 minutes after it made them and whenever the app starts; this side
// remembers a copy for less than that, so it never hands out one that is gone. Decrypting runs
// on a background isolate. Paths and contents are never logged.

import 'dart:convert';
import 'dart:ffi';
import 'dart:isolate';

import 'package:ffi/ffi.dart';

import 'core_library.dart';

/// Gives the path of a clear copy of one attachment or project resource.
///
/// Throws `ClearCopyException` when there is none.
typedef ClearCopyResolver = Future<String> Function(String attachmentId);

/// Why a clear copy could not be made.
final class ClearCopyException implements Exception {
  /// Creates the exception with the core's stable code.
  const ClearCopyException(this.code);

  /// `not_found` (the file has not arrived), `unreadable`, `invalid_argument`,
  /// `storage_failed`, `runtime_unavailable` or `bad_response`.
  final String code;

  @override
  String toString() => 'ClearCopyException($code)';
}

/// How long a copy is reused here: well under the core's 30 minutes.
const Duration _rememberedFor = Duration(minutes: 20);

/// Copies asked for recently, by attachment ID.
final Map<String, ({Future<String> path, DateTime at})> _recent =
    <String, ({Future<String> path, DateTime at})>{};

/// The resolver the viewers use; tests replace it.
ClearCopyResolver clearCopyResolver = rememberedClearCopy;

/// Asks the core for a clear copy, reusing one asked for in the last 20 minutes.
///
/// Inputs: The attachment ID. Outputs: The copy's path; throws `ClearCopyException`.
/// Side effects: The core decrypts the file into its `clear-copies` folder.
Future<String> rememberedClearCopy(String attachmentId) {
  final DateTime now = DateTime.now();
  _recent.removeWhere((_, ({Future<String> path, DateTime at}) copy) =>
      now.difference(copy.at) > _rememberedFor);
  final ({Future<String> path, DateTime at})? known = _recent[attachmentId];
  if (known != null) {
    return known.path;
  }
  final Future<String> path = rustClearCopy(attachmentId);
  _recent[attachmentId] = (path: path, at: now);
  // A failure is not remembered, so the next view asks again (a file may arrive meanwhile).
  path.catchError((Object _) {
    _recent.remove(attachmentId);
    return '';
  });
  return path;
}

/// Reads the core's answer.
///
/// Outputs: The path; throws `ClearCopyException` with the core's code, or `bad_response`.
String decodeClearCopyResponse(String responseJson) {
  final Object? decoded = jsonDecode(responseJson);
  if (decoded is! Map) {
    throw const ClearCopyException('bad_response');
  }
  final Object? path = decoded['path'];
  if (decoded['ok'] == true && path is String && path.isNotEmpty) {
    return path;
  }
  final Object? code = decoded['error_code'];
  throw ClearCopyException(code is String ? code : 'bad_response');
}

/// Calls `note_taker_clear_copy` on a background isolate.
///
/// Inputs: The attachment ID. Outputs: The copy's path; throws `ClearCopyException`.
/// Side effects: The core decrypts the file into its `clear-copies` folder.
Future<String> rustClearCopy(String attachmentId) async =>
    decodeClearCopyResponse(await _clearCopyInIsolate(attachmentId));

/// Runs `note_taker_clear_copy` and copies its answer out, freeing every native buffer.
///
/// Declared at top level so the isolate closure captures only the ID.
Future<String> _clearCopyInIsolate(String attachmentId) => Isolate.run(() {
      final DynamicLibrary coreLibrary = _openCoreLibrary();
      final _DartClearCopy clearCopy =
          coreLibrary.lookupFunction<_NativeClearCopy, _DartClearCopy>(
              'note_taker_clear_copy');
      final _DartFreeString freeString =
          coreLibrary.lookupFunction<_NativeFreeString, _DartFreeString>(
              'note_taker_free_string');
      final Pointer<Utf8> nativeId = attachmentId.toNativeUtf8();
      try {
        final Pointer<Utf8> answer = clearCopy(nativeId);
        if (answer == nullptr) {
          return '{}';
        }
        try {
          return answer.toDartString();
        } finally {
          freeString(answer);
        }
      } finally {
        calloc.free(nativeId);
      }
    });

/// Opens the platform-specific core library.
DynamicLibrary _openCoreLibrary() {
  try {
    return openCoreLibrary();
  } on UnsupportedError {
    throw const ClearCopyException('runtime_unavailable');
  }
}

typedef _NativeClearCopy = Pointer<Utf8> Function(Pointer<Utf8> attachmentId);
typedef _DartClearCopy = Pointer<Utf8> Function(Pointer<Utf8> attachmentId);
typedef _NativeFreeString = Void Function(Pointer<Utf8> value);
typedef _DartFreeString = void Function(Pointer<Utf8> value);
