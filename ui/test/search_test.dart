// Verifies search in the app (#100): the request per scope, results that say where each match is
// with the matched words marked, opening a result, and an open meeting scrolled to the passage.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/search_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';
import 'package:note_taker_ui/presentation/search_screen.dart';
import 'package:note_taker_ui/presentation/meeting_list_screen.dart';

import 'fake_llm_core.dart';
import 'fake_meeting_bridge.dart';

const Map<String, Object?> transcriptHit = <String, Object?>{
  'place': 'transcript',
  'start_time_ms': 61000,
  'end_time_ms': 64000,
  'speaker': 'Anna',
  'meeting': <String, Object?>{
    'meeting_id': 'meeting-titled',
    'title': 'Budget review',
    'created_at': '2026-11-09T10:00:00Z',
  },
  'project': null,
  'resource': null,
  'snippet': <Object?>[
    <String, Object?>{'text': 'The ', 'match': false},
    <String, Object?>{'text': 'budget', 'match': true},
    <String, Object?>{'text': ' is approved.', 'match': false},
  ],
};

const Map<String, Object?> fileHit = <String, Object?>{
  'place': 'resource_text',
  'meeting': null,
  'project': <String, Object?>{'project_id': 'p-1', 'name': 'Launch'},
  'resource': <String, Object?>{
    'attachment_id': 'a-1',
    'file_name': 'plan.md',
    'mime_type': 'text/markdown',
    'kind': 'markdown',
  },
  'snippet': <Object?>[
    <String, Object?>{'text': 'budget', 'match': true},
  ],
};

void main() {
  late FakeLlmCore core;
  late SearchBridge bridge;

  setUp(() {
    core = FakeLlmCore();
    bridge = SearchBridge(call: core.call);
  });

  test('the request carries the scope and the hits are read', () async {
    core.answer('note_taker_search', <String, Object?>{
      'ok': true,
      'hits': <Object?>[transcriptHit, fileHit],
      'truncated': true,
    });

    final SearchResults results =
        await bridge.search('budget', SearchScope.project, projectId: 'p-1');

    expect(core.calls.single.$2.single,
        '{"query":"budget","scope":"project","project_id":"p-1"}');
    expect(results.truncated, isTrue);
    final SearchHit first = results.hits.first;
    expect(first.place, SearchPlace.transcript);
    expect(first.meetingId, 'meeting-titled');
    expect(first.startTimeMs, 61000);
    expect(first.placeLabel, '1:01 · Anna');
    expect(first.snippet.where((SnippetPart part) => part.matched).single.text,
        'budget');
    expect(results.hits.last.placeLabel, 'In plan.md');
    expect(results.hits.last.heading, 'Launch');

    core.answer('note_taker_search',
        <String, Object?>{'ok': false, 'error_code': 'too_short'});
    await expectLater(
      bridge.search('b', SearchScope.meetings),
      throwsA(isA<SearchException>().having(
          (SearchException error) => error.message,
          'message',
          'Type at least two letters.')),
    );
  });

  testWidgets('results are listed, marked and opened',
      (WidgetTester tester) async {
    core.answer('note_taker_search', <String, Object?>{
      'ok': true,
      'hits': <Object?>[transcriptHit, fileHit],
      'truncated': false,
    });
    SearchHit? opened;
    await tester.pumpWidget(MaterialApp(
      home: SearchScreen(
        bridge: bridge,
        scope: SearchScope.meetings,
        title: 'Search meetings',
        onOpen: (BuildContext context, SearchHit hit) => opened = hit,
      ),
    ));

    await tester.enterText(find.byType(TextField), 'budget');
    await tester.testTextInput.receiveAction(TextInputAction.search);
    await tester.pumpAndSettle();

    expect(find.text('2 results'), findsOneWidget);
    expect(find.text('Budget review'), findsOneWidget);
    expect(find.text('1:01 · Anna'), findsOneWidget);
    final Text snippet = tester
        .widget<Text>(find.byKey(const ValueKey<String>('search-snippet-0')));
    final TextSpan root = snippet.textSpan! as TextSpan;
    final TextSpan marked = root.children!
        .cast<TextSpan>()
        .firstWhere((TextSpan span) => span.text == 'budget');
    expect(marked.style?.backgroundColor, isNotNull);

    await tester.tap(find.text('Budget review'));
    expect(opened?.startTimeMs, 61000);
  });

  testWidgets('nothing found and failures are said plainly',
      (WidgetTester tester) async {
    core.answer('note_taker_search',
        <String, Object?>{'ok': true, 'hits': <Object?>[], 'truncated': false});
    core.answer('note_taker_search',
        <String, Object?>{'ok': false, 'error_code': 'too_short'});
    await tester.pumpWidget(MaterialApp(
      home: SearchScreen(
        bridge: bridge,
        scope: SearchScope.projects,
        title: 'Search projects',
        onOpen: (BuildContext context, SearchHit hit) {},
      ),
    ));

    await tester.enterText(find.byType(TextField), 'courier');
    await tester.tap(find.byTooltip('Search'));
    await tester.pumpAndSettle();
    expect(find.text('Nothing found.'), findsOneWidget);

    await tester.enterText(find.byType(TextField), 'c');
    await tester.tap(find.byTooltip('Search'));
    await tester.pumpAndSettle();
    expect(find.text('Type at least two letters.'), findsOneWidget);
  });

  testWidgets('/ opens the meeting search from the list (#215)',
      (WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(
      home: MeetingListScreen(
        controller: MeetingListController(meetingBridge: FakeMeetingBridge()),
        searchBridge: bridge,
      ),
    ));
    await tester.pumpAndSettle();

    await simulateKeyDownEvent(LogicalKeyboardKey.slash, character: '/');
    await simulateKeyUpEvent(LogicalKeyboardKey.slash);
    await tester.pumpAndSettle();
    expect(find.byType(SearchScreen), findsOneWidget);
  });

  testWidgets('an open meeting scrolls to the passage found and marks it',
      (WidgetTester tester) async {
    final List<MeetingTranscriptSegment> segments = <MeetingTranscriptSegment>[
      for (int minute = 0; minute < 80; minute++)
        MeetingTranscriptSegment(
          startTimeMs: minute * 60000,
          endTimeMs: minute * 60000 + 5000,
          speaker: 'Anna',
          text: minute == 70 ? 'The budget is approved.' : 'Filler $minute.',
        ),
    ];
    final MeetingListController meetings = MeetingListController(
      meetingBridge: FakeMeetingBridge(details: <String, MeetingDetail>{
        'meeting-titled': MeetingDetail(
            summary: titledMeeting, transcript: null, segments: segments),
      }),
    );
    await meetings.selectMeeting('meeting-titled');

    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: MeetingDetailView(controller: meetings, focusStartMs: 70 * 60000),
      ),
    ));
    await tester.pumpAndSettle();

    final Finder focused = find.byKey(focusedSegmentKey);
    expect(focused, findsOneWidget);
    expect(find.text('The budget is approved.'), findsOneWidget);
    expect(tester.getTopLeft(focused).dy, lessThan(600));
  });
}
