// Asks for microphone access before a recording starts (Issue #45). Android grants it at run
// time, through `MicrophonePermission.kt` on the `note_taker/microphone` channel; Linux and
// Windows have no runtime prompt (Windows' privacy setting makes the open fail instead, which
// Rust reports as status 7).

import 'dart:io' show Platform;

import 'package:flutter/services.dart' show MethodChannel, PlatformException;

/// The platform channel `MainActivity` registers for the microphone permission.
const String microphoneChannelName = 'note_taker/microphone';

/// Microphone access, as the recorder needs it.
abstract interface class MicrophonePermission {
  /// Ensures the app may record.
  ///
  /// Inputs: None.
  /// Outputs: `true` once access is granted; `false` when the owner refuses it.
  /// Side effects: May show the system permission dialog.
  Future<bool> ensureGranted();
}

/// A permission that is always granted, for platforms without a runtime prompt.
final class GrantedMicrophonePermission implements MicrophonePermission {
  /// Creates the permission.
  const GrantedMicrophonePermission();

  /// Outputs: Always `true`. Side effects: None.
  @override
  Future<bool> ensureGranted() async => true;
}

/// Android's `RECORD_AUDIO` runtime permission over the `note_taker/microphone` channel.
final class AndroidMicrophonePermission implements MicrophonePermission {
  /// Creates the permission.
  const AndroidMicrophonePermission();

  static const MethodChannel _channel = MethodChannel(microphoneChannelName);

  /// Side effects: Shows Android's dialog unless access was granted already (or refused
  /// permanently, which answers `false` at once). A channel failure counts as refused.
  @override
  Future<bool> ensureGranted() async {
    try {
      return await _channel.invokeMethod<bool>('ensureGranted') ?? false;
    } on PlatformException {
      return false;
    }
  }
}

/// The microphone permission for the running platform.
///
/// Inputs: Reads the current platform. Outputs: The Android permission on Android, otherwise
/// one that is always granted. Side effects: None.
MicrophonePermission platformMicrophonePermission() => Platform.isAndroid
    ? const AndroidMicrophonePermission()
    : const GrantedMicrophonePermission();
