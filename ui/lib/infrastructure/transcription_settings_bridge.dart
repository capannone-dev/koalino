// Reaches the Rust core's Whisper model and OpenAI settings (Issue #67). The core downloads,
// verifies and stores everything; the OpenAI key is passed in once and never read back.

import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:ffi/ffi.dart';

/// Core status: done.
const int transcriptionStatusOk = 0;

/// Core status: the value was refused, such as a glossary term that is too long.
const int transcriptionStatusInvalid = 1;

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

/// A model the core proposes from how the current one runs on this device (#68).
final class WhisperProposal {
  /// Creates the proposal.
  const WhisperProposal({
    required this.fileName,
    required this.reason,
    required this.measured,
  });

  /// Reads the proposal from the core's JSON.
  factory WhisperProposal.fromJson(Map<String, Object?> json) =>
      WhisperProposal(
        fileName: json['file_name']! as String,
        reason: json['reason']! as String,
        measured: json['measured']! as bool,
      );

  /// The catalog file name.
  final String fileName;

  /// Why, in plain language.
  final String reason;

  /// Whether the model's speed was measured here, rather than estimated.
  final bool measured;
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

/// Recording what the computer plays beside the microphone (#108).
final class ComputerSoundSetting {
  const ComputerSoundSetting({this.available = false, this.enabled = true});

  /// Reads `computer_sound` from the core's JSON.
  factory ComputerSoundSetting.fromJson(Map<String, Object?> json) =>
      ComputerSoundSetting(
        available: json['available'] as bool? ?? false,
        enabled: json['enabled'] as bool? ?? true,
      );

  /// Whether this build and platform can record it (desktop only).
  final bool available;

  /// Whether the owner wants it recorded; on by default.
  final bool enabled;
}

/// Efficient mode's state on this device.
final class AutomaticModelStatus {
  /// Creates the state; by default a model was chosen by hand, as with a core from before #106.
  const AutomaticModelStatus({
    this.enabled = false,
    this.fileName,
    this.efficient = false,
    this.waitingForDownload,
    this.speedCheck,
  });

  /// Reads the state from the core's JSON.
  factory AutomaticModelStatus.fromJson(Map<String, Object?> json) =>
      AutomaticModelStatus(
        enabled: json['enabled']! as bool,
        fileName: json['file_name'] as String?,
        efficient: json['efficient'] as bool? ?? false,
        waitingForDownload: json['waiting_for_download'] as String?,
        speedCheck: json['speed_check'] as String?,
      );

  /// Whether Automatic is chosen, rather than a model picked by hand.
  final bool enabled;

  /// The model Automatic runs on this device, or `null` when none is here.
  final String? fileName;

  /// Whether Automatic runs it in efficient mode.
  final bool efficient;

  /// The model suggested for this device while it waits to be downloaded, if any.
  final String? waitingForDownload;

  /// How Automatic's model ran on a clip timed on this device (#109), worded by the core, or
  /// `null` before it was timed.
  final String? speedCheck;
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
    this.proposal,
    this.efficient = const EfficientModeStatus(),
    this.glossary = const <String>[],
    this.keepSpeech = const SpeechRetention(),
    this.afterMeetingPass = true,
    this.afterMeetingPassModel,
    this.computerSound = const ComputerSoundSetting(),
    this.automatic = const AutomaticModelStatus(),
  });

  /// Reads the view from the core's JSON.
  factory TranscriptionSettings.fromJson(Map<String, Object?> json) {
    final Map<String, Object?> download =
        json['download']! as Map<String, Object?>;
    return TranscriptionSettings(
      provider: json['provider']! as String,
      proposal: json['proposal'] == null
          ? null
          : WhisperProposal.fromJson(json['proposal']! as Map<String, Object?>),
      suggestion: json['suggestion'] == null
          ? null
          : WhisperSuggestion.fromJson(
              json['suggestion']! as Map<String, Object?>),
      efficient: json['efficient'] == null
          ? const EfficientModeStatus()
          : EfficientModeStatus.fromJson(
              json['efficient']! as Map<String, Object?>),
      glossary: (json['glossary'] as List<Object?>? ?? const <Object?>[])
          .whereType<String>()
          .toList(growable: false),
      keepSpeech: json['keep_speech'] == null
          ? const SpeechRetention()
          : SpeechRetention.fromJson(
              json['keep_speech']! as Map<String, Object?>),
      afterMeetingPass: json['after_meeting_pass'] as bool? ?? true,
      afterMeetingPassModel: json['after_meeting_pass_model'] as String?,
      computerSound: json['computer_sound'] == null
          ? const ComputerSoundSetting()
          : ComputerSoundSetting.fromJson(
              json['computer_sound']! as Map<String, Object?>),
      automatic: json['automatic'] == null
          ? const AutomaticModelStatus()
          : AutomaticModelStatus.fromJson(
              json['automatic']! as Map<String, Object?>),
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
  /// A model proposed from this device's measurements, or `null`.
  final WhisperProposal? proposal;

  /// The model suggested for this device, or `null` when there is none.
  final WhisperSuggestion? suggestion;

  /// Efficient mode's state.
  final EfficientModeStatus efficient;

  /// Names and terms Whisper is prompted with, in order.
  final List<String> glossary;

  /// How long a recording's speech waits to be combined with another device's (#93).
  final SpeechRetention keepSpeech;

  /// Whether each meeting is transcribed again after Stop, with longer context (#87). On by
  /// default; a phone runs it only while charging.
  final bool afterMeetingPass;

  /// The catalog file name of the model the pass re-transcribes with, or `null` for the model
  /// recordings use (#87).
  final String? afterMeetingPassModel;

  /// Whether recordings also capture what the computer plays (#108).
  final ComputerSoundSetting computerSound;

  /// The "Automatic" model choice (#106); `selectedModel` is `null` while it is on.
  final AutomaticModelStatus automatic;

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

/// How many days a recording's speech may wait to be combined with another device's recording
/// of the same meeting before it is deleted (#93).
final class SpeechRetention {
  /// Creates the window; the defaults are the core's (three weeks, 1 to 60 days).
  const SpeechRetention({this.days = 21, this.minDays = 1, this.maxDays = 60});

  /// Reads `{"days","min_days","max_days"}` from the core's JSON.
  factory SpeechRetention.fromJson(Map<String, Object?> json) =>
      SpeechRetention(
        days: json['days'] as int? ?? 21,
        minDays: json['min_days'] as int? ?? 1,
        maxDays: json['max_days'] as int? ?? 60,
      );

  /// The current window in days.
  final int days;

  /// The shortest window the core accepts.
  final int minDays;

  /// The longest window the core accepts.
  final int maxDays;

  /// The windows Settings offers, in increasing order: common ones within the bounds, plus the
  /// current one when it was set elsewhere (the CLI or the web).
  List<int> get choices => <int>{
        minDays,
        ...<int>[3, 7, 14, 21, 30, 45]
            .where((int day) => day > minDays && day < maxDays),
        maxDays,
        days,
      }.toList()
        ..sort();
}

/// The core's transcription settings.
abstract interface class TranscriptionSettingsBridge {
  /// Reads the view. Outputs: The view, or `null` when the core can't be reached.
  /// Side effects: Reads files and the key store.
  Future<TranscriptionSettings?> read();

  /// Stops proposing a model, returning the core's status.
  Future<int> dismissProposal(String fileName);

  /// Chooses a model on this device. Outputs: A core status. Side effects: Stores the choice.
  Future<int> selectModel(String fileName);

  /// Chooses `local` or `openai`. Outputs: A core status. Side effects: Stores the choice.
  Future<int> setProvider(String provider);

  /// Turns efficient mode on or off for the next recording.
  Future<int> setEfficientGpu({required bool enabled});

  /// Turns the after-meeting pass (#87) on or off from the next recording.
  /// Outputs: A core status. Side effects: Stores the choice.
  Future<int> setAfterMeetingPass({required bool enabled});

  /// Turns recording the computer's sound (#108) on or off from the next start or resume.
  /// Outputs: A core status. Side effects: Stores the choice.
  Future<int> setComputerSound({required bool enabled});

  /// Chooses the after-meeting pass's model (#87): a model on this device, or `null` for the one
  /// recordings use. Outputs: A core status, `transcriptionStatusInvalid` for a model not here.
  /// Side effects: Stores the choice for the next recording.
  Future<int> setAfterMeetingPassModel(String? fileName);

  /// Chooses "Automatic" (#106): the model suggested for this device.
  /// Outputs: A core status. Side effects: Forgets the hand-picked model.
  Future<int> useAutomaticModel();

  /// Lets the GPU be tried again after a crash stopped it.
  Future<int> retryGpu();

  /// Saves the glossary of names and terms; an empty list clears it.
  /// Outputs: A core status, `transcriptionStatusInvalid` for a term or list that is too long.
  /// Side effects: Stores it; the next recording reads it.
  Future<int> setGlossary(List<String> terms);

  /// Sets how many days speech waits to be combined (#93).
  /// Outputs: A core status, `transcriptionStatusInvalid` outside the bounds.
  /// Side effects: Stores it; speech older than a shorter window is deleted at the next round.
  Future<int> setKeepSpeechDays(int days);

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
    if (json == null) {
      return null;
    }
    final Map<String, Object?> settings =
        jsonDecode(json) as Map<String, Object?>;
    // A separate call, since it needs the database while Settings otherwise doesn't.
    final String? proposal = await Isolate.run<String?>(
        () => _callForText('note_taker_whisper_proposal'));
    settings['proposal'] = proposal == null ? null : jsonDecode(proposal);
    return TranscriptionSettings.fromJson(settings);
  }

  @override
  Future<int> dismissProposal(String fileName) => Isolate.run(
      () => _callWithText('note_taker_dismiss_whisper_proposal', fileName));

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
  Future<int> useAutomaticModel() => Isolate.run(() => _openCoreLibrary()
      .lookupFunction<Int32 Function(), int Function()>(
          'note_taker_use_automatic_model')());

  @override
  Future<int> setAfterMeetingPass({required bool enabled}) {
    final int value = enabled ? 1 : 0;
    return Isolate.run(() => _openCoreLibrary()
        .lookupFunction<Int32 Function(Int32), int Function(int)>(
            'note_taker_set_after_meeting_pass')(value));
  }

  @override
  Future<int> setAfterMeetingPassModel(String? fileName) => Isolate.run(() =>
      _callWithText('note_taker_set_after_meeting_pass_model', fileName ?? ''));

  @override
  Future<int> setComputerSound({required bool enabled}) {
    final int value = enabled ? 1 : 0;
    return Isolate.run(() => _openCoreLibrary()
        .lookupFunction<Int32 Function(Int32), int Function(int)>(
            'note_taker_set_computer_sound')(value));
  }

  @override
  Future<int> setGlossary(List<String> terms) {
    final String lines = terms.join('\n');
    return Isolate.run(() => _callWithText('note_taker_set_glossary', lines));
  }

  @override
  Future<int> setKeepSpeechDays(int days) =>
      Isolate.run(() => _openCoreLibrary()
          .lookupFunction<Int32 Function(Int32), int Function(int)>(
              'note_taker_set_keep_speech_days')(days));

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
