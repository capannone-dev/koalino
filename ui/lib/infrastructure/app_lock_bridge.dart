// Talks to the Android app lock in Kotlin (`AppLock.kt`): the stored setting, whether the device
// can authenticate, and the system BiometricPrompt (fingerprint, face, or the screen lock's PIN,
// pattern, or password).

import 'package:flutter/services.dart' show MethodChannel;

/// The platform channel `MainActivity` registers for the app lock.
const String appLockChannelName = 'note_taker/app_lock';

/// What the device and the stored setting allow.
final class AppLockStatus {
  /// Creates a status.
  ///
  /// Inputs: `supported` (the Android version can show the prompt with a screen-lock fallback,
  /// Android 10 or later), `deviceSecure` (a screen lock is set), and `enabled` (the setting).
  const AppLockStatus({
    required this.supported,
    required this.deviceSecure,
    required this.enabled,
  });

  /// Decodes the `status` channel answer, a map of the three booleans.
  ///
  /// Inputs: The answer; a missing or non-boolean value reads as false.
  /// Outputs: The status. Side effects: None.
  factory AppLockStatus.fromChannel(Map<Object?, Object?>? answer) =>
      AppLockStatus(
        supported: answer?['supported'] == true,
        deviceSecure: answer?['deviceSecure'] == true,
        enabled: answer?['enabled'] == true,
      );

  /// Whether the Android version can show the prompt.
  final bool supported;

  /// Whether the device has a screen lock to authenticate with.
  final bool deviceSecure;

  /// Whether the owner turned the app lock on.
  final bool enabled;

  /// Whether the app should lock: the setting is on and the device can authenticate.
  bool get locks => enabled && supported && deviceSecure;
}

/// The app lock's platform operations.
abstract interface class AppLockBridge {
  /// Reads the setting and the device's capabilities.
  Future<AppLockStatus> status();

  /// Shows the system prompt; true when the owner authenticated, false when they cancelled.
  /// Throws `PlatformException` when no prompt can be shown.
  Future<bool> authenticate();

  /// Stores the setting.
  Future<void> setEnabled(bool enabled);
}

/// Implements the app lock over the `note_taker/app_lock` method channel.
final class AndroidAppLockBridge implements AppLockBridge {
  /// Creates the bridge.
  const AndroidAppLockBridge();

  static const MethodChannel _channel = MethodChannel(appLockChannelName);

  /// Side effects: Reads SharedPreferences and the KeyguardManager in Kotlin.
  @override
  Future<AppLockStatus> status() async => AppLockStatus.fromChannel(
      await _channel.invokeMethod<Map<Object?, Object?>>('status'));

  /// Side effects: Shows the system prompt over the app; the PIN screen hides the app while it
  /// is open.
  @override
  Future<bool> authenticate() async =>
      await _channel.invokeMethod<bool>('authenticate') ?? false;

  /// Side effects: Writes SharedPreferences and hides the app's content from the recent-apps
  /// screen while the lock is on (Android 13 and later).
  @override
  Future<void> setEnabled(bool enabled) => _channel
      .invokeMethod<void>('setEnabled', <String, Object?>{'enabled': enabled});
}
