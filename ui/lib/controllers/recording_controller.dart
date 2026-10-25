// Coordinates recording UI state exclusively through the Flutter recording bridge.

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/recording_bridge.dart';

/// Describes the recording status rendered by the Flutter presentation layer.
enum RecordingUiState {
  /// No recording request is currently active.
  idle,

  /// A start request has been sent to the Rust bridge and is awaiting its result.
  starting,

  /// The Rust bridge accepted the recording request.
  recording,
}

/// Owns user-visible recording state while depending only on the Dart bridge interface.
final class RecordingController extends ChangeNotifier {
  /// Creates a controller whose native work is delegated to `recordingBridge`.
  ///
  /// Inputs: `recordingBridge` must implement the stable Dart bridge contract.
  /// Outputs: A controller in the idle state.
  /// Side effects: None.
  RecordingController({required RecordingBridge recordingBridge})
      : _recordingBridge = recordingBridge;

  final RecordingBridge _recordingBridge;
  RecordingUiState _state = RecordingUiState.idle;
  String? _errorMessage;

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

  /// Requests that the Rust core start a recording session.
  ///
  /// Inputs: None; duplicate requests while a request is pending or active are ignored.
  /// Outputs: Completes once the bridge reports whether the lifecycle command succeeded.
  /// Side effects: Invokes the FFI bridge, updates UI state, and notifies listening widgets.
  Future<void> startRecording() async {
    if (_state != RecordingUiState.idle) {
      return;
    }

    _state = RecordingUiState.starting;
    _errorMessage = null;
    notifyListeners();

    final bool didStart = await _recordingBridge.startRecording();
    if (didStart) {
      _state = RecordingUiState.recording;
    } else {
      _state = RecordingUiState.idle;
      _errorMessage = 'Unable to start recording';
    }
    notifyListeners();
  }
}
