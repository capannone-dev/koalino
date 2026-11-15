import 'package:note_taker_ui/l10n/l10n.dart';
// Validates meeting titles in Dart with the same rules Rust's `MeetingTitle::parse` enforces.

/// Longest title Rust accepts, in Unicode characters (`MeetingTitle::MAX_CHARACTERS`).
const int maxMeetingTitleCharacters = 200;

/// Why a meeting title was rejected.
enum MeetingTitleError {
  /// Nothing remains after trimming whitespace.
  empty,

  /// More than `maxMeetingTitleCharacters` characters remain after trimming.
  tooLong,

  /// The title contains a line break, tab, or other control character.
  controlCharacter,
}

/// The outcome of validating one title the person typed.
final class MeetingTitleValidation {
  const MeetingTitleValidation._(this.trimmedTitle, this.error);

  /// The title with surrounding whitespace removed; this is what gets saved.
  final String trimmedTitle;

  /// Why the title is invalid, or `null` when it is valid.
  final MeetingTitleError? error;

  /// Whether the title can be sent to Rust.
  bool get isValid => error == null;

  /// A message to show under the title field, or `null` when the title is valid.
  String? get errorMessage => switch (error) {
        null => null,
        MeetingTitleError.empty => l10n.enterATitle,
        MeetingTitleError.tooLong => l10n.titlesCanBeAtMostCharacters(
            maxMeetingTitleCharacters, trimmedTitle.runes.length),
        MeetingTitleError.controlCharacter => l10n.titlesCanTContainLineBreaks,
      };
}

/// Validates `rawTitle` exactly as Rust will, so invalid titles never cross the FFI boundary.
///
/// Inputs: The text as typed, possibly with surrounding whitespace.
/// Outputs: The trimmed title and the first rule it breaks, checked in Rust's order: empty,
/// too long (counted in Unicode characters, so an emoji counts once), control character
/// (Unicode category Cc: U+0000–U+001F and U+007F–U+009F, which Rust's `char::is_control`
/// matches).
/// Side effects: None.
MeetingTitleValidation validateMeetingTitle(String rawTitle) {
  final String trimmedTitle = rawTitle.trim();
  final MeetingTitleError? error;
  if (trimmedTitle.isEmpty) {
    error = MeetingTitleError.empty;
  } else if (trimmedTitle.runes.length > maxMeetingTitleCharacters) {
    error = MeetingTitleError.tooLong;
  } else if (trimmedTitle.runes.any(_isControlCharacter)) {
    error = MeetingTitleError.controlCharacter;
  } else {
    error = null;
  }
  return MeetingTitleValidation._(trimmedTitle, error);
}

/// Reports whether `codePoint` is in Unicode general category Cc.
bool _isControlCharacter(int codePoint) =>
    codePoint <= 0x1F || (codePoint >= 0x7F && codePoint <= 0x9F);
