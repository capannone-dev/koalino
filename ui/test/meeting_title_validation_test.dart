// Defines the Dart meeting title rules, which must match Rust's `MeetingTitle::parse`.

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/meeting_title_validation.dart';

/// Verifies trimming, the empty, length, and control-character rules, and their messages.
void main() {
  test('trims surrounding whitespace and accepts the result', () {
    final MeetingTitleValidation validation =
        validateMeetingTitle('  Project Sync \t');

    expect(validation.isValid, isTrue);
    expect(validation.trimmedTitle, 'Project Sync');
    expect(validation.errorMessage, isNull);
  });

  test('rejects empty and whitespace-only titles', () {
    for (final String rawTitle in <String>['', '   ', '\t\n ']) {
      final MeetingTitleValidation validation = validateMeetingTitle(rawTitle);
      expect(validation.error, MeetingTitleError.empty, reason: rawTitle);
      expect(validation.errorMessage, 'Enter a title.');
    }
  });

  test('accepts 200 characters and rejects 201', () {
    expect(validateMeetingTitle('a' * 200).isValid, isTrue);

    final MeetingTitleValidation validation = validateMeetingTitle('a' * 201);
    expect(validation.error, MeetingTitleError.tooLong);
    expect(validation.errorMessage,
        'Titles can be at most 200 characters (you have 201).');
  });

  test('counts characters like Rust, so 200 emoji are accepted', () {
    expect(validateMeetingTitle('🎉' * 200).isValid, isTrue);
    expect(validateMeetingTitle('🎉' * 201).error, MeetingTitleError.tooLong);
  });

  test('rejects line breaks, tabs, and other control characters inside', () {
    for (final String rawTitle in <String>[
      'Weekly\nsync',
      'Weekly\tsync',
      'Weekly\u0007sync',
      'Weekly\u0085sync',
    ]) {
      final MeetingTitleValidation validation = validateMeetingTitle(rawTitle);
      expect(validation.error, MeetingTitleError.controlCharacter,
          reason: rawTitle);
      expect(validation.errorMessage,
          "Titles can't contain line breaks or control characters.");
    }
  });

  test('accepts non-Latin text and emoji', () {
    expect(validateMeetingTitle('项目周会 🎉').isValid, isTrue);
  });
}
