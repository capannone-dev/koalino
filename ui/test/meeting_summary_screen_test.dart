// Verifies the meeting summary page and menu (#95): writing a summary with a chosen connection,
// its progress and report, a failure, the meeting's own prompt, and a summary written by hand
// (#96).

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/summaries_controller.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_edit_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_edit_widgets.dart';
import 'package:note_taker_ui/presentation/meeting_summary_screen.dart';

import 'fake_llm_core.dart';

void main() {
  late FakeLlmCore core;
  late SummariesController controller;

  setUp(() {
    core = FakeLlmCore();
    controller = SummariesController(
        bridge: LlmBridge(call: core.call),
        editBridge: MeetingEditBridge(call: core.call));
  });

  Widget page() => MaterialApp(
        home: MeetingSummaryScreen(
            controller: controller, meetingId: 'm-1', title: 'Budget review'),
      );

  testWidgets('writes a summary with the default connection and shows it',
      (WidgetTester tester) async {
    core.answer(
        'command:settings',
        settings(connections: <Object?>[
          connection('c-1', 'Office', isDefault: true),
          connection('c-2', 'Claude'),
        ]));
    core.answer('note_taker_get_meeting_summary',
        <String, Object?>{'ok': true, 'summary': null, 'meeting_prompt': null});
    await tester.pumpWidget(page());
    await tester.pumpAndSettle();

    expect(find.text('No summary yet.'), findsOneWidget);
    final Completer<String> answer = Completer<String>();
    core.pending['note_taker_summarize_meeting'] = answer;
    await tester.tap(find.text('Summarize'));
    await tester.pump();

    expect(find.byType(LinearProgressIndicator), findsOneWidget);
    answer.complete(
        '{"ok":true,"summary":${summaryJson()},"parts":1,"images_sent":0,"images_skipped":0}');
    await tester.pumpAndSettle();

    expect(find.text('Summary written.'), findsOneWidget);
    expect(find.text('Budget approved.'), findsOneWidget);
    expect(
        find.textContaining('Written by llama3.1:8b (Office)'), findsOneWidget);
    expect(find.text('Summarize again'), findsOneWidget);
    expect(core.calls.last.$2, <String?>['m-1', 'c-1']);
  });

  testWidgets('a summary written by hand is saved and shown as yours',
      (WidgetTester tester) async {
    core.answer('command:settings', settings());
    core.answer('note_taker_get_meeting_summary',
        <String, Object?>{'ok': true, 'summary': null, 'meeting_prompt': null});
    core.answer('edit:edit_summary', <String, Object?>{
      'ok': true,
      'summary': <String, Object?>{
        ...summary,
        'text': 'Decisions: none.',
        'connection_label': '',
        'model': '',
        'edited': true,
      },
    });
    await tester.pumpWidget(page());
    await tester.pumpAndSettle();

    await tester.tap(find.text('Write a summary'));
    await tester.pumpAndSettle();
    await tester.enterText(
        find.byKey(markdownEditorFieldKey), 'Decisions: none.');
    await tester.tap(find.byTooltip('Save'));
    await tester.pumpAndSettle();

    expect(find.text('Summary saved.'), findsOneWidget);
    expect(find.text('Decisions: none.'), findsOneWidget);
    expect(find.textContaining('Written by you'), findsOneWidget);
    expect(find.text('Edit summary'), findsOneWidget);
  });

  testWidgets('a failure keeps the page and explains it',
      (WidgetTester tester) async {
    core.answer('command:settings', settings());
    core.answer('note_taker_get_meeting_summary', <String, Object?>{
      'ok': true,
      'summary': summary,
      'meeting_prompt': null
    });
    core.answer('note_taker_summarize_meeting',
        <String, Object?>{'ok': false, 'error_code': 'llm_unreachable'});
    await tester.pumpWidget(page());
    await tester.pumpAndSettle();

    await tester.tap(find.text('Summarize again'));
    await tester.pumpAndSettle();

    expect(find.text('The AI server could not be reached.'), findsOneWidget);
    expect(find.text('Budget approved.'), findsOneWidget);
  });

  testWidgets("the meeting's prompt starts from Settings and can be reset",
      (WidgetTester tester) async {
    core.answer('command:settings', settings());
    core.answer('note_taker_get_meeting_summary',
        <String, Object?>{'ok': true, 'summary': null, 'meeting_prompt': null});
    core.answer('note_taker_set_meeting_summary_prompt',
        <String, Object?>{'ok': true, 'meeting_prompt': 'Only decisions.'});
    core.answer('note_taker_set_meeting_summary_prompt',
        <String, Object?>{'ok': true, 'meeting_prompt': null});
    await tester.pumpWidget(page());
    await tester.pumpAndSettle();

    await tester.tap(find.text('Edit prompt for this meeting'));
    await tester.pumpAndSettle();
    expect(find.text('Summarize {{title}}.'), findsOneWidget);
    await tester.enterText(
        find.byKey(meetingPromptFieldKey), 'Only decisions.');
    await tester.tap(find.text('Save prompt'));
    await tester.pumpAndSettle();

    expect(find.text('Prompt saved for this meeting.'), findsOneWidget);
    expect(core.calls.last.$2, <String?>['m-1', 'Only decisions.']);
    await tester.tap(find.text('Use the Settings prompt'));
    await tester.pumpAndSettle();
    expect(find.text('This meeting uses the Settings prompt again.'),
        findsOneWidget);
    expect(core.calls.last.$2, <String?>['m-1', null]);
  });

  testWidgets('without connections the page points to Settings',
      (WidgetTester tester) async {
    core.answer('command:settings', settings(connections: <Object?>[]));
    core.answer('note_taker_get_meeting_summary',
        <String, Object?>{'ok': true, 'summary': null, 'meeting_prompt': null});
    await tester.pumpWidget(page());
    await tester.pumpAndSettle();

    expect(find.text('Add an AI connection in Settings to write summaries.'),
        findsOneWidget);
    expect(find.text('Summarize'), findsNothing);
  });

  testWidgets('the menu summarizes with the chosen connection',
      (WidgetTester tester) async {
    core.answer(
        'command:settings',
        settings(connections: <Object?>[
          connection('c-1', 'Office', isDefault: true),
          connection('c-2', 'Claude'),
        ]));
    // The menu reads the meeting first; the page reads it again once the summary is stored.
    core.answer('note_taker_get_meeting_summary',
        <String, Object?>{'ok': true, 'summary': null, 'meeting_prompt': null});
    core.answer('note_taker_get_meeting_summary', <String, Object?>{
      'ok': true,
      'summary': summary,
      'meeting_prompt': null
    });
    core.answer('note_taker_summarize_meeting', <String, Object?>{
      'ok': true,
      'summary': summary,
      'parts': 1,
      'images_sent': 0,
      'images_skipped': 0,
    });
    final MeetingSummary meeting = MeetingSummary(
      meetingId: 'm-1',
      title: 'Budget review',
      createdAt: DateTime.utc(2026, 11, 9),
      durationMs: 60000,
    );
    await tester.pumpWidget(MaterialApp(
      home: Builder(
        builder: (BuildContext context) => TextButton(
          onPressed: () => showMeetingSummaryMenu(context,
              controller: controller, meeting: meeting),
          child: const Text('open'),
        ),
      ),
    ));
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();

    expect(find.text('Summarize with Office'), findsOneWidget);
    await tester.tap(find.text('Summarize with Claude'));
    await tester.pumpAndSettle();

    expect(
        core.calls
            .firstWhere((call) => call.$1 == 'note_taker_summarize_meeting')
            .$2,
        <String?>['m-1', 'c-2']);
    expect(find.text('Budget approved.'), findsOneWidget);
  });

  group('on a phone, a paired computer writes the summary (#107)', () {
    Widget phonePage() {
      controller = SummariesController(
          bridge: LlmBridge(call: core.call), asksPairedComputer: true);
      return page();
    }

    testWidgets('asking shows that the request waits for the computer',
        (WidgetTester tester) async {
      core.answer('command:settings', settings(connections: <Object?>[]));
      core.answer('note_taker_get_meeting_summary', <String, Object?>{
        'ok': true,
        'summary': null,
        'meeting_prompt': null,
        'request': null
      });
      core.answer('note_taker_request_paired_summary',
          <String, Object?>{'ok': true, 'request': pairedRequest('waiting')});
      await tester.pumpWidget(phonePage());
      await tester.pumpAndSettle();

      await tester.tap(find.text('Summarize on my computer'));
      await tester.pumpAndSettle();

      expect(find.textContaining('Waiting for your computer'), findsOneWidget);
      expect(find.text('Cancel request'), findsOneWidget);
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets('a computer at work shows progress',
        (WidgetTester tester) async {
      core.answer('command:settings', settings(connections: <Object?>[]));
      core.answer('note_taker_get_meeting_summary', <String, Object?>{
        'ok': true,
        'summary': null,
        'meeting_prompt': null,
        'request': pairedRequest('working'),
      });
      await tester.pumpWidget(phonePage());
      // The progress bar animates for ever, so settle by frames instead.
      for (int frame = 0; frame < 5; frame++) {
        await tester.pump(const Duration(milliseconds: 100));
      }

      expect(
          find.text('Your computer is writing the summary…'), findsOneWidget);
      expect(find.byType(LinearProgressIndicator), findsOneWidget);
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets('a failure explains it and offers to try again',
        (WidgetTester tester) async {
      core.answer('command:settings', settings(connections: <Object?>[]));
      core.answer('note_taker_get_meeting_summary', <String, Object?>{
        'ok': true,
        'summary': null,
        'meeting_prompt': null,
        'request': pairedRequest('failed', errorCode: 'llm_rate_limited'),
      });
      await tester.pumpWidget(phonePage());
      await tester.pumpAndSettle();

      expect(find.textContaining('Your computer could not write the summary'),
          findsOneWidget);
      expect(find.text('Try again on my computer'), findsOneWidget);
      await tester.pumpWidget(const SizedBox());
    });

    testWidgets('a desktop does not offer it', (WidgetTester tester) async {
      core.answer('command:settings', settings());
      core.answer('note_taker_get_meeting_summary', <String, Object?>{
        'ok': true,
        'summary': null,
        'meeting_prompt': null
      });
      await tester.pumpWidget(page());
      await tester.pumpAndSettle();

      expect(find.text('Summarize on my computer'), findsNothing);
    });
  });
}
