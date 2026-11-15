// Keeps the recording visible and controllable outside the app (Issue #47). On Android the
// recorder's state is mirrored into `RecordingService.kt`, a microphone foreground service whose
// notification (shown on the lock screen too) offers Pause, Resume, and Stop. Those actions call
// the Rust core over JNI directly, so they work while the app is in the background or locked,
// and the service reports each change back on the same channel. Desktops need no service: the
// core keeps capturing while the window is minimized.

import 'dart:async';
import 'dart:io' show Platform;

import 'package:note_taker_ui/l10n/l10n.dart';
import 'package:flutter/services.dart'
    show MethodCall, MethodChannel, PlatformException;

/// The platform channel `MainActivity` registers for the recording service.
const String recordingServiceChannelName = 'note_taker/recording_service';

/// What the notification shows.
enum RecordingNotificationState {
  /// Recording; offers Pause and Stop.
  recording,

  /// Paused; offers Resume and Stop.
  paused,

  /// Transcribing the rest after a stop; no actions.
  stopping,

  /// Nothing to show; the service ends.
  idle,
}

/// Mirrors the recorder outside the app and reports changes made there.
abstract interface class RecordingNotifier {
  /// Shows `state` with the meeting's `title` (`null` when untitled).
  ///
  /// Outputs: `false` when the platform refused the service, so a recording would be silenced
  /// once the app leaves the screen; `true` otherwise, including where no service is needed.
  /// Side effects: On Android starts, updates, or ends the foreground service.
  Future<bool> show(RecordingNotificationState state, {String? title});

  /// States the owner reached from the notification; Rust has already changed.
  Stream<RecordingNotificationState> get externalChanges;
}

/// A notifier that shows nothing, for desktops and tests.
final class NoRecordingNotifier implements RecordingNotifier {
  /// Creates the notifier.
  const NoRecordingNotifier();

  /// Outputs: Always `true`: desktops keep capturing without a service.
  /// Side effects: None.
  @override
  Future<bool> show(RecordingNotificationState state, {String? title}) async =>
      true;

  /// Never emits.
  @override
  Stream<RecordingNotificationState> get externalChanges =>
      const Stream<RecordingNotificationState>.empty();
}

/// The Android foreground service over `note_taker/recording_service`.
final class AndroidRecordingNotifier implements RecordingNotifier {
  /// Creates the notifier and starts listening for the service's reports.
  ///
  /// Side effects: Installs the channel's method-call handler.
  AndroidRecordingNotifier() {
    _channel.setMethodCallHandler(_handleServiceCall);
  }

  static const MethodChannel _channel =
      MethodChannel(recordingServiceChannelName);

  final StreamController<RecordingNotificationState> _changes =
      StreamController<RecordingNotificationState>.broadcast();

  /// Outputs: `false` when Android refused to start the service (for example when asked while
  /// the app is not visible); the caller tells the owner to keep the app open.
  /// Side effects: Starts or updates the service, or ends it for `idle`.
  @override
  Future<bool> show(RecordingNotificationState state, {String? title}) async {
    try {
      await _channel.invokeMethod<void>('show', <String, String?>{
        'state': state.name,
        'title': title,
        // Shown by the service while Android silences the recording for a call (#111).
        'silencedText': l10n.microphoneSilencedByCall,
      });
      return true;
    } on PlatformException {
      return false;
    }
  }

  @override
  Stream<RecordingNotificationState> get externalChanges => _changes.stream;

  /// Decodes `stateChanged(state)` from the service; unknown states are ignored.
  Future<void> _handleServiceCall(MethodCall call) async {
    if (call.method != 'stateChanged') {
      return;
    }
    for (final RecordingNotificationState state
        in RecordingNotificationState.values) {
      if (state.name == call.arguments) {
        _changes.add(state);
      }
    }
  }
}

/// The notifier for the running platform: the service on Android, nothing elsewhere.
RecordingNotifier platformRecordingNotifier() => Platform.isAndroid
    ? AndroidRecordingNotifier()
    : const NoRecordingNotifier();
