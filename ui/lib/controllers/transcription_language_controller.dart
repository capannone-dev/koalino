// Holds the transcription language (Issue #62): the default kept in Settings, and the language of
// the next or current recording, which starts on the default and can change at any time. The core
// applies and stores the choices; this only offers them and reports a refusal in plain words.

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/infrastructure/language_bridge.dart';

/// One option in the selectors.
typedef LanguageChoice = ({String code, String label});

/// The code that lets Whisper detect the language.
const String autoLanguageCode = 'auto';

/// AUTO, then every language Whisper knows, by name (`whisper.cpp`'s list).
const List<LanguageChoice> transcriptionLanguageChoices = <LanguageChoice>[
  (code: autoLanguageCode, label: 'Auto'),
  (code: 'af', label: 'Afrikaans'),
  (code: 'sq', label: 'Albanian'),
  (code: 'am', label: 'Amharic'),
  (code: 'ar', label: 'Arabic'),
  (code: 'hy', label: 'Armenian'),
  (code: 'as', label: 'Assamese'),
  (code: 'az', label: 'Azerbaijani'),
  (code: 'ba', label: 'Bashkir'),
  (code: 'eu', label: 'Basque'),
  (code: 'be', label: 'Belarusian'),
  (code: 'bn', label: 'Bengali'),
  (code: 'bs', label: 'Bosnian'),
  (code: 'br', label: 'Breton'),
  (code: 'bg', label: 'Bulgarian'),
  (code: 'yue', label: 'Cantonese'),
  (code: 'ca', label: 'Catalan'),
  (code: 'zh', label: 'Chinese'),
  (code: 'hr', label: 'Croatian'),
  (code: 'cs', label: 'Czech'),
  (code: 'da', label: 'Danish'),
  (code: 'nl', label: 'Dutch'),
  (code: 'en', label: 'English'),
  (code: 'et', label: 'Estonian'),
  (code: 'fo', label: 'Faroese'),
  (code: 'fi', label: 'Finnish'),
  (code: 'fr', label: 'French'),
  (code: 'gl', label: 'Galician'),
  (code: 'ka', label: 'Georgian'),
  (code: 'de', label: 'German'),
  (code: 'el', label: 'Greek'),
  (code: 'gu', label: 'Gujarati'),
  (code: 'ht', label: 'Haitian Creole'),
  (code: 'ha', label: 'Hausa'),
  (code: 'haw', label: 'Hawaiian'),
  (code: 'he', label: 'Hebrew'),
  (code: 'hi', label: 'Hindi'),
  (code: 'hu', label: 'Hungarian'),
  (code: 'is', label: 'Icelandic'),
  (code: 'id', label: 'Indonesian'),
  (code: 'it', label: 'Italian'),
  (code: 'ja', label: 'Japanese'),
  (code: 'jw', label: 'Javanese'),
  (code: 'kn', label: 'Kannada'),
  (code: 'kk', label: 'Kazakh'),
  (code: 'km', label: 'Khmer'),
  (code: 'ko', label: 'Korean'),
  (code: 'lo', label: 'Lao'),
  (code: 'la', label: 'Latin'),
  (code: 'lv', label: 'Latvian'),
  (code: 'ln', label: 'Lingala'),
  (code: 'lt', label: 'Lithuanian'),
  (code: 'lb', label: 'Luxembourgish'),
  (code: 'mk', label: 'Macedonian'),
  (code: 'mg', label: 'Malagasy'),
  (code: 'ms', label: 'Malay'),
  (code: 'ml', label: 'Malayalam'),
  (code: 'mt', label: 'Maltese'),
  (code: 'mi', label: 'Maori'),
  (code: 'mr', label: 'Marathi'),
  (code: 'mn', label: 'Mongolian'),
  (code: 'my', label: 'Myanmar'),
  (code: 'ne', label: 'Nepali'),
  (code: 'no', label: 'Norwegian'),
  (code: 'nn', label: 'Nynorsk'),
  (code: 'oc', label: 'Occitan'),
  (code: 'ps', label: 'Pashto'),
  (code: 'fa', label: 'Persian'),
  (code: 'pl', label: 'Polish'),
  (code: 'pt', label: 'Portuguese'),
  (code: 'pa', label: 'Punjabi'),
  (code: 'ro', label: 'Romanian'),
  (code: 'ru', label: 'Russian'),
  (code: 'sa', label: 'Sanskrit'),
  (code: 'sr', label: 'Serbian'),
  (code: 'sn', label: 'Shona'),
  (code: 'sd', label: 'Sindhi'),
  (code: 'si', label: 'Sinhala'),
  (code: 'sk', label: 'Slovak'),
  (code: 'sl', label: 'Slovenian'),
  (code: 'so', label: 'Somali'),
  (code: 'es', label: 'Spanish'),
  (code: 'su', label: 'Sundanese'),
  (code: 'sw', label: 'Swahili'),
  (code: 'sv', label: 'Swedish'),
  (code: 'tl', label: 'Tagalog'),
  (code: 'tg', label: 'Tajik'),
  (code: 'ta', label: 'Tamil'),
  (code: 'tt', label: 'Tatar'),
  (code: 'te', label: 'Telugu'),
  (code: 'th', label: 'Thai'),
  (code: 'bo', label: 'Tibetan'),
  (code: 'tr', label: 'Turkish'),
  (code: 'tk', label: 'Turkmen'),
  (code: 'uk', label: 'Ukrainian'),
  (code: 'ur', label: 'Urdu'),
  (code: 'uz', label: 'Uzbek'),
  (code: 'vi', label: 'Vietnamese'),
  (code: 'cy', label: 'Welsh'),
  (code: 'yi', label: 'Yiddish'),
  (code: 'yo', label: 'Yoruba'),
];

/// Owns the language choices for Settings and the Record screen.
final class TranscriptionLanguageController extends ChangeNotifier {
  /// Creates the controller.
  ///
  /// Inputs: The bridge to the core and, when given, the recording whose lifecycle ends a choice.
  /// Side effects: Listens to `recordingController`; calls nothing until `load`.
  TranscriptionLanguageController({
    required LanguageBridge languageBridge,
    RecordingController? recordingController,
  })  : _languageBridge = languageBridge,
        _recordingController = recordingController {
    _recordingController?.addListener(_followRecording);
    _recordingState = _recordingController?.state ?? RecordingUiState.idle;
  }

  final LanguageBridge _languageBridge;
  final RecordingController? _recordingController;

  String _defaultLanguage = autoLanguageCode;

  /// The language chosen for the next or current recording; `null` follows the default.
  String? _chosenForRecording;
  RecordingUiState _recordingState = RecordingUiState.idle;
  String? _message;
  bool _isSaving = false;
  bool _disposed = false;

  /// Every option, AUTO first.
  List<LanguageChoice> get choices => transcriptionLanguageChoices;

  /// The default language: a code from `choices`.
  String get defaultLanguage => _defaultLanguage;

  /// The language of the next or current recording: a code from `choices`.
  String get recordingLanguage => _chosenForRecording ?? _defaultLanguage;

  /// Why the last change failed, or `null`.
  String? get message => _message;

  /// Whether a change is being saved; the selectors are disabled meanwhile.
  bool get isSaving => _isSaving;

  /// Reads the default language.
  ///
  /// Side effects: Calls the core; a failure keeps AUTO, which the core also falls back to.
  Future<void> load() async {
    final String? stored = await _languageBridge.defaultLanguage();
    if (stored != null &&
        choices.any((LanguageChoice choice) => choice.code == stored)) {
      _defaultLanguage = stored;
    }
    _notify();
  }

  /// Stores a new default, used from the next recording.
  ///
  /// Inputs: A code from `choices`. Side effects: The core stores it; a refusal keeps the old one.
  Future<void> chooseDefault(String language) => _save(
        () => _languageBridge.setDefaultLanguage(language),
        () => _defaultLanguage = language,
      );

  /// Chooses the language of the next recording, or of the rest of the current one.
  ///
  /// Inputs: A code from `choices`.
  /// Side effects: The core applies it to audio from now on; a refusal keeps the old one.
  Future<void> chooseForRecording(String language) => _save(
        () => _languageBridge.setRecordingLanguage(language),
        () => _chosenForRecording = language,
      );

  /// Runs one change, applying it only if the core accepted it.
  Future<void> _save(
      Future<bool> Function() change, void Function() apply) async {
    _isSaving = true;
    _message = null;
    _notify();
    if (await change()) {
      apply();
    } else {
      _message = 'The language could not be changed.';
    }
    _isSaving = false;
    _notify();
  }

  /// Fixes the language when a recording starts, and returns to the default when it ends.
  ///
  /// The core does the same, so what is shown is what the recording uses.
  void _followRecording() {
    final RecordingUiState previous = _recordingState;
    final RecordingUiState current =
        _recordingController?.state ?? RecordingUiState.idle;
    _recordingState = current;
    if (previous == current) {
      return;
    }
    if (current == RecordingUiState.recording) {
      _chosenForRecording ??= _defaultLanguage;
    } else if (current == RecordingUiState.idle &&
        previous != RecordingUiState.starting) {
      _chosenForRecording = null;
    }
    _notify();
  }

  /// Notifies listeners unless disposed while a call was running.
  void _notify() {
    if (!_disposed) {
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    _recordingController?.removeListener(_followRecording);
    super.dispose();
  }
}
