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
    expect(find.textContaining('No API key'), findsOneWidget);
    expect(find.textContaining('Unencrypted http'), findsOneWidget);
    expect(find.text('Summarize {{title}}.'), findsOneWidget);
    expect(
        tester.widget<SwitchListTile>(find.byKey(autoSummarizeSwitchKey)).value,
        isTrue);
  });

  testWidgets('adds a connection from a preset; the key goes to the core only',
      (WidgetTester tester) async {
    core.answer('command:settings', settings());
    core.answer('command:save_connection', <String, Object?>{
      'ok': true,
      'connection': connection('c-2', 'Anthropic')
    });
    core.answer('command:settings', settings());
    await show(tester);

    await tester.tap(find.text('Add connection'));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(ActionChip, 'Anthropic'));
    await tester.pumpAndSettle();
    await tester.enterText(
        find.widgetWithText(TextField, 'API key'), 'sk-ant-1');
    await tester.tap(find.text('Save connection'));
    await tester.pumpAndSettle();

    final Map<String, Object?> request = core.lastCommand('save_connection');
    expect(request['label'], 'Anthropic');
    expect(request['kind'], 'anthropic');
    expect(request['base_url'], 'https://api.anthropic.com');
    expect(request['model'], 'claude-opus-5-5');
    expect(request['api_key'], 'sk-ant-1');
    expect(find.text('Saved Anthropic.'), findsOneWidget);
    expect(find.text('sk-ant-1'), findsNothing);
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
}
