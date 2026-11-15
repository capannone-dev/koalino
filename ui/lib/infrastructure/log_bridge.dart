// Reaches the Rust core's log level and log export (`note_taker_get_log_level`,
// `note_taker_set_log_level`, `note_taker_export_logs`; Issue #46), and Android's share sheet for
// the exported file (`SharedFiles.kt` on `note_taker/share`).

import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:ffi/ffi.dart';
import 'package:flutter/services.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// The levels Settings offers, from least to most detail.
enum LogLevel {
  error('error'),
  warn('warn'),
  info('info'),
  debug('debug'),
  trace('trace');

  const LogLevel(this.directive);

  /// The `tracing` directive the core stores.
  final String directive;

  /// The name shown in Settings, in the app's language (#124).
  String get label => switch (this) {
        LogLevel.error => l10n.error,
        LogLevel.warn => l10n.warning,
        LogLevel.info => l10n.infoDefault,
        LogLevel.debug => l10n.debug,
        LogLevel.trace => l10n.trace,
      };

  /// Maps a stored directive to its level.
  ///
  /// Inputs: The core's directive. Outputs: The level, ignoring case and surrounding spaces, or
  /// `null` for a directive Settings does not offer (one set with `cli log-level`, for example).
  /// Side effects: None.
  static LogLevel? fromDirective(String directive) {
    final String normalized = directive.trim().toLowerCase();
    for (final LogLevel level in values) {
      if (level.directive == normalized) {
        return level;
      }
    }
    return null;
  }
}

/// What a log export produced.
sealed class LogExportResult {
  const LogExportResult();
}

/// The core wrote the logs to `filePath` (`sizeBytes` long).
final class LogExportSuccess extends LogExportResult {
  const LogExportSuccess({required this.filePath, required this.sizeBytes});

  final String filePath;
  final int sizeBytes;

  @override
  bool operator ==(Object other) =>
      other is LogExportSuccess &&
      other.filePath == filePath &&
      other.sizeBytes == sizeBytes;

  @override
  int get hashCode => Object.hash(filePath, sizeBytes);
}

/// The export failed; `errorCode` is the core's (`no_logs`, `export_failed`, …).
final class LogExportFailure extends LogExportResult {
  const LogExportFailure({required this.errorCode});

  final String errorCode;

  @override
  bool operator ==(Object other) =>
      other is LogExportFailure && other.errorCode == errorCode;

  @override
  int get hashCode => errorCode.hashCode;
}

/// Decodes the core's log export answer.
///
/// Inputs: The JSON returned by `note_taker_export_logs`, or `null` when there was none.
/// Outputs: A success with the file, or a failure with the core's error code
/// (`runtime_unavailable` without an answer, `invalid_response` for malformed JSON).
/// Side effects: None.
LogExportResult logExportResultFromJson(String? responseJson) {
  if (responseJson == null) {
    return const LogExportFailure(errorCode: 'runtime_unavailable');
  }
  final Object? decoded;
  try {
    decoded = jsonDecode(responseJson);
  } on FormatException {
    return const LogExportFailure(errorCode: 'invalid_response');
  }
  if (decoded is! Map<String, Object?>) {
    return const LogExportFailure(errorCode: 'invalid_response');
  }
  final Object? filePath = decoded['file_path'];
  final Object? sizeBytes = decoded['size_bytes'];
  if (decoded['ok'] == true && filePath is String && sizeBytes is int) {
    return LogExportSuccess(filePath: filePath, sizeBytes: sizeBytes);
  }
  final Object? errorCode = decoded['error_code'];
  return LogExportFailure(
      errorCode: errorCode is String ? errorCode : 'invalid_response');
}

/// The core's logging settings.
abstract interface class LogBridge {
  /// Reads the stored level.
  ///
  /// Outputs: The core's directive (`info` when none was chosen), or `null` when the core cannot
  /// be reached. Side effects: Reads the level file.
  Future<String?> currentLevel();

  /// Stores and applies a level.
  ///
  /// Inputs: A directive. Outputs: true when the core accepted it.
  /// Side effects: The core keeps it for the next start and logs at it from now on.
  Future<bool> setLevel(String directive);

  /// Writes the newest week of logs to one file.
  ///
  /// Inputs: An absolute file path, or `null` for the core's export folder.
  /// Outputs: The written file, or the failure. Side effects: Writes the file.
  Future<LogExportResult> exportLogs({String? outputPath});
}

/// Calls the core over FFI, on background isolates since the calls touch files.
final class RustLogBridge implements LogBridge {
  const RustLogBridge();

  @override
  Future<String?> currentLevel() => Isolate.run(_currentLevel);

  @override
  Future<bool> setLevel(String directive) =>
      Isolate.run(() => _setLevel(directive));

  @override
  Future<LogExportResult> exportLogs({String? outputPath}) async =>
      logExportResultFromJson(
          await Isolate.run(() => _exportLogsJson(outputPath)));
}

/// Hands an exported file to another app.
abstract interface class LogSharer {
  /// Opens the system share sheet for `filePath`.
  ///
  /// Throws when the sheet cannot be shown. Side effects: Grants the chosen app read access to
  /// that one file.
  Future<void> share(String filePath);
}

/// Shares through `SharedFiles.kt`, which serves only files in the app's `cache/shared` folder.
final class AndroidLogSharer implements LogSharer {
  const AndroidLogSharer();

  static const MethodChannel _channel = MethodChannel('note_taker/share');

  @override
  Future<void> share(String filePath) => _channel.invokeMethod<void>(
      'shareFile',
      <String, String>{'path': filePath, 'mimeType': 'text/plain'});
}

/// Reads the stored level; runs inside an isolate.
String? _currentLevel() {
  final DynamicLibrary coreLibrary = _openCoreLibrary();
  final Pointer<Utf8> level =
      coreLibrary.lookupFunction<_NativeGetLogLevel, _NativeGetLogLevel>(
          'note_taker_get_log_level')();
  return _takeString(coreLibrary, level);
}

/// Stores a level; runs inside an isolate.
bool _setLevel(String directive) {
  final _DartSetLogLevel setLogLevel = _openCoreLibrary()
      .lookupFunction<_NativeSetLogLevel, _DartSetLogLevel>(
          'note_taker_set_log_level');
  final Pointer<Utf8> nativeDirective = directive.toNativeUtf8();
  try {
    return setLogLevel(nativeDirective) == 0;
  } finally {
    calloc.free(nativeDirective);
  }
}

/// Exports the logs; runs inside an isolate.
String? _exportLogsJson(String? outputPath) {
  final DynamicLibrary coreLibrary = _openCoreLibrary();
  final _NativeExportLogs exportLogs =
      coreLibrary.lookupFunction<_NativeExportLogs, _NativeExportLogs>(
          'note_taker_export_logs');
  final Pointer<Utf8> nativePath =
      outputPath == null ? nullptr : outputPath.toNativeUtf8();
  try {
    return _takeString(coreLibrary, exportLogs(nativePath));
  } finally {
    if (nativePath != nullptr) {
      calloc.free(nativePath);
    }
  }
}

/// Copies a core-owned string into Dart and frees it.
///
/// Outputs: The string, or `null` for a null pointer. Side effects: Frees `value`.
String? _takeString(DynamicLibrary coreLibrary, Pointer<Utf8> value) {
  if (value == nullptr) {
    return null;
  }
  try {
    return value.toDartString();
  } finally {
    coreLibrary.lookupFunction<_NativeFreeString, _DartFreeString>(
        'note_taker_free_string')(value);
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
  throw UnsupportedError('The Rust log bridge is unavailable here.');
}

/// Represents `char *note_taker_get_log_level(void)`.
typedef _NativeGetLogLevel = Pointer<Utf8> Function();

/// Represents `int32_t note_taker_set_log_level(const char *)`.
typedef _NativeSetLogLevel = Int32 Function(Pointer<Utf8> directive);

/// Represents the Dart callable form of `note_taker_set_log_level`.
typedef _DartSetLogLevel = int Function(Pointer<Utf8> directive);

/// Represents `char *note_taker_export_logs(const char *)`.
typedef _NativeExportLogs = Pointer<Utf8> Function(Pointer<Utf8> outputPath);

/// Represents `void note_taker_free_string(char *)`.
typedef _NativeFreeString = Void Function(Pointer<Utf8> value);

/// Represents the Dart callable form of `note_taker_free_string`.
typedef _DartFreeString = void Function(Pointer<Utf8> value);
