// A language bridge for tests: it remembers what the core was told and can refuse every change.

import 'package:note_taker_ui/infrastructure/language_bridge.dart';

/// Holds a default language in memory.
final class FakeLanguageBridge implements LanguageBridge {
  FakeLanguageBridge({this.storedDefault = 'auto', this.refuses = false});

  /// The stored default.
  String? storedDefault;

  /// Whether the core refuses changes.
  bool refuses;

  /// The languages chosen for recordings, in order.
  final List<String> recordingLanguages = <String>[];

  @override
  Future<String?> defaultLanguage() async => storedDefault;

  @override
  Future<bool> setDefaultLanguage(String language) async {
    if (refuses) {
      return false;
    }
    storedDefault = language;
    return true;
  }

  @override
  Future<bool> setRecordingLanguage(String language) async {
    if (refuses) {
      return false;
    }
    recordingLanguages.add(language);
    return true;
  }
}
