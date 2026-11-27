// Specifies renaming a speaker from a meeting's transcript (#186): double-clicking or
// long-pressing the speaker's label opens a dialog; the name is sent for that voice; a refused
// name keeps the dialog open with the reason; a label without a voice does nothing.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/speaker_rename_bridge.dart';
import 'package:note_taker_ui/presentation/speaker_label.dart';

/// Records renames and answers them as told.
final class FakeSpeakerRenamer implements SpeakerRenamer {
  /// `(voice, name)` pairs sent to `rename`.
  final List<(SpeakerVoice, String)> renames = <(SpeakerVoice, String)>[];

  /// When set, `rename` throws this code instead of succeeding.
  String? errorCode;

  @override
  Future<List<String>> savedNames() async =>
      const <String>['Anna Verdi', 'Bruno Neri'];

  @override
  Future<String> rename(SpeakerVoice voice, String name) async {
    renames.add((voice, name));
    if (errorCode case final String code) {
      throw SpeakerRenameException(code);
    }
    return name.trim();
  }
}

const SpeakerVoice _person =
    SpeakerVoice(kind: SpeakerVoiceKind.person, id: 'p-1');
const SpeakerVoice _unknown =
    SpeakerVoice(kind: SpeakerVoiceKind.unknown, id: 'c-1');

/// Shows one label; `renamed` counts the reloads it asks for.
Future<void> _pumpLabel(
  WidgetTester tester, {
  required FakeSpeakerRenamer renamer,
  required SpeakerVoice? voice,
  required String name,
  required List<String> renamed,
}) =>
    tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: SpeakerLabel(
          text: '$name · 0:05',
          name: name,
          voice: voice,
          renamer: renamer,
          onRenamed: renamed.add,
        ),
      ),
    ));

Future<void> _doubleTap(WidgetTester tester, Finder finder) async {
  await tester.tap(finder);
  await tester.pump(const Duration(milliseconds: 50));
  await tester.tap(finder);
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('double-clicking a saved person renames them',
      (WidgetTester tester) async {
    final FakeSpeakerRenamer renamer = FakeSpeakerRenamer();
    final List<String> renamed = <String>[];
    await _pumpLabel(tester,
        renamer: renamer, voice: _person, name: 'Anna', renamed: renamed);

    await _doubleTap(tester, find.text('Anna · 0:05'));
    expect(find.byType(AlertDialog), findsOneWidget);
    final Finder field = find.byKey(speakerNameFieldKey);
    expect(tester.widget<TextField>(field).controller?.text, 'Anna');

    await tester.enterText(field, 'Anna Rossi');
    await tester.tap(find.byKey(renameSpeakerButtonKey));
    await tester.pumpAndSettle();

    expect(renamer.renames, <(SpeakerVoice, String)>[(_person, 'Anna Rossi')]);
    expect(renamed, <String>['Anna Rossi']);
    expect(find.byType(AlertDialog), findsNothing);
  });

  testWidgets(
      'a long press names an unknown voice, starting from an empty field',
      (WidgetTester tester) async {
    final FakeSpeakerRenamer renamer = FakeSpeakerRenamer();
    final List<String> renamed = <String>[];
    await _pumpLabel(tester,
        renamer: renamer, voice: _unknown, name: 'Speaker 2', renamed: renamed);

    await tester.longPress(find.text('Speaker 2 · 0:05'));
    await tester.pumpAndSettle();
    final Finder field = find.byKey(speakerNameFieldKey);
    expect(tester.widget<TextField>(field).controller?.text, isEmpty);

    await tester.enterText(field, 'Bru');
    await tester.pumpAndSettle();
    expect(find.text('Bruno Neri'), findsOneWidget,
        reason: 'saved people are suggested');
    await tester.tap(find.text('Bruno Neri'));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(renameSpeakerButtonKey));
    await tester.pumpAndSettle();

    expect(renamer.renames, <(SpeakerVoice, String)>[(_unknown, 'Bruno Neri')]);
    expect(renamed, <String>['Bruno Neri']);
  });

  testWidgets('a name another person has keeps the dialog open and says why',
      (WidgetTester tester) async {
    final FakeSpeakerRenamer renamer = FakeSpeakerRenamer()
      ..errorCode = 'name_taken';
    final List<String> renamed = <String>[];
    await _pumpLabel(tester,
        renamer: renamer, voice: _person, name: 'Bruno', renamed: renamed);

    await tester.longPress(find.text('Bruno · 0:05'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(speakerNameFieldKey), 'Anna Verdi');
    await tester.tap(find.byKey(renameSpeakerButtonKey));
    await tester.pumpAndSettle();

    expect(find.byType(AlertDialog), findsOneWidget);
    expect(find.textContaining('Voices'), findsWidgets);
    expect(renamed, isEmpty);
  });

  testWidgets('a label without a voice does not react',
      (WidgetTester tester) async {
    final FakeSpeakerRenamer renamer = FakeSpeakerRenamer();
    final List<String> renamed = <String>[];
    await _pumpLabel(tester,
        renamer: renamer, voice: null, name: 'Unattributed', renamed: renamed);

    await tester.longPress(find.text('Unattributed · 0:05'));
    await _doubleTap(tester, find.text('Unattributed · 0:05'));
    expect(find.byType(AlertDialog), findsNothing);
  });

  test('a segment decodes its voice, and a core without one reads as none', () {
    final MeetingTranscriptSegment withVoice =
        MeetingTranscriptSegment.fromJson(
      <String, Object?>{
        'start_time_ms': 0,
        'end_time_ms': 1000,
        'speaker': 'Speaker 1',
        'text': 'Hello.',
        'voice': <String, Object?>{'kind': 'unknown', 'id': 'c-1'},
      },
    );
    expect(withVoice.voice, _unknown);
    final MeetingTranscriptSegment older = MeetingTranscriptSegment.fromJson(
      <String, Object?>{
        'start_time_ms': 0,
        'end_time_ms': 1000,
        'speaker': 'Speaker 1',
        'text': 'Hello.',
        'voice': null,
      },
    );
    expect(older.voice, isNull);
  });
}
