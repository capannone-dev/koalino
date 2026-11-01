// Reaches the Rust core's Whisper model and OpenAI settings (Issue #67). The core downloads,
// verifies and stores everything; the OpenAI key is passed in once and never read back.

import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:ffi/ffi.dart';

/// Core status: done.
const int transcriptionStatusOk = 0;

/// Core status: the keychain is missing, so the key was not saved.
const int transcriptionStatusNoKeychain = 3;

/// Core status: OpenAI was chosen with no key.
const int transcriptionStatusNoKey = 4;

/// Core status: another download is running.
const int transcriptionStatusBusy = 5;

/// Core status: the file won't fit on disk.
const int transcriptionStatusNoSpace = 6;

/// One Whisper model in the list.
final class WhisperModelInfo {
  /// Creates a row.
  const WhisperModelInfo({
    required this.fileName,
    required this.name,
    required this.family,
    required this.sizeBytes,
    required this.englishOnly,
    required this.onDevice,
    required this.removable,
    required this.tooLargeForMemory,
  });

  /// Reads a row from the core's JSON.
  factory WhisperModelInfo.fromJson(Map<String, Object?> json) =>
      WhisperModelInfo(
        fileName: json['file_name']! as String,
        name: json['name']! as String,
        family: json['family']! as String,
        sizeBytes: json['size_bytes']! as int,
        englishOnly: json['english_only']! as bool,
        onDevice: json['on_device']! as bool,
        removable: json['removable']! as bool,
        tooLargeForMemory: json['too_large_for_memory']! as bool,
      );

  final String fileName;
  final String name;
  final String family;
  final int sizeBytes;
  final bool englishOnly;
  final bool onDevice;
  final bool removable;
  final bool tooLargeForMemory;
}

/// The model the core suggests for this device.
final class WhisperSuggestion {
  /// Creates the suggestion.
  const WhisperSuggestion({
    required this.fileName,
    required this.efficient,
    required this.reason,
  });

  /// Reads the suggestion from the core's JSON.
  factory WhisperSuggestion.fromJson(Map<String, Object?> json) =>
      WhisperSuggestion(
        fileName: json['file_name']! as String,
        efficient: json['efficient']! as bool,
        reason: json['reason']! as String,
      );

  final String fileName;

  /// Whether the suggestion includes efficient mode.
  final bool efficient;

  /// Why, in plain language.
  final String reason;
}

/// Efficient mode's state on this device.
final class EfficientModeStatus {
  /// Creates the state; by default the device has no GPU for it.
  const EfficientModeStatus({
    this.gpuName,
    this.enabled = false,
    this.gpuBlocked = false,
    this.gpuInUse = false,
  });

  /// Reads the state from the core's JSON.
  factory EfficientModeStatus.fromJson(Map<String, Object?> json) =>
      EfficientModeStatus(
        gpuName: json['gpu_name'] as String?,
        enabled: json['enabled']! as bool,
        gpuBlocked: json['gpu_blocked']! as bool,
        gpuInUse: json['gpu_in_use']! as bool,
      );

  /// The supported GPU's name, or `null` when this device has none.
  final String? gpuName;

  /// Whether the owner turned efficient mode on.
  final bool enabled;

  /// Whether a crash during GPU work moved transcription to the processor.
  final bool gpuBlocked;

  /// Whether the latest loaded model runs on a GPU.
  final bool gpuInUse;
}

/// What the core reports about the current or last download.
final class WhisperDownload {
  /// Creates the report.
  const WhisperDownload({
    this.fileName,
    this.receivedBytes = 0,
    this.totalBytes = 0,
    this.state = '',
    this.error,
  });

  final String? fileName;
  final int receivedBytes;
  final int totalBytes;

  /// `downloading`, `done`, `cancelled`, `failed`, or empty before any download.
  final String state;

  /// `no_network`, `server`, `corrupted` or `storage` when failed.
  final String? error;
}

/// Everything Settings shows. It holds whether a key exists, never the key.
final class TranscriptionSettings {
  /// Creates the view.
  const TranscriptionSettings({
    required this.provider,
    required this.openAiModel,
    required this.keySaved,
    required this.environmentKey,
    required this.keychainAvailable,
    required this.selectedModel,
    required this.availableDiskBytes,
    required this.usingLocalFallback,
    required this.models,
    required this.download,
    this.suggestion,
    this.efficient = const EfficientModeStatus(),
  });

  /// Reads the view from the core's JSON.
  factory TranscriptionSettings.fromJson(Map<String, Object?> json) {
    final Map<String, Object?> download =
        json['download']! as Map<String, Object?>;
    return TranscriptionSettings(
      provider: json['provider']! as String,
      suggestion: json['suggestion'] == null
          ? null
          : WhisperSuggestion.fromJson(
              json['suggestion']! as Map<String, Object?>),
      efficient: json['efficient'] == null
          ? const EfficientModeStatus()
          : EfficientModeStatus.fromJson(
              json['efficient']! as Map<String, Object?>),
      openAiModel: json['openai_model']! as String,
      keySaved: json['key_saved']! as bool,
      environmentKey: json['environment_key']! as bool,
      keychainAvailable: json['keychain_available']! as bool,
      selectedModel: json['selected_model'] as String?,
      availableDiskBytes: json['available_disk_bytes'] as int?,
      usingLocalFallback: json['using_local_fallback']! as bool,
      models: (json['models']! as List<Object?>)
          .map((Object? row) =>
              WhisperModelInfo.fromJson(row! as Map<String, Object?>))
          .toList(growable: false),
      download: WhisperDownload(
        fileName: download['file_name'] as String?,
        receivedBytes: download['received_bytes']! as int,
        totalBytes: download['total_bytes']! as int,
        state: download['state']! as String,
        error: download['error'] as String?,
      ),
    );
  }

  /// `local` or `openai`.
  /// The model suggested for this device, or `null` when there is none.
  final WhisperSuggestion? suggestion;

  /// Efficient mode's state.
  final EfficientModeStatus efficient;

  final String provider;
  final String openAiModel;
  final bool keySaved;
  final bool environmentKey;
  final bool keychainAvailable;
  final String? selectedModel;
  final int? availableDiskBytes;
  final bool usingLocalFallback;
  final List<WhisperModelInfo> models;
  final WhisperDownload download;
}

/// The core's transcription settings.
abstract interface class TranscriptionSettingsBridge {
  /// Reads the view. Outputs: The view, or `null` when the core can't be reached.
  /// Side effects: Reads files and the key store.
  Future<TranscriptionSettings?> read();

  /// Chooses a model on this device. Outputs: A core status. Side effects: Stores the choice.
  Future<int> selectModel(String fileName);

  /// Chooses `local` or `openai`. Outputs: A core status. Side effects: Stores the choice.
  Future<int> setProvider(String provider);

  /// Turns efficient mode on or off for the next recording.
  Future<int> setEfficientGpu({required bool enabled});

  /// Lets the GPU be tried again after a crash stopped it.
  Future<int> retryGpu();

  /// Chooses the OpenAI model. Outputs: A core status. Side effects: Stores the choice.
  Future<int> setOpenAiModel(String model);

  /// Saves the key. Outputs: A core status. Side effects: Writes the key store.
  Future<int> saveKey(String key);

  /// Removes the key and returns to local. Outputs: A core status.
  /// Side effects: Writes the key store and the choices.
  Future<int> removeKey();

  /// Tests the saved key without sending audio.
  /// Outputs: `valid`, `invalid`, `quota_used`, `no_network`, `unexpected` or `no_key`.
  /// Side effects: One request to OpenAI.
  Future<String> testKey();

  /// Starts a download. Outputs: A core status. Side effects: The core downloads in the background.
  Future<int> download(String fileName);

  /// Cancels the running download. Side effects: The partial file is removed.
  Future<void> cancelDownload();

  /// Removes a downloaded model. Outputs: A core status. Side effects: Deletes the file.
  Future<int> removeModel(String fileName);
}

/// Calls the core over FFI, on background isolates since the calls touch files and the network.
final class RustTranscriptionSettingsBridge
    implements TranscriptionSettingsBridge {
  const RustTranscriptionSettingsBridge();

  @override
  Future<TranscriptionSettings?> read() async {
    final String? json = await Isolate.run<String?>(
        () => _callForText('note_taker_transcription_settings'));
    return json == null
        ? null
        : TranscriptionSettings.fromJson(
            jsonDecode(json) as Map<String, Object?>);
  }

  @override
  Future<int> selectModel(String fileName) => Isolate.run(
      () => _callWithText('note_taker_set_whisper_model', fileName));

  @override
  Future<int> setProvider(String provider) => Isolate.run(
      () => _callWithText('note_taker_set_transcription_provider', provider));

  @override
  Future<int> setEfficientGpu({required bool enabled}) {
    final int value = enabled ? 1 : 0;
    return Isolate.run(() => _openCoreLibrary()
        .lookupFunction<Int32 Function(Int32), int Function(int)>(
            'note_taker_set_efficient_gpu')(value));
  }

  @override
  Future<int> retryGpu() => Isolate.run(() => _openCoreLibrary()
      .lookupFunction<Int32 Function(), int Function()>(
          'note_taker_retry_gpu')());

  @override
  Future<int> setOpenAiModel(String model) =>
      Isolate.run(() => _callWithText('note_taker_set_openai_model', model));

  @override
  Future<int> saveKey(String key) =>
      Isolate.run(() => _callWithText('note_taker_save_openai_key', key));

  @override
  Future<int> removeKey() => Isolate.run(() => _openCoreLibrary()
      .lookupFunction<Int32 Function(), int Function()>(
          'note_taker_remove_openai_key')());

  @override
  Future<String> testKey() async =>
      await Isolate.run<String?>(
          () => _callForText('note_taker_test_openai_key')) ??
      'unexpected';

  @override
  Future<int> download(String fileName) => Isolate.run(
      () => _callWithText('note_taker_download_whisper_model', fileName));

  @override
  Future<void> cancelDownload() => Isolate.run(() => _openCoreLibrary()
      .lookupFunction<Void Function(), void Function()>(
          'note_taker_cancel_whisper_download')());

  @override
  Future<int> removeModel(String fileName) => Isolate.run(
      () => _callWithText('note_taker_remove_whisper_model', fileName));
}

/// Calls a function returning a caller-freed string; runs inside an isolate.
String? _callForText(String functionName) {
  final DynamicLibrary coreLibrary = _openCoreLibrary();
  final Pointer<Utf8> text = coreLibrary.lookupFunction<
      Pointer<Utf8> Function(), Pointer<Utf8> Function()>(functionName)();
  if (text == nullptr) {
    return null;
  }
  try {
    return text.toDartString();
  } finally {
    coreLibrary.lookupFunction<Void Function(Pointer<Utf8>),
        void Function(Pointer<Utf8>)>('note_taker_free_string')(text);
  }
}

/// Calls a function taking one string and returning a status; runs inside an isolate.
int _callWithText(String functionName, String text) {
  final int Function(Pointer<Utf8>) function = _openCoreLibrary()
      .lookupFunction<Int32 Function(Pointer<Utf8>),
          int Function(Pointer<Utf8>)>(functionName);
  final Pointer<Utf8> nativeText = text.toNativeUtf8();
  try {
    return function(nativeText);
  } finally {
    calloc.free(nativeText);
  }
}

/// Opens the platform's core library.
DynamicLibrary _openCoreLibrary() => Platform.isWindows
    ? DynamicLibrary.open('note_taker_core.dll')
    : DynamicLibrary.open('libnote_taker_core.so');
