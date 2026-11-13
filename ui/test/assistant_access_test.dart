// Verifies the AI assistant access page (#112) over a fake core: off at first with the warning,
// turned on to show how to connect Claude Code and other clients, copying a snippet, and hidden
// controls when the server is not installed.

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/assistant_access_bridge.dart';
import 'package:note_taker_ui/presentation/assistant_access_screen.dart';

void main() {
  late List<Map<String, Object?>> requests;
  late List<Map<String, Object?>> answers;

  Future<String> call(String function, List<String?> arguments) async {
    expect(function, 'note_taker_assistant_access');
    requests.add(jsonDecode(arguments.single!) as Map<String, Object?>);
    return jsonEncode(answers.removeAt(0));
  }

  Map<String, Object?> answer({required bool enabled}) => <String, Object?>{
        'ok': true,
        'enabled': enabled,
        'database_path': '/home/anna/notes.sqlite3',
        'server_path': '/opt/koalino/note_taker_mcp',
        'config': <String, Object?>{
          'mcpServers': <String, Object?>{
            'koalino': <String, Object?>{
              'command': '/opt/koalino/note_taker_mcp',
              'args': <Object?>[],
              'env': <String, Object?>{
                'NOTE_TAKER_DATABASE_PATH': '/home/anna/notes.sqlite3',
              },
            },
          },
        },
        'claude_code': 'claude mcp add koalino --env '
            'NOTE_TAKER_DATABASE_PATH=/home/anna/notes.sqlite3 -- '
            '/opt/koalino/note_taker_mcp',
        'passphrase_needed': false,
      };

  setUp(() {
    requests = <Map<String, Object?>>[];
    answers = <Map<String, Object?>>[answer(enabled: false)];
  });

  testWidgets('turning it on shows how to connect, and a snippet copies',
      (WidgetTester tester) async {
    final List<String> copied = <String>[];
    await tester.pumpWidget(MaterialApp(
      home: AssistantAccessScreen(
        bridge: AssistantAccessBridge(call: call),
        copyText: (String text) async => copied.add(text),
      ),
    ));
    await tester.pumpAndSettle();
    expect(find.textContaining('online model'), findsOneWidget);
    expect(find.textContaining('claude mcp add'), findsNothing);

    answers.add(answer(enabled: true));
    await tester.tap(find.byType(Switch));
    await tester.pumpAndSettle();
    expect(requests.last, <String, Object?>{'command': 'set', 'enabled': true});
    expect(find.textContaining('claude mcp add koalino'), findsOneWidget);
    expect(find.textContaining('"mcpServers"'), findsOneWidget);

    await tester.tap(find.byTooltip('Copy: Claude Code: run in a terminal'));
    await tester.pumpAndSettle();
    expect(copied.single, startsWith('claude mcp add koalino'));
    expect(find.text('Command copied'), findsOneWidget);
  });

  testWidgets('without the server the switch is off and explained',
      (WidgetTester tester) async {
    answers = <Map<String, Object?>>[
      <String, Object?>{
        'ok': true,
        'enabled': false,
        'server_path': null,
        'config': null,
        'claude_code': null,
      },
    ];
    await tester.pumpWidget(MaterialApp(
      home: AssistantAccessScreen(bridge: AssistantAccessBridge(call: call)),
    ));
    await tester.pumpAndSettle();
    expect(find.textContaining('not installed'), findsOneWidget);
    final Switch toggle = tester.widget(find.byType(Switch));
    expect(toggle.onChanged, isNull);
  });

  testWidgets('a failure is worded', (WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(
      home: AssistantAccessScreen(bridge: AssistantAccessBridge(call: call)),
    ));
    await tester.pumpAndSettle();
    answers.add(<String, Object?>{'ok': false, 'error_code': 'storage_failed'});
    await tester.tap(find.byType(Switch));
    await tester.pumpAndSettle();
    expect(find.text('The setting could not be saved.'), findsOneWidget);
  });
}
