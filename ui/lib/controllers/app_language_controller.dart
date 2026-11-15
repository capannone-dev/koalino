// Holds the language of the app's own text (Issue #124): the system's by default, or one the
// owner picks in Settings. The choice is kept on this device only; the transcription language
// (Issue #62) is a separate setting.

import 'package:flutter/widgets.dart';
import 'package:note_taker_ui/infrastructure/app_language_store.dart';

/// One language the app is translated into: its code and its name in itself.
typedef AppLanguage = ({String code, String nativeName});

/// The app's languages, in the order Settings lists them. The names are endonyms, so the owner
/// can find their own language whatever the app currently shows.
const List<AppLanguage> appLanguages = <AppLanguage>[
  (code: 'en', nativeName: 'English'),
  (code: 'it', nativeName: 'Italiano'),
  (code: 'es', nativeName: 'Español'),
  (code: 'fr', nativeName: 'Français'),
  (code: 'de', nativeName: 'Deutsch'),
  (code: 'pt', nativeName: 'Português'),
  (code: 'zh', nativeName: '简体中文'),
  (code: 'ja', nativeName: '日本語'),
  (code: 'ru', nativeName: 'Русский'),
  (code: 'ar', nativeName: 'العربية'),
];

/// The app language choice: `null` follows the system.
final class AppLanguageController extends ChangeNotifier {
  /// Creates the controller over `store`, which keeps the choice between runs.
  ///
  /// Inputs: The store. Outputs: A controller following the system until [load] reads a choice.
  /// Side effects: None.
  AppLanguageController({required AppLanguageStore store}) : _store = store;

  final AppLanguageStore _store;
  String? _languageCode;

  /// The chosen language's code, or `null` to follow the system.
  String? get languageCode => _languageCode;

  /// The locale to give `MaterialApp.locale`: `null` follows the system.
  Locale? get locale {
    final String? code = _languageCode;
    return code == null ? null : Locale(code);
  }

  /// Reads the stored choice.
  ///
  /// Inputs: None. Outputs: Completes once read; an unreadable or unknown choice follows the
  /// system. Side effects: Reads the store; notifies listeners.
  Future<void> load() async {
    final String? stored = await _store.read();
    _languageCode = _isKnown(stored) ? stored : null;
    notifyListeners();
  }

  /// Picks the app's language, or the system's with `null`.
  ///
  /// Inputs: A code from [appLanguages], or `null`. Outputs: Completes once stored; an unknown
  /// code is ignored. Side effects: Notifies listeners at once, then writes the store (a failed
  /// write keeps the choice for this run only).
  Future<void> choose(String? code) async {
    if (code != null && !_isKnown(code)) {
      return;
    }
    _languageCode = code;
    notifyListeners();
    await _store.write(code);
  }

  static bool _isKnown(String? code) =>
      code != null &&
      appLanguages.any((AppLanguage language) => language.code == code);
}
