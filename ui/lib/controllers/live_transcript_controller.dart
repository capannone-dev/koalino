// Follows the transcript of the meeting being recorded, chunk by chunk (Issue #52). Rust saves
// each chunk's segments as soon as they are transcribed, so this controller polls the active
// meeting while a recording runs and once more after Stop, when the last chunks are saved. The
// lines stay on screen until the next recording starts.

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/recording_bridge.dart';

/// Owns the live transcript shown on the Record screen.
final class LiveTranscriptController extends ChangeNotifier {
  /// Creates the controller and starts following `recordingController`.
  ///
  /// Inputs: The app's recording controller (whose state drives polling), the recording bridge
  /// (which reports the active meeting), the meeting bridge (which reads its segments), and the
  /// poll interval.
  /// Outputs: A controller with no segments.
  /// Side effects: Listens to `recordingController` until `dispose`.
  LiveTranscriptController({
    required RecordingController recordingController,
    required RecordingBridge recordingBridge,
    required MeetingBridge meetingBridge,
    Duration pollInterval = const Duration(seconds: 2),
  })  : _recordingController = recordingController,
        _recordingBridge = recordingBridge,
        _meetingBridge = meetingBridge,
        _pollInterval = pollInterval {
    _recordingController.addListener(_followRecordingState);
  }

  final RecordingController _recordingController;
  final RecordingBridge _recordingBridge;
  final MeetingBridge _meetingBridge;
  final Duration _pollInterval;

  Timer? _pollTimer;
  RecordingUiState _lastState = RecordingUiState.idle;

  /// The meeting being followed, learned from Rust once the recording started.
  String? _meetingId;
  List<MeetingTranscriptSegment> _segments = const <MeetingTranscriptSegment>[];
  bool _isRefreshing = false;

  /// Set when a read was asked for during another, so the newest state is read afterwards.
  bool _refreshRequested = false;
  bool _isDisposed = false;

  /// The followed meeting's segments in timeline order; empty before the first chunk.
  List<MeetingTranscriptSegment> get segments => _segments;

  /// Whether the Record screen should show the transcript area: during a recording, and
  /// afterwards while a finished transcript is still shown.
  bool get isVisible =>
      _recordingController.state != RecordingUiState.idle ||
      _segments.isNotEmpty;

  /// Starts, keeps or stops polling as the recording state changes.
  ///
  /// Side effects: A new recording clears the previous transcript; leaving the recording states
  /// stops the timer after one last read. Notifies listeners when the transcript is cleared.
  void _followRecordingState() {
    final RecordingUiState state = _recordingController.state;
    final RecordingUiState previousState = _lastState;
    _lastState = state;
    if (state == RecordingUiState.starting &&
        previousState != RecordingUiState.starting) {
      _pollTimer?.cancel();
      _pollTimer = null;
      _meetingId = null;
      _segments = const <MeetingTranscriptSegment>[];
      notifyListeners();
      return;
    }
    final bool isActive = state == RecordingUiState.recording ||
        state == RecordingUiState.paused ||
        state == RecordingUiState.stopping;
    if (isActive && _pollTimer == null) {
      unawaited(refresh());
      _pollTimer = Timer.periodic(_pollInterval, (_) => unawaited(refresh()));
    } else if (!isActive && _pollTimer != null) {
      _pollTimer?.cancel();
      _pollTimer = null;
      // The last chunks were saved during Stop: read them once more.
      unawaited(refresh());
    }
  }

  /// Reads the followed meeting's segments.
  ///
  /// Outputs: Completes after the read; a failed read keeps the lines already shown.
  /// Side effects: Asks Rust for the active meeting until it is known, then reads the meeting
  /// over FFI. Notifies listeners when the segments change. A call made during another read
  /// runs once that read ends.
  Future<void> refresh() async {
    if (_isDisposed) {
      return;
    }
    if (_isRefreshing) {
      _refreshRequested = true;
      return;
    }
    _isRefreshing = true;
    try {
      final String? meetingId =
          _meetingId ??= await _recordingBridge.activeMeetingId();
      if (meetingId == null) {
        return;
      }
      final MeetingDetail detail = await _meetingBridge.getMeeting(meetingId);
      if (_isDisposed || meetingId != _meetingId) {
        return;
      }
      if (!listEquals(_segments.map(_segmentKey).toList(),
          detail.segments.map(_segmentKey).toList())) {
        _segments =
            List<MeetingTranscriptSegment>.unmodifiable(detail.segments);
        notifyListeners();
      }
    } on Object {
      // A missed read is retried at the next tick; the lines already shown stay.
    } finally {
      _isRefreshing = false;
    }
    if (_refreshRequested) {
      _refreshRequested = false;
      await refresh();
    }
  }

  @override
  void dispose() {
    _isDisposed = true;
    _pollTimer?.cancel();
    _recordingController.removeListener(_followRecordingState);
    super.dispose();
  }
}

/// What identifies a segment's displayed content.
String _segmentKey(MeetingTranscriptSegment segment) =>
    '${segment.startTimeMs}|${segment.endTimeMs}|${segment.speaker}|${segment.text}';
