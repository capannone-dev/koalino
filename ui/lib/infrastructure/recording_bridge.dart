// Defines the Dart FFI boundary through which Flutter invokes the Rust recording core.

import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:ffi/ffi.dart';

/// Declares Flutter operations supported by the native recording boundary.
abstract interface class RecordingBridge {
  /// Requests a recording start through the Rust core, or resumes a paused recording.
  ///
  /// Inputs: None.
  /// Outputs: `true` when Rust accepted the request; otherwise `false`.
  /// Side effects: Resumes a paused or interrupted recording into the same meeting, or starts a
  /// new untitled one.
  Future<bool> startRecording();

  /// Pauses the active recording; audio pushed while paused is discarded.
  ///
  /// Inputs: None.
  /// Outputs: `true` when Rust accepted the request; otherwise `false`.
  /// Side effects: Transitions the Rust-owned recording to paused.
  Future<bool> pauseRecording();

  /// Stops the active or paused recording.
  ///
  /// Inputs: None.
  /// Outputs: `true` once Rust has transcribed and persisted every chunk; otherwise `false`.
  /// Side effects: Flushes buffered audio, waits for transcription (this can take as long as the
  /// backlog), and deletes unprocessed raw audio.
  Future<bool> stopRecording();

  /// Requests a recording start with an optional meeting title through the Rust core.
  ///
  /// Inputs: `title` is the meeting title, or `null` (or blank) for an untitled meeting, which
  /// Rust shows as `Meeting YYYY-MM-DD HH:MM UTC`.
  /// Outputs: `true` when Rust accepted the request; otherwise `false`, including for a title
  /// Rust rejects (over 200 characters or containing control characters).
  /// Side effects: May start a Rust-owned recording into a new meeting and store its title.
  Future<bool> startRecordingWithTitle(String? title);

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
    _setLogLevel =
        _dynamicLibrary.lookupFunction<_NativeSetLogLevel, _DartSetLogLevel>(
      'note_taker_set_log_level',
    );
  }

  final DynamicLibrary _dynamicLibrary;
  late final _DartSetLogLevel _setLogLevel;

  /// Requests a start, or a resume of a paused recording, through the C ABI.
  ///
  /// Inputs: None.
  /// Outputs: `true` only for the Rust success status code.
  /// Side effects: Invokes `note_taker_start_recording` on a background isolate, because the
  /// first start composes the meeting pipeline and may load a model.
  @override
  Future<bool> startRecording() async =>
      await _runLifecycleCommand('note_taker_start_recording') == _ffiSuccess;

  /// Requests a pause through the C ABI.
  ///
  /// Inputs: None.
  /// Outputs: `true` only for the Rust success status code.
  /// Side effects: Invokes `note_taker_pause_recording` on a background isolate.
  @override
  Future<bool> pauseRecording() async =>
      await _runLifecycleCommand('note_taker_pause_recording') == _ffiSuccess;

  /// Requests a stop through the C ABI.
  ///
  /// Inputs: None.
  /// Outputs: `true` only for the Rust success status code.
  /// Side effects: Invokes `note_taker_stop_recording` on a background isolate, because it
  /// returns only after every buffered chunk is transcribed and persisted.
  @override
  Future<bool> stopRecording() async =>
      await _runLifecycleCommand('note_taker_stop_recording') == _ffiSuccess;

  /// Requests a titled recording start through the C ABI.
  ///
  /// Inputs: `title` is trimmed; a null or blank title is sent as a null pointer, because Rust
  /// rejects an empty string. The meeting ID is always null, so Rust generates one.
  /// Outputs: `true` only for the Rust success status code.
  /// Side effects: Allocates a temporary UTF-8 buffer and invokes
  /// `note_taker_start_recording_with_title`, which may load a model on first use.
  @override
  Future<bool> startRecordingWithTitle(String? title) async {
    final String? trimmedTitle = title?.trim();
    final String? requestedTitle =
        trimmedTitle == null || trimmedTitle.isEmpty ? null : trimmedTitle;
    return await _startTitledRecording(requestedTitle) == _ffiSuccess;
  }

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

/// Calls a no-argument lifecycle function (`note_taker_start_recording`, `..._pause_recording`,
/// or `..._stop_recording`) on a background isolate.
///
/// Inputs: The C symbol name. Outputs: Its status code.
/// Side effects: Opens the core library inside the isolate (a `DynamicLibrary` cannot cross
/// isolates; the process has it loaded already) and runs the Rust transition, which may block
/// for model loading (start) or transcription (stop) without freezing the UI isolate.
Future<int> _runLifecycleCommand(String symbolName) =>
    Isolate.run(() => RustRecordingBridge._openCoreLibrary()
        .lookupFunction<_NativeLifecycleCommand, _DartLifecycleCommand>(
            symbolName)());

/// Calls `note_taker_start_recording_with_title` with a null meeting ID on a background isolate.
///
/// Inputs: The trimmed title, or `null` for an untitled meeting (Rust rejects an empty string).
/// Outputs: The status code.
/// Side effects: Allocates and frees a temporary UTF-8 buffer inside the isolate.
Future<int> _startTitledRecording(String? title) => Isolate.run(() {
      final _DartStartRecordingWithTitle startRecordingWithTitle =
          RustRecordingBridge._openCoreLibrary().lookupFunction<
                  _NativeStartRecordingWithTitle, _DartStartRecordingWithTitle>(
              'note_taker_start_recording_with_title');
      if (title == null) {
        return startRecordingWithTitle(nullptr, nullptr);
      }
      final Pointer<Utf8> nativeTitle = title.toNativeUtf8();
      try {
        return startRecordingWithTitle(nullptr, nativeTitle);
      } finally {
        calloc.free(nativeTitle);
      }
    });

/// Represents `int32_t note_taker_{start,pause,stop}_recording(void)` in the Rust C ABI.
typedef _NativeLifecycleCommand = Int32 Function();

/// Represents the Dart callable form of the lifecycle functions.
typedef _DartLifecycleCommand = int Function();

/// Represents `int32_t note_taker_start_recording_with_title(const char *, const char *)`.
typedef _NativeStartRecordingWithTitle = Int32 Function(
    Pointer<Utf8> meetingId, Pointer<Utf8> title);

/// Represents the Dart callable form of `note_taker_start_recording_with_title`.
typedef _DartStartRecordingWithTitle = int Function(
    Pointer<Utf8> meetingId, Pointer<Utf8> title);

/// Represents `int32_t note_taker_set_log_level(const char *)` in the Rust C ABI.
typedef _NativeSetLogLevel = Int32 Function(Pointer<Utf8> filterDirective);

/// Represents the Dart callable form of `note_taker_set_log_level`.
typedef _DartSetLogLevel = int Function(Pointer<Utf8> filterDirective);

/// Matches the success status documented by the Rust FFI contract.
const int _ffiSuccess = 0;
