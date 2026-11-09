// Verifies the notes folder page (#98) over a fake core: off at first with the suggested folder,
// turned on with a chosen folder, the last round shown, and a refusal worded.

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/notes_folder_bridge.dart';
import 'package:note_taker_ui/presentation/notes_folder_screen.dart';

void main() {
  late List<Map<String, Object?>> requests;
  late List<Map<String, Object?>> answers;

  Future<String> call(String function, List<String?> arguments) async {
    expect(function, 'note_taker_notes_folder');
    requests.add(jsonDecode(arguments.single!) as Map<String, Object?>);
    return jsonEncode(answers.removeAt(0));
  }

  setUp(() {
    requests = <Map<String, Object?>>[];
    answers = <Map<String, Object?>>[
      <String, Object?>{
        'ok': true,
        'enabled': false,
        'path': null,
        'suggested_path': '/home/anna/NoteTaker',
      },
    ];
  });

  testWidgets('turning it on sends the chosen folder and shows the round',
      (WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(
      home: NotesFolderScreen(
        bridge: NotesFolderBridge(call: call),
        pickFolder: () async => '/home/anna/Notes',
      ),
    ));
    await tester.pumpAndSettle();
    expect(find.text('/home/anna/NoteTaker'), findsOneWidget);
    expect(find.textContaining('decrypted copy'), findsOneWidget);
    expect(find.text('Sync now'), findsNothing);

    await tester.tap(find.byTooltip('Choose a folder'));
    await tester.pumpAndSettle();
    answers.add(<String, Object?>{
      'ok': true,
      'enabled': true,
      'path': '/home/anna/Notes',
      'last_round_at': '2026-11-09T12:00:00Z',
      'written': 7,
      'imported': 0,
    });
    await tester.tap(find.byType(Switch));
    await tester.pumpAndSettle();

    expect(requests.last, <String, Object?>{
      'command': 'set',
      'enabled': true,
      'path': '/home/anna/Notes',
    });
    expect(find.textContaining('7 files written'), findsOneWidget);
    expect(find.text('Sync now'), findsOneWidget);
  });

  testWidgets('a refused folder is worded', (WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(
      home: NotesFolderScreen(bridge: NotesFolderBridge(call: call)),
    ));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(notesFolderPathFieldKey), 'notes');
    answers.add(<String, Object?>{'ok': false, 'error_code': 'invalid_folder'});
    await tester.tap(find.byType(Switch));
    await tester.pumpAndSettle();

    expect(requests.last['path'], 'notes');
    expect(find.textContaining('Choose a full folder path'), findsOneWidget);
  });
}
