// Defines widget-first requirements for the meeting detail title header and transcript.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';
import 'package:note_taker_ui/presentation/rename_meeting_dialog.dart';

import 'fake_meeting_bridge.dart';

/// Pumps the detail screen for `meetingId`, optionally with an attachment screen builder.
Future<FakeMeetingBridge> pumpMeetingDetail(
  WidgetTester tester,
  String meetingId, {
  WidgetBuilder? attachmentScreenBuilder,
}) async {
  final FakeMeetingBridge meetingBridge =
      FakeMeetingBridge(details: <String, MeetingDetail>{
    'meeting-titled': titledMeetingDetail,
    'meeting-untitled': untitledMeetingDetail,
  });
  final MeetingListController controller =
      MeetingListController(meetingBridge: meetingBridge);
  await tester.pumpWidget(
    MaterialApp(
      home: MeetingDetailScreen(
        controller: controller,
        meetingId: meetingId,
        attachmentScreenBuilder: attachmentScreenBuilder,
      ),
    ),
  );
  await tester.pumpAndSettle();
  return meetingBridge;
}

/// Verifies the title header, fallback title, transcript, and attachment action.
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
        Theme.of(context).textTheme.headlineMedium!.fontSize);
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

    expect(find.text('2026-10-27 09:05 UTC · 1:05'), findsOneWidget);
    expect(find.text('meeting-titled'), findsOneWidget);
    expect(find.text('Alice · 0:00'), findsOneWidget);
    expect(find.text('Hello team.'), findsOneWidget);
    expect(find.text('Unattributed · 1:01'), findsOneWidget);
  });

  testWidgets('shows the fallback title and an empty transcript message',
      (WidgetTester tester) async {
    await pumpMeetingDetail(tester, 'meeting-untitled');

    expect(find.text('Meeting 2026-10-26 18:00 UTC'), findsOneWidget);
    expect(find.text('No transcript is available.'), findsOneWidget);
  });

  testWidgets('reports a meeting that cannot be loaded',
      (WidgetTester tester) async {
    await pumpMeetingDetail(tester, 'meeting-missing');

    expect(find.text('Unable to load this meeting'), findsOneWidget);
  });

  testWidgets('the paperclip action opens the attachment import screen',
      (WidgetTester tester) async {
    await pumpMeetingDetail(
      tester,
      'meeting-titled',
      attachmentScreenBuilder: (BuildContext context) =>
          const Scaffold(body: Text('Attachment import')),
    );

    await tester.tap(find.byTooltip('Attach file'));
    await tester.pumpAndSettle();

    expect(find.text('Attachment import'), findsOneWidget);
  });

  testWidgets('the pencil next to the title opens the rename dialog',
      (WidgetTester tester) async {
    await pumpMeetingDetail(tester, 'meeting-titled');

    await tester.tap(find.byTooltip('Rename meeting'));
    await tester.pumpAndSettle();

    expect(find.byType(RenameMeetingDialog), findsOneWidget);
    final TextField titleField = tester.widget<TextField>(find.descendant(
        of: find.byType(RenameMeetingDialog),
        matching: find.byType(TextField)));
    expect(titleField.controller?.text, 'Project Sync');
  });

  testWidgets('the pencil sits on the same line as the title',
      (WidgetTester tester) async {
    await pumpMeetingDetail(tester, 'meeting-titled');

    final Rect titleRect = tester.getRect(find.text('Project Sync'));
    final Rect pencilRect = tester.getRect(find.byTooltip('Rename meeting'));
    expect(pencilRect.left, greaterThanOrEqualTo(titleRect.right));
    expect(
        pencilRect.center.dy, closeTo(titleRect.center.dy, titleRect.height));
  });

  testWidgets('a rename from the detail updates the header immediately',
      (WidgetTester tester) async {
    final FakeMeetingBridge meetingBridge =
        await pumpMeetingDetail(tester, 'meeting-titled');

    await tester.tap(find.byTooltip('Rename meeting'));
    await tester.pumpAndSettle();
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

    await tester.tap(find.byTooltip('Rename meeting'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'Roadmap Review');
    await tester.tap(find.text('Save'));
    await tester.pumpAndSettle();

    expect(
        find.text("Couldn't save the new title. Try again."), findsOneWidget);
    expect(find.text('Project Sync'), findsOneWidget);
  });
}
