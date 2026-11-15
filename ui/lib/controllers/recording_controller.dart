// Coordinates recording UI state exclusively through the Flutter recording bridge.

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/microphone_permission.dart';
import 'package:note_taker_ui/infrastructure/recording_bridge.dart';
import 'package:note_taker_ui/infrastructure/recording_notifier.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Describes the recording status rendered by the Flutter presentation layer.
enum RecordingUiState {
  /// No recording request is currently active.
  idle,

  /// A start request has been sent to the Rust bridge and is awaiting its result.
  starting,

  /// The Rust bridge accepted the recording request.
  recording,

  /// The recording is paused; it can be resumed into the same meeting or stopped.
  paused,

  /// A stop was requested; Rust is transcribing the remaining audio.
  stopping,
}

/// Owns user-visible recording state while depending only on the Dart bridge interface.
final class RecordingController extends ChangeNotifier {
  /// Creates a controller whose native work is delegated to `recordingBridge`.
  ///
  /// Inputs: `recordingBridge` must implement the stable Dart bridge contract;
  /// `microphonePermission` is asked before every start and resume (Android's runtime
  /// permission; granted elsewhere); `recordingNotifier` mirrors every state into the Android
  /// notification and reports the changes made from it (Issue #47).
  /// Outputs: A controller in the idle state.
  /// Side effects: Subscribes to the notifier's changes until `dispose`.
  RecordingController({
    required RecordingBridge recordingBridge,
    MicrophonePermission microphonePermission =
        const GrantedMicrophonePermission(),
    RecordingNotifier recordingNotifier = const NoRecordingNotifier(),
  })  : _recordingBridge = recordingBridge,
        _microphonePermission = microphonePermission,
        _recordingNotifier = recordingNotifier {
    _externalChanges =
        recordingNotifier.externalChanges.listen(_applyExternalChange);
  }

  final RecordingBridge _recordingBridge;
  final MicrophonePermission _microphonePermission;
  final RecordingNotifier _recordingNotifier;
  late final StreamSubscription<RecordingNotificationState> _externalChanges;
  RecordingUiState _state = RecordingUiState.idle;
  String? _errorMessage;
  String? _activeTitle;
  String? _completionMessage;

  /// Whether a pause or resume request is awaiting Rust, so duplicate taps are ignored.
  bool _isTransitioning = false;

  /// Whether the app's own stop is awaiting Rust; the notification's reports wait for it.
  bool _isStopping = false;

  /// Returns the state that presentation widgets should render.
  ///
  /// Inputs: None.
  /// Outputs: The current UI-safe recording state.
  /// Side effects: None.
  RecordingUiState get state => _state;

  /// Returns a privacy-safe error message when the latest start request failed.
  ///
  /// Inputs: None.
  /// Outputs: A displayable error or `null` after a successful or unattempted request.
  /// Side effects: None.
  String? get errorMessage => _errorMessage;

  /// Returns the title of the recording in progress, or `null` when untitled or not recording.
  String? get activeTitle => _activeTitle;

  /// Returns "Recording saved" after a successful stop, until the next recording starts.
  String? get completionMessage => _completionMessage;

  /// Requests that the Rust core start a recording session, optionally titled.
  ///
  /// Inputs: `title` is user text; it is trimmed, and a blank title becomes `null` so Rust
  /// applies its dated fallback. Duplicate requests while one is pending or active are ignored.
  /// Outputs: Completes once the bridge reports whether the lifecycle command succeeded.
  /// Side effects: Starts the Android foreground service before invoking the FFI bridge (and
  /// ends it if Rust fails to start), updates UI state, and notifies listening widgets. A refused
  /// service leaves the recording running with `backgroundRecordingRefusedMessage` shown.
  Future<void> startRecording({String? title}) async {
    if (_state != RecordingUiState.idle) {
      return;
    }

    final String? trimmedTitle = title?.trim();
    final String? requestedTitle =
        trimmedTitle == null || trimmedTitle.isEmpty ? null : trimmedTitle;
    _state = RecordingUiState.starting;
    _errorMessage = null;
    _completionMessage = null;
    notifyListeners();

    if (!await _microphonePermission.ensureGranted()) {
      _state = RecordingUiState.idle;
      _errorMessage = _permissionDeniedMessage;
      notifyListeners();
      return;
    }
    // The service starts now, while the app is surely visible: Rust may take seconds to start
    // (model loading), Android refuses a service asked for from the background, and it silences
    // the microphone of an app that leaves the screen without one.
    final bool keepsRecordingInBackground = await _recordingNotifier
        .show(RecordingNotificationState.recording, title: requestedTitle);
    final RecordingStartOutcome outcome =
        await _recordingBridge.startRecordingWithTitle(requestedTitle);
    if (outcome == RecordingStartOutcome.started) {
      _state = RecordingUiState.recording;
      _activeTitle = requestedTitle;
      if (!keepsRecordingInBackground) {
        _errorMessage = backgroundRecordingRefusedMessage;
      }
    } else {
      _state = RecordingUiState.idle;
      _errorMessage = _startErrorMessage(outcome, l10n.unableToStartRecording);
      unawaited(_recordingNotifier.show(RecordingNotificationState.idle));
    }
    notifyListeners();
  }

  /// Pauses the active recording.
  ///
  /// Inputs: None; ignored unless recording and no other request is pending.
  /// Outputs: Completes once Rust reports the outcome.
  /// Side effects: Invokes the FFI bridge; on success the state becomes `paused`, on failure the
  /// recording continues and an error is shown. Notifies listeners.
  Future<void> pauseRecording() async {
    if (_state != RecordingUiState.recording || _isTransitioning) {
      return;
    }
    _isTransitioning = true;
    _errorMessage = null;
    final bool didPause = await _recordingBridge.pauseRecording();
    _isTransitioning = false;
    if (didPause) {
      _state = RecordingUiState.paused;
      unawaited(_recordingNotifier.show(RecordingNotificationState.paused,
          title: _activeTitle));
    } else {
      _errorMessage = l10n.unableToPauseRecording;
    }
    notifyListeners();
  }

  /// Resumes a paused recording into the same meeting.
  ///
  /// Inputs: None; ignored unless paused and no other request is pending.
  /// Outputs: Completes once Rust reports the outcome.
  /// Side effects: Shows the recording in the notification first (back to paused on failure),
  /// then invokes `startRecording` on the bridge, which resumes the paused meeting (a
  /// titled start would create a new meeting). Notifies listeners.
  Future<void> resumeRecording() async {
    if (_state != RecordingUiState.paused || _isTransitioning) {
      return;
    }
    _isTransitioning = true;
    _errorMessage = null;
    if (!await _microphonePermission.ensureGranted()) {
      _isTransitioning = false;
      _errorMessage = _permissionDeniedMessage;
      notifyListeners();
      return;
    }
    // Shown before Rust reopens the microphone, for the reason given in `startRecording`.
    final bool keepsRecordingInBackground = await _recordingNotifier
        .show(RecordingNotificationState.recording, title: _activeTitle);
    final RecordingStartOutcome outcome =
        await _recordingBridge.startRecording();
    _isTransitioning = false;
    if (outcome == RecordingStartOutcome.started) {
      _state = RecordingUiState.recording;
      if (!keepsRecordingInBackground) {
        _errorMessage = backgroundRecordingRefusedMessage;
      }
    } else {
      _errorMessage = _startErrorMessage(outcome, l10n.unableToResumeRecording);
      unawaited(_recordingNotifier.show(RecordingNotificationState.paused,
          title: _activeTitle));
    }
    notifyListeners();
  }

  /// Stops the active or paused recording and waits until its audio is transcribed.
  ///
  /// Inputs: None; ignored unless recording or paused and no other request is pending.
  /// Outputs: Completes once Rust has persisted every chunk, or reported a failure.
  /// Side effects: Shows `stopping` meanwhile; on success returns to `idle` with "Recording
  /// saved", on failure restores the previous state and shows an error. Notifies listeners.
  Future<void> stopRecording() async {
    final RecordingUiState stateBeforeStop = _state;
    if ((stateBeforeStop != RecordingUiState.recording &&
            stateBeforeStop != RecordingUiState.paused) ||
        _isTransitioning) {
      return;
    }
    _state = RecordingUiState.stopping;
    _errorMessage = null;
    notifyListeners();
    unawaited(_recordingNotifier.show(RecordingNotificationState.stopping,
        title: _activeTitle));
    _isStopping = true;
    final bool didStop = await _recordingBridge.stopRecording();
    _isStopping = false;
    if (didStop) {
      _finishStop();
      unawaited(_recordingNotifier.show(RecordingNotificationState.idle));
    } else {
      _state = stateBeforeStop;
      _errorMessage = l10n.unableToStopRecording;
      unawaited(_recordingNotifier.show(
          stateBeforeStop == RecordingUiState.paused
              ? RecordingNotificationState.paused
              : RecordingNotificationState.recording,
          title: _activeTitle));
    }
    notifyListeners();
  }

  /// Returns to idle with "Recording saved" after Rust finished a stop.
  void _finishStop() {
    _state = RecordingUiState.idle;
    _activeTitle = null;
    _completionMessage = l10n.recordingSaved;
  }

  /// Follows a change the owner made from the notification or lock screen.
  ///
  /// Inputs: The state Rust has already reached. Changes that do not follow from the current
  /// state (such as a late report after the app stopped the recording itself) are ignored.
  /// Side effects: Updates the state and notifies listeners; never calls the bridge.
  void _applyExternalChange(RecordingNotificationState change) {
    final RecordingUiState previousState = _state;
    switch (change) {
      case RecordingNotificationState.recording
          when previousState == RecordingUiState.paused:
        _state = RecordingUiState.recording;
      case RecordingNotificationState.paused
          when previousState == RecordingUiState.recording:
        _state = RecordingUiState.paused;
      case RecordingNotificationState.stopping
          when previousState == RecordingUiState.recording ||
              previousState == RecordingUiState.paused:
        _state = RecordingUiState.stopping;
      case RecordingNotificationState.idle
          when previousState == RecordingUiState.stopping && !_isStopping:
        _finishStop();
      default:
        return;
    }
    _errorMessage = null;
    notifyListeners();
  }

  /// Stops listening to the notifier. Side effects: Cancels the subscription.
  @override
  void dispose() {
    unawaited(_externalChanges.cancel());
    super.dispose();
  }
}

/// Shown when the owner refuses microphone access.
String get _permissionDeniedMessage => l10n.microphoneAccessIsNeededToRecord;

/// Shown while recording when Android refused the foreground service: leaving the app would
/// silence the microphone.
String get backgroundRecordingRefusedMessage => l10n.keepTheAppOpenAndroidDid;

/// The error for a start or resume that did not record.
///
/// Inputs: The outcome and the generic message for it. Outputs: The message to show.
/// Side effects: None.
String _startErrorMessage(RecordingStartOutcome outcome, String fallback) =>
    outcome == RecordingStartOutcome.microphoneUnavailable
        ? l10n.unableToOpenTheMicrophone
        : fallback;
