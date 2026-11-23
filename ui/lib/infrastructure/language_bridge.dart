// Reaches the Rust core's transcription language (`note_taker_get_default_language`,
// `note_taker_set_default_language`, `note_taker_set_recording_language`; Issue #62).

import 'dart:ffi';
import 'dart:isolate';

import 'package:ffi/ffi.dart';

import 'core_library.dart';

/// The core's language settings. Codes are `auto` or a Whisper language code.
abstract interface class LanguageBridge {
  /// Reads the default language.
  ///
  /// Outputs: The code, or `null` when the core cannot be reached. Side effects: Reads a file.
  Future<String?> defaultLanguage();

  /// Stores the default language.
  ///
  /// Inputs: A code. Outputs: true when the core stored it. Side effects: Writes a file.
  Future<bool> setDefaultLanguage(String language);

  /// Chooses the language of the next recording, or of the rest of the current one.
  ///
  /// Inputs: A code. Outputs: true when the core accepted it.
  /// Side effects: The core applies it to audio from now on, until the recording stops.
  Future<bool> setRecordingLanguage(String language);
}

/// Calls the core over FFI, on background isolates since the calls touch files and a lock the
/// recording start can hold while it loads a model.
final class RustLanguageBridge implements LanguageBridge {
  const RustLanguageBridge();

  @override
  Future<String?> defaultLanguage() => Isolate.run(_defaultLanguage);

  @override
  Future<bool> setDefaultLanguage(String language) => Isolate.run(
      () => _setLanguage('note_taker_set_default_language', language));

  @override
  Future<bool> setRecordingLanguage(String language) => Isolate.run(
      () => _setLanguage('note_taker_set_recording_language', language));
}

/// Reads the default; runs inside an isolate.
String? _defaultLanguage() {
  final DynamicLibrary coreLibrary = _openCoreLibrary();
  final Pointer<Utf8> language =
      coreLibrary.lookupFunction<_NativeGetLanguage, _NativeGetLanguage>(
          'note_taker_get_default_language')();
  if (language == nullptr) {
    return null;
  }
  try {
    return language.toDartString();
  } finally {
    coreLibrary.lookupFunction<_NativeFreeString, _DartFreeString>(
        'note_taker_free_string')(language);
  }
}

/// Calls one of the setters, which return 0 on success; runs inside an isolate.
bool _setLanguage(String functionName, String language) {
  final _DartSetLanguage setLanguage = _openCoreLibrary()
      .lookupFunction<_NativeSetLanguage, _DartSetLanguage>(functionName);
  final Pointer<Utf8> nativeLanguage = language.toNativeUtf8();
  try {
    return setLanguage(nativeLanguage) == 0;
  } finally {
    calloc.free(nativeLanguage);
  }
}

/// Opens the platform's core library.
DynamicLibrary _openCoreLibrary() => openCoreLibrary();

/// Represents `char *note_taker_get_default_language(void)`.
typedef _NativeGetLanguage = Pointer<Utf8> Function();

/// Represents `int32_t note_taker_set_*_language(const char *)`.
typedef _NativeSetLanguage = Int32 Function(Pointer<Utf8> language);

/// Represents the Dart callable form of the setters.
typedef _DartSetLanguage = int Function(Pointer<Utf8> language);

/// Represents `void note_taker_free_string(char *)`.
typedef _NativeFreeString = Void Function(Pointer<Utf8> value);

/// Represents the Dart callable form of `note_taker_free_string`.
typedef _DartFreeString = void Function(Pointer<Utf8> value);
