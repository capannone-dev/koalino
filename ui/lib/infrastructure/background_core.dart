// Hands the Android host the core configuration, so its charging job (`WaitingPassJob.kt`) can
// open the core without Flutter and run a waiting after-meeting pass while Koalino is closed (#87).
// The configuration holds paths, the device name and peers; no secrets cross the channel.

import 'package:flutter/services.dart'
    show MethodChannel, MissingPluginException, PlatformException;

/// The platform channel `MainActivity` registers for the background job's configuration.
const String backgroundCoreChannelName = 'note_taker/background_core';

/// Stores the configuration the app passed to the core, for the background job.
///
/// Inputs: The JSON given to `note_taker_configure` and the directory holding the wrapped key.
/// Outputs: Completes when stored, or at once when the host cannot store it (the job then does
/// nothing; the pass still runs while the app is open).
/// Side effects: Kotlin writes them to private preferences.
Future<void> rememberBackgroundCoreConfiguration({
  required String configurationJson,
  required String keyDirectory,
}) async {
  try {
    await const MethodChannel(backgroundCoreChannelName)
        .invokeMethod<void>('remember', <String, Object?>{
      'configuration': configurationJson,
      'keyDirectory': keyDirectory,
    });
  } on MissingPluginException {
    // No host job on this platform.
  } on PlatformException {
    // Not stored; the pass still runs while the app is open.
  }
}

/// Asks the Android host whether the after-meeting pass may run while Koalino is closed (#87).
///
/// With Power saving on, Android holds every app's background jobs unless the app is exempt from
/// battery optimisation; the owner grants that once, in a system dialog.
class BackgroundPassBridge {
  /// Creates the bridge over [backgroundCoreChannelName].
  const BackgroundPassBridge();

  /// Reports whether Koalino is exempt from battery optimisation.
  ///
  /// Outputs: true or false on Android; null where the host has no such setting.
  /// Side effects: None.
  Future<bool?> isExempt() async {
    try {
      return await const MethodChannel(backgroundCoreChannelName)
          .invokeMethod<bool>('isExempt');
    } on MissingPluginException {
      return null;
    } on PlatformException {
      return null;
    }
  }

  /// Shows Android's dialog asking to exempt Koalino from battery optimisation.
  ///
  /// Outputs: Completes once the dialog is shown; the answer is read with [isExempt] when the
  /// app is back in front. Side effects: Opens a system dialog.
  Future<void> requestExemption() async {
    try {
      await const MethodChannel(backgroundCoreChannelName)
          .invokeMethod<void>('requestExemption');
    } on MissingPluginException {
      // No such setting here.
    } on PlatformException {
      // The dialog could not be shown; the tile keeps offering it.
    }
  }
}
