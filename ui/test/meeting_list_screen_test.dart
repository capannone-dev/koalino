// Defines widget-first requirements for the responsive, title-first meeting list.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';
import 'package:note_taker_ui/presentation/meeting_list_screen.dart';

import 'fake_meeting_bridge.dart';

/// Pumps the meeting list at `size` (logical pixels) around `meetingBridge`.
Future<void> pumpMeetingList(
    WidgetTester tester, FakeMeetingBridge meetingBridge,
    {Size size = const Size(390, 844)}) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = size;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    MaterialApp(
      home: MeetingListScreen(
        controller: MeetingListController(meetingBridge: meetingBridge),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

/// Verifies titles, metadata, states, and the phone and desktop layouts of the meeting list.
void main() {
  testWidgets('shows custom and fallback titles with their date and duration',
      (WidgetTester tester) async {
    await pumpMeetingList(
      tester,
      FakeMeetingBridge(
          meetings: <MeetingSummary>[titledMeeting, untitledMeeting]),
    );

    expect(find.text('Project Sync'), findsOneWidget);
    expect(find.text('Meeting 2026-10-26 18:00 UTC'), findsOneWidget);
    expect(find.text('2026-10-27 09:05 UTC · 1:05'), findsOneWidget);
    expect(find.text('2026-10-26 18:00 UTC · 0:00'), findsOneWidget);
  });

  testWidgets('places the title above, and larger than, its metadata',
      (WidgetTester tester) async {
    await pumpMeetingList(
        tester, FakeMeetingBridge(meetings: <MeetingSummary>[titledMeeting]));

    final Finder titleText = find.text('Project Sync');
    final Finder metadataText = find.text('2026-10-27 09:05 UTC · 1:05');
    expect(tester.getTopLeft(titleText).dy,
        lessThan(tester.getTopLeft(metadataText).dy));
    final TextStyle titleStyle =
        tester.renderObject<RenderParagraph>(titleText).text.style!;
    final TextStyle metadataStyle =
        tester.renderObject<RenderParagraph>(metadataText).text.style!;
    expect(titleStyle.fontSize!, greaterThan(metadataStyle.fontSize!));
    expect(titleStyle.fontWeight, FontWeight.w600);
  });

  testWidgets('tapping a meeting on a phone opens its detail screen',
      (WidgetTester tester) async {
    final FakeMeetingBridge meetingBridge = FakeMeetingBridge(
      meetings: <MeetingSummary>[titledMeeting],
      details: <String, MeetingDetail>{'meeting-titled': titledMeetingDetail},
    );
    await pumpMeetingList(tester, meetingBridge);

    await tester.tap(find.text('Project Sync'));
    await tester.pumpAndSettle();

    expect(find.byType(MeetingDetailScreen), findsOneWidget);
    expect(meetingBridge.requestedMeetings, <String>['meeting-titled']);
    expect(find.text('Hello team.'), findsOneWidget);
  });

  testWidgets('a desktop window shows the list and the detail side by side',
      (WidgetTester tester) async {
    final FakeMeetingBridge meetingBridge = FakeMeetingBridge(
      meetings: <MeetingSummary>[titledMeeting, untitledMeeting],
      details: <String, MeetingDetail>{'meeting-titled': titledMeetingDetail},
    );
    await pumpMeetingList(tester, meetingBridge, size: const Size(1280, 800));

    expect(
        find.text('Select a meeting to see its transcript.'), findsOneWidget);

    await tester.tap(find.text('Project Sync'));
    await tester.pumpAndSettle();

    expect(find.byType(MeetingDetailScreen), findsNothing);
    expect(find.text('Hello team.'), findsOneWidget);
    expect(find.text('Project Sync'), findsNWidgets(2));
    final ListTile selectedTile = tester.widget<ListTile>(find.ancestor(
        of: find.text('Meeting 2026-10-26 18:00 UTC'),
        matching: find.byType(ListTile)));
    expect(selectedTile.selected, isFalse);
  });

  testWidgets('explains when there are no meetings yet',
      (WidgetTester tester) async {
    await pumpMeetingList(tester, FakeMeetingBridge());

    expect(find.text('No meetings yet'), findsOneWidget);
  });

  testWidgets('shows an error with a retry action when loading fails',
      (WidgetTester tester) async {
    final FakeMeetingBridge meetingBridge =
        FakeMeetingBridge(meetings: <MeetingSummary>[titledMeeting])
          ..listError = StateError('disk unavailable');
    await pumpMeetingList(tester, meetingBridge);

    expect(find.text('Unable to load meetings'), findsOneWidget);

    meetingBridge.listError = null;
    await tester.tap(find.text('Retry'));
    await tester.pumpAndSettle();

    expect(find.text('Project Sync'), findsOneWidget);
  });
}
