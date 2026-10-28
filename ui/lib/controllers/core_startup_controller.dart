// Tracks start-up of the Rust core (database, encryption key, device sync) so the app opens only
// once its data is unlocked, and can retry after a keychain failure.

import 'package:flutter/foundation.dart';

/// `note_taker_start_runtime`'s status when the keychain is missing, locked, or refused.
const int keychainUnavailableStatus = 6;

/// Where start-up stands.
enum CoreStartupState {
  /// The core is opening; the keychain may be showing an unlock prompt.
  starting,

  /// The core is open; the app can be shown.
  ready,

  /// The keychain holding the encryption key could not be used. There is no key-file fallback,
  /// so the owner unlocks or allows it and retries.
  keychainUnavailable,

  /// The core could not open the database or its settings (or the library is missing).
  failed,
}

/// Starts the core and exposes the outcome to `CoreStartupGate`.
final class CoreStartupController extends ChangeNotifier {
  /// Creates a controller that starts the core with `startCore` (for example `startCore` from
  /// `app_configuration.dart`), which returns Rust's status.
  CoreStartupController({required Future<int> Function() startCore})
      : _startCore = startCore;

  final Future<int> Function() _startCore;

  /// The current state; `starting` until the first `start` completes.
  CoreStartupState state = CoreStartupState.starting;

  /// Starts (or retries starting) the core.
  ///
  /// Outputs: Completes when the state reflects the outcome.
  /// Side effects: Opens the core through `startCore`; notifies listeners when it begins and
  /// when it ends. A throwing start, such as a missing native library, counts as `failed`.
  Future<void> start() async {
    state = CoreStartupState.starting;
    notifyListeners();
    try {
      final int status = await _startCore();
      state = switch (status) {
        0 => CoreStartupState.ready,
        keychainUnavailableStatus => CoreStartupState.keychainUnavailable,
        _ => CoreStartupState.failed,
      };
    } on Object {
      state = CoreStartupState.failed;
    }
    notifyListeners();
  }
}
