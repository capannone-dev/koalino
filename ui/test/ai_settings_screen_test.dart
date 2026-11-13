// Verifies Settings → AI connections (#95): the connection list with its notes, adding a
// connection from a preset with a key that is never shown again, testing, the prompt and
// automatic summaries (#101).

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/summaries_controller.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/presentation/ai_settings_screen.dart';

import 'fake_llm_core.dart';

void main() {
  late FakeLlmCore core;
  late SummariesController controller;

  setUp(() {
    core = FakeLlmCore();
    controller = SummariesController(bridge: LlmBridge(call: core.call));
  });

  Future<void> show(WidgetTester tester) async {
    tester.view.physicalSize = const Size(1200, 2000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    await tester.pumpWidget(
        MaterialApp(home: AiSettingsScreen(controller: controller)));
    await tester.pumpAndSettle();
  }

  testWidgets('lists connections with their notes, the prompt and the switch',
      (WidgetTester tester) async {
    core.answer('command:settings', settings());
    await show(tester);

    expect(find.text('Office'), findsOneWidget);
    expect(find.textContaining('Default'), findsOneWidget);
    expect(find.textContaining('API key'), findsNothing);
    expect(find.textContaining('Unencrypted http'), findsOneWidget);
    expect(find.text('Summarize {{title}}.'), findsOneWidget);
    expect(
        tester.widget<SwitchListTile>(find.byKey(autoSummarizeSwitchKey)).value,
        isTrue);
  });

  testWidgets('adds a connection from a preset, with no API key field (#117)',
      (WidgetTester tester) async {
    core.answer('command:settings', settings());
    core.answer('command:save_connection', <String, Object?>{
      'ok': true,
      'connection': connection('c-2', 'LM Studio')
    });
    core.answer('command:settings', settings());
    await show(tester);

    await tester.tap(find.text('Add connection'));
    await tester.pumpAndSettle();
    expect(find.widgetWithText(ActionChip, 'Anthropic'), findsNothing);
    expect(find.widgetWithText(ActionChip, 'OpenAI'), findsNothing);
    await tester.tap(find.widgetWithText(ActionChip, 'LM Studio'));
    await tester.pumpAndSettle();
    expect(find.widgetWithText(TextField, 'API key'), findsNothing);
    await tester.tap(find.text('Save connection'));
    await tester.pumpAndSettle();

    final Map<String, Object?> request = core.lastCommand('save_connection');
    expect(request['label'], 'LM Studio');
    expect(request['kind'], 'open_ai_compatible');
    expect(request['base_url'], 'http://192.168.1.10:1234/v1');
    expect(request['model'], 'qwen3-8b');
    expect(request.containsKey('api_key'), isFalse);
    expect(find.text('Saved LM Studio.'), findsOneWidget);
  });

  testWidgets('tests a connection and turns automatic summaries off',
      (WidgetTester tester) async {
    core.answer('command:settings', settings());
    core.answer('command:test_connection',
        <String, Object?>{'ok': true, 'reply': 'OK'});
    core.answer('command:update', settings(auto: false));
    await show(tester);

    await tester.tap(find.byTooltip('Actions for Office'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Test'));
    await tester.pumpAndSettle();
    expect(find.text('Office answered: OK'), findsOneWidget);

    await tester.tap(find.byKey(autoSummarizeSwitchKey));
    await tester.pumpAndSettle();
    expect(core.lastCommand('update')['auto_summarize'], isFalse);
    expect(find.text('Automatic summaries are off.'), findsOneWidget);
  });

  testWidgets('saves the prompt', (WidgetTester tester) async {
    core.answer('command:settings', settings());
    core.answer('command:update', settings());
    await show(tester);

    await tester.enterText(
        find.byKey(summaryTemplateFieldKey), 'Decisions of {{title}}.');
    await tester.tap(find.text('Save prompt'));
    await tester.pumpAndSettle();

    expect(core.lastCommand('update')['template'], 'Decisions of {{title}}.');
    expect(find.text('Prompt saved.'), findsOneWidget);
  });

  testWidgets(
      'adds Claude Code from its preset without an address or a key (#105)',
      (WidgetTester tester) async {
    core.answer('command:settings', settings());
    core.answer('command:save_connection', <String, Object?>{
      'ok': true,
      'connection': connection('c-2', 'Claude Code')
    });
    core.answer('command:settings', settings());
    await show(tester);

    await tester.tap(find.text('Add connection'));
    await tester.pumpAndSettle();
    await tester
        .tap(find.widgetWithText(ActionChip, 'Claude Code (Claude plan)'));
    await tester.pumpAndSettle();

    expect(find.widgetWithText(TextField, 'Address'), findsNothing);
    expect(find.widgetWithText(TextField, 'API key'), findsNothing);
    await tester.tap(find.text('Save connection'));
    await tester.pumpAndSettle();

    final Map<String, Object?> request = core.lastCommand('save_connection');
    expect(request['kind'], 'claude_code');
    expect(request['model'], '');
    expect(request.containsKey('api_key'), isFalse);
  });

  testWidgets('says whether Claude Code or Codex is installed here',
      (WidgetTester tester) async {
    core.answer(
        'command:settings',
        settings(connections: <Object?>[
          <String, Object?>{
            ...connection('c-1', 'Claude Pro', isDefault: true),
            'kind': 'claude_code',
            'base_url': '',
            'model': '',
            'plain_http': false,
            'tool_installed': true,
          },
          <String, Object?>{
            ...connection('c-2', 'ChatGPT'),
            'kind': 'codex',
            'base_url': '',
            'model': 'gpt-5.5',
            'plain_http': false,
            'tool_installed': false,
          },
        ]));
    await show(tester);

    expect(find.textContaining('Claude Code · default model'), findsOneWidget);
    expect(find.textContaining('Installed'), findsOneWidget);
    expect(find.textContaining('Codex · gpt-5.5'), findsOneWidget);
    expect(find.textContaining('Not found on this computer'), findsOneWidget);
    expect(find.textContaining('API key'), findsNothing);
  });
}
