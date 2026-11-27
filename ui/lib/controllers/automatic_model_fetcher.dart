// Downloads Automatic's model by itself (#189), so a fresh install reaches the best model for the
// device without the owner opening Settings. The app transcribes with the shipped model meanwhile,
// and the core switches to the new one once its hash checks out. A phone waits for an unmetered
// network; a model that does not fit in the free space is left for Settings to explain. A stop or a
// failure is not retried until the next start.
import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/network_state.dart';
import 'package:note_taker_ui/infrastructure/transcription_settings_bridge.dart';

/// Chooses the model to fetch now, if any.
///
/// Inputs: The core's settings. Outputs: The catalog file name of Automatic's model when the owner
/// leaves Automatic and the automatic download on, the model is not on the device, it fits in the
/// free space, and no download runs; otherwise `null`. Side effects: None.
String? automaticModelToFetch(TranscriptionSettings settings) {
  final String? waiting = settings.automatic.waitingForDownload;
  if (!settings.autoDownload ||
      !settings.automatic.enabled ||
      waiting == null ||
      settings.download.state == 'downloading') {
    return null;
  }
  final WhisperModelInfo? model = settings.models
      .where((WhisperModelInfo row) => row.fileName == waiting)
      .firstOrNull;
  if (model == null || model.onDevice) {
    return null;
  }
  final int? free = settings.availableDiskBytes;
  if (free != null && free < model.sizeBytes) {
    return null;
  }
  return waiting;
}

/// Runs the automatic download and reports its progress for the Record screen.
final class AutomaticModelFetcher extends ChangeNotifier {
  /// Creates the fetcher; `pollInterval` is shortened in tests.
  AutomaticModelFetcher({
    required TranscriptionSettingsBridge bridge,
    required NetworkState network,
    Duration pollInterval = const Duration(seconds: 1),
  })  : _bridge = bridge,
        _network = network,
        _pollInterval = pollInterval;

  final TranscriptionSettingsBridge _bridge;
  final NetworkState _network;
  final Duration _pollInterval;

  /// Models already tried in this run: stopped or failed ones wait for the next start.
  final Set<String> _tried = <String>{};
  Timer? _timer;
  bool _checking = false;
  bool _disposed = false;

  /// The model being downloaded, as the catalog names it (`small`), or `null` when none is.
  String? get modelName => _modelName;
  String? _modelName;

  /// Whether a model already transcribes meanwhile; `false` on builds that ship none (F-Droid).
  bool get hasModelMeanwhile => _hasModelMeanwhile;
  bool _hasModelMeanwhile = true;

  /// The share downloaded, from 0 to 1, or `null` before the size is known.
  double? get progress => _progress;
  double? _progress;

  /// Starts the download when it is due.
  ///
  /// Called at start and whenever the app returns to the foreground. Side effects: Reads the core's
  /// settings; may ask the system about the network and start a download; notifies listeners.
  Future<void> check() async {
    if (_checking || _modelName != null || _disposed) {
      return;
    }
    _checking = true;
    try {
      final TranscriptionSettings? settings = await _bridge.read();
      final String? fileName =
          settings == null ? null : automaticModelToFetch(settings);
      if (settings == null ||
          fileName == null ||
          _tried.contains(fileName) ||
          !await _network.isUnmetered()) {
        return;
      }
      _tried.add(fileName);
      if (await _bridge.download(fileName) != transcriptionStatusOk) {
        return;
      }
      _modelName = settings.models
              .where((WhisperModelInfo row) => row.fileName == fileName)
              .firstOrNull
              ?.name ??
          fileName;
      _hasModelMeanwhile = settings.automatic.fileName != null;
      _progress = null;
      _notify();
      _schedulePoll();
    } on Object {
      // The shipped model keeps working; Settings still offers the download.
    } finally {
      _checking = false;
    }
  }

  /// Reads the download's progress, and stops reporting once it ended.
  ///
  /// Side effects: Reads the core's settings; notifies listeners; schedules the next poll.
  Future<void> poll() async {
    if (_modelName == null || _disposed) {
      return;
    }
    WhisperDownload? download;
    try {
      download = (await _bridge.read())?.download;
    } on Object {
      download = null;
    }
    if (download == null || download.state != 'downloading') {
      _modelName = null;
      _progress = null;
    } else {
      _progress = download.totalBytes > 0
          ? download.receivedBytes / download.totalBytes
          : null;
      _schedulePoll();
    }
    _notify();
  }

  /// Stops the download for this run; the next start offers it again.
  ///
  /// Side effects: Cancels the core's download, which removes the partial file; notifies listeners.
  Future<void> stop() async {
    _timer?.cancel();
    try {
      await _bridge.cancelDownload();
    } on Object {
      // Nothing more to do: the core reports the download's end itself.
    }
    _modelName = null;
    _progress = null;
    _notify();
  }

  void _schedulePoll() {
    _timer?.cancel();
    if (!_disposed && _pollInterval > Duration.zero) {
      _timer = Timer(_pollInterval, () => unawaited(poll()));
    }
  }

  void _notify() {
    if (!_disposed) {
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _timer?.cancel();
    super.dispose();
  }
}
