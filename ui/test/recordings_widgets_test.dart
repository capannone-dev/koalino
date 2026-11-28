// Verifies kept meeting recordings in the app (#140): turning keeping on only after the privacy
// warning, what is kept, and wiping them all after confirming. The meeting page's player is in
// `recording_player_test.dart`.

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/recordings_bridge.dart';
import 'package:note_taker_ui/presentation/recordings_widgets.dart';

/// Answers recordings commands from a script and records the requests.
final class FakeRecordingsCore {
  final List<Map<String, Object?>> requests = <Map<String, Object?>>[];
  bool keep = false;
  int count = 2;
  bool available = true;

  Future<String> call(String function, List<String?> arguments) async {
    final Map<String, Object?> request =
        jsonDecode(arguments.single!) as Map<String, Object?>;
    requests.add(request);
    Map<String, Object?> status() => <String, Object?>{
          'ok': true,
          'keep': keep,
          'count': count,
          'bytes': count * 1048576,
        };
    return jsonEncode(switch (request['command']) {
      'status' => status(),
      'set_keep' => () {
          keep = request['keep']! as bool;
          return status();
        }(),
      'wipe' => () {
          final int deleted = count;
          count = 0;
          return <String, Object?>{'ok': true, 'deleted': deleted, 'bytes': 0};
        }(),
      'has' => <String, Object?>{'ok': true, 'available': available},
      _ => <String, Object?>{'ok': false, 'error_code': 'invalid_request'},
    });
  }

  List<String?> get commands => requests
      .map((Map<String, Object?> r) => r['command'] as String?)
      .toList();
}

void main() {
  late FakeRecordingsCore core;

  setUp(() => core = FakeRecordingsCore());

  Future<void> showSection(WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(
        home: Scaffold(
            body:
                RecordingsSection(bridge: RecordingsBridge(call: core.call)))));
    await tester.pumpAndSettle();
  }

  testWidgets('keeping is turned on only after the warning is confirmed',
      (WidgetTester tester) async {
    await showSection(tester);
    expect(find.text('2 recordings kept · 2.0 MB'), findsOneWidget);

    await tester.tap(find.byKey(keepRecordingsSwitchKey));
    await tester.pumpAndSettle();
    expect(find.textContaining('consent'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(core.commands.contains('set_keep'), isFalse);

    await tester.tap(find.byKey(keepRecordingsSwitchKey));
    await tester.pumpAndSettle();
    await tester
        .tap(find.byKey(const ValueKey<String>('confirm-keep-recordings')));
    await tester.pumpAndSettle();

    expect(core.keep, isTrue);
    expect(
        tester
            .widget<SwitchListTile>(find.byKey(keepRecordingsSwitchKey))
            .value,
        isTrue);
  });

  testWidgets('turning keeping off needs no confirmation',
      (WidgetTester tester) async {
    core.keep = true;
    await showSection(tester);
    await tester.tap(find.byKey(keepRecordingsSwitchKey));
    await tester.pumpAndSettle();
    expect(core.keep, isFalse);
  });

  testWidgets('wiping asks first, then deletes them all',
      (WidgetTester tester) async {
    await showSection(tester);
    await tester.tap(find.byKey(wipeRecordingsKey));
    await tester.pumpAndSettle();
    expect(find.text('Wipe all recordings?'), findsOneWidget);
    await tester
        .tap(find.byKey(const ValueKey<String>('confirm-wipe-recordings')));
    await tester.pumpAndSettle();

    expect(core.commands.contains('wipe'), isTrue);
    expect(find.text('2 recordings wiped.'), findsOneWidget);
    expect(find.text('No recordings kept'), findsOneWidget);
    expect(
        tester.widget<ButtonStyleButton>(find.byKey(wipeRecordingsKey)).enabled,
        isFalse,
        reason: 'nothing left to wipe');
  });
}
