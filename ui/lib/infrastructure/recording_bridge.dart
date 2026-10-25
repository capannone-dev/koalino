// Defines the Dart FFI boundary through which Flutter invokes the Rust recording core.

import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';

/// Declares Flutter operations supported by the native recording boundary.
abstract interface class RecordingBridge {
  /// Requests a recording start through the Rust core.
  ///
  /// Inputs: None.
  /// Outputs: `true` when Rust accepted the request; otherwise `false`.
  /// Side effects: May transition a Rust-owned audio session to recording.
  Future<bool> startRecording();

  /// Sets the Rust tracing filter without exposing Rust logging types to Flutter.
  ///
  /// Inputs: `filterDirective` must be a valid, non-empty tracing filter directive.
  /// Outputs: `true` when Rust accepted the filter; otherwise `false`.
  /// Side effects: Changes process logging verbosity for subsequent Rust events.
  Future<bool> setLogLevel(String filterDirective);
}

/// Invokes the C-compatible Rust symbols used by the Flutter application.
final class RustRecordingBridge implements RecordingBridge {
  /// Resolves the recording symbols from the provided library or platform library name.
  ///
  /// Inputs: `dynamicLibrary` optionally supplies a pre-opened native library for embedding tests.
  /// Outputs: A bridge ready to invoke Rust C ABI functions.
  /// Side effects: Opens the platform native library when `dynamicLibrary` is omitted.
  RustRecordingBridge({DynamicLibrary? dynamicLibrary})
      : _dynamicLibrary = dynamicLibrary ?? _openCoreLibrary() {
    _startRecording = _dynamicLibrary
        .lookupFunction<_NativeStartRecording, _DartStartRecording>(
      'note_taker_start_recording',
    );
    _setLogLevel =
        _dynamicLibrary.lookupFunction<_NativeSetLogLevel, _DartSetLogLevel>(
      'note_taker_set_log_level',
    );
  }

  final DynamicLibrary _dynamicLibrary;
  late final _DartStartRecording _startRecording;
  late final _DartSetLogLevel _setLogLevel;

  /// Requests a recording lifecycle transition through the C ABI.
  ///
  /// Inputs: None.
  /// Outputs: `true` only for the Rust success status code.
  /// Side effects: Invokes `note_taker_start_recording` in the loaded Rust library.
  @override
  Future<bool> startRecording() async => _startRecording() == _ffiSuccess;

  /// Requests a tracing filter change through the C ABI.
  ///
  /// Inputs: `filterDirective` must not be empty and is encoded as UTF-8 for Rust.
  /// Outputs: `true` only for the Rust success status code.
  /// Side effects: Allocates a temporary UTF-8 buffer and invokes the Rust logging bridge.
  @override
  Future<bool> setLogLevel(String filterDirective) async {
    if (filterDirective.isEmpty) {
      return false;
    }

    final Pointer<Utf8> nativeFilterDirective = filterDirective.toNativeUtf8();
    try {
      return _setLogLevel(nativeFilterDirective) == _ffiSuccess;
    } finally {
      calloc.free(nativeFilterDirective);
    }
  }

  /// Opens the platform-specific dynamic library produced by the Rust core crate.
  ///
  /// Inputs: Reads the current runtime platform.
  /// Outputs: A loaded Rust core library.
  /// Side effects: Loads native executable code into the Flutter process.
  static DynamicLibrary _openCoreLibrary() {
    if (Platform.isAndroid || Platform.isLinux) {
      return DynamicLibrary.open('libnote_taker_core.so');
    }
    if (Platform.isWindows) {
      return DynamicLibrary.open('note_taker_core.dll');
    }
    throw UnsupportedError(
        'The Rust recording bridge is not configured for this platform.');
  }
}

/// Represents `int32_t note_taker_start_recording(void)` in the Rust C ABI.
typedef _NativeStartRecording = Int32 Function();

/// Represents the Dart callable form of `note_taker_start_recording`.
typedef _DartStartRecording = int Function();

/// Represents `int32_t note_taker_set_log_level(const char *)` in the Rust C ABI.
typedef _NativeSetLogLevel = Int32 Function(Pointer<Utf8> filterDirective);

/// Represents the Dart callable form of `note_taker_set_log_level`.
typedef _DartSetLogLevel = int Function(Pointer<Utf8> filterDirective);

/// Matches the success status documented by the Rust FFI contract.
const int _ffiSuccess = 0;
