// Holds Settings → Developer's state (#187): whether the section is unlocked, the seven-tap count
// on the version row that unlocks it, and the core's list of overridable app variables. Every
// change goes to the core, which validates and saves it for the next start.
import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/developer_settings_bridge.dart';

/// How many taps on the version row unlock the section, as Android's "Build number" does.
const int developerUnlockTaps = 7;

/// Taps further apart than this start the count again.
const Duration _tapWindow = Duration(seconds: 3);

/// Owns the developer section's state for Settings.
final class DeveloperSettingsController extends ChangeNotifier {
  /// Creates a controller over `bridge`; `now` is replaced in tests.
  DeveloperSettingsController({
    required DeveloperSettingsBridge bridge,
    DateTime Function() now = DateTime.now,
  })  : _bridge = bridge,
        _now = now;

  final DeveloperSettingsBridge _bridge;
  final DateTime Function() _now;

  /// The last state from the core, or null before the first load.
  DeveloperSettingsState? get state => _state;
  DeveloperSettingsState? _state;

  /// Whether the section is shown.
  bool get unlocked => _state?.unlocked ?? false;

  int _taps = 0;
  DateTime? _lastTap;

  /// Loads the state; a core that cannot answer leaves the section hidden.
  ///
  /// Side effects: Asks the core; notifies listeners.
  Future<void> load() async {
    try {
      _apply(await _bridge.get());
    } on Object {
      // The section stays hidden; nothing else depends on it.
    }
  }

  /// Counts a tap on the version row and unlocks the section at the seventh.
  ///
  /// Outputs: The taps still needed (0 once unlocked, also when it already was).
  /// Side effects: At the seventh tap, asks the core to unlock and notifies listeners.
  Future<int> tapVersion() async {
    if (unlocked) {
      return 0;
    }
    final DateTime now = _now();
    final DateTime? last = _lastTap;
    _taps = last != null && now.difference(last) <= _tapWindow ? _taps + 1 : 1;
    _lastTap = now;
    final int remaining = developerUnlockTaps - _taps;
    if (remaining > 0) {
      return remaining;
    }
    _taps = 0;
    _apply(await _bridge.unlock());
    return 0;
  }

  /// Saves `value` for `name` (null goes back to the default).
  ///
  /// Throws `DeveloperSettingsException` when the core refuses it.
  /// Side effects: The core writes its file; notifies listeners.
  Future<void> set(String name, String? value) async =>
      _apply(await _bridge.set(name, value));

  /// Clears every saved value. Side effects: As `set`.
  Future<void> reset() async => _apply(await _bridge.reset());

  /// Clears every value and hides the section. Side effects: As `set`.
  Future<void> lock() async => _apply(await _bridge.lock());

  void _apply(DeveloperSettingsState state) {
    _state = state;
    notifyListeners();
  }
}
