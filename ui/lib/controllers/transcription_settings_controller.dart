// Holds the Settings page for the Whisper model (Issue #67). The core does the work;
// this keeps the view fresh while a download runs and reports refusals in plain words.

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/npu_download.dart';
import 'package:note_taker_ui/infrastructure/transcription_settings_bridge.dart';

/// Owns the transcription settings for its page.
final class TranscriptionSettingsController extends ChangeNotifier {
  /// Creates the controller.
  ///
  /// Inputs: The bridge, how often to refresh during a download, and the NPU files' downloader
  /// with its refresh interval (#75).
  /// Side effects: None until `load`.
  TranscriptionSettingsController({
    required TranscriptionSettingsBridge bridge,
    Duration refreshInterval = const Duration(seconds: 1),
    NpuDownloader npuDownloader = const NoNpuDownloader(),
    Duration npuRefreshInterval = const Duration(seconds: 1),
  })  : _bridge = bridge,
        _refreshInterval = refreshInterval,
        _npuDownloader = npuDownloader,
        _npuRefreshInterval = npuRefreshInterval;

  final TranscriptionSettingsBridge _bridge;
  final Duration _refreshInterval;
  final NpuDownloader _npuDownloader;
  final Duration _npuRefreshInterval;
  NpuDownloadProgress? _npuProgress;
  Timer? _npuTimer;
  bool _npuChecked = false;

  /// The NPU files' download while it runs, else `null`.
  NpuDownloadProgress? get npuProgress => _npuProgress;

  TranscriptionSettings? _settings;
  String? _message;
  bool _isBusy = false;
  bool _disposed = false;
  Timer? _refreshTimer;

  /// The latest view, or `null` before it loads or when the core can't be reached.
  TranscriptionSettings? get settings => _settings;

  /// A refusal or confirmation to show, or `null`.
  String? get message => _message;

  /// Whether a save is in progress.
  bool get isBusy => _isBusy;

  /// Whether a download is running.
  bool get isDownloading => _settings?.download.state == 'downloading';

  /// Reads the view, and keeps refreshing while a download runs.
  ///
  /// Side effects: Calls the bridge, notifies listeners, and may start a timer.
  Future<void> load() async {
    final TranscriptionSettings? previous = _settings;
    try {
      _settings = await _bridge.read();
    } on Object {
      _settings = null;
    }
    _reportDownloadEnd(previous);
    await _resumeNpuDownload();
    _refreshTimer?.cancel();
    if (isDownloading && !_disposed) {
      _refreshTimer = Timer(_refreshInterval, () => unawaited(load()));
    }
    _notify();
  }

  /// Reports a download that has just ended.
  void _reportDownloadEnd(TranscriptionSettings? previous) {
    final WhisperDownload? download = _settings?.download;
    if (previous?.download.state != 'downloading' || download == null) {
      return;
    }
    _message = switch ((download.state, download.error)) {
      ('done', _) => 'Download finished. You can now choose this model.',
      ('cancelled', _) => 'Download cancelled.',
      ('failed', 'no_network') =>
        'The download stopped: no connection to the server. Your current model still works.',
      ('failed', 'corrupted') =>
        'The downloaded file failed its check and was deleted. Your current model still works.',
      ('failed', 'storage') =>
        'The model could not be saved. Check the free space. Your current model still works.',
      ('failed', _) =>
        'The download server refused the file. Your current model still works.',
      _ => _message,
    };
  }

  /// Chooses a model for the next recording.
  ///
  /// Inputs: A file name on this device. Side effects: Stores the choice and reloads.
  Future<void> selectModel(String fileName) =>
      _run(() => _bridge.selectModel(fileName),
          success: 'Saved. The next recording uses this model.');

  /// Turns efficient mode on or off.
  ///
  /// Inputs: The new state. Side effects: Stores the choice and reloads.
  Future<void> setEfficientGpu({required bool enabled}) =>
      _run(() => _bridge.setEfficientGpu(enabled: enabled),
          success: 'Saved. The next recording uses this.');

  /// Chooses "Automatic", the model suggested for this device.
  ///
  /// Side effects: Forgets the hand-picked model and reloads.
  Future<void> useAutomaticModel() => _run(_bridge.useAutomaticModel,
      success: 'Saved. The next recording uses the automatic choice.');

  /// Turns the after-meeting pass on or off.
  ///
  /// Inputs: The new state. Side effects: Stores the choice and reloads.
  Future<void> setAfterMeetingPass({required bool enabled}) =>
      _run(() => _bridge.setAfterMeetingPass(enabled: enabled),
          success: 'Saved. The next recording uses this.');

  /// Chooses the after-meeting pass's model (#87), or `null` for the recording model.
  ///
  /// Inputs: A model on this device, or `null`. Side effects: Stores the choice and reloads.
  Future<void> setAfterMeetingPassModel(String? fileName) =>
      _run(() => _bridge.setAfterMeetingPassModel(fileName),
          success: 'Saved. Meetings recorded from now on use this.');

  /// Turns recording the computer's sound (#108) on or off.
  ///
  /// Inputs: The new state. Side effects: Stores the choice and reloads.
  Future<void> setComputerSound({required bool enabled}) =>
      _run(() => _bridge.setComputerSound(enabled: enabled),
          success: 'Saved. The next recording uses this.');

  /// Saves the glossary from text holding one term per line.
  ///
  /// Inputs: The text; lines are trimmed and blank ones dropped.
  /// Side effects: Stores the terms and reloads; a refused list keeps the saved one.
  Future<void> setGlossary(String text) {
    final List<String> terms = text
        .split('\n')
        .map((String line) => line.trim())
        .where((String line) => line.isNotEmpty)
        .toList(growable: false);
    return _run(() => _bridge.setGlossary(terms),
        success: 'Saved. The next recording uses these names and terms.',
        invalid: 'Each term can have at most 60 characters, and all of them '
            'together at most 500.');
  }

  /// Sets how many days a recording's speech waits to be combined (#93).
  ///
  /// Inputs: The days. Side effects: Stores the window and reloads.
  Future<void> setKeepSpeechDays(int days) =>
      _run(() => _bridge.setKeepSpeechDays(days),
          success: days == 1
              ? 'Saved. Speech now waits up to 1 day to be combined.'
              : 'Saved. Speech now waits up to $days days to be combined.',
          invalid: 'Choose a window between 1 and 60 days.');

  /// Downloads the NPU files, then turns the NPU on (#75).
  ///
  /// Side effects: The host downloads about 360 MB from Qualcomm in the background; progress is
  /// polled until it ends, and the outcome is reported in `message`.
  Future<void> downloadNpu() async {
    final String? directory = _settings?.npu.directory;
    if (directory == null || directory.isEmpty || _npuProgress != null) {
      return;
    }
    _message = null;
    _npuProgress = const NpuDownloadProgress(state: 'downloading');
    _notify();
    try {
      await _npuDownloader.download(directory);
    } on Object {
      _npuProgress = null;
      _message = 'The download could not start. Please try again.';
      _notify();
      return;
    }
    _scheduleNpuPoll();
  }

  /// Stops the NPU download; nothing partial is kept.
  Future<void> cancelNpuDownload() => _npuDownloader.cancel();

  /// Turns Whisper on the NPU on or off.
  ///
  /// Inputs: The new state. Side effects: Stores the choice and reloads.
  Future<void> setNpuWhisper({required bool enabled}) =>
      _run(() => _bridge.setNpuWhisper(enabled: enabled),
          success: enabled
              ? 'Saved. The next recording is transcribed on the NPU.'
              : 'Saved. The next recording is transcribed on the processor.',
          invalid: 'The NPU files are not all downloaded yet.');

  /// Turns the NPU off and deletes its files.
  ///
  /// Side effects: Stores the choice, deletes about 460 MB, and reloads.
  Future<void> removeNpu() async {
    final String? directory = _settings?.npu.directory;
    if (directory == null || directory.isEmpty) {
      return;
    }
    await _bridge.setNpuWhisper(enabled: false);
    bool removed;
    try {
      removed = await _npuDownloader.remove(directory);
    } on Object {
      removed = false;
    }
    _message = removed
        ? 'The NPU files were removed.'
        : 'The NPU files could not be removed. Please try again.';
    await load();
  }

  /// Shows a download the host still runs from an earlier visit to the page (#75).
  ///
  /// Side effects: Asks the host once per controller, only when the NPU could be downloaded.
  Future<void> _resumeNpuDownload() async {
    final NpuStatus? npu = _settings?.npu;
    if (_npuChecked || npu == null || !npu.supported || npu.installed) {
      return;
    }
    _npuChecked = true;
    try {
      final NpuDownloadProgress progress = await _npuDownloader.progress();
      if (progress.state == 'downloading' && _npuProgress == null) {
        _npuProgress = progress;
        _scheduleNpuPoll();
      }
    } on Object {
      // No host answer: the Download button stays.
    }
  }

  /// Checks the NPU download again after the refresh interval.
  void _scheduleNpuPoll() {
    _npuTimer?.cancel();
    if (!_disposed) {
      _npuTimer = Timer(_npuRefreshInterval, () => unawaited(_pollNpu()));
    }
  }

  /// Reads the NPU download's progress; when it ends, reports it and turns the NPU on.
  Future<void> _pollNpu() async {
    NpuDownloadProgress progress;
    try {
      progress = await _npuDownloader.progress();
    } on Object {
      progress = const NpuDownloadProgress(state: 'failed', error: 'server');
    }
    if (_disposed) {
      return;
    }
    if (progress.state == 'downloading') {
      _npuProgress = progress;
      _notify();
      _scheduleNpuPoll();
      return;
    }
    _npuProgress = null;
    if (progress.state == 'done') {
      await setNpuWhisper(enabled: true);
      _message = 'Downloaded. Recordings now transcribe on the NPU.';
      _notify();
      return;
    }
    _message = switch ((progress.state, progress.error)) {
      ('cancelled', _) => 'Download cancelled.',
      ('failed', 'no_network') =>
        'The download stopped: no connection to the server.',
      ('failed', 'corrupted') =>
        'A downloaded file failed its check and was deleted.',
      ('failed', 'storage') =>
        'The files could not be saved. Check the free space (about 800 MB is needed).',
      _ => 'The download server refused the files.',
    };
    await load();
  }

  /// Lets the GPU be tried again after a crash stopped it.
  ///
  /// Side effects: Clears the core's record and reloads.
  Future<void> retryGpu() => _run(_bridge.retryGpu,
      success: 'The next recording tries the graphics chip again.');

  /// Applies the suggestion: chooses it when on this device, or starts its download.
  ///
  /// Inputs: The suggestion and whether its file is here. A download changes nothing else, so
  /// the current model keeps working until the owner chooses the new one.
  /// Side effects: Stores choices or starts a download, and reloads.
  Future<void> applySuggestion(WhisperSuggestion suggestion,
      {required bool onDevice}) async {
    if (!onDevice) {
      await download(suggestion.fileName);
      return;
    }
    await setEfficientGpu(enabled: suggestion.efficient);
    await selectModel(suggestion.fileName);
  }

  /// Applies a proposal: chooses it when on this device, or starts its download.
  ///
  /// Inputs: The proposal and whether its file is here. Efficient mode is left as it is.
  /// Side effects: Stores the choice or starts a download, and reloads.
  Future<void> applyProposal(WhisperProposal proposal,
      {required bool onDevice}) async {
    if (!onDevice) {
      await download(proposal.fileName);
      return;
    }
    await selectModel(proposal.fileName);
  }

  /// Stops proposing a model; the model in use doesn't change.
  ///
  /// Side effects: Stores the dismissal and reloads.
  Future<void> dismissProposal(WhisperProposal proposal) => _run(
        () => _bridge.dismissProposal(proposal.fileName),
        success: 'That model won\'t be proposed again.',
      );

  /// Starts downloading a model.
  ///
  /// Inputs: A catalog file name. Side effects: The core downloads in the background.
  Future<void> download(String fileName) => _run(
        () => _bridge.download(fileName),
        success: null,
      );

  /// Cancels the running download. Side effects: The partial file is removed.
  Future<void> cancelDownload() async {
    await _bridge.cancelDownload();
    await load();
  }

  /// Removes a downloaded model.
  ///
  /// Inputs: A file name. Side effects: Deletes the file and reloads.
  Future<void> removeModel(String fileName) =>
      _run(() => _bridge.removeModel(fileName), success: 'Model removed.');

  /// Runs one change, reports the result, and reloads.
  Future<void> _run(Future<int> Function() change,
      {required String? success, String? invalid}) async {
    _isBusy = true;
    _message = null;
    _notify();
    int status;
    try {
      status = await change();
    } on Object {
      status = -1;
    }
    _message = switch (status) {
      transcriptionStatusOk => success,
      transcriptionStatusInvalid when invalid != null => invalid,
      transcriptionStatusBusy =>
        'Another model is downloading. Wait for it to finish.',
      transcriptionStatusNoSpace =>
        'There isn\'t enough free space for this model.',
      _ => 'That didn\'t work. Please try again.',
    };
    _isBusy = false;
    await load();
  }

  /// Notifies listeners unless disposed.
  void _notify() {
    if (!_disposed) {
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _refreshTimer?.cancel();
    _npuTimer?.cancel();
    super.dispose();
  }
}
