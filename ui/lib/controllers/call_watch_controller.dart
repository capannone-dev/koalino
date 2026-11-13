// Offers to record when a call starts and stops that recording when the call ends (#111).
//
// Polls the core's call watch every few seconds on desktops. A call noticed while nothing is
// recorded becomes an offer, which the app shows; only the owner's tap starts a recording. A
// recording started from the offer is tied to the call: when the call ends, and the owner keeps
// "Stop when the call ends" on, it stops. A recording the owner started themselves is never
// stopped by a call.

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/infrastructure/call_watch_bridge.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';

/// Watches for calls and owns the offer.
final class CallWatchController extends ChangeNotifier {
  /// Creates the controller.
  ///
  /// Inputs: The bridge, the app's recording controller, and how often to look.
  /// Outputs: A controller that does nothing until `start`. Side effects: Listens to the
  /// recording controller until `dispose`.
  CallWatchController({
    required CallWatchBridge bridge,
    required RecordingController recordingController,
    this.interval = const Duration(seconds: 3),
  })  : _bridge = bridge,
        _recording = recordingController {
    _recording.addListener(_onRecordingChanged);
  }

  final CallWatchBridge _bridge;
  final RecordingController _recording;

  /// The time between looks.
  final Duration interval;

  Timer? _timer;
  bool _polling = false;
  CallWatchSettings _settings =
      const CallWatchSettings(offer: true, autoStop: true);
  bool _available = false;
  String? _offeredApp;
  String? _tiedApp;
  String? _message;

  /// The settings as last read or saved.
  CallWatchSettings get settings => _settings;

  /// Whether this system can tell which apps use the microphone (known after the first look).
  bool get available => _available;

  /// The call app to offer recording for, or null.
  String? get offeredApp => _offeredApp;

  /// Whether the recording in progress was started from an offer.
  bool get recordingTiedToCall => _tiedApp != null;

  /// A sentence to show once, such as why a recording stopped; cleared by `clearMessage`.
  String? get message => _message;

  /// Reads the settings and starts looking.
  ///
  /// Side effects: One settings read, then a look every `interval`.
  Future<void> start() async {
    try {
      _settings = await _bridge.settings();
    } on LlmException {
      // Keeps the defaults; the looks report whether the watch works at all.
    }
    _timer?.cancel();
    _timer = Timer.periodic(interval, (_) => unawaited(pollOnce()));
    await pollOnce();
  }

  /// Takes one look and reacts to what it noticed. Public for tests.
  ///
  /// Side effects: May set or clear the offer, stop a recording tied to an ended call, and
  /// stop looking where the system cannot tell.
  Future<void> pollOnce() async {
    if (_polling || !(_settings.offer || _settings.autoStop)) {
      return;
    }
    _polling = true;
    try {
      final CallPoll poll = await _bridge.poll();
      _available = poll.available;
      if (!poll.available) {
        _timer?.cancel();
        _timer = null;
        notifyListeners();
        return;
      }
      switch (poll.event) {
        case 'started':
          if (_settings.offer && _recording.state == RecordingUiState.idle) {
            _offeredApp = poll.app;
          }
        case 'ended':
          _offeredApp = null;
          final String? tiedApp = _tiedApp;
          if (tiedApp != null && _settings.autoStop && _isRecording) {
            _tiedApp = null;
            await _recording.stopRecording();
            _message = 'The $tiedApp call ended, so the recording stopped.';
          }
      }
      notifyListeners();
    } on LlmException {
      // A failed look changes nothing; the next one may work.
    } finally {
      _polling = false;
    }
  }

  /// Records the offered call.
  ///
  /// Side effects: Starts a recording titled after the app, tied to the call.
  Future<void> acceptOffer() async {
    final String? app = _offeredApp;
    if (app == null) {
      return;
    }
    _offeredApp = null;
    notifyListeners();
    await _recording.startRecording(title: '$app call');
    if (_isRecording) {
      _tiedApp = app;
    }
  }

  /// Declines the offer for this call. Side effects: Notifies listeners.
  void dismissOffer() {
    _offeredApp = null;
    notifyListeners();
  }

  /// Clears `message` once shown. Side effects: None beyond the field.
  void clearMessage() {
    _message = null;
  }

  /// Saves the settings. Side effects: Writes the settings file; a look resumes if needed.
  Future<void> setSettings({bool? offer, bool? autoStop}) async {
    try {
      _settings = await _bridge.set(offer: offer, autoStop: autoStop);
    } on LlmException {
      return;
    }
    if (!_settings.offer) {
      _offeredApp = null;
    }
    notifyListeners();
  }

  bool get _isRecording =>
      _recording.state == RecordingUiState.recording ||
      _recording.state == RecordingUiState.paused;

  void _onRecordingChanged() {
    // The owner stopped it, or it never started: no longer tied to a call.
    if (_recording.state == RecordingUiState.idle) {
      _tiedApp = null;
    }
    // Recording already: the offer is moot.
    if (_recording.state != RecordingUiState.idle && _offeredApp != null) {
      _offeredApp = null;
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _timer?.cancel();
    _recording.removeListener(_onRecordingChanged);
    super.dispose();
  }
}
