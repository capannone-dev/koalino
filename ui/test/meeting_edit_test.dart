// Verifies the owner's edits in the app (#96) over a fake core: notes, the summary's text, and
// correcting or removing a transcript segment in the open meeting without reloading it.

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/meeting_edit_controller.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/controllers/summaries_controller.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_edit_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_edit_widgets.dart';

import 'fake_llm_core.dart';
import 'fake_meeting_bridge.dart';

const MeetingTranscriptSegment friday = MeetingTranscriptSegment(
  startTimeMs: 61000,
  endTimeMs: 65000,
  speaker: 'Anna',
  text: 'we ship on Friday',
  uncertainWords: <int>[1],
);

void main() {
  late FakeLlmCore core;
  late MeetingEditController edits;

  setUp(() {
    core = FakeLlmCore();
    edits = MeetingEditController(bridge: MeetingEditBridge(call: core.call));
  });

  test('notes are loaded and saved', () async {
    core.answer('edit:notes', <String, Object?>{'ok': true, 'notes': 'Agenda'});
    await edits.loadNotes('m-1');
    expect(edits.notesFor('m-1'), 'Agenda');

    core.answer(
        'edit:set_notes', <String, Object?>{'ok': true, 'notes': 'More'});
    expect(await edits.saveNotes('m-1', 'More'), isNull);
    expect(edits.notesFor('m-1'), 'More');

    core.answer('edit:set_notes',
        <String, Object?>{'ok': false, 'error_code': 'too_long'});
    expect(await edits.saveNotes('m-1', 'x'), 'The text is too long.');
    expect(edits.notesFor('m-1'), 'More');
  });

  test('a meeting is created by writing and the list reloads (#129)', () async {
    final FakeMeetingBridge meetingBridge = FakeMeetingBridge();
    final MeetingListController meetings =
        MeetingListController(meetingBridge: meetingBridge);
    core.answer('edit:create_meeting',
        <String, Object?>{'ok': true, 'meeting_id': 'meeting-new'});

    final ({String? meetingId, String? error}) created =
        await edits.createMeeting(meetings, title: 'Supplier call');

    expect(created.meetingId, 'meeting-new');
    expect(created.error, isNull);
    final Map<String, Object?> request = core.calls
        .where(((String, List<String?>) call) =>
            call.$1 == 'note_taker_meeting_edit')
        .map(((String, List<String?>) call) =>
            jsonDecode(call.$2.single!) as Map<String, Object?>)
        .single;
    expect(request, <String, Object?>{
      'command': 'create_meeting',
      'title': 'Supplier call',
    });
    expect(meetingBridge.meetingListRequestCount, 1);

    core.answer('edit:create_meeting',
        <String, Object?>{'ok': false, 'error_code': 'too_long'});
    final ({String? meetingId, String? error}) refused =
        await edits.createMeeting(meetings, title: 'x');
    expect(refused.meetingId, isNull);
    expect(refused.error, 'The text is too long.');
  });

  test('a corrected segment replaces the open one and a removed one goes',
      () async {
    final MeetingListController meetings = MeetingListController(
      meetingBridge: FakeMeetingBridge(details: <String, MeetingDetail>{
        'meeting-titled': MeetingDetail(
          summary: titledMeeting,
          transcript: null,
          segments: const <MeetingTranscriptSegment>[friday],
        ),
      }),
    );
    await meetings.selectMeeting('meeting-titled');
    core.answer('edit:edit_segment', <String, Object?>{
      'ok': true,
      'segment': <String, Object?>{
        'start_time_ms': 61000,
        'end_time_ms': 65000,
        'text': 'we ship on Monday',
      },
    });

    expect(
        await edits.editSegment(
            meetings, 'meeting-titled', friday, 'we ship on Monday'),
        isNull);
    final MeetingTranscriptSegment edited =
        meetings.selectedMeeting!.segments.single;
    expect(edited.text, 'we ship on Monday');
    expect(edited.speaker, 'Anna');
    expect(edited.uncertainWords, isEmpty);

    core.answer(
        'edit:edit_segment', <String, Object?>{'ok': true, 'segment': null});
    expect(await edits.editSegment(meetings, 'meeting-titled', edited, ''),
        isNull);
    expect(meetings.selectedMeeting!.segments, isEmpty);
  });

  test('the summary text is edited through the summaries controller', () async {
    final SummariesController summaries = SummariesController(
      bridge: LlmBridge(call: core.call),
      editBridge: MeetingEditBridge(call: core.call),
    );
    core.answer('edit:edit_summary', <String, Object?>{
      'ok': true,
      'summary': <String, Object?>{...summary, 'text': 'Mine.', 'edited': true},
    });

    expect(await summaries.editSummary('m-1', 'Mine.'), isNull);
    expect(summaries.stateFor('m-1')?.summary?.text, 'Mine.');
    expect(summaries.stateFor('m-1')?.summary?.edited, isTrue);

    core.answer('edit:edit_summary',
        <String, Object?>{'ok': false, 'error_code': 'empty'});
    expect(await summaries.editSummary('m-1', ' '), 'Write something first.');
  });

  testWidgets('the notes section shows, edits and saves notes',
      (WidgetTester tester) async {
    core.answer('edit:notes', <String, Object?>{'ok': true, 'notes': null});
    core.answer(
        'edit:set_notes', <String, Object?>{'ok': true, 'notes': '## Actions'});
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: ListView(children: <Widget>[
          MeetingNotesSection(controller: edits, meetingId: 'm-1'),
        ]),
      ),
    ));
    await tester.pumpAndSettle();

    expect(find.text('No notes yet.'), findsOneWidget);
    await tester.tap(find.text('Write notes'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(markdownEditorFieldKey), '## Actions');
    await tester.tap(find.text('Preview'));
    await tester.pumpAndSettle();
    expect(find.text('Actions'), findsOneWidget);
    await tester.tap(find.byTooltip('Save'));
    await tester.pumpAndSettle();

    expect(find.text('Edit notes'), findsOneWidget);
    expect(find.text('Actions'), findsOneWidget);
  });

  testWidgets('a long press corrects a segment', (WidgetTester tester) async {
    final MeetingListController meetings = MeetingListController(
      meetingBridge: FakeMeetingBridge(details: <String, MeetingDetail>{
        'meeting-titled': MeetingDetail(
          summary: titledMeeting,
          transcript: null,
          segments: const <MeetingTranscriptSegment>[friday],
        ),
      }),
    );
    await meetings.selectMeeting('meeting-titled');
    core.answer('edit:edit_segment', <String, Object?>{
      'ok': true,
      'segment': <String, Object?>{
        'start_time_ms': 61000,
        'end_time_ms': 65000,
        'text': 'we ship on Monday',
      },
    });
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: Builder(
          builder: (BuildContext context) => EditableSegment(
            controller: edits,
            meetings: meetings,
            meetingId: 'meeting-titled',
            segment: friday,
            child: const Text('we ship on Friday'),
          ),
        ),
      ),
    ));

    await tester.longPress(find.text('we ship on Friday'));
    await tester.pumpAndSettle();
    await tester.enterText(
        find.byKey(segmentEditFieldKey), 'we ship on Monday');
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();

    expect(meetings.selectedMeeting!.segments.single.text, 'we ship on Monday');
    expect(find.textContaining("won't be re-transcribed"), findsOneWidget);
  });
}
