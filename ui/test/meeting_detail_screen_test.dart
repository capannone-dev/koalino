// Defines widget-first requirements for the meeting detail title header and transcript.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/presentation/meeting_time.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';
import 'package:note_taker_ui/presentation/rename_meeting_dialog.dart';

import 'fake_meeting_bridge.dart';

/// Pumps the detail screen for `meetingId`.
Future<FakeMeetingBridge> pumpMeetingDetail(
  WidgetTester tester,
  String meetingId,
) async {
  final FakeMeetingBridge meetingBridge =
      FakeMeetingBridge(details: <String, MeetingDetail>{
    'meeting-titled': titledMeetingDetail,
    'meeting-untitled': untitledMeetingDetail,
    'meeting-combined': combinedMeetingDetail,
    'meeting-doubtful': doubtfulMeetingDetail,
  });
  final MeetingListController controller =
      MeetingListController(meetingBridge: meetingBridge);
  await tester.pumpWidget(
    MaterialApp(
      home: MeetingDetailScreen(
        controller: controller,
        meetingId: meetingId,
      ),
    ),
  );
  await tester.pumpAndSettle();
  return meetingBridge;
}

/// Opens the meeting's More menu and chooses `label` (#201).
Future<void> chooseFromMore(WidgetTester tester, String label) async {
  await tester.tap(find.byKey(meetingMoreKey));
  await tester.pumpAndSettle();
  await tester.tap(find.text(label).last);
  await tester.pumpAndSettle();
}

/// Verifies the title header, fallback title, and transcript.
void main() {
  testWidgets('shows the title as a large bold header above the content',
      (WidgetTester tester) async {
    await pumpMeetingDetail(tester, 'meeting-titled');

    final Finder header = find.text('Project Sync');
    expect(header, findsOneWidget);
    final TextStyle headerStyle =
        tester.renderObject<RenderParagraph>(header).text.style!;
    final BuildContext context = tester.element(header);
    expect(headerStyle.fontWeight, FontWeight.bold);
    expect(headerStyle.fontSize,
        Theme.of(context).textTheme.headlineSmall!.fontSize);
    expect(tester.getTopLeft(header).dy,
        lessThan(tester.getTopLeft(find.text('Hello team.')).dy));
    expect(
      tester.getSemantics(header),
      isSemantics(label: 'Project Sync', isHeader: true),
    );
  });

  testWidgets('shows the metadata and speaker-attributed transcript',
      (WidgetTester tester) async {
    await pumpMeetingDetail(tester, 'meeting-titled');

    expect(
        find.text(
            '${formatMeetingWhen(DateTime.utc(2026, 10, 27, 9, 5))} · 1:05'),
        findsOneWidget);
    expect(
        find.text('Transcription languages: Italian, English'), findsOneWidget);
    expect(find.text('meeting-titled'), findsNothing,
        reason: 'the internal meeting ID is not shown (#211)');
    expect(find.text('Alice · 0:00'), findsOneWidget);
    expect(find.text('Hello team.'), findsOneWidget);
    expect(find.text('Unattributed · 1:01'), findsOneWidget);
  });

  testWidgets('marks a guessed speaker, overlapping speech and unclear words',
      (WidgetTester tester) async {
    await pumpMeetingDetail(tester, 'meeting-doubtful');

    expect(find.text('Unknown speaker (?) · 0:00'), findsOneWidget);
    expect(find.text('Overlapping speech'), findsOneWidget);
    final Finder text = find.text('we ship on Friday', findRichText: true);
    expect(text, findsOneWidget);
    final TextSpan span =
        tester.renderObject<RenderParagraph>(text).text as TextSpan;
    final List<TextSpan> words = <TextSpan>[];
    span.visitChildren((InlineSpan child) {
      if (child is TextSpan) words.add(child);
      return true;
    });
    final BuildContext context = tester.element(text);
    final TextSpan unclear =
        words.firstWhere((TextSpan word) => word.text == 'ship');
    expect(unclear.style!.color, Theme.of(context).colorScheme.outline);
    expect(unclear.style!.fontStyle, FontStyle.italic);
    expect(words.firstWhere((TextSpan word) => word.text == 'Friday').style,
        isNull);
  });

  testWidgets('shows the fallback title and an empty transcript message',
      (WidgetTester tester) async {
    await pumpMeetingDetail(tester, 'meeting-untitled');

    expect(find.text('Meeting 2026-10-26 18:00 UTC'), findsOneWidget);
    // No duration and no words: nothing was recorded (#129).
    expect(find.textContaining('No recording.'), findsOneWidget);
    // A meeting recorded before languages were kept shows no language line.
    expect(find.textContaining('Transcription language'), findsNothing);
  });

  testWidgets('reports a meeting that cannot be loaded',
      (WidgetTester tester) async {
    await pumpMeetingDetail(tester, 'meeting-missing');

    expect(find.text('Unable to load this meeting'), findsOneWidget);
  });

  testWidgets('the pencil next to the title opens the rename dialog',
      (WidgetTester tester) async {
    await pumpMeetingDetail(tester, 'meeting-titled');

    await chooseFromMore(tester, 'Rename meeting');

    expect(find.byType(RenameMeetingDialog), findsOneWidget);
    final TextField titleField = tester.widget<TextField>(find.descendant(
        of: find.byType(RenameMeetingDialog),
        matching: find.byType(TextField)));
    expect(titleField.controller?.text, 'Project Sync');
  });

  testWidgets(
      'the header offers at most three actions, the rest under More (#201)',
      (WidgetTester tester) async {
    await pumpMeetingDetail(tester, 'meeting-titled');

    expect(find.byKey(meetingMoreKey), findsOneWidget);
    expect(find.byTooltip('Rename meeting'), findsNothing);
    await tester.tap(find.byKey(meetingMoreKey));
    await tester.pumpAndSettle();
    expect(find.text('Rename meeting'), findsOneWidget);
    expect(find.text('Delete meeting'), findsOneWidget);
  });

  testWidgets(
      'the page has Transcript and Files tabs and opens on the transcript (#201)',
      (WidgetTester tester) async {
    await pumpMeetingDetail(tester, 'meeting-titled');

    expect(find.byKey(meetingTranscriptTabKey), findsOneWidget);
    expect(find.byKey(meetingFilesTabKey), findsOneWidget);
    expect(find.byKey(meetingSummaryTabKey), findsNothing,
        reason: 'no summaries without their controller');
    expect(find.text('Hello team.'), findsOneWidget);
  });

  testWidgets('a rename from the detail updates the header immediately',
      (WidgetTester tester) async {
    final FakeMeetingBridge meetingBridge =
        await pumpMeetingDetail(tester, 'meeting-titled');

    await chooseFromMore(tester, 'Rename meeting');
    await tester.enterText(
        find.descendant(
            of: find.byType(RenameMeetingDialog),
            matching: find.byType(TextField)),
        'Roadmap Review');
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();

    expect(meetingBridge.renameRequests, <RenameRequest>[
      (meetingId: 'meeting-titled', newTitle: 'Roadmap Review')
    ]);
    expect(find.text('Roadmap Review'), findsOneWidget);
    expect(find.text('Project Sync'), findsNothing);
  });

  testWidgets('a failed rename from the detail shows the error in the dialog',
      (WidgetTester tester) async {
    final FakeMeetingBridge meetingBridge =
        await pumpMeetingDetail(tester, 'meeting-titled');
    meetingBridge.renameError = meetingRenameExceptionFor(2);

    await chooseFromMore(tester, 'Rename meeting');
    await tester.enterText(find.byType(TextField), 'Roadmap Review');
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();

    expect(
        find.text("Couldn't save the new title. Try again."), findsOneWidget);
    expect(find.text('Project Sync'), findsOneWidget);
  });

  testWidgets('a combined meeting names its devices and can be split (#88)',
      (WidgetTester tester) async {
    final FakeMeetingBridge meetingBridge =
        await pumpMeetingDetail(tester, 'meeting-combined');

    expect(find.text('Recorded on Laptop + Pixel'), findsOneWidget);
    expect(
        find.descendant(
            of: find.byKey(const ValueKey<String>('meeting-combining')),
            matching: find.text(
                'Waiting for audio from Pixel (not connected). It is kept encrypted until ${formatCalendarDay(DateTime.utc(2026, 11, 23, 15))}.')),
        findsOneWidget);
    await tester.tap(find.byKey(meetingMoreKey));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const ValueKey<String>('split-open-meeting')));
    await tester.pumpAndSettle();
    expect(find.text('Split this meeting?'), findsOneWidget);
    await tester.tap(find.descendant(
        of: find.byType(AlertDialog), matching: find.text('Split')));
    await tester.pumpAndSettle();

    expect(meetingBridge.splitRequests, <String>['meeting-combined']);
  });

  testWidgets('a meeting recorded on one device offers no split',
      (WidgetTester tester) async {
    await pumpMeetingDetail(tester, 'meeting-titled');

    expect(find.textContaining('Recorded on'), findsNothing);
    await tester.tap(find.byKey(meetingMoreKey));
    await tester.pumpAndSettle();
    expect(
        find.byKey(const ValueKey<String>('split-open-meeting')), findsNothing);
  });
}
