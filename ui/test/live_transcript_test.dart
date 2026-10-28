// Specifies the live transcript on the Record screen (Issue #52): segments appear while the
// recording runs, the last ones are fetched after Stop, a new recording starts empty, and the
// screen shows each line with its time.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/live_transcript_controller.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/recording_screen.dart';

import 'fake_meeting_bridge.dart';
import 'recording_screen_test.dart'
    show FakeMicrophonePermission, FakeRecordingBridge;

const String liveMeetingId = 'meeting-live';

/// A meeting detail holding `texts` as consecutive 5-second Unattributed segments.
MeetingDetail liveDetail(List<String> texts) => MeetingDetail(
      summary: MeetingSummary(
        meetingId: liveMeetingId,
        title: 'Live',
        createdAt: DateTime.utc(2026, 10, 28, 9),
        durationMs: texts.length * 5000,
      ),
      transcript: null,
      segments: <MeetingTranscriptSegment>[
        for (int index = 0; index < texts.length; index++)
          MeetingTranscriptSegment(
            startTimeMs: index * 5000,
            endTimeMs: (index + 1) * 5000,
            speaker: 'Unattributed',
            text: texts[index],
          ),
      ],
    );

/// The pieces every test drives.
typedef LiveFixture = ({
  FakeRecordingBridge recordingBridge,
  FakeMeetingBridge meetingBridge,
  RecordingController recordingController,
  LiveTranscriptController liveTranscript,
});

LiveFixture liveFixture() {
  final FakeRecordingBridge recordingBridge =
      FakeRecordingBridge(startResult: true)
        ..recordingMeetingId = liveMeetingId;
  final FakeMeetingBridge meetingBridge = FakeMeetingBridge(
      details: <String, MeetingDetail>{liveMeetingId: liveDetail(<String>[])});
  final RecordingController recordingController = RecordingController(
    recordingBridge: recordingBridge,
    microphonePermission: FakeMicrophonePermission(granted: true),
  );
  return (
    recordingBridge: recordingBridge,
    meetingBridge: meetingBridge,
    recordingController: recordingController,
    liveTranscript: LiveTranscriptController(
      recordingController: recordingController,
      recordingBridge: recordingBridge,
      meetingBridge: meetingBridge,
    ),
  );
}

void main() {
  testWidgets('segments appear while recording, every two seconds',
      (WidgetTester tester) async {
    final LiveFixture fixture = liveFixture();
    await fixture.recordingController.startRecording();
    await tester.pump();
    expect(fixture.liveTranscript.segments, isEmpty);

    fixture.meetingBridge.details[liveMeetingId] =
        liveDetail(<String>['Hello team.']);
    await tester.pump(const Duration(seconds: 2));
    expect(fixture.liveTranscript.segments.map((segment) => segment.text),
        <String>['Hello team.']);

    fixture.meetingBridge.details[liveMeetingId] =
        liveDetail(<String>['Hello team.', 'Let us start.']);
    await tester.pump(const Duration(seconds: 2));
    expect(fixture.liveTranscript.segments, hasLength(2));

    await fixture.recordingController.stopRecording();
    fixture.liveTranscript.dispose();
  });

  testWidgets('stop fetches the last segments and then stops polling',
      (WidgetTester tester) async {
    final LiveFixture fixture = liveFixture();
    await fixture.recordingController.startRecording();
    await tester.pump();

    fixture.meetingBridge.details[liveMeetingId] =
        liveDetail(<String>['First.', 'Last words.']);
    // Rust no longer reports a meeting once the recording has stopped.
    fixture.recordingBridge.recordingMeetingId = null;
    await fixture.recordingController.stopRecording();
    await tester.pump();

    expect(fixture.liveTranscript.segments.map((segment) => segment.text),
        <String>['First.', 'Last words.']);
    final int requestsAfterStop =
        fixture.meetingBridge.requestedMeetings.length;
    await tester.pump(const Duration(seconds: 10));
    expect(fixture.meetingBridge.requestedMeetings.length, requestsAfterStop);
    fixture.liveTranscript.dispose();
  });

  testWidgets('a new recording starts with an empty transcript',
      (WidgetTester tester) async {
    final LiveFixture fixture = liveFixture();
    fixture.meetingBridge.details[liveMeetingId] =
        liveDetail(<String>['Old meeting.']);
    await fixture.recordingController.startRecording();
    await tester.pump(const Duration(seconds: 2));
    await fixture.recordingController.stopRecording();
    await tester.pump();
    expect(fixture.liveTranscript.segments, isNotEmpty);

    fixture.recordingBridge.recordingMeetingId = 'meeting-next';
    fixture.meetingBridge.details['meeting-next'] = liveDetail(<String>[]);
    await fixture.recordingController.startRecording();
    await tester.pump();

    expect(fixture.liveTranscript.segments, isEmpty);
    await fixture.recordingController.stopRecording();
    fixture.liveTranscript.dispose();
  });

  testWidgets('the Record screen lists each line with its time',
      (WidgetTester tester) async {
    final LiveFixture fixture = liveFixture();
    await tester.pumpWidget(MaterialApp(
      home: RecordingScreen(
        recordingController: fixture.recordingController,
        liveTranscriptController: fixture.liveTranscript,
      ),
    ));
    expect(find.text('Live transcript'), findsNothing);

    await tester.tap(find.text('Start recording'));
    await tester.pump();
    expect(find.text('Live transcript'), findsOneWidget);
    expect(find.textContaining('appears here'), findsOneWidget);

    fixture.meetingBridge.details[liveMeetingId] =
        liveDetail(<String>['Hello team.', 'Let us start.']);
    await tester.pump(const Duration(seconds: 2));
    await tester.pump();

    expect(find.text('0:00'), findsOneWidget);
    expect(find.text('0:05'), findsOneWidget);
    expect(find.text('Let us start.'), findsOneWidget);
    // Unattributed lines show no speaker label.
    expect(find.text('Unattributed'), findsNothing);

    await fixture.recordingController.stopRecording();
    fixture.liveTranscript.dispose();
  });
}
