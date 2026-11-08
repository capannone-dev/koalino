// Provides a deterministic meeting bridge and fixtures shared by the meeting widget tests.

import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';

/// One `renameMeeting` request received by the fake.
typedef RenameRequest = ({String meetingId, String newTitle});

/// Serves fixed meetings without loading the Rust library and records every request.
final class FakeMeetingBridge implements MeetingBridge {
  /// Creates a fake serving `meetings` and the details keyed by meeting ID.
  FakeMeetingBridge({
    this.meetings = const <MeetingSummary>[],
    this.details = const <String, MeetingDetail>{},
  });

  /// Rows returned by `getMeetings`.
  final List<MeetingSummary> meetings;

  /// Details returned by `getMeeting`; a missing ID throws like Rust's `meeting_not_found`.
  final Map<String, MeetingDetail> details;

  /// When set, `getMeetings` throws this error.
  Object? listError;

  /// Meeting IDs requested through `getMeeting`.
  final List<String> requestedMeetings = <String>[];

  /// Number of `getMeetings` calls, so tests can prove a rename did not reload the list.
  int meetingListRequestCount = 0;

  /// Requests received through `renameMeeting`, in order.
  final List<RenameRequest> renameRequests = <RenameRequest>[];

  /// When set, `renameMeeting` records the request and then throws this error.
  Object? renameError;

  /// Meeting IDs sent to `deleteMeeting`.
  final List<String> deleteRequests = <String>[];

  /// When set, `deleteMeeting` records the request and then throws this error.
  Object? deleteError;

  /// Meeting IDs sent to `splitMeeting`.
  final List<String> splitRequests = <String>[];

  /// When set, `splitMeeting` records the request and then throws this error.
  Object? splitError;

  @override
  Future<List<MeetingSummary>> getMeetings() async {
    meetingListRequestCount += 1;
    if (listError case final Object error) {
      throw error;
    }
    return meetings;
  }

  @override
  Future<MeetingDetail> getMeeting(String meetingId) async {
    requestedMeetings.add(meetingId);
    return details[meetingId] ??
        (throw StateError('Rust rejected the meeting request '
            '(meeting_not_found): $meetingId'));
  }

  @override
  Future<void> renameMeeting(String meetingId, String newTitle) async {
    renameRequests.add((meetingId: meetingId, newTitle: newTitle));
    if (renameError case final Object error) {
      throw error;
    }
  }

  @override
  Future<void> splitMeeting(String meetingId) async {
    splitRequests.add(meetingId);
    if (splitError case final Object error) {
      throw error;
    }
  }

  @override
  Future<void> deleteMeeting(String meetingId) async {
    deleteRequests.add(meetingId);
    if (deleteError case final Object error) {
      throw error;
    }
  }
}

/// A meeting with a person-chosen title, 1:05 long.
final MeetingSummary titledMeeting = MeetingSummary(
  meetingId: 'meeting-titled',
  title: 'Project Sync',
  createdAt: DateTime.utc(2026, 10, 27, 9, 5),
  durationMs: 65000,
  transcriptionLanguages: const <String>['it', 'en'],
);

/// A meeting whose title is the Rust fallback, with no recorded audio.
final MeetingSummary untitledMeeting = MeetingSummary(
  meetingId: 'meeting-untitled',
  title: 'Meeting 2026-10-26 18:00 UTC',
  createdAt: DateTime.utc(2026, 10, 26, 18),
  durationMs: 0,
);

/// The detail of `titledMeeting`, with two attributed segments.
final MeetingDetail titledMeetingDetail = MeetingDetail(
  summary: titledMeeting,
  transcript: null,
  segments: const <MeetingTranscriptSegment>[
    MeetingTranscriptSegment(
        startTimeMs: 0, endTimeMs: 1500, speaker: 'Alice', text: 'Hello team.'),
    MeetingTranscriptSegment(
        startTimeMs: 61000,
        endTimeMs: 65000,
        speaker: 'Unattributed',
        text: 'See you next week.'),
  ],
);

/// The detail of `untitledMeeting`, which has no transcript at all.
final MeetingDetail untitledMeetingDetail = MeetingDetail(
  summary: untitledMeeting,
  transcript: null,
  segments: const <MeetingTranscriptSegment>[],
);

/// A meeting recorded on this laptop and on a paired phone, combined into one (#88).
final MeetingSummary combinedMeeting = MeetingSummary(
  meetingId: 'meeting-combined',
  title: 'Supplier call',
  createdAt: DateTime.utc(2026, 11, 2, 15),
  durationMs: 120000,
  recordedOn: const <RecordingDevice>[
    RecordingDevice(
        deviceId: 'device-laptop', name: 'Laptop', thisDevice: true),
    RecordingDevice(deviceId: 'device-phone', name: 'Pixel', thisDevice: false),
  ],
  combinedRecordings: const <String>['meeting-phone'],
  combining: MeetingCombining(
    state: CombiningState.waiting,
    combineOn: const RecordingDevice(
        deviceId: 'device-laptop', name: 'Laptop', thisDevice: true),
    waitingFor: const <RecordingDevice>[
      RecordingDevice(
          deviceId: 'device-phone',
          name: 'Pixel',
          thisDevice: false,
          connected: false),
    ],
    keptUntil: DateTime.utc(2026, 11, 23),
  ),
);

/// The detail of `combinedMeeting`.
final MeetingDetail combinedMeetingDetail = MeetingDetail(
  summary: combinedMeeting,
  transcript: null,
  segments: const <MeetingTranscriptSegment>[
    MeetingTranscriptSegment(
        startTimeMs: 0, endTimeMs: 2000, speaker: 'Bruno', text: 'Ciao.'),
  ],
);
