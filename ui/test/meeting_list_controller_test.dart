// Defines how the meeting controller renames meetings through the bridge.

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';

import 'fake_meeting_bridge.dart';

/// Builds a controller with both fixture meetings listed and `meeting-titled` selected.
Future<(MeetingListController, FakeMeetingBridge)> loadedController() async {
  final FakeMeetingBridge meetingBridge = FakeMeetingBridge(
    meetings: <MeetingSummary>[titledMeeting, untitledMeeting],
    details: <String, MeetingDetail>{'meeting-titled': titledMeetingDetail},
  );
  final MeetingListController controller =
      MeetingListController(meetingBridge: meetingBridge);
  await controller.loadMeetings();
  await controller.selectMeeting('meeting-titled');
  return (controller, meetingBridge);
}

/// Verifies renames update the list and the open detail in place, and surface failures.
void main() {
  test('a rename sends the trimmed title and updates list and detail in place',
      () async {
    final (MeetingListController controller, FakeMeetingBridge meetingBridge) =
        await loadedController();
    int notificationCount = 0;
    controller.addListener(() => notificationCount += 1);

    await controller.renameMeeting('meeting-titled', '  Roadmap Review  ');

    expect(meetingBridge.renameRequests, <RenameRequest>[
      (meetingId: 'meeting-titled', newTitle: 'Roadmap Review')
    ]);
    expect(controller.meetings.first.title, 'Roadmap Review');
    expect(controller.meetings.first.createdAt, titledMeeting.createdAt);
    expect(controller.meetings.last.title, untitledMeeting.title);
    expect(controller.selectedMeeting?.summary.title, 'Roadmap Review');
    expect(controller.selectedMeeting?.segments, titledMeetingDetail.segments);
    expect(meetingBridge.meetingListRequestCount, 1);
    expect(notificationCount, greaterThan(0));
  });

  test('a failed rename rethrows and leaves the titles unchanged', () async {
    final (MeetingListController controller, FakeMeetingBridge meetingBridge) =
        await loadedController();
    meetingBridge.renameError = meetingRenameExceptionFor(2);

    await expectLater(
      controller.renameMeeting('meeting-titled', 'Roadmap Review'),
      throwsA(isA<MeetingRenameException>()),
    );

    expect(controller.meetings.first.title, 'Project Sync');
    expect(controller.selectedMeeting?.summary.title, 'Project Sync');
  });

  test('renaming a meeting that no longer exists reloads the list', () async {
    final (MeetingListController controller, FakeMeetingBridge meetingBridge) =
        await loadedController();
    meetingBridge.renameError = meetingRenameExceptionFor(5);

    await expectLater(
      controller.renameMeeting('meeting-titled', 'Roadmap Review'),
      throwsA(isA<MeetingRenameException>()),
    );

    expect(meetingBridge.meetingListRequestCount, 2);
  });

  test('a split reloads the list and the open meeting (#88)', () async {
    final FakeMeetingBridge meetingBridge = FakeMeetingBridge(
      meetings: <MeetingSummary>[combinedMeeting],
      details: <String, MeetingDetail>{
        'meeting-combined': combinedMeetingDetail
      },
    );
    final MeetingListController controller =
        MeetingListController(meetingBridge: meetingBridge);
    await controller.loadMeetings();
    await controller.selectMeeting('meeting-combined');

    expect(await controller.splitMeeting('meeting-combined'), isTrue);
    expect(meetingBridge.splitRequests, <String>['meeting-combined']);
    expect(meetingBridge.meetingListRequestCount, 2);
    expect(meetingBridge.requestedMeetings,
        <String>['meeting-combined', 'meeting-combined']);

    meetingBridge.splitError = StateError('refused');
    expect(await controller.splitMeeting('meeting-combined'), isFalse);
    expect(meetingBridge.meetingListRequestCount, 2, reason: 'no reload');
  });

  test(
      'a refresh after a speaker rename keeps the meeting shown, with no spinner',
      () async {
    final (MeetingListController controller, FakeMeetingBridge meetingBridge) =
        await loadedController();
    final List<bool> loadingStates = <bool>[];
    controller.addListener(() => loadingStates.add(controller.isLoadingDetail));
    await controller.refreshSelectedMeeting();
    expect(meetingBridge.requestedMeetings.last, 'meeting-titled');
    expect(loadingStates, isNot(contains(true)));
    expect(controller.selectedMeeting?.summary.meetingId, 'meeting-titled');
  });
}
