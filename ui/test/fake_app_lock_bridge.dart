// A scripted app-lock bridge for controller and widget tests, standing in for Android's
// BiometricPrompt and the stored setting.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:note_taker_ui/infrastructure/app_lock_bridge.dart';

/// Answers status, authentication, and setting changes from scripted values.
final class FakeAppLockBridge implements AppLockBridge {
  /// Creates a bridge for a device with or without a screen lock and with the app lock on or off.
  FakeAppLockBridge({
    this.supported = true,
    this.deviceSecure = true,
    this.enabled = false,
  });

  /// Whether this Android version can show the prompt.
  bool supported;

  /// Whether the device has a PIN, pattern, password, or biometric screen lock.
  bool deviceSecure;

  /// The stored app-lock setting.
  bool enabled;

  /// When true, `status` throws, as if the Kotlin channel were missing.
  bool statusFails = false;

  /// The answers the next prompts give, in order; an empty queue means success.
  final List<bool> authenticationResults = <bool>[];

  /// When set, prompts wait on it instead of answering at once.
  Completer<bool>? pendingAuthentication;

  /// How many prompts were shown.
  int authenticationCount = 0;

  /// The values passed to `setEnabled`.
  final List<bool> storedValues = <bool>[];

  @override
  Future<AppLockStatus> status() async {
    if (statusFails) {
      throw PlatformException(code: 'unavailable');
    }
    return AppLockStatus(
      supported: supported,
      deviceSecure: deviceSecure,
      enabled: enabled,
    );
  }

  @override
  Future<bool> authenticate() async {
    authenticationCount += 1;
    if (pendingAuthentication case final Completer<bool> pending) {
      return pending.future;
    }
    return authenticationResults.isEmpty
        ? true
        : authenticationResults.removeAt(0);
  }

  @override
  Future<void> setEnabled(bool value) async {
    storedValues.add(value);
    enabled = value;
  }
}
