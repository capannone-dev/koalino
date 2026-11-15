// Offers to record when a call starts and stops that recording when the call ends (#111).
//
// Polls the core's call watch every few seconds (on Android with the users Kotlin saw, since the
// core cannot look there). A call noticed while nothing is
// recorded becomes an offer, shown in the app and as a system notification where there is one;
// only the owner's tap (on either) starts a recording. A
// recording started from the offer is tied to the call: when the call ends, and the owner keeps
// "Stop when the call ends" on, it stops. A recording the owner started themselves is never
// stopped by a call.

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/infrastructure/call_notifier.dart';
import 'package:note_taker_ui/infrastructure/call_watch_bridge.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Reads the microphone's users on the host, where the core cannot (Android).
typedef HostMicrophoneUsers = Future<List<HostMicrophoneUser>> Function();

/// The name the core gives a call whose app is unknown (Android).
String get unnamedCallApp => l10n.aCallApp;

/// The title of a recording started from the offer for `app`.
String callRecordingTitle(String app) =>
    app == unnamedCallApp ? l10n.call : l10n.call2(app);

/// Watches for calls and owns the offer.
final class CallWatchController extends ChangeNotifier {
  /// Creates the controller.
  ///
  /// Inputs: The bridge, the app's recording controller, the system notification (none by
  /// default), the host's own view of the microphone (null: the core looks), and how often to
  /// look.
  /// Outputs: A controller that does nothing until `start`. Side effects: Listens to the
  /// recording controller until `dispose`.
  CallWatchController({
    required CallWatchBridge bridge,
    required RecordingController recordingController,
    CallNotifier notifier = const NoCallNotifier(),
    HostMicrophoneUsers? hostUsers,
    this.interval = const Duration(seconds: 3),
  })  : _bridge = bridge,
        _recording = recordingController,
        _notifier = notifier,
        _hostUsers = hostUsers {
    _recording.addListener(_onRecordingChanged);
  }

  final CallWatchBridge _bridge;
  final RecordingController _recording;
  final CallNotifier _notifier;
  final HostMicrophoneUsers? _hostUsers;

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
  bool _disposed = false;

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
      _tellRestartIfNeeded();
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
      final HostMicrophoneUsers? hostUsers = _hostUsers;
      final List<HostMicrophoneUser>? users;
      try {
        users = hostUsers == null ? null : await hostUsers();
      } on Exception {
        // The host could not look this time; the next look may work.
        return;
      }
      final CallPoll poll = await _bridge.poll(users: users);
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
            if (poll.app case final String app) {
              unawaited(_notify(app));
            }
          }
        case 'ended':
          _withdrawOffer();
          final String? tiedApp = _tiedApp;
          if (tiedApp != null && _settings.autoStop && _isRecording) {
            _tiedApp = null;
            await _recording.stopRecording();
            _message = l10n.theCallEndedSoTheRecording(tiedApp);
          }
      }
      notifyListeners();
    } on LlmException {
      // A failed look changes nothing; the next one may work.
    } finally {
      _polling = false;
    }
  }

  /// Shows the system notification and acts on its answer, if the offer still stands.
  Future<void> _notify(String app) async {
    final CallNotificationAnswer answer = await _notifier.offer(app);
    if (_disposed || _offeredApp != app) {
      return;
    }
    switch (answer) {
      case CallNotificationAnswer.record:
        await acceptOffer();
      case CallNotificationAnswer.dismissed:
        dismissOffer();
      case CallNotificationAnswer.unavailable:
        // No notification was shown; the banner still offers.
        break;
    }
  }

  /// Clears the offer and the notification.
  void _withdrawOffer() {
    _offeredApp = null;
    _notifier.withdraw();
  }

  /// Records the offered call.
  ///
  /// Side effects: Withdraws the notification and starts a recording titled after the app,
  /// tied to the call.
  Future<void> acceptOffer() async {
    final String? app = _offeredApp;
    if (app == null) {
      return;
    }
    _withdrawOffer();
    notifyListeners();
    await _recording.startRecording(title: callRecordingTitle(app));
    if (_isRecording) {
      _tiedApp = app;
    }
  }

  /// Declines the offer for this call. Side effects: Withdraws the notification and notifies
  /// listeners.
  void dismissOffer() {
    _withdrawOffer();
    notifyListeners();
  }

  /// Clears `message` once shown. Side effects: None beyond the field.
  void clearMessage() {
    _message = null;
  }

  /// Saves the settings. Side effects: Writes the settings file; a look resumes if needed.
  Future<void> setSettings(
      {bool? offer, bool? autoStop, bool? readNames}) async {
    try {
      _settings = await _bridge.set(
          offer: offer, autoStop: autoStop, readNames: readNames);
      _tellRestartIfNeeded();
    } on LlmException {
      return;
    }
    if (!_settings.offer) {
      _withdrawOffer();
    }
    notifyListeners();
  }

  /// Asks for a restart of call apps once the core turned the desktop's accessibility on (#113).
  void _tellRestartIfNeeded() {
    if (_settings.restartApps) {
      _message = l10n.restartYourBrowserAndCallApps;
    }
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
      _withdrawOffer();
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _timer?.cancel();
    // Cleared first, so the withdrawn notification's answer finds no offer to act on.
    _offeredApp = null;
    _notifier.withdraw();
    _recording.removeListener(_onRecordingChanged);
    super.dispose();
  }
}
