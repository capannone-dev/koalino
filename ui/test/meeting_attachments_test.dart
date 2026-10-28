// Defines how a meeting shows and gains attachments (Issue #54): photos and documents appear in
// the transcript where they were taken, several files can be attached at once, and the camera
// opens for the meeting being viewed.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/meeting_attachments_controller.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';

import 'fake_meeting_bridge.dart';
import 'fake_photo_camera.dart';

/// `titledMeetingDetail` with a photo taken at 0:30 and a document added afterwards.
final MeetingDetail meetingWithAttachments = MeetingDetail(
  summary: titledMeeting,
  transcript: null,
  segments: titledMeetingDetail.segments,
  attachments: const <MeetingAttachment>[
    MeetingAttachment(
      attachmentId: 'a1',
      fileName: 'whiteboard.jpg',
      filePath: '/data/attachments/meeting-titled/a1-whiteboard.jpg',
      mimeType: 'image/jpeg',
      meetingTimeMs: 30000,
    ),
    MeetingAttachment(
      attachmentId: 'a2',
      fileName: 'agenda.pdf',
      filePath: '/data/attachments/meeting-titled/a2-agenda.pdf',
      mimeType: 'application/pdf',
      meetingTimeMs: null,
    ),
  ],
);

/// The pumped screen's fakes.
typedef AttachmentHarness = ({
  FakeMeetingBridge meetingBridge,
  FakeMeetingAttachmentBridge attachmentBridge,
  FakeFilesPicker filesPicker,
  List<String> photoScreenMeetings,
});

/// Pumps the detail screen of `meeting-titled` with both attachment actions.
Future<AttachmentHarness> pumpDetail(WidgetTester tester) async {
  final FakeMeetingBridge meetingBridge = FakeMeetingBridge(
      details: <String, MeetingDetail>{
        'meeting-titled': meetingWithAttachments
      });
  final FakeMeetingAttachmentBridge attachmentBridge =
      FakeMeetingAttachmentBridge();
  final FakeFilesPicker filesPicker = FakeFilesPicker();
  final List<String> photoScreenMeetings = <String>[];
  await tester.pumpWidget(MaterialApp(
    home: MeetingDetailScreen(
      controller: MeetingListController(meetingBridge: meetingBridge),
      meetingId: 'meeting-titled',
      attachmentsController: MeetingAttachmentsController(
        attachmentBridge: attachmentBridge,
        filesPicker: filesPicker,
      ),
      photoScreenBuilder: (BuildContext context, String meetingId) {
        photoScreenMeetings.add(meetingId);
        return const Scaffold(body: Text('Camera'));
      },
    ),
  ));
  await tester.pumpAndSettle();
  return (
    meetingBridge: meetingBridge,
    attachmentBridge: attachmentBridge,
    filesPicker: filesPicker,
    photoScreenMeetings: photoScreenMeetings,
  );
}

/// Verifies attachment display and both attachment actions.
void main() {
  testWidgets(
      'a photo appears where it was taken and a later document at the end',
      (WidgetTester tester) async {
    await pumpDetail(tester);

    final double hello = tester.getTopLeft(find.text('Hello team.')).dy;
    final double photo = tester.getTopLeft(find.text('Photo · 0:30')).dy;
    final double seeYou = tester.getTopLeft(find.text('See you next week.')).dy;
    await tester.scrollUntilVisible(find.text('agenda.pdf'), 200,
        scrollable: find.byType(Scrollable).first);
    expect(hello, lessThan(photo));
    expect(photo, lessThan(seeYou));
    expect(find.text('Added after the meeting'), findsOneWidget);
  });

  testWidgets('several files are attached at once and the meeting reloads',
      (WidgetTester tester) async {
    final AttachmentHarness harness = await pumpDetail(tester);
    harness.filesPicker.pickedPaths = <String>['/tmp/a.pdf', '/tmp/b.png'];
    final int loadsBefore = harness.meetingBridge.requestedMeetings.length;

    await tester.tap(find.byTooltip('Attach files'));
    await tester.pumpAndSettle();

    expect(
        harness.attachmentBridge.requests.single.meetingId, 'meeting-titled');
    expect(harness.attachmentBridge.requests.single.filePaths,
        <String>['/tmp/a.pdf', '/tmp/b.png']);
    expect(find.text('Attached 2 files'), findsOneWidget);
    expect(harness.meetingBridge.requestedMeetings.length, loadsBefore + 1);
  });

  testWidgets('files that could not be attached are counted',
      (WidgetTester tester) async {
    final AttachmentHarness harness = await pumpDetail(tester);
    harness.filesPicker.pickedPaths = <String>['/tmp/a.pdf', '/tmp/tool.exe'];
    harness.attachmentBridge.failingPaths = <String>{'/tmp/tool.exe'};

    await tester.tap(find.byTooltip('Attach files'));
    await tester.pumpAndSettle();

    expect(
        find.text('Attached 1 file; 1 could not be attached'), findsOneWidget);
  });

  testWidgets('cancelling the picker attaches nothing',
      (WidgetTester tester) async {
    final AttachmentHarness harness = await pumpDetail(tester);

    await tester.tap(find.byTooltip('Attach files'));
    await tester.pumpAndSettle();

    expect(harness.attachmentBridge.requests, isEmpty);
    expect(find.byType(SnackBar), findsNothing);
  });

  testWidgets('Take photos opens the camera for this meeting and reloads it',
      (WidgetTester tester) async {
    final AttachmentHarness harness = await pumpDetail(tester);
    final int loadsBefore = harness.meetingBridge.requestedMeetings.length;

    await tester.tap(find.byTooltip('Take photos'));
    await tester.pumpAndSettle();
    expect(find.text('Camera'), findsOneWidget);
    expect(harness.photoScreenMeetings, <String>['meeting-titled']);

    Navigator.of(tester.element(find.text('Camera'))).pop();
    await tester.pumpAndSettle();
    expect(harness.meetingBridge.requestedMeetings.length, loadsBefore + 1);
  });

  testWidgets('without a camera or controller the actions are hidden',
      (WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(
      home: MeetingDetailScreen(
        controller: MeetingListController(
            meetingBridge: FakeMeetingBridge(details: <String, MeetingDetail>{
          'meeting-titled': titledMeetingDetail
        })),
        meetingId: 'meeting-titled',
      ),
    ));
    await tester.pumpAndSettle();

    expect(find.byTooltip('Take photos'), findsNothing);
    expect(find.byTooltip('Attach files'), findsNothing);
  });
}
