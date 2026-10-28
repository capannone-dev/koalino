// Coordinates recording UI state exclusively through the Flutter recording bridge.

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/microphone_permission.dart';
import 'package:note_taker_ui/infrastructure/recording_bridge.dart';

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
  /// permission; granted elsewhere).
  /// Outputs: A controller in the idle state.
  /// Side effects: None.
  RecordingController({
    required RecordingBridge recordingBridge,
    MicrophonePermission microphonePermission =
        const GrantedMicrophonePermission(),
  })  : _recordingBridge = recordingBridge,
        _microphonePermission = microphonePermission;

  final RecordingBridge _recordingBridge;
  final MicrophonePermission _microphonePermission;
  RecordingUiState _state = RecordingUiState.idle;
  String? _errorMessage;
  String? _activeTitle;
  String? _completionMessage;

  /// Whether a pause or resume request is awaiting Rust, so duplicate taps are ignored.
  bool _isTransitioning = false;

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
  /// Side effects: Invokes the FFI bridge, updates UI state, and notifies listening widgets.
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
    final RecordingStartOutcome outcome =
        await _recordingBridge.startRecordingWithTitle(requestedTitle);
    if (outcome == RecordingStartOutcome.started) {
      _state = RecordingUiState.recording;
      _activeTitle = requestedTitle;
    } else {
      _state = RecordingUiState.idle;
      _errorMessage = _startErrorMessage(outcome, 'Unable to start recording');
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
    } else {
      _errorMessage = 'Unable to pause recording';
    }
    notifyListeners();
  }

  /// Resumes a paused recording into the same meeting.
  ///
  /// Inputs: None; ignored unless paused and no other request is pending.
  /// Outputs: Completes once Rust reports the outcome.
  /// Side effects: Invokes `startRecording` on the bridge, which resumes the paused meeting (a
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
    final RecordingStartOutcome outcome =
        await _recordingBridge.startRecording();
    _isTransitioning = false;
    if (outcome == RecordingStartOutcome.started) {
      _state = RecordingUiState.recording;
    } else {
      _errorMessage = _startErrorMessage(outcome, 'Unable to resume recording');
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
    final bool didStop = await _recordingBridge.stopRecording();
    if (didStop) {
      _state = RecordingUiState.idle;
      _activeTitle = null;
      _completionMessage = 'Recording saved';
    } else {
      _state = stateBeforeStop;
      _errorMessage = 'Unable to stop recording';
    }
    notifyListeners();
  }
}

/// Shown when the owner refuses microphone access.
const String _permissionDeniedMessage = 'Microphone access is needed to record';

/// The error for a start or resume that did not record.
///
/// Inputs: The outcome and the generic message for it. Outputs: The message to show.
/// Side effects: None.
String _startErrorMessage(RecordingStartOutcome outcome, String fallback) =>
    outcome == RecordingStartOutcome.microphoneUnavailable
        ? 'Unable to open the microphone'
        : fallback;
