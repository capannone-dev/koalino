// Verifies the AI connections and meeting summaries state (#95) over a fake core: decoding the
// answers, the requests sent (keys only when given), and the plain messages for failures.

import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/summaries_controller.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';

import 'fake_llm_core.dart';

void main() {
  late FakeLlmCore core;
  late SummariesController controller;

  setUp(() {
    core = FakeLlmCore();
    controller = SummariesController(bridge: LlmBridge(call: core.call));
  });

  test('loads the settings with their connections', () async {
    core.answer('command:settings', settings());
    await controller.load();

    expect(controller.settings?.connections.single.label, 'Office');
    expect(controller.settings?.connections.single.plainHttp, isTrue);
    expect(controller.defaultConnection?.id, 'c-1');
    expect(controller.settings?.autoSummarize, isTrue);
  });

  test('a core that is not ready leaves no settings and says so', () async {
    await controller.load();
    expect(controller.settings, isNull);
    expect(controller.message, 'The app is still starting. Try again.');
  });

  test('saving a connection sends it and reloads', () async {
    core.answer('command:settings', settings());
    await controller.load();
    core.answer('command:save_connection', <String, Object?>{
      'ok': true,
      'connection': connection('c-2', 'Studio')
    });
    core.answer(
        'command:settings',
        settings(connections: <Object?>[
          connection('c-1', 'Office', isDefault: true),
          connection('c-2', 'Studio'),
        ]));

    final bool saved = await controller.saveConnection(const LlmConnectionInput(
      label: 'Studio',
      kind: LlmKind.openAiCompatible,
      baseUrl: 'http://192.168.1.10:1234/v1',
      model: 'qwen3-8b',
      contextTokens: 32768,
    ));

    expect(saved, isTrue);
    expect(controller.message, 'Saved Studio.');
    expect(controller.settings?.connections, hasLength(2));
    final Map<String, Object?> request = core.lastCommand('save_connection');
    expect(request.containsKey('api_key'), isFalse);
    expect(request['kind'], 'open_ai_compatible');
    expect(request.containsKey('id'), isFalse);
  });

  test('editing sends the connection\'s id and no key (#117)', () async {
    core.answer('command:save_connection', <String, Object?>{
      'ok': true,
      'connection': connection('c-1', 'Office')
    });
    core.answer('command:settings', settings());

    await controller.saveConnection(
        const LlmConnectionInput(
          label: 'Office',
          kind: LlmKind.openAiCompatible,
          baseUrl: 'http://192.168.1.5:11434/v1',
          model: 'llama3.1:8b',
          contextTokens: 8192,
        ),
        id: 'c-1');

    final Map<String, Object?> request = core.lastCommand('save_connection');
    expect(request['id'], 'c-1');
    expect(request.containsKey('api_key'), isFalse);
  });

  test('a refused connection is explained and not saved', () async {
    core.answer('command:save_connection', <String, Object?>{
      'ok': false,
      'error_code': 'invalid_connection',
      'error': 'internal detail',
    });
    final bool saved = await controller.saveConnection(const LlmConnectionInput(
      label: 'Public',
      kind: LlmKind.openAiCompatible,
      baseUrl: 'http://example.com/v1',
      model: 'm',
      contextTokens: 8192,
    ));

    expect(saved, isFalse);
    expect(controller.message, contains('Plain http:// works only'));
    expect(controller.message, isNot(contains('internal detail')));
  });

  test('testing a connection reports its reply', () async {
    core.answer('command:settings', settings());
    await controller.load();
    core.answer('command:test_connection',
        <String, Object?>{'ok': true, 'reply': 'OK'});
    await controller.testConnection('c-1');
    expect(controller.message, 'Office answered: OK');

    core.answer('command:test_connection',
        <String, Object?>{'ok': false, 'error_code': 'llm_unauthorized'});
    await controller.testConnection('c-1');
    expect(controller.message,
        'The AI server refused access. Use a server that needs no API key.');
  });

  test('the template, its reset and automatic summaries are saved', () async {
    core.answer('command:update', settings());
    await controller.saveTemplate('Only {{title}}.');
    expect(core.lastCommand('update')['template'], 'Only {{title}}.');

    core.answer('command:update', settings());
    await controller.resetTemplate();
    final Map<String, Object?> reset = core.lastCommand('update');
    expect(reset.containsKey('template'), isTrue);
    expect(reset['template'], isNull);

    core.answer('command:update', settings(auto: false));
    await controller.setAutoSummarize(enabled: false);
    expect(core.lastCommand('update')['auto_summarize'], isFalse);
    expect(controller.settings?.autoSummarize, isFalse);
    expect(controller.message, 'Automatic summaries are off.');
  });

  test('summarizes a meeting with a chosen connection and keeps the result',
      () async {
    core.answer('note_taker_summarize_meeting', <String, Object?>{
      'ok': true,
      'summary': summary,
      'parts': 3,
      'images_sent': 2,
      'images_skipped': 1,
    });

    final Future<void> running =
        controller.summarize('m-1', connectionId: 'c-2');
    expect(controller.isSummarizing('m-1'), isTrue);
    await running;

    expect(controller.isSummarizing('m-1'), isFalse);
    expect(controller.stateFor('m-1')?.summary?.text,
        contains('Budget approved.'));
    expect(controller.meetingMessage('m-1'),
        'Summary written. The transcript was read in 3 parts. 2 pictures were included, 1 left out.');
    expect(core.calls.last.$2, <String?>['m-1', 'c-2']);
  });

  test('a failed summary keeps the previous one and explains why', () async {
    core.answer('note_taker_get_meeting_summary', <String, Object?>{
      'ok': true,
      'summary': summary,
      'meeting_prompt': 'Only decisions.'
    });
    await controller.loadMeeting('m-1');
    expect(controller.stateFor('m-1')?.meetingPrompt, 'Only decisions.');

    core.answer('note_taker_summarize_meeting',
        <String, Object?>{'ok': false, 'error_code': 'too_long'});
    await controller.summarize('m-1');

    expect(controller.stateFor('m-1')?.summary?.text,
        contains('Budget approved.'));
    expect(controller.meetingMessage('m-1'),
        contains('too long for this connection'));
    expect(core.calls.last.$2, <String?>['m-1', null]);
  });

  test("a rejected summary shows the AI server's own reason", () async {
    core.answer('note_taker_summarize_meeting', <String, Object?>{
      'ok': false,
      'error_code': 'llm_rejected',
      'error_detail': 'invalid_request_error: Your credit balance is too low',
    });
    await controller.summarize('m-1');

    expect(
        controller.meetingMessage('m-1'),
        'The AI server rejected the request.\n'
        'The server said: invalid_request_error: Your credit balance is too low');
  });

  test('a failed local tool says what it said (#105)', () async {
    core.answer('note_taker_summarize_meeting', <String, Object?>{
      'ok': false,
      'error_code': 'llm_tool_failed',
      'error_detail': 'Not logged in · Please run /login',
    });
    await controller.summarize('m-1');
    expect(
        controller.meetingMessage('m-1'),
        'Claude Code or Codex could not answer.\n'
        'It said: Not logged in · Please run /login');

    core.answer('note_taker_summarize_meeting',
        <String, Object?>{'ok': false, 'error_code': 'llm_tool_missing'});
    await controller.summarize('m-1');
    expect(controller.meetingMessage('m-1'),
        contains('is not installed on this computer'));
  });

  test("a meeting's own prompt is saved and cleared", () async {
    core.answer('note_taker_set_meeting_summary_prompt',
        <String, Object?>{'ok': true, 'meeting_prompt': 'Only decisions.'});
    expect(await controller.setMeetingPrompt('m-1', 'Only decisions.'), isTrue);
    expect(controller.stateFor('m-1')?.meetingPrompt, 'Only decisions.');
    expect(controller.meetingMessage('m-1'), 'Prompt saved for this meeting.');

    core.answer('note_taker_set_meeting_summary_prompt',
        <String, Object?>{'ok': true, 'meeting_prompt': null});
    await controller.setMeetingPrompt('m-1', null);
    expect(core.calls.last.$2, <String?>['m-1', null]);
    expect(controller.stateFor('m-1')?.meetingPrompt, isNull);
    expect(controller.meetingMessage('m-1'),
        'This meeting uses the Settings prompt again.');
  });

  test('a slower read never replaces a summary written meanwhile', () async {
    final Completer<String> slowRead = Completer<String>();
    core.pending['note_taker_get_meeting_summary'] = slowRead;
    final Future<void> reading = controller.loadMeeting('m-1');
    core.answer('note_taker_summarize_meeting', <String, Object?>{
      'ok': true,
      'summary': summary,
      'parts': 1,
      'images_sent': 0,
      'images_skipped': 0,
    });
    await controller.summarize('m-1');
    slowRead.complete('{"ok":true,"summary":null,"meeting_prompt":null}');
    await reading;

    expect(controller.stateFor('m-1')?.summary?.text,
        contains('Budget approved.'));
  });
}
