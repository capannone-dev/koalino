// Decides when the Android app lock covers the app: at start-up and after the app spent
// `lockAfterBackground` out of sight, while the setting is on. Only the UI is locked; the Rust
// core keeps running underneath, so recording and device sync are not interrupted.

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/app_lock_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// How long the app may be out of sight before it locks again, so a quick switch to another
/// app (or the file picker) does not ask again.
const Duration lockAfterBackground = Duration(seconds: 30);

/// Whether the app's screens may show.
enum AppLockPhase {
  /// The setting is being read; nothing of the app shows yet.
  checking,

  /// The lock covers the app.
  locked,

  /// The app is open.
  unlocked,
}

/// Owns the app-lock state and the setting.
final class AppLockController extends ChangeNotifier {
  /// Creates the controller.
  ///
  /// Inputs: The platform bridge, and a clock (tests pass a fake one).
  /// Side effects: None until `initialize`.
  AppLockController({
    required AppLockBridge appLockBridge,
    DateTime Function()? clock,
  })  : _appLockBridge = appLockBridge,
        _clock = clock ?? DateTime.now;

  final AppLockBridge _appLockBridge;
  final DateTime Function() _clock;

  AppLockPhase _phase = AppLockPhase.checking;
  AppLockStatus? _status;
  String? _message;
  bool _isAuthenticating = false;

  /// Whether the next `promptIfPending` should show the prompt (once per lock).
  bool _promptPending = false;

  /// When the app went out of sight while unlocked, or null.
  DateTime? _hiddenAt;

  /// Whether the app's screens may show.
  AppLockPhase get phase => _phase;

  /// The last status read, or null before `initialize` or after it failed.
  AppLockStatus? get status => _status;

  /// A display-safe explanation of the last failure, or null.
  String? get message => _message;

  /// Whether the system prompt is open.
  bool get isAuthenticating => _isAuthenticating;

  /// Reads the setting and locks the app when it is on.
  ///
  /// Outputs: Completes with `phase` locked or unlocked. A status failure keeps the app locked
  /// (the lock fails closed), with a message.
  /// Side effects: Calls the platform; notifies listeners.
  Future<void> initialize() async {
    try {
      _status = await _appLockBridge.status();
      _lock(when: _status?.locks ?? false);
    } on Object {
      _status = null;
      _message = l10n.unableToCheckTheAppLock;
      _lock(when: true);
    }
    notifyListeners();
  }

  /// Shows the prompt if this lock has not shown it yet; the gate calls it once the app is in
  /// view, so the owner does not need to tap Unlock first.
  ///
  /// Side effects: As `unlock`, at most once per lock.
  Future<void> promptIfPending() async {
    if (!_promptPending || _phase != AppLockPhase.locked) {
      return;
    }
    _promptPending = false;
    await unlock();
  }

  /// Shows the system prompt and opens the app when the owner authenticates.
  ///
  /// Outputs: Completes when the prompt closes; a cancel keeps the app locked silently. A
  /// prompt already open makes this a no-op.
  /// Side effects: Shows the system prompt; re-reads the status when the last read failed;
  /// notifies listeners.
  Future<void> unlock() async {
    if (_isAuthenticating) {
      return;
    }
    _isAuthenticating = true;
    _message = null;
    notifyListeners();
    try {
      _status ??= await _appLockBridge.status();
      if (!(_status?.locks ?? false) || await _appLockBridge.authenticate()) {
        _phase = AppLockPhase.unlocked;
      }
    } on Object {
      _message = l10n.unableToShowTheUnlockPrompt;
    } finally {
      _isAuthenticating = false;
      _hiddenAt = null;
      notifyListeners();
    }
  }

  /// Notes that the app went out of sight.
  ///
  /// Side effects: Starts the grace period, unless the app is locked or the prompt itself
  /// hid it (the PIN screen is a separate activity).
  void appHidden() {
    if (_phase == AppLockPhase.unlocked && !_isAuthenticating) {
      _hiddenAt = _clock();
    }
  }

  /// Notes that the app is back in view, and locks it when it was away for at least
  /// `lockAfterBackground` while the setting is on.
  ///
  /// Side effects: May lock the app and notify listeners.
  void appShown() {
    final DateTime? hiddenAt = _hiddenAt;
    _hiddenAt = null;
    if (hiddenAt == null || !(_status?.locks ?? false)) {
      return;
    }
    if (_clock().difference(hiddenAt) >= lockAfterBackground) {
      _lock(when: true);
      notifyListeners();
    }
  }

  /// Turns the app lock on or off, after the owner authenticates.
  ///
  /// Inputs: The new value.
  /// Outputs: Completes when stored, or unchanged when the owner cancelled or the device has
  /// no screen lock (with a message).
  /// Side effects: Shows the system prompt; writes the setting; notifies listeners.
  Future<void> setEnabled(bool enabled) async {
    final AppLockStatus? current = _status;
    if (current == null || _isAuthenticating) {
      return;
    }
    if (!current.supported || !current.deviceSecure) {
      _message = l10n.setAScreenLockOnThis;
      notifyListeners();
      return;
    }
    _isAuthenticating = true;
    _message = null;
    notifyListeners();
    try {
      if (await _appLockBridge.authenticate()) {
        await _appLockBridge.setEnabled(enabled);
        _status = await _appLockBridge.status();
      }
    } on Object {
      _message = l10n.unableToChangeTheAppLock;
    } finally {
      _isAuthenticating = false;
      _hiddenAt = null;
      notifyListeners();
    }
  }

  /// Sets the phase to locked (and arms the automatic prompt) when `when` is true, otherwise
  /// to unlocked.
  void _lock({required bool when}) {
    _phase = when ? AppLockPhase.locked : AppLockPhase.unlocked;
    _promptPending = when;
  }
}
